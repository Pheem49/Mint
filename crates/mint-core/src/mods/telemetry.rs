use crate::{ChatResponse, MintConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
struct Measurement {
    provider: String,
    model: String,
    window: Option<u64>,
    used: Option<u64>,
    #[serde(default)]
    input: Option<u64>,
}
/// Latest provider measurement and a separate per-turn API usage counter.
pub struct UsageTracker {
    path: Option<PathBuf>,
    previous: Measurement,
    current: Measurement,
    turn: i64,
    total: u64,
    total_available: bool,
    total_incomplete: bool,
    input: Option<u32>,
    output: Option<u32>,
    compacted: bool,
    status: &'static str,
}
impl UsageTracker {
    pub(super) fn snapshot(&self) -> Value {
        self.payload(None, None)
    }
    pub fn new(root: &Path, chat: &str, turn: i64) -> Self {
        let path = super::manager::telemetry_path(root, chat).ok();
        let previous = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|v| serde_json::from_slice(&v).ok())
            .unwrap_or_default();
        Self {
            path,
            previous,
            current: Measurement::default(),
            turn,
            total: 0,
            total_available: false,
            total_incomplete: false,
            input: None,
            output: None,
            compacted: false,
            status: "unavailable",
        }
    }
    fn payload(&self, delta: Option<i64>, input_delta: Option<i64>) -> Value {
        let percent = self
            .current
            .used
            .zip(self.current.window)
            .map(|(used, window)| used as f64 / window as f64 * 100.0);
        json!({"turnId":self.turn,"totalTokens":self.total_available.then_some(self.total),
            "contextUsedTokens":self.current.used,"contextWindowTokens":self.current.window,
            "contextPercent":percent,"contextInputTokens":self.input,"contextOutputTokens":self.output,
            "contextStatus":self.status,"contextWindowSource":"mint_config","provider":self.current.provider,
            "model":self.current.model,"lastTurnDelta":delta,"compacted":self.compacted,
            "context":{"tokens":self.current.input,"window":self.current.window,"percent":self.current.input.zip(self.current.window).map(|(i,w)|i as f64/w as f64*100.0),"basis":"input","lastTurnDelta":input_delta},
            "contextIncludingOutput":{"tokens":self.current.used,"window":self.current.window,"percent":percent,"basis":"input_output","lastTurnDelta":delta}})
    }
    pub fn record(&mut self, config: &MintConfig, response: &ChatResponse) -> Value {
        self.total = self
            .total
            .saturating_add(response.total_tokens.unwrap_or(0) as u64);
        self.total_incomplete |= response.total_tokens.is_none();
        self.total_available = !self.total_incomplete;
        let mut actual = config.clone();
        actual.ai_provider = response.provider.clone();
        actual.openai_model = response.model.clone();
        let window = actual.context_window_tokens() as u64;
        self.input = response.input_tokens;
        self.output = response.output_tokens;
        self.current = Measurement {
            provider: response.provider.clone(),
            model: response.model.clone(),
            window: (window > 0).then_some(window),
            input: self.input.map(u64::from),
            used: self
                .input
                .zip(self.output)
                .map(|(i, o)| i as u64 + o as u64),
        };
        self.status = if self.current.used.is_some() {
            "measured"
        } else {
            "unavailable"
        };
        self.payload(None, None)
    }
    pub fn compacted(&mut self) -> Value {
        self.current.used = None;
        self.current.input = None;
        self.input = None;
        self.output = None;
        self.compacted = true;
        self.status = "compacted";
        self.payload(None, None)
    }
    pub fn finish(&mut self) -> Value {
        let same = self.current.provider == self.previous.provider
            && self.current.model == self.previous.model
            && self.current.window == self.previous.window;
        let delta = same
            .then(|| {
                self.current
                    .used
                    .zip(self.previous.used)
                    .map(|(now, before)| now as i64 - before as i64)
            })
            .flatten();
        // Telemetry must never turn a successful agent turn into an error.
        if let Some(path) = &self.path {
            let _ = super::manager::atomic_json(path, &self.current);
        }
        let input_delta = same
            .then(|| {
                self.current
                    .input
                    .zip(self.previous.input)
                    .map(|(now, before)| now as i64 - before as i64)
            })
            .flatten();
        self.payload(delta, input_delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_window_uses_actual_fallback_model_and_delta_resets_on_change() {
        let root = workspace();
        let config = MintConfig {
            ai_provider: "gemini".into(),
            openai_model: "gpt-4o".into(),
            ..Default::default()
        };
        let mut first = UsageTracker::new(&root, "fallback", 1);
        let mut reply = response(Some(80_000), Some(100));
        reply.provider = "openai".into();
        reply.model = "gpt-5-test".into();
        let value = first.record(&config, &reply);
        assert_eq!(value["contextWindowTokens"], 400_000);
        first.finish();
        let mut next = UsageTracker::new(&root, "fallback", 2);
        reply.model = "gpt-4o".into();
        assert_eq!(next.record(&config, &reply)["contextWindowTokens"], 128_000);
        assert!(next.finish()["lastTurnDelta"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn weather_consumer_keeps_twelve_completed_turns_and_survives_compaction() {
        let root = workspace();
        let source = r#"export function register(mint) {
            function draw(e) {
                const color=e.contextPercent==null?'default':e.contextPercent>=90?'red':e.contextPercent>=75?'magenta':e.contextPercent>=50?'blue':e.contextPercent>=25?'cyan':'yellow';
                mint.ui.set('above_composer',{type:'stack',direction:'row',children:[{type:'text',color,text:e.contextUsedTokens==null?'Unavailable':'Weather'},{type:'text',text:String(e.contextUsedTokens)}]});
            }
            mint.on('usage_update',draw); mint.on('context_compacted',draw);
            mint.on('turn_end',e=>{if(e.contextUsedTokens!=null)mint.state.set('history',[...(mint.state.get('history')||[]),e.contextUsedTokens].slice(-12));draw(e);});
        }"#;
        let compiled = mint_mod_runtime::validate_package(&json!({"id":"weather-test","name":"Weather test","version":"1","sdkVersion":1,"entry":"main.ts"}),source).unwrap();
        let mut engine = mint_mod_runtime::Engine::new(&compiled, &json!({}), |_| {
            panic!("No external capability needed")
        })
        .unwrap();
        for turn in 1..=14 {
            let mut tracker = UsageTracker::new(&root, "weather", turn);
            // Multiple model responses must still append one completed-turn point.
            let payload =
                tracker.record(&MintConfig::default(), &response(Some(90_000), Some(100)));
            engine.dispatch("usage_update", &payload).unwrap();
            if turn == 14 {
                engine
                    .dispatch("context_compacted", &tracker.compacted())
                    .unwrap();
                assert_eq!(
                    engine.snapshot().unwrap()["ui"]["above_composer"]["children"][0]["text"],
                    "Unavailable"
                );
            }
            let payload = tracker.record(
                &MintConfig::default(),
                &response(Some(if turn == 14 { 30_000 } else { 100_000 }), Some(100)),
            );
            engine.dispatch("usage_update", &payload).unwrap();
            let end = tracker.finish();
            if turn == 14 {
                assert_eq!(end["lastTurnDelta"], -70_000);
            }
            engine.dispatch("turn_end", &end).unwrap();
            // Recreate the engine to exercise the state contract across reloads.
            let state = engine.snapshot().unwrap()["state"].clone();
            engine = mint_mod_runtime::Engine::new(&compiled, &state, |_| {
                panic!("No external capability needed")
            })
            .unwrap();
        }
        let state = engine.snapshot().unwrap()["state"].clone();
        assert_eq!(state["history"].as_array().unwrap().len(), 12);
        assert_eq!(state["history"][11], 30_100);
        std::fs::remove_dir_all(root).unwrap();
    }
    fn workspace() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("mint-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        root
    }
    fn response(input: Option<u32>, output: Option<u32>) -> ChatResponse {
        ChatResponse {
            provider: "anthropic".into(),
            model: "claude-test".into(),
            text: "Done".into(),
            fallback_provider: None,
            fallback_reason: None,
            tool_calls: None,
            stop_reason: None,
            total_tokens: input.zip(output).map(|(i, o)| i + o),
            input_tokens: input,
            output_tokens: output,
            thought: None,
        }
    }
    #[test]
    fn context_uses_latest_response_instead_of_recounting_resent_history() {
        let root = workspace();
        let mut tracker = UsageTracker::new(&root, "a", 1);
        let config = MintConfig::default();
        tracker.record(&config, &response(Some(100_000), Some(1_000)));
        let value = tracker.record(&config, &response(Some(130_000), Some(4_400)));
        assert_eq!(value["totalTokens"], 235_400);
        assert_eq!(value["contextUsedTokens"], 134_400);
        assert_eq!(value["contextWindowTokens"], 200_000);
        assert_eq!(value["contextPercent"], 67.2);
        assert_eq!(value["context"]["tokens"], 130_000);
        assert_eq!(value["context"]["percent"], 65.0);
        assert_eq!(value["contextIncludingOutput"]["tokens"], 134_400);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn completed_turn_delta_survives_restart_and_stays_separate_per_chat() {
        let root = workspace();
        let config = MintConfig::default();
        let mut first = UsageTracker::new(&root, "a", 1);
        first.record(&config, &response(Some(36_000), Some(100)));
        assert_eq!(first.finish()["lastTurnDelta"], Value::Null);
        let mut next = UsageTracker::new(&root, "a", 2);
        next.record(&config, &response(Some(134_000), Some(400)));
        assert_eq!(next.finish()["lastTurnDelta"], 98_300);
        let mut other = UsageTracker::new(&root, "b", 2);
        other.record(&config, &response(Some(134_000), Some(400)));
        assert_eq!(other.finish()["lastTurnDelta"], Value::Null);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn compaction_invalidates_old_measurement_until_a_new_response_arrives() {
        let root = workspace();
        let config = MintConfig::default();
        let mut tracker = UsageTracker::new(&root, "a", 1);
        tracker.record(&config, &response(Some(180_000), Some(100)));
        let compacted = tracker.compacted();
        assert_eq!(compacted["contextStatus"], "compacted");
        assert!(compacted["contextUsedTokens"].is_null());
        assert!(compacted["contextPercent"].is_null());
        let updated = tracker.record(&config, &response(Some(30_000), Some(100)));
        assert_eq!(updated["contextUsedTokens"], 30_100);
        assert_eq!(updated["compacted"], true);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_usage_does_not_reuse_old_counts_or_invent_a_delta() {
        let root = workspace();
        let config = MintConfig::default();
        let mut tracker = UsageTracker::new(&root, "a", 1);
        tracker.record(&config, &response(Some(80_000), Some(100)));
        let value = tracker.record(&config, &response(None, None));
        assert_eq!(value["contextStatus"], "unavailable");
        assert!(value["contextUsedTokens"].is_null());
        assert!(tracker.finish()["lastTurnDelta"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cumulative_usage_remains_unknown_after_a_missing_middle_response() {
        let root = workspace();
        let mut tracker = UsageTracker::new(&root, "gap", 1);
        let config = MintConfig::default();
        tracker.record(&config, &response(Some(100), Some(0)));
        tracker.record(&config, &response(None, None));
        let value = tracker.record(&config, &response(Some(200), Some(0)));
        assert!(
            value["totalTokens"].is_null(),
            "Missing usage cannot be counted as zero"
        );
        assert_eq!(value["context"]["tokens"], 200);
        assert!(tracker.finish()["totalTokens"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }
}
