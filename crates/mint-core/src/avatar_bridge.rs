//! Bridge to Project Avatar (https://github.com/projectavatar/projectavatar).
//!
//! Mirrors the design of `@projectavatar/openclaw-avatar`: translate agent
//! lifecycle signals into the relay's `AvatarEvent` schema and POST them to
//! `relay_url/push/:token`, fire-and-forget. The relay fans out over
//! WebSocket to whatever viewer (browser tab / desktop app / OBS source) has
//! the same token open — see `docs/RELAY.md` in the projectavatar repo.
//!
//! SKETCH STATUS: this is a first pass wired into one hook point
//! (`orchestrate_agent_loop`'s `progress` callback, via `on_agent_progress`).
//! It intentionally does not yet cover `message_received` / `agent_end` /
//! `session_end` equivalents — those would hang off wherever chat turns and
//! sessions start/end in `orchestration/mod.rs` and `chat.rs`, the same way
//! the OpenClaw plugin hangs off `api.on('agent_end', ...)`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::orchestration::AgentProgress;

// ── Schema (mirrors packages/openclaw-avatar/src/types.ts) ─────────────────

pub type EmotionBlend = HashMap<&'static str, &'static str>;

#[derive(Debug, Clone, Default, Serialize)]
pub struct AvatarSignal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emotions: Option<EmotionBlend>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prop: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intensity: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub talking: Option<bool>,
}

impl AvatarSignal {
    fn emotions(pairs: &[(&'static str, &'static str)]) -> Self {
        Self {
            emotions: Some(pairs.iter().copied().collect()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct AvatarEvent {
    emotions: EmotionBlend,
    action: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    prop: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    intensity: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    talking: Option<bool>,
}

impl AvatarEvent {
    fn idle() -> Self {
        Self {
            emotions: EmotionBlend::new(),
            action: "idle",
            prop: Some("none"),
            intensity: Some("medium"),
            color: None,
            talking: Some(false),
        }
    }

    fn apply(&self, signal: &AvatarSignal) -> Self {
        Self {
            emotions: signal
                .emotions
                .clone()
                .unwrap_or_else(|| self.emotions.clone()),
            action: signal.action.unwrap_or(self.action),
            prop: signal.prop.or(self.prop),
            intensity: signal.intensity.or(self.intensity),
            color: signal.color.clone().or_else(|| self.color.clone()),
            talking: signal.talking.or(self.talking),
        }
    }
}

// One-shot actions play once and shouldn't be cancelled by the next routine
// tool call landing a moment later.
const ONE_SHOT_ACTIONS: &[&str] = &["celebrating", "greeting", "laughing", "dismissive"];

// ── Full schema vocabulary (mirrors packages/openclaw-avatar/src/types.ts
// exactly) — named here so both the heuristic tool-map above and the
// `avatar_signal` tool's validation (`parse_avatar_signal`, below) check
// against the same lists instead of each hardcoding its own subset. ────────

pub const PRIMARY_EMOTIONS: &[&str] = &[
    "joy", "sadness", "anger", "fear", "surprise", "disgust", "interest",
];
/// Per-emotion blend intensity — finer-grained than `INTENSITIES` below,
/// which only applies to the top-level `intensity` field.
pub const WORD_INTENSITIES: &[&str] = &["subtle", "low", "medium", "high"];
pub const ACTIONS: &[&str] = &[
    "idle",
    "typing",
    "nodding",
    "laughing",
    "celebrating",
    "dismissive",
    "searching",
    "nervous",
    "sad",
    "plotting",
    "greeting",
    "talking",
];
pub const PROPS: &[&str] = &[
    "none",
    "keyboard",
    "magnifying_glass",
    "coffee_cup",
    "book",
    "phone",
    "scroll",
];
pub const INTENSITIES: &[&str] = &["low", "medium", "high"];

fn intern(valid: &'static [&'static str], value: &str) -> Option<&'static str> {
    valid.iter().find(|v| **v == value).copied()
}

/// Validates and converts the `avatar_signal` tool's raw JSON args (built
/// from `AgentInput`'s `avatar_*` fields, re-serialized under their
/// unprefixed wire names — see `orchestration::tools::avatar::execute`) into
/// a real `AvatarSignal`. Shared by that tool's own validation (so the model
/// gets a corrective error string back) and by `on_agent_progress` (so the
/// exact same call actually pushes to the relay) — one validator, not two.
pub fn parse_avatar_signal(input: &serde_json::Value) -> Result<AvatarSignal, String> {
    let mut signal = AvatarSignal::default();

    if let Some(obj) = input.get("emotions").and_then(|v| v.as_object())
        && !obj.is_empty()
    {
        let mut blend = EmotionBlend::new();
        for (emotion, intensity) in obj {
            let emotion = intern(PRIMARY_EMOTIONS, emotion).ok_or_else(|| {
                format!("Unknown emotion \"{emotion}\". Valid emotions: {PRIMARY_EMOTIONS:?}")
            })?;
            let intensity_str = intensity.as_str().unwrap_or("");
            let intensity = intern(WORD_INTENSITIES, intensity_str).ok_or_else(|| {
                format!(
                    "Unknown emotion intensity \"{intensity_str}\" for \"{emotion}\". Valid: {WORD_INTENSITIES:?}"
                )
            })?;
            blend.insert(emotion, intensity);
        }
        signal.emotions = Some(blend);
    }

    if let Some(action) = input.get("action").and_then(|v| v.as_str())
        && !action.is_empty()
    {
        signal.action =
            Some(intern(ACTIONS, action).ok_or_else(|| {
                format!("Unknown action \"{action}\". Valid actions: {ACTIONS:?}")
            })?);
    }

    if let Some(prop) = input.get("prop").and_then(|v| v.as_str())
        && !prop.is_empty()
    {
        signal.prop = Some(
            intern(PROPS, prop)
                .ok_or_else(|| format!("Unknown prop \"{prop}\". Valid props: {PROPS:?}"))?,
        );
    }

    if let Some(intensity) = input.get("intensity").and_then(|v| v.as_str())
        && !intensity.is_empty()
    {
        signal.intensity =
            Some(intern(INTENSITIES, intensity).ok_or_else(|| {
                format!("Unknown intensity \"{intensity}\". Valid: {INTENSITIES:?}")
            })?);
    }

    if let Some(color) = input.get("color").and_then(|v| v.as_str())
        && !color.is_empty()
    {
        signal.color = Some(color.to_string());
    }

    if let Some(talking) = input.get("talking").and_then(|v| v.as_bool()) {
        signal.talking = Some(talking);
    }

    Ok(signal)
}

// ── Multi-session arbitration (mirrors docs/RELAY.md's "Multi-Session
// Arbitration" and session-utils.ts's SessionMeta) ──────────────────────────
//
// The relay lets several concurrent pushers share one channel/token: while a
// lower-`priority`-number session is active (pushed within the last 10s),
// the viewer suppresses events from any higher-numbered (less important)
// session instead of visually fighting over the same avatar. Mint has
// exactly one such case today — `dispatch_subagent` running a nested agent
// loop whose own `ToolStart`/`ToolEnd` events would otherwise stomp on the
// top-level loop's current display state.

/// Per-push session identity. Attached to the outgoing JSON only —
/// deliberately not a field on `AvatarEvent`, since it's push metadata, not
/// part of the avatar's persistent display state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMeta {
    pub session_id: String,
    /// Lower = more important. 0 = main agent loop, 1 = subagent.
    pub priority: u32,
}

impl SessionMeta {
    fn main() -> Self {
        Self {
            session_id: "main".into(),
            priority: 0,
        }
    }

    /// `Thinking`/`Thought` carry no subagent identity in `AgentProgress`
    /// (only `ToolStart`/`ToolEnd` do — see `dispatch_one_subagent`'s
    /// `nested_progress` wrapper in `orchestration/mod.rs`, which retags
    /// only those two variants), so this is only ever called from tool-call
    /// handling.
    fn for_subagent(subagent: Option<&str>) -> Self {
        match subagent {
            None => Self::main(),
            Some(name) => Self {
                session_id: format!("subagent:{name}"),
                priority: 1,
            },
        }
    }
}

// ── Config ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AvatarBridgeConfig {
    pub relay_url: String,
    pub app_url: String,
    pub token: Option<String>,
    pub enabled: bool,
    pub idle_timeout: Duration,
    pub emotion_cooldown: Duration,
    pub action_cooldown: Duration,
    pub one_shot_cooldown: Duration,
}

impl Default for AvatarBridgeConfig {
    fn default() -> Self {
        Self {
            relay_url: "https://relay.projectavatar.io".into(),
            app_url: "https://app.projectavatar.io".into(),
            token: None,
            enabled: true,
            idle_timeout: Duration::from_secs(5),
            emotion_cooldown: Duration::from_millis(2_000),
            action_cooldown: Duration::from_millis(1_500),
            one_shot_cooldown: Duration::from_millis(3_000),
        }
    }
}

impl AvatarBridgeConfig {
    /// Same env-var convention as the OpenClaw plugin's `AVATAR_TOKEN`, plus
    /// optional overrides for self-hosted relays. Mainly useful for headless
    /// gateway deployments where editing `MintConfig` isn't convenient.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        cfg.token = std::env::var("AVATAR_TOKEN").ok().filter(|s| !s.is_empty());
        if let Ok(url) = std::env::var("AVATAR_RELAY_URL") {
            cfg.relay_url = url.trim_end_matches('/').to_string();
        }
        if let Ok(url) = std::env::var("AVATAR_APP_URL") {
            cfg.app_url = url.trim_end_matches('/').to_string();
        }
        cfg.enabled = cfg.token.is_some();
        cfg
    }

    /// Primary source: `MintConfig`'s `avatar_*` fields (set via `mint avatar
    /// link`, persisted to disk). Falls back to the `AVATAR_*` env vars for
    /// anything left blank, so a gateway/CI deployment can still override
    /// without touching the config file.
    pub fn from_mint_config(config: &crate::system::config::MintConfig) -> Self {
        let env = Self::from_env();
        let mut cfg = Self::default();

        cfg.relay_url = if !config.avatar_relay_url.is_empty() {
            config.avatar_relay_url.trim_end_matches('/').to_string()
        } else {
            env.relay_url
        };
        cfg.app_url = if !config.avatar_app_url.is_empty() {
            config.avatar_app_url.trim_end_matches('/').to_string()
        } else {
            env.app_url
        };
        cfg.token = if !config.avatar_token.is_empty() {
            Some(config.avatar_token.clone())
        } else {
            env.token
        };
        cfg.enabled = cfg.token.is_some();
        cfg
    }

    /// A fresh 32-char relay token: `[a-f0-9]{32}`, a subset of the relay's
    /// accepted `[a-zA-Z0-9_-]{32,64}` alphabet. Uses `uuid` (already a
    /// workspace dependency) rather than pulling in `rand` directly — its v4
    /// generator is backed by the OS CSPRNG, which is all a cosmetic
    /// channel token needs.
    pub fn generate_token() -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }

    pub fn share_link(&self) -> Option<String> {
        self.token
            .as_ref()
            .map(|t| format!("{}/?token={}", self.app_url, t))
    }
}

// ── Tool → signal map (mirrors tool-map.ts, using Mint's actual action names) ──

fn resolve_tool_signal(action: &str, had_error: bool, is_start: bool) -> Option<AvatarSignal> {
    match action {
        "run_shell" | "verify" => Some(if is_start {
            AvatarSignal {
                action: Some("typing"),
                prop: Some("keyboard"),
                intensity: Some("high"),
                ..AvatarSignal::emotions(&[("interest", "high")])
            }
        } else if had_error {
            AvatarSignal {
                action: Some("nervous"),
                intensity: Some("high"),
                ..AvatarSignal::emotions(&[("fear", "medium"), ("surprise", "low")])
            }
        } else {
            return None; // silent on success — avatar_signal from the model is the source of truth
        }),

        "browser_open"
        | "browser_click"
        | "browser_type"
        | "browser_read"
        | "browser_mouse_move"
        | "browser_mouse_click"
        | "browser_key_press"
        | "browser_screenshot" => Some(if is_start {
            AvatarSignal {
                action: Some("searching"),
                prop: Some("magnifying_glass"),
                ..AvatarSignal::emotions(&[("interest", "high")])
            }
        } else if had_error {
            AvatarSignal {
                action: Some("dismissive"),
                ..AvatarSignal::emotions(&[("fear", "medium"), ("surprise", "low")])
            }
        } else {
            return None;
        }),

        "generate_image"
        | "image_studio.generate"
        | "generate_video"
        | "veo.generate"
        | "video_generate" => Some(if is_start {
            AvatarSignal {
                action: Some("typing"),
                prop: Some("keyboard"),
                ..AvatarSignal::emotions(&[("joy", "medium"), ("interest", "low")])
            }
        } else if had_error {
            AvatarSignal {
                action: Some("dismissive"),
                ..AvatarSignal::emotions(&[("fear", "medium")])
            }
        } else {
            AvatarSignal {
                action: Some("celebrating"),
                ..AvatarSignal::emotions(&[("joy", "high")])
            }
        }),

        "dispatch_subagent" => Some(if is_start {
            AvatarSignal {
                action: Some("typing"),
                prop: Some("keyboard"),
                ..AvatarSignal::emotions(&[("joy", "medium"), ("interest", "low")])
            }
        } else if had_error {
            AvatarSignal {
                action: Some("nervous"),
                ..AvatarSignal::emotions(&[("fear", "medium")])
            }
        } else {
            AvatarSignal {
                action: Some("celebrating"),
                ..AvatarSignal::emotions(&[("joy", "high")])
            }
        }),

        // Everything else (list_files, read_file, semantic_search, ask_user, ...)
        // is intentionally silent — same "less is more" rule as the TS tool map,
        // to avoid visual jitter when several routine tools fire in one turn.
        _ => None,
    }
}

fn had_error(result: &str) -> bool {
    result.starts_with("Error") || result.starts_with("Blocked") || result.starts_with("Skipped")
}

// ── `mint avatar status` — GET /channel/:token/state (mirrors avatar-command-tool.ts) ──

#[derive(Debug, Deserialize)]
pub struct ChannelState {
    pub model: Option<String>,
    #[serde(rename = "lastAgentEventAt")]
    pub last_agent_event_at: Option<i64>,
    #[serde(rename = "connectedClients")]
    pub connected_clients: u32,
}

pub async fn fetch_channel_state(cfg: &AvatarBridgeConfig) -> Result<ChannelState, String> {
    let token = cfg.token.as_deref().ok_or("AVATAR_TOKEN not set")?;
    let url = format!("{}/channel/{}/state", cfg.relay_url, token);
    let res = reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Relay timed out — check the relay URL and network.".to_string()
            } else {
                format!("Could not reach relay: {e}")
            }
        })?;
    if !res.status().is_success() {
        return Err(format!("Relay returned HTTP {}", res.status()));
    }
    res.json::<ChannelState>()
        .await
        .map_err(|e| format!("Unexpected relay response: {e}"))
}

// ── State machine + relay client ────────────────────────────────────────────

struct State {
    current: AvatarEvent,
    // `None` = no change yet this session, so the very first signal is never
    // rate-limited against the bridge's own construction time.
    last_emotion_change: Option<Instant>,
    last_action_change: Option<Instant>,
    one_shot_active_at: Option<Instant>,
    /// Fallback session for a `transition` call that doesn't know its own
    /// session (`Thinking`/`on_talking`'s idle-out) — mirrors the TS state
    /// machine's `lastSession`.
    last_session: Option<SessionMeta>,
}

pub struct AvatarBridge {
    cfg: AvatarBridgeConfig,
    client: reqwest::Client,
    state: Mutex<State>,
}

impl AvatarBridge {
    pub fn new(cfg: AvatarBridgeConfig) -> Self {
        Self {
            cfg,
            client: reqwest::Client::new(),
            state: Mutex::new(State {
                current: AvatarEvent::idle(),
                last_emotion_change: None,
                last_action_change: None,
                one_shot_active_at: None,
                last_session: None,
            }),
        }
    }

    pub fn is_active(&self) -> bool {
        self.cfg.enabled && self.cfg.token.is_some()
    }

    /// Hook this into whatever emits `AgentProgress` for a turn (currently
    /// wired at the `progress_cb` closure in `src-tauri/src/lib.rs` around
    /// the `orchestrate_agent_loop` call).
    pub fn on_agent_progress(&self, progress: &AgentProgress) {
        if !self.is_active() {
            return;
        }
        match progress {
            AgentProgress::ToolStart {
                action,
                input,
                subagent,
            } => {
                let session = SessionMeta::for_subagent(subagent.as_deref());
                // The model's own explicit signal is the primary source of
                // truth (see avatar_bridge module docs) — it IS the signal,
                // so it skips the heuristic tool map entirely rather than
                // stacking on top of it. Validation errors are surfaced to
                // the model via `orchestration::tools::avatar::execute`'s
                // own call to `parse_avatar_signal`, not here — a bad call
                // just silently doesn't move the avatar.
                if action == "avatar_signal" {
                    if let Ok(signal) = parse_avatar_signal(input) {
                        self.transition(signal, Some(session));
                    }
                    return;
                }
                if let Some(signal) = resolve_tool_signal(action, false, true) {
                    self.transition(signal, Some(session));
                }
            }
            AgentProgress::ToolEnd {
                action,
                result,
                subagent,
                ..
            } => {
                if action == "avatar_signal" {
                    return; // already handled on ToolStart
                }
                if let Some(signal) = resolve_tool_signal(action, had_error(result), false) {
                    let session = SessionMeta::for_subagent(subagent.as_deref());
                    self.transition(signal, Some(session));
                }
            }
            AgentProgress::Thinking { .. } => {
                // No subagent identity available here (see `SessionMeta::for_subagent`'s
                // doc) — falls back to whichever session last pushed.
                self.transition(AvatarSignal::emotions(&[("interest", "medium")]), None);
            }
            AgentProgress::Thought { .. } => {}
            AgentProgress::WaitingForNetwork { .. } => {}
        }
    }

    /// Mouth/talking layer — orthogonal to `action`, mirrors the OpenClaw
    /// plugin's `onAgentEvent` subscription (`stream === 'assistant'` →
    /// `talking: true`, lifecycle end/error → `talking: false`). Not
    /// cooldown-gated like emotions/action, since it needs to flip promptly
    /// around actual speech rather than being rate-limited.
    pub fn on_talking(&self, talking: bool) {
        if !self.is_active() {
            return;
        }
        // Only ever called for the top-level turn (the outer `on_chunk` in
        // src-tauri/mint-cli, never a subagent's own text) — always priority 0.
        self.transition(
            AvatarSignal {
                talking: Some(talking),
                ..Default::default()
            },
            Some(SessionMeta::main()),
        );
    }

    /// Called once a turn finishes, success or failure — mirrors `agent_end`.
    /// Same as `on_talking`: only ever the top-level turn, so always `main`.
    pub fn on_turn_end(&self, success: bool) {
        if !self.is_active() {
            return;
        }
        if !success {
            self.transition(
                AvatarSignal {
                    action: Some("dismissive"),
                    intensity: Some("high"),
                    ..AvatarSignal::emotions(&[("fear", "medium")])
                },
                Some(SessionMeta::main()),
            );
        }
        self.schedule_idle(Some(SessionMeta::main()));
    }

    fn transition(&self, signal: AvatarSignal, session: Option<SessionMeta>) {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap();

        if let Some(session) = &session {
            state.last_session = Some(session.clone());
        }
        let effective_session = session.or_else(|| state.last_session.clone());

        let emotion_changed = signal
            .emotions
            .as_ref()
            .is_some_and(|e| e != &state.current.emotions);
        let action_changed = signal.action.is_some_and(|a| a != state.current.action);

        // Per-category cooldowns, same idea as state-machine.ts — an emotion
        // update and an action update rate-limit independently.
        let mut applied = signal.clone();
        if emotion_changed
            && state
                .last_emotion_change
                .is_some_and(|t| now.duration_since(t) < self.cfg.emotion_cooldown)
        {
            applied.emotions = None;
        }
        if action_changed {
            let one_shot_blocking = state
                .one_shot_active_at
                .is_some_and(|t| now.duration_since(t) < self.cfg.one_shot_cooldown);
            let cooling_down = state
                .last_action_change
                .is_some_and(|t| now.duration_since(t) < self.cfg.action_cooldown);
            if one_shot_blocking || cooling_down {
                applied.action = None;
            }
        }

        if applied.emotions.is_none()
            && applied.action.is_none()
            && applied.prop.is_none()
            && applied.intensity.is_none()
            && applied.talking.is_none()
        {
            return; // fully suppressed by cooldowns — drop it (no pending-flush retry in this sketch)
        }

        let next = state.current.apply(&applied);
        if applied.emotions.is_some() {
            state.last_emotion_change = Some(now);
        }
        if applied.action.is_some() {
            state.last_action_change = Some(now);
            if ONE_SHOT_ACTIONS.contains(&next.action) {
                state.one_shot_active_at = Some(now);
            }
        }
        state.current = next.clone();
        drop(state);

        self.push(next, effective_session);
    }

    fn schedule_idle(&self, session: Option<SessionMeta>) {
        // Simplified vs. the TS version's cancellable timer: just push idle
        // straight away once a turn ends. Good enough for a first pass —
        // swap for a real delayed task if turns overlap in practice.
        self.transition(
            AvatarSignal {
                action: Some("idle"),
                talking: Some(false),
                ..Default::default()
            },
            session,
        );
    }

    fn push(&self, event: AvatarEvent, session: Option<SessionMeta>) {
        let Some(token) = self.cfg.token.clone() else {
            return;
        };
        let url = format!("{}/push/{}", self.cfg.relay_url, token);
        let client = self.client.clone();
        let payload = PushPayload {
            event: &event,
            session_id: session.as_ref().map(|s| s.session_id.as_str()),
            priority: session.as_ref().map(|s| s.priority),
        };
        let body = match serde_json::to_string(&payload) {
            Ok(b) => b,
            Err(_) => return,
        };
        tokio::spawn(async move {
            // Fire-and-forget, same as relay-client.ts — the avatar is cosmetic.
            let _ = client
                .post(url)
                .header("Content-Type", "application/json")
                .body(body)
                .timeout(Duration::from_secs(5))
                .send()
                .await;
        });
    }
}

/// Wire shape actually POSTed — `AvatarEvent`'s fields flattened alongside
/// the relay's `sessionId`/`priority`, which are push metadata rather than
/// part of the avatar's persistent display state (see `SessionMeta`'s docs).
#[derive(Serialize)]
struct PushPayload<'a> {
    #[serde(flatten)]
    event: &'a AvatarEvent,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> AvatarBridgeConfig {
        AvatarBridgeConfig {
            token: Some("test-token".into()),
            emotion_cooldown: Duration::from_millis(200),
            action_cooldown: Duration::from_millis(200),
            one_shot_cooldown: Duration::from_millis(200),
            ..AvatarBridgeConfig::default()
        }
    }

    #[tokio::test]
    async fn first_signal_after_construction_is_not_cooldown_blocked() {
        // Regression test: last_change used to be seeded with Instant::now()
        // at construction, so a signal arriving milliseconds after startup —
        // well within a typical cooldown window — would be silently dropped.
        let bridge = AvatarBridge::new(test_cfg());
        bridge.transition(
            AvatarSignal {
                action: Some("typing"),
                ..Default::default()
            },
            None,
        );
        assert_eq!(bridge.state.lock().unwrap().current.action, "typing");
    }

    #[tokio::test]
    async fn action_cooldown_suppresses_rapid_repeat_changes() {
        let bridge = AvatarBridge::new(test_cfg());
        bridge.transition(
            AvatarSignal {
                action: Some("typing"),
                ..Default::default()
            },
            None,
        );
        bridge.transition(
            AvatarSignal {
                action: Some("searching"),
                ..Default::default()
            },
            None,
        );
        // Still "typing" — the second action change lands inside the cooldown window.
        assert_eq!(bridge.state.lock().unwrap().current.action, "typing");
    }

    #[tokio::test]
    async fn one_shot_action_blocks_action_changes_until_its_cooldown_expires() {
        let bridge = AvatarBridge::new(test_cfg());
        bridge.transition(
            AvatarSignal {
                action: Some("celebrating"),
                ..Default::default()
            },
            None,
        );
        bridge.transition(
            AvatarSignal {
                action: Some("idle"),
                ..Default::default()
            },
            None,
        );
        assert_eq!(bridge.state.lock().unwrap().current.action, "celebrating");
    }

    #[tokio::test]
    async fn tool_start_without_subagent_uses_main_session_at_priority_zero() {
        let bridge = AvatarBridge::new(test_cfg());
        bridge.on_agent_progress(&AgentProgress::ToolStart {
            action: "run_shell".into(),
            input: serde_json::Value::Null,
            subagent: None,
        });
        let session = bridge.state.lock().unwrap().last_session.clone();
        assert_eq!(
            session,
            Some(SessionMeta {
                session_id: "main".into(),
                priority: 0
            })
        );
    }

    #[tokio::test]
    async fn tool_start_inside_a_subagent_gets_its_own_lower_priority_session() {
        let bridge = AvatarBridge::new(test_cfg());
        bridge.on_agent_progress(&AgentProgress::ToolStart {
            action: "run_shell".into(),
            input: serde_json::Value::Null,
            subagent: Some("researcher".into()),
        });
        let session = bridge.state.lock().unwrap().last_session.clone();
        assert_eq!(
            session,
            Some(SessionMeta {
                session_id: "subagent:researcher".into(),
                priority: 1
            })
        );
    }

    #[tokio::test]
    async fn thinking_falls_back_to_the_last_known_session() {
        let bridge = AvatarBridge::new(test_cfg());
        bridge.on_agent_progress(&AgentProgress::ToolStart {
            action: "run_shell".into(),
            input: serde_json::Value::Null,
            subagent: Some("researcher".into()),
        });
        bridge.on_agent_progress(&AgentProgress::Thinking {
            elapsed_secs: 1,
            agent_name: None,
            model_name: None,
            context_pct: None,
        });
        // Thinking carries no subagent identity of its own — it should still
        // be attributed to whichever session pushed last, not silently reset
        // to "main" (which would let a subagent's own Thinking event
        // wrongly outrank/interleave with the top-level loop's session).
        let session = bridge.state.lock().unwrap().last_session.clone();
        assert_eq!(
            session,
            Some(SessionMeta {
                session_id: "subagent:researcher".into(),
                priority: 1
            })
        );
    }

    #[test]
    fn generated_token_matches_relay_alphabet_and_length() {
        let token = AvatarBridgeConfig::generate_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn resolve_tool_signal_is_silent_for_routine_and_successful_high_signal_tools() {
        assert!(resolve_tool_signal("read_file", false, true).is_none());
        assert!(resolve_tool_signal("run_shell", false, false).is_none());
        assert!(resolve_tool_signal("run_shell", true, false).is_some());
        assert!(resolve_tool_signal("run_shell", false, true).is_some());
    }

    #[test]
    fn from_mint_config_prefers_config_token_over_env_var() {
        // SAFETY: test-only env mutation; no other test in this module reads AVATAR_TOKEN.
        unsafe { std::env::remove_var("AVATAR_TOKEN") };
        let mut config = crate::system::config::MintConfig::default();
        config.avatar_token = "from-config".into();
        let cfg = AvatarBridgeConfig::from_mint_config(&config);
        assert_eq!(cfg.token.as_deref(), Some("from-config"));
        assert!(cfg.enabled);
    }

    #[test]
    fn from_mint_config_disabled_when_token_blank_and_no_env_var() {
        unsafe { std::env::remove_var("AVATAR_TOKEN") };
        let config = crate::system::config::MintConfig::default();
        let cfg = AvatarBridgeConfig::from_mint_config(&config);
        assert!(cfg.token.is_none());
        assert!(!cfg.enabled);
    }

    #[test]
    fn parse_avatar_signal_accepts_a_valid_partial_signal() {
        let value = serde_json::json!({
            "emotions": { "joy": "high", "interest": "medium" },
            "action": "greeting",
            "prop": "coffee_cup",
            "intensity": "low",
            "color": "#ff00ff",
            "talking": true,
        });
        let signal = parse_avatar_signal(&value).expect("should parse");
        assert_eq!(signal.action, Some("greeting"));
        assert_eq!(signal.prop, Some("coffee_cup"));
        assert_eq!(signal.intensity, Some("low"));
        assert_eq!(signal.color.as_deref(), Some("#ff00ff"));
        assert_eq!(signal.talking, Some(true));
        let emotions = signal.emotions.expect("emotions should be set");
        assert_eq!(emotions.get("joy"), Some(&"high"));
        assert_eq!(emotions.get("interest"), Some(&"medium"));
    }

    #[test]
    fn parse_avatar_signal_ignores_fields_the_model_left_unset() {
        let signal = parse_avatar_signal(&serde_json::json!({})).expect("empty call is valid");
        assert!(signal.emotions.is_none());
        assert!(signal.action.is_none());
        assert!(signal.prop.is_none());
        assert!(signal.intensity.is_none());
        assert!(signal.color.is_none());
        assert!(signal.talking.is_none());
    }

    #[test]
    fn parse_avatar_signal_rejects_an_unknown_action_with_a_corrective_message() {
        let err = parse_avatar_signal(&serde_json::json!({ "action": "backflip" }))
            .expect_err("backflip is not a valid action");
        assert!(err.contains("backflip"));
        assert!(err.contains("greeting")); // names a real valid option
    }

    #[test]
    fn parse_avatar_signal_rejects_an_unknown_emotion() {
        let err = parse_avatar_signal(&serde_json::json!({ "emotions": { "hangry": "high" } }))
            .expect_err("hangry is not a real emotion");
        assert!(err.contains("hangry"));
    }

    #[test]
    fn parse_avatar_signal_rejects_an_unknown_emotion_intensity() {
        let err = parse_avatar_signal(&serde_json::json!({ "emotions": { "joy": "ecstatic" } }))
            .expect_err("ecstatic is not a valid word intensity");
        assert!(err.contains("ecstatic"));
    }
}
