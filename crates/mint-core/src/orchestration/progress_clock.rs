use super::AgentProgress;
use std::time::Instant;

/// Captures tool durations at the shared progress boundary before events are persisted.
#[derive(Default)]
pub(super) struct ProgressClock {
    started: Vec<(String, Instant)>,
}
impl ProgressClock {
    pub fn stamp(&mut self, event: &mut AgentProgress, now: Instant) {
        let (call_id, action, input, subagent) = match &*event {
            AgentProgress::ToolStart {
                call_id,
                action,
                input,
                subagent,
            }
            | AgentProgress::ToolEnd {
                call_id,
                action,
                input,
                subagent,
                ..
            } => (call_id, action, input, subagent),
            _ => return,
        };
        let key = if call_id.is_some() {
            serde_json::json!([subagent, call_id])
        } else {
            serde_json::json!([subagent, action, input])
        }
        .to_string();
        match event {
            AgentProgress::ToolStart { .. } => self.started.push((key, now)),
            AgentProgress::ToolEnd { elapsed_ms, .. } => {
                if let Some(index) = self
                    .started
                    .iter()
                    .position(|(candidate, _)| candidate == &key)
                {
                    let (_, start) = self.started.remove(index);
                    if elapsed_ms.is_none() {
                        *elapsed_ms = Some(
                            now.saturating_duration_since(start)
                                .as_millis()
                                .min(u64::MAX as u128) as u64,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn event(kind: &str, id: &str) -> AgentProgress {
        serde_json::from_value(serde_json::json!({"type":kind,"data":{"callId":id,"action":"run_shell","input":{"command":"same command"},"result":"exit: 0"}})).unwrap()
    }
    #[test]
    fn elapsed_time_survives_serialization_and_out_of_order_tool_completion() {
        let mut clock = ProgressClock::default();
        let now = Instant::now();
        clock.stamp(&mut event("ToolStart", "a"), now);
        clock.stamp(
            &mut event("ToolStart", "b"),
            now + Duration::from_millis(100),
        );
        let mut b = event("ToolEnd", "b");
        clock.stamp(&mut b, now + Duration::from_millis(900));
        let mut a = event("ToolEnd", "a");
        clock.stamp(&mut a, now + Duration::from_millis(1200));
        let saved = serde_json::to_value(super::super::compact_agent_progress(&[b, a])).unwrap();
        assert_eq!(saved[0]["data"]["elapsed_ms"], 800);
        assert_eq!(saved[1]["data"]["elapsed_ms"], 1200);
    }
}
