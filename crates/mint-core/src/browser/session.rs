//! Run-scoped browser control. Task locals pin every nested CDP call; file locks
//! also prevent a second Mint process from operating on a leased tab.
use super::{
    cdp::{cdp_call_raw, fetch_pages},
    interact::selector_to_js_find,
    logging::log_action,
};
use crate::MintConfig;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    future::Future,
    hash::{Hash, Hasher},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

tokio::task_local! {
    static SESSION: Arc<Mutex<BrowserSession>>;
    static TARGET: String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserObservation {
    pub tab_id: String,
    pub url: String,
    pub title: String,
    pub observation_id: String,
    pub text: String,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub elements: Vec<Value>,
    pub text_truncated: bool,
    pub elements_truncated: bool,
    pub next_text_offset: Option<usize>,
    pub next_element_offset: Option<usize>,
    pub unsupported_frames: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserActionResult {
    pub status: String,
    pub verification: String,
    pub observed_changes: Vec<String>,
    pub new_tabs: Vec<super::BrowserTab>,
    pub observation: Option<BrowserObservation>,
    pub error: Option<String>,
}

pub struct BrowserSession {
    id: String,
    tab: Option<String>,
    leases: BTreeMap<String, TabLease>,
    observation: Option<BrowserObservation>,
    last_known: Option<BrowserObservation>,
    screenshot: Option<super::evidence::ScreenshotEvidence>,
    attempts: BTreeMap<ActionAttemptKey, usize>,
    preceding_attempt: Option<ActionAttemptKey>,
    blocked: bool,
    used: bool,
}

impl Default for BrowserSession {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tab: None,
            leases: BTreeMap::new(),
            observation: None,
            last_known: None,
            screenshot: None,
            attempts: BTreeMap::new(),
            preceding_attempt: None,
            blocked: false,
            used: false,
        }
    }
}

pub async fn run_session<F: Future>(future: F) -> F::Output {
    SESSION
        .scope(Arc::new(Mutex::new(BrowserSession::default())), future)
        .await
}

pub(crate) fn selected_tab() -> Option<String> {
    TARGET.try_with(Clone::clone).ok()
}

// Explicit unlock also releases a briefly inherited descriptor during a
// concurrent fork/exec; relying on the last descriptor closing can leave a
// finished tab looking busy until the child reaches exec.
struct TabLease(File);
impl Drop for TabLease {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn lease(config: &MintConfig, tab: &str) -> Result<TabLease, String> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    endpoint(config).hash(&mut hash);
    tab.hash(&mut hash);
    let dir = std::env::temp_dir().join("mint-browser-leases");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(format!("{:x}.lock", hash.finish())))
        .map_err(|e| e.to_string())?;
    file.try_lock()
        .map_err(|_| "tab_busy: another Mint operation owns this tab".to_string())?;
    Ok(TabLease(file))
}

/// Desktop callers can opt into explicit targeting without breaking old calls.
pub async fn with_tab<F: Future>(
    config: &MintConfig,
    tab: Option<&str>,
    future: F,
) -> Result<F::Output, String> {
    let id = match tab {
        Some(id) => id.to_owned(),
        None => fetch_pages(config)
            .await?
            .iter()
            .find(|p| p["type"] == "page")
            .and_then(|p| p["id"].as_str())
            .ok_or("tab_unavailable")?
            .to_owned(),
    };
    let _lease = lease(config, &id)?;
    Ok(TARGET.scope(id, future).await)
}

fn endpoint(config: &MintConfig) -> &str {
    config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list")
}

async fn tab_request(config: &MintConfig, operation: &str) -> Result<Value, String> {
    let mut url = reqwest::Url::parse(endpoint(config)).map_err(|e| e.to_string())?;
    let (path, query) = operation
        .split_once('?')
        .map_or((operation, None), |(p, q)| (p, Some(q)));
    url.set_path(&format!("/json/{path}"));
    url.set_query(query);
    let response = crate::HTTP_CLIENT
        .put(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if operation.starts_with("close/") {
        Ok(Value::Null)
    } else {
        response.json().await.map_err(|e| e.to_string())
    }
}

pub(super) async fn evaluate(config: &MintConfig, expression: String) -> Result<Value, String> {
    let response = cdp_call_raw(
        config,
        "Runtime.evaluate",
        json!({"expression":expression,"returnByValue":true}),
    )
    .await?;
    Ok(response["result"]["result"]["value"].clone())
}

async fn observe(
    config: &MintConfig,
    tab: &str,
    args: &Value,
) -> Result<BrowserObservation, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let text_offset = args["textOffset"].as_u64().unwrap_or(0);
    let element_offset = args["elementOffset"].as_u64().unwrap_or(0);
    let script = format!(
        "const observationId={}; const textOffset={text_offset}, elementOffset={element_offset};",
        json!(id)
    ) + include_str!("observe.js");
    let value = evaluate(config, format!("(() => {{ {script} }})()")).await?;
    let mut observation: BrowserObservation =
        serde_json::from_value(value).map_err(|e| format!("observation_invalid: {e}"))?;
    observation.tab_id = tab.to_owned();
    Ok(observation)
}

async fn observe_retry(
    config: &MintConfig,
    tab: &str,
    args: &Value,
) -> Result<BrowserObservation, String> {
    match observe(config, tab, args).await {
        Ok(value) => Ok(value),
        Err(first)
            if first.contains("timeout")
                || first.contains("disconnected")
                || first.contains("context") =>
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
            observe(config, tab, args).await
        }
        Err(error) => Err(error),
    }
}

fn target(args: &Value, session: &BrowserSession) -> Result<String, String> {
    let reference = args["elementRef"].as_str().unwrap_or("");
    let selector = args["selector"].as_str().unwrap_or("").trim();
    if !reference.is_empty() && !selector.is_empty() {
        return Err("Provide elementRef or selector, not both".into());
    }
    if !reference.is_empty() {
        if !session.observation.as_ref().is_some_and(|o| {
            o.elements
                .iter()
                .any(|e| e["ref"].as_str() == Some(reference))
        }) {
            return Err("stale_reference: refresh browser_observe".into());
        }
        Ok(format!("mint-ref={reference}"))
    } else if selector.is_empty()
        || [
            "mint-ref=",
            "mint-edit=",
            "mint-attempt=",
            "mint-focus=",
            "mint-point=",
        ]
        .iter()
        .any(|prefix| selector.starts_with(prefix))
    {
        Err("A selector or observed elementRef is required".into())
    } else if selector.len() > 500 {
        Err("browser selector must contain between 1 and 500 characters".into())
    } else {
        Ok(selector.to_owned())
    }
}

fn modifying(action: &str) -> bool {
    matches!(
        action,
        "browser_open"
            | "browser_click"
            | "browser_type"
            | "browser_fill"
            | "browser_select"
            | "browser_scroll"
            | "browser_mouse_click"
            | "browser_key_press"
            | "browser_mouse_move"
    )
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ActionAttemptKey {
    tab: String,
    action: String,
    target: String,
    arguments: String,
}

async fn target_state(config: &MintConfig, selector: Option<&str>) -> Result<Value, String> {
    let Some(selector) = selector else {
        return Ok(Value::Null);
    };
    evaluate(config, format!("(() => {{ {} globalThis.__mintAttemptTarget=el; if (!el?.isConnected) return null; const nodes=globalThis.__mintNodes ||= {{ids:new WeakMap(),next:0,documentId:(crypto.randomUUID?.() || String(performance.timeOrigin)+Math.random())}}; if (!nodes.ids.has(el)) nodes.ids.set(el,++nodes.next); return {{id:nodes.documentId+':'+nodes.ids.get(el),value:el.value ?? null,checked:el.checked ?? null,expanded:el.getAttribute('aria-expanded')}}; }})()",selector_to_js_find(selector))).await
}

fn attempt_arguments(action: &str, args: &Value) -> String {
    match action {
        "browser_type" | "browser_fill" => json!([args["text"].as_str().unwrap_or("")]),
        "browser_select" => json!([args["value"].as_str().unwrap_or("")]),
        "browser_open" => json!([args["url"].as_str().unwrap_or("").trim()]),
        "browser_key_press" => json!([args["key"].as_str().unwrap_or("")]),
        "browser_scroll" => json!([
            args["x"].as_f64().unwrap_or(0.0),
            args["y"].as_f64().unwrap_or(600.0)
        ]),
        "browser_mouse_click" | "browser_mouse_move" => json!([
            args["x"],
            args["y"],
            args["button"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("left")
        ]),
        _ => json!([]),
    }
    .to_string()
}

pub(crate) async fn finish_evidence() -> bool {
    match SESSION.try_with(|s| s.try_lock().map(|s| s.used).unwrap_or(true)) {
        Ok(used) => used,
        Err(_) => false,
    }
}

pub(crate) async fn last_state() -> String {
    SESSION
        .try_with(|s| {
            s.try_lock().ok().and_then(|s| {
                s.last_known
                    .as_ref()
                    .map(|o| format!("tab={}, url={}, title={}", o.tab_id, o.url, o.title))
            })
        })
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Box the operation future so callers can compose many browser steps without
/// multiplying the CDP transport's async state on their stack.
pub fn execute_action<'a>(
    config: &'a MintConfig,
    action: &'a str,
    args: &'a Value,
) -> std::pin::Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(async move {
        if SESSION.try_with(|_| ()).is_err() {
            return run_session(execute_action(config, action, args)).await;
        }
        let started = Instant::now();
        let state = SESSION.with(Arc::clone);
        let mut session = state.lock().await;
        let result = execute_owned(config, action, args, &mut session).await;
        let report = result
            .as_ref()
            .ok()
            .and_then(|r| serde_json::from_str::<Value>(r).ok());
        log_action(
            "SESSION_ACTION",
            &format!(
                "session={} tab={} action={} durationMs={} status={} verification={}",
                session.id,
                session.tab.as_deref().unwrap_or("none"),
                action,
                started.elapsed().as_millis(),
                report
                    .as_ref()
                    .and_then(|r| r["status"].as_str())
                    .unwrap_or(if result.is_ok() { "returned" } else { "failed" }),
                report
                    .as_ref()
                    .and_then(|r| r["verification"].as_str())
                    .unwrap_or("unverified")
            ),
        );
        result
    })
}

async fn execute_owned(
    config: &MintConfig,
    action: &str,
    args: &Value,
    session: &mut BrowserSession,
) -> Result<String, String> {
    session.used = true;
    let tab_operation = args["operation"].as_str().unwrap_or("list");
    if session.blocked
        && (modifying(action)
            || (action == "browser_tabs" && matches!(tab_operation, "open" | "close")))
    {
        return Err(
            "browser_blocked: three attempts without progress; ask the user for takeover".into(),
        );
    }
    if action == "browser_open" {
        super::navigate::validate_url(args["url"].as_str().unwrap_or("").trim())?;
    }
    if session.tab.is_none() && action != "browser_tabs" {
        let page = tab_request(config, "new?about:blank").await?;
        let tab = page["id"]
            .as_str()
            .ok_or("Could not create task tab")?
            .to_owned();
        session.leases.insert(tab.clone(), lease(config, &tab)?);
        session.tab = Some(tab);
    }
    if action == "browser_tabs" {
        return tabs(config, args, session).await;
    }
    let tab = session.tab.clone().ok_or("tab_unavailable")?;
    if !fetch_pages(config)
        .await?
        .iter()
        .any(|p| p["type"] == "page" && p["id"] == tab)
    {
        return Err("tab_unavailable: select another tab explicitly".into());
    }
    TARGET
        .scope(tab.clone(), async {
            if action == "browser_screenshot" {
                let (data, evidence) = super::evidence::capture(config, &tab).await?;
                session.screenshot = Some(evidence);
                return Ok(format!("data:image/png;base64,{data}"));
            }
            if action == "browser_read" {
                return super::read_page_text(config).await;
            }
            if action == "browser_observe" {
                let observation = observe_retry(config, &tab, args).await?;
                session.observation = Some(observation.clone());
                session.last_known = Some(observation.clone());
                // Explicit takeover confirmation is the only way to clear a recovery stop.
                if args["takeoverComplete"] == true {
                    session.blocked = false;
                    session.attempts.clear();
                    session.preceding_attempt = None;
                }
                return serde_json::to_string(&observation).map_err(|e| e.to_string());
            }
            if action == "browser_wait" {
                wait(config, args, session).await?;
                if let Some(key) = session.preceding_attempt.take() {
                    session.attempts.remove(&key);
                }
                let observation = observe_retry(config, &tab, &json!({})).await?;
                session.observation = Some(observation.clone());
                session.last_known = Some(observation.clone());
                return serde_json::to_string(&BrowserActionResult {
                    status: "condition_met".into(),
                    verification: "verified".into(),
                    observed_changes: vec!["Requested condition observed".into()],
                    new_tabs: vec![],
                    observation: Some(observation),
                    error: None,
                })
                .map_err(|e| e.to_string());
            }
            let mut selector = if matches!(
                action,
                "browser_click" | "browser_type" | "browser_fill" | "browser_select"
            ) || (action == "browser_scroll"
                && (args["elementRef"].as_str().is_some_and(|s| !s.is_empty())
                    || args["selector"].as_str().is_some_and(|s| !s.is_empty())))
            {
                Some(target(args, session)?)
            } else {
                None
            };
            if matches!(action, "browser_mouse_click" | "browser_mouse_move") {
                let evidence = session
                    .screenshot
                    .as_ref()
                    .ok_or("screenshot_required: capture this tab before coordinate input")?;
                super::evidence::validate(config, evidence, &tab, args).await?;
            }
            if matches!(action, "browser_mouse_click" | "browser_mouse_move") {
                selector = Some(format!(
                    "mint-point={},{}",
                    args["x"].as_f64().ok_or("x required")?,
                    args["y"].as_f64().ok_or("y required")?
                ));
            } else if action == "browser_key_press" {
                selector = Some("mint-focus=".into());
            }
            let relevant_before = target_state(config, selector.as_deref()).await?;
            let pinned_selector = selector.as_ref().map(|_| "mint-attempt=".to_string());
            let before = observe_retry(config, &tab, &json!({})).await?;
            let before_document =
                evaluate(config, "globalThis.__mintNodes?.documentId".into()).await?;
            let before_tabs = super::list_tabs(config).await?;
            let key = ActionAttemptKey {
                tab: tab.clone(),
                action: action.into(),
                target: relevant_before["id"]
                    .as_str()
                    .unwrap_or(selector.as_deref().unwrap_or("viewport"))
                    .into(),
                arguments: attempt_arguments(action, args),
            };
            session.preceding_attempt = Some(key.clone());
            let result = if matches!(action, "browser_mouse_click" | "browser_mouse_move") {
                super::evidence::input(
                    config,
                    session.screenshot.as_ref().unwrap(),
                    &tab,
                    action,
                    args,
                )
                .await
            } else {
                perform(config, action, args, pinned_selector.as_deref()).await
            };
            let relevant_after = target_state(config, pinned_selector.as_deref())
                .await
                .unwrap_or(Value::Null);
            // Never repeat side effects to recover a failed or timed-out response.
            let after = observe_retry(config, &tab, &json!({})).await;
            let after_document = evaluate(config, "globalThis.__mintNodes?.documentId".into())
                .await
                .unwrap_or(Value::Null);
            session.screenshot = None;
            let new_tabs: Vec<super::BrowserTab> = super::list_tabs(config)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|t| !before_tabs.iter().any(|old| old.id == t.id))
                .collect();
            let mut changes = Vec::new();
            let mut verification = "unverified".to_string();
            let observation = after.ok();
            if let Some(after) = &observation {
                if after.url != before.url {
                    changes.push("URL changed".into());
                }
                let viewport_changed =
                    after.scroll_x != before.scroll_x || after.scroll_y != before.scroll_y;
                if viewport_changed {
                    changes.push("Viewport moved".into());
                }
                let progress = after.url != before.url
                    || (!before_document.is_null()
                        && !after_document.is_null()
                        && before_document != after_document)
                    || !new_tabs.is_empty()
                    || (!relevant_before.is_null()
                        && relevant_before["id"] == relevant_after["id"]
                        && relevant_before != relevant_after)
                    || (action == "browser_scroll"
                        && (viewport_changed
                            || result.as_ref().is_ok_and(|v| v == "scroll_verified")));
                if progress {
                    changes.push(
                        "Action outcome changed; inspect observation for task completion".into(),
                    );
                    session.attempts.remove(&key);
                } else {
                    let attempts = session.attempts.entry(key).or_default();
                    *attempts += 1;
                    if *attempts >= 3 {
                        session.blocked = true;
                    }
                }
                if result.as_ref().is_ok_and(|value| {
                    matches!(value.as_str(), "value_verified" | "scroll_verified")
                }) {
                    verification = "verified".into();
                }
                session.observation = Some(after.clone());
                session.last_known = Some(after.clone());
            } else {
                session.observation = None;
                let attempts = session.attempts.entry(key).or_default();
                *attempts += 1;
                if *attempts >= 3 {
                    session.blocked = true;
                }
            }
            let error = result.err().or_else(|| {
                if session.blocked {
                    Some("browser_blocked: no progress after three attempts".into())
                } else if observation.is_none() {
                    Some("observation_unavailable: action outcome is ambiguous".into())
                } else {
                    None
                }
            });
            serde_json::to_string(&BrowserActionResult {
                status: if error.is_some() {
                    "failed_or_ambiguous"
                } else {
                    "input_dispatched"
                }
                .into(),
                verification,
                observed_changes: changes,
                new_tabs,
                observation,
                error,
            })
            .map_err(|e| e.to_string())
        })
        .await
}

async fn tabs(
    config: &MintConfig,
    args: &Value,
    session: &mut BrowserSession,
) -> Result<String, String> {
    let operation = args["operation"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("list");
    match operation {
        "list" => {}
        "open" => {
            let url = args["url"].as_str().unwrap_or("").trim();
            if !url.is_empty() {
                super::navigate::validate_url(url)?;
            }
            let tab = tab_request(config, "new?about:blank").await?;
            let id = tab["id"].as_str().ok_or("Missing tab ID")?.to_owned();
            session.leases.insert(id.clone(), lease(config, &id)?);
            session.tab = Some(id.clone());
            session.observation = None;
            session.screenshot = None;
            session.preceding_attempt = None;
            if !url.is_empty() {
                TARGET.scope(id, super::navigate(config, url)).await?;
            }
            session.observation = None;
            session.screenshot = None;
            session.preceding_attempt = None;
        }
        "select" | "close" => {
            let id = args["tabId"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Explicit tabId required")?;
            if !super::list_tabs(config).await?.iter().any(|t| t.id == id) {
                return Err("tab_unavailable".into());
            }
            if !session.leases.contains_key(id) {
                session.leases.insert(id.to_owned(), lease(config, id)?);
            }
            if operation == "close" {
                tab_request(config, &format!("close/{id}")).await?;
                session.leases.remove(id);
                // Retain the closed ID: later calls must fail instead of choosing a new tab.
            } else {
                session.tab = Some(id.to_owned());
            }
            session.observation = None;
            session.screenshot = None;
            session.preceding_attempt = None;
        }
        _ => return Err("Unknown tab operation".into()),
    }
    Ok(json!({"selectedTabId":session.tab,"tabs":super::list_tabs(config).await?}).to_string())
}

async fn perform(
    config: &MintConfig,
    action: &str,
    args: &Value,
    selector: Option<&str>,
) -> Result<String, String> {
    let text = args["text"].as_str().unwrap_or("");
    match action {
        "browser_open" => {
            super::navigate(config, args["url"].as_str().ok_or("url required")?.trim()).await
        }
        "browser_click" => super::click(config, selector.ok_or("target required")?).await,
        "browser_type" => super::type_text(config, selector.ok_or("target required")?, text).await,
        "browser_fill" => {
            super::interact::edit_text(config, selector.ok_or("target required")?, text, true).await
        }
        "browser_select" => {
            let selector = selector.ok_or("target required")?;
            super::get_element_coordinates(config, selector).await?;
            let value = args["value"].as_str().ok_or("value required")?;
            let find = selector_to_js_find(selector);
            let verified = evaluate(config,format!("(() => {{ {find} if (el.tagName !== 'SELECT' || el.multiple) throw new Error('unsupported_select_target'); const option = Array.from(el.options).find(o => o.value === {}); if (!option || option.disabled || option.closest('optgroup')?.disabled) throw new Error('option_unavailable'); el.value = {}; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); return el.value; }})()",json!(value),json!(value))).await?;
            if verified.as_str() != Some(value) {
                return Err("field_value_mismatch".into());
            }
            Ok("value_verified".into())
        }
        "browser_scroll" => {
            let x = args["x"].as_f64().unwrap_or(0.0);
            let y = args["y"].as_f64().unwrap_or(600.0);
            let expression = if let Some(selector) = selector {
                format!(
                    "(() => {{ {} if (!el || !el.isConnected) throw new Error('stale_reference'); const old=[el.scrollLeft,el.scrollTop]; el.scrollBy({{left:{x},top:{y},behavior:'instant'}}); return old[0] !== el.scrollLeft || old[1] !== el.scrollTop; }})()",
                    selector_to_js_find(selector)
                )
            } else {
                format!(
                    "(() => {{ const old=[scrollX,scrollY]; window.scrollBy({{left:{x},top:{y},behavior:'instant'}}); return old[0] !== scrollX || old[1] !== scrollY; }})()"
                )
            };
            let changed = evaluate(config, expression).await?;
            Ok(if changed == true {
                "scroll_verified"
            } else {
                "scroll_dispatched"
            }
            .into())
        }
        "browser_mouse_move" | "browser_mouse_click" => {
            let x = args["x"].as_f64().ok_or("x required")?;
            let y = args["y"].as_f64().ok_or("y required")?;
            if action == "browser_mouse_move" {
                super::mouse_move(config, x, y).await
            } else {
                super::mouse_click(config, x, y, args["button"].as_str().unwrap_or("left")).await
            }
        }
        "browser_key_press" => {
            super::key_press(config, args["key"].as_str().ok_or("key required")?).await
        }
        _ => Err(format!("Unsupported browser action: {action}")),
    }
}

async fn wait(config: &MintConfig, args: &Value, session: &BrowserSession) -> Result<(), String> {
    let condition = args["condition"].as_str().ok_or("condition required")?;
    let value = args["value"].as_str().unwrap_or("");
    if matches!(condition, "url" | "text") && value.is_empty() {
        return Err("Nonempty value required for url/text waits".into());
    }
    let expression = match condition {
        "url" => format!("location.href === {}", json!(value)),
        "text" => format!(
            "(document.body?.innerText || '').includes({})",
            json!(value)
        ),
        "visible" | "hidden" => {
            let selector = target(args, session)?;
            format!(
                "(() => {{ {} const visible=!!el && el.isConnected && el.getBoundingClientRect().width>0 && el.getBoundingClientRect().height>0 && getComputedStyle(el).visibility !== 'hidden'; return {}; }})()",
                selector_to_js_find(&selector),
                if condition == "visible" {
                    "visible"
                } else {
                    "!visible"
                }
            )
        }
        _ => return Err("condition must be url, text, visible, or hidden".into()),
    };
    let duration =
        Duration::from_millis(args["timeoutMs"].as_u64().unwrap_or(10000).clamp(1, 30000));
    tokio::time::timeout(duration, async {
        loop {
            if evaluate(config, expression.clone()).await? == true {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .map_err(|_| "condition_timeout".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn composing_browser_actions_keeps_the_caller_future_small() {
        let config = MintConfig::default();
        let args = json!({});
        let operation = execute_action(&config, "browser_observe", &args);
        assert!(std::mem::size_of_val(&operation) <= 32);
    }

    #[test]
    fn references_cannot_cross_observations_or_accept_two_targets() {
        let mut session = BrowserSession::default();
        let observation: BrowserObservation = serde_json::from_value(json!({
            "tabId":"tab","url":"http://localhost","title":"test","observationId":"a",
            "text":"","scrollX":0,"scrollY":0,"elements":[{"ref":"a:1"}],"textTruncated":false,
            "elementsTruncated":false,"nextTextOffset":null,"nextElementOffset":null,"unsupportedFrames":0
        })).unwrap();
        session.observation = Some(observation);
        assert!(target(&json!({"elementRef":"a:1","selector":"input"}), &session).is_err());
        assert_eq!(
            target(&json!({"elementRef":"a:1"}), &session).unwrap(),
            "mint-ref=a:1"
        );
        session.observation = None;
        assert!(
            target(&json!({"elementRef":"a:1"}), &session)
                .unwrap_err()
                .contains("stale_reference")
        );
        assert!(target(&json!({"selector":"mint-ref=a:1"}), &session).is_err());
    }

    #[test]
    fn tab_lease_is_exclusive_and_released_on_drop() {
        let config = MintConfig::default();
        let tab = uuid::Uuid::new_v4().to_string();
        let lock = lease(&config, &tab).unwrap();
        assert!(lease(&config, &tab).is_err());
        drop(lock);
        assert!(lease(&config, &tab).is_ok());
    }

    #[test]
    fn cdp_errors_are_not_successful_actions() {
        assert!(
            super::super::cdp::validate_response(&json!({"error":{"message":"bad command"}}))
                .is_err()
        );
        assert!(super::super::cdp::validate_response(&json!({"result":{"exceptionDetails":{"text":"Uncaught","exception":{"description":"Error: stale"}}}})).unwrap_err().contains("stale"));
        assert!(super::super::cdp::validate_response(&json!({"result":{}})).is_ok());
    }

    #[test]
    fn selectors_do_not_interpolate_javascript() {
        let expression = selector_to_js_find("text=${globalThis.compromised=true}`\\\"");
        assert!(expression.contains("=== \"${globalThis.compromised=true}`"));
        assert!(!expression.contains("=== `"));
    }

    struct BrowserFixture {
        config: MintConfig,
        url: String,
        child: Child,
        server: tokio::task::JoinHandle<()>,
        profile: std::path::PathBuf,
    }
    impl Drop for BrowserFixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.server.abort();
            let _ = std::fs::remove_dir_all(&self.profile);
        }
    }

    impl BrowserFixture {
        async fn start() -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else {
                        break;
                    };
                    tokio::spawn(async move {
                        let mut buffer = [0; 4096];
                        let _ = stream.read(&mut buffer).await;
                        let request = String::from_utf8_lossy(&buffer);
                        let body = if request.starts_with("GET /popup ") {
                            "<h1>Popup destination</h1>"
                        } else {
                            FIXTURE
                        };
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                    });
                }
            });
            let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = socket.local_addr().unwrap().port();
            drop(socket);
            let profile =
                std::env::temp_dir().join(format!("mint-browser-test-{}", uuid::Uuid::new_v4()));
            let child = Command::new(
                std::env::var("MINT_BROWSER_TEST_EXECUTABLE").unwrap_or("google-chrome".into()),
            )
            .args([
                "--headless=new",
                "--no-sandbox",
                "--disable-gpu",
                "--disable-dev-shm-usage",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .arg(format!("--remote-debugging-port={port}"))
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Chrome required for ignored browser tests");
            let mut config = MintConfig::default();
            config.extra.insert(
                "browserDebugUrl".into(),
                json!(format!("http://127.0.0.1:{port}/json/list")),
            );
            let fixture = Self {
                config,
                url,
                child,
                server,
                profile,
            };
            for _ in 0..100 {
                if super::super::is_browser_running(&fixture.config).await {
                    return fixture;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            panic!("Chrome debugging endpoint did not become ready");
        }
    }

    const FIXTURE: &str = r#"<!doctype html><html><head><title>Mint Browser Fixture</title></head><body>
      <form onsubmit="event.preventDefault(); document.querySelector('#result').textContent='Saved '+document.querySelector('#name').value; ++window.submissions;">
        <label>Name<input id="name" value="old"></label><input id="number" type="number" value="12"><input id="secret" type="password" value="do-not-expose">
        <select id="choice"><option value="a">Alpha</option><option value="b">Beta</option></select>
        <button id="save">Save</button>
      </form>
      <input id="query" aria-label="Search"><button id="search" onclick="document.querySelector('#result').innerHTML='<a id=resultlink href=/popup>Matching result</a>'">Search</button>
      <button id="delay" onclick="setTimeout(()=>document.querySelector('#result').textContent='Delayed result',300)">Delayed</button>
      <a id="popup" href="/popup" target="_blank">Open popup</a>
      <button id="noop">No operation</button><button id="disabled" disabled>Disabled</button>
      <div style="position:relative;width:150px;height:40px"><button id="covered">Covered</button><div style="position:absolute;inset:0;z-index:3;background:white">Overlay</div></div>
      <div id="result"></div><div style="height:1800px"></div><button id="bottom" onclick="document.querySelector('#result').textContent='Bottom clicked'">Bottom</button>
      <div id="shadow"></div>
      <script>window.submissions=0;const root=document.querySelector('#shadow').attachShadow({mode:'open'});root.innerHTML='<button>Shadow button</button>';</script>
    </body></html>"#;

    fn agent_execute<'a>(
        config: &'a MintConfig,
        action: &'a str,
        args: Value,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            crate::orchestration::execute_tool_from_json(
                std::path::Path::new("/tmp"),
                config,
                action,
                args,
                "browser-test",
                &mut |_| Ok(crate::orchestration::ApprovalOutcome::Approved),
            )
            .await
            .map_err(|e| e.to_string())
        })
    }
    async fn call(config: &MintConfig, action: &str, args: Value) -> Value {
        serde_json::from_str(&agent_execute(config, action, args).await.unwrap()).unwrap()
    }
    fn element(observation: &Value, name: &str) -> String {
        observation["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == name)
            .unwrap()["ref"]
            .as_str()
            .unwrap()
            .into()
    }

    #[tokio::test]
    #[ignore = "launches an isolated headless Chrome and local HTTP fixture"]
    async fn browser_controlled_acceptance_three_runs() {
        let fixture = BrowserFixture::start().await;
        for _ in 0..3 {
            run_session(async {
                let config = &fixture.config;
                let initial = call(config, "browser_open", json!({"url":fixture.url})).await;
                assert_eq!(initial["status"], "input_dispatched");
                let tab = initial["observation"]["tabId"].as_str().unwrap().to_owned();
                let observation = call(config, "browser_observe", json!({})).await;
                assert!(
                    observation["elements"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["name"] == "Shadow button")
                );
                assert!(
                    observation["elements"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["inputType"] == "password" && e["value"].is_null())
                );
                let stale = element(&observation, "Name");
                let filled = call(
                    config,
                    "browser_fill",
                    json!({"elementRef":stale,"text":"Mint ภาษาไทย"}),
                )
                .await;
                assert_eq!(filled["verification"], "verified");
                assert!(
                    execute_action(config, "browser_click", &json!({"elementRef":stale}))
                        .await
                        .unwrap_err()
                        .contains("stale_reference")
                );
                let appended = call(
                    config,
                    "browser_type",
                    json!({"selector":"#name","text":"!"}),
                )
                .await;
                assert_eq!(appended["verification"], "verified");
                assert_eq!(
                    call(
                        config,
                        "browser_select",
                        json!({"selector":"#choice","value":"b"})
                    )
                    .await["verification"],
                    "verified"
                );
                let submitted = call(config, "browser_click", json!({"selector":"#save"})).await;
                assert_eq!(submitted["verification"], "unverified");
                assert_eq!(
                    call(
                        config,
                        "browser_wait",
                        json!({"condition":"text","value":"Saved Mint ภาษาไทย!"})
                    )
                    .await["verification"],
                    "verified"
                );
                TARGET
                    .scope(tab.clone(), async {
                        assert_eq!(
                            evaluate(config, "window.submissions".into()).await.unwrap(),
                            1
                        );
                    })
                    .await;
                assert!(
                    call(config, "browser_click", json!({"selector":"#covered"})).await["error"]
                        .as_str()
                        .unwrap()
                        .contains("element_obstructed")
                );
                assert!(
                    call(config, "browser_click", json!({"selector":"#disabled"})).await["error"]
                        .as_str()
                        .unwrap()
                        .contains("element_disabled")
                );
                assert!(
                    call(config, "browser_click", json!({"selector":"#bottom"})).await["error"]
                        .is_null()
                );
                assert_eq!(
                    call(
                        config,
                        "browser_wait",
                        json!({"condition":"text","value":"Bottom clicked"})
                    )
                    .await["status"],
                    "condition_met"
                );
                call(config, "browser_click", json!({"selector":"#delay"})).await;
                assert_eq!(
                    call(
                        config,
                        "browser_wait",
                        json!({"condition":"text","value":"Delayed result","timeoutMs":2000})
                    )
                    .await["verification"],
                    "verified"
                );
                assert!(
                    execute_action(
                        config,
                        "browser_wait",
                        &json!({"condition":"text","value":"does not exist","timeoutMs":10})
                    )
                    .await
                    .unwrap_err()
                    .contains("condition_timeout")
                );
                let popup = call(config, "browser_click", json!({"selector":"#popup"})).await;
                assert_eq!(popup["observation"]["tabId"], tab);
                let popup_id = popup["newTabs"][0]["id"].as_str().unwrap();
                call(
                    config,
                    "browser_tabs",
                    json!({"operation":"select","tabId":popup_id}),
                )
                .await;
                assert!(
                    call(config, "browser_observe", json!({})).await["text"]
                        .as_str()
                        .unwrap()
                        .contains("Popup destination")
                );
                call(
                    config,
                    "browser_tabs",
                    json!({"operation":"close","tabId":popup_id}),
                )
                .await;
                assert!(
                    execute_action(config, "browser_observe", &json!({}))
                        .await
                        .unwrap_err()
                        .contains("tab_unavailable")
                );
                call(
                    config,
                    "browser_tabs",
                    json!({"operation":"select","tabId":tab}),
                )
                .await;
                // Another session cannot steal a tab, even though its own tab is independent.
                run_session(async {
                    assert!(
                        execute_action(
                            config,
                            "browser_tabs",
                            &json!({"operation":"select","tabId":tab})
                        )
                        .await
                        .unwrap_err()
                        .contains("tab_busy")
                    );
                    let independent = call(config, "browser_observe", json!({})).await;
                    assert_ne!(independent["tabId"], tab);
                })
                .await;
                for _ in 0..3 {
                    call(config, "browser_click", json!({"selector":"#noop"})).await;
                }
                assert!(
                    execute_action(config, "browser_click", &json!({"selector":"#save"}))
                        .await
                        .unwrap_err()
                        .contains("browser_blocked")
                );
                call(config, "browser_observe", json!({"takeoverComplete":true})).await;
                call(
                    config,
                    "browser_fill",
                    json!({"selector":"#query","text":"Mint"}),
                )
                .await;
                call(config, "browser_click", json!({"selector":"#search"})).await;
                call(config, "browser_click", json!({"selector":"#resultlink"})).await;
                assert_eq!(
                    call(
                        config,
                        "browser_wait",
                        json!({"condition":"url","value":format!("{}/popup",fixture.url)})
                    )
                    .await["verification"],
                    "verified"
                );
                assert!(
                    execute_action(config, "browser_mouse_click", &json!({"x":1,"y":1}))
                        .await
                        .unwrap_err()
                        .contains("screenshot_required")
                );
                assert!(
                    execute_action(config, "browser_screenshot", &json!({}))
                        .await
                        .unwrap()
                        .starts_with("data:image/png;base64,")
                );
                call(config, "browser_scroll", json!({"y":500})).await;
                assert!(
                    execute_action(config, "browser_mouse_click", &json!({"x":1,"y":1}))
                        .await
                        .unwrap_err()
                        .contains("screenshot_required")
                );
            })
            .await;
        }
    }

    #[tokio::test]
    #[ignore = "launches isolated headless Chrome for pagination and changing DOM checks"]
    async fn browser_edge_case_acceptance() {
        let fixture = BrowserFixture::start().await;
        run_session(async {
            let config = &fixture.config;
            call(config,"browser_open",json!({"url":fixture.url})).await;
            let tab = call(config,"browser_observe",json!({})).await["tabId"].as_str().unwrap().to_owned();
            TARGET.scope(tab.clone(),async {
                evaluate(config,"for(let i=0;i<105;i++){const b=document.createElement('button');b.textContent='Duplicate label';b.id='duplicate'+i;document.body.appendChild(b);}const p=document.createElement('p');p.textContent='z'.repeat(13000);document.body.appendChild(p);true".into()).await.unwrap();
            }).await;
            let first = call(config,"browser_observe",json!({})).await;
            assert_eq!(first["elements"].as_array().unwrap().len(),100);
            assert_eq!(first["elementsTruncated"],true);
            assert_eq!(first["textTruncated"],true);
            let second = call(config,"browser_observe",json!({"elementOffset":100,"textOffset":12000})).await;
            assert!(!second["elements"].as_array().unwrap().is_empty());
            assert_eq!(second["textTruncated"],false);
            let reference = element(&second,"Duplicate label");
            TARGET.scope(tab.clone(),async {
                evaluate(config,format!("globalThis.__mintObservation.elements.get({}).remove();true",json!(reference))).await.unwrap();
            }).await;
            let removed = call(config,"browser_click",json!({"elementRef":reference})).await;
            assert!(removed["error"].as_str().unwrap().contains("stale"));
            let fresh = call(config,"browser_observe",json!({})).await;
            let shadow = element(&fresh,"Shadow button");
            assert!(call(config,"browser_click",json!({"elementRef":shadow})).await["error"].is_null());
            assert!(execute_action(config,"browser_wait",&json!({"condition":"text","value":""})).await.is_err());
            assert_eq!(call(config,"browser_fill",json!({"selector":"#number","text":"34"})).await["verification"],"verified");
            assert_eq!(call(config,"browser_type",json!({"selector":"#number","text":"5"})).await["verification"],"verified");
            let masked = call(config,"browser_fill",json!({"selector":"#secret","text":"new-secret"})).await;
            assert_eq!(masked["verification"],"verified");
            assert!(!masked.to_string().contains("new-secret"));
            call(config,"browser_fill",json!({"selector":"#name","text":""})).await;
            assert_eq!(call(config,"browser_wait",json!({"condition":"visible","selector":"#name"})).await["verification"],"verified");
            assert_eq!(call(config,"browser_wait",json!({"condition":"hidden","selector":"#missing"})).await["verification"],"verified");
            // A submitted form is dispatched once, even when the expected confirmation never appears.
            call(config,"browser_click",json!({"selector":"#save"})).await;
            assert!(execute_action(config,"browser_wait",&json!({"condition":"text","value":"Never confirmed","timeoutMs":20})).await.is_err());
            TARGET.scope(tab.clone(),async {assert_eq!(evaluate(config,"window.submissions".into()).await.unwrap(),1);}).await;
            // Re-observing and changing reference IDs must not evade the recovery limit.
            for _ in 0..3 {
                let observed = call(config,"browser_observe",json!({})).await;
                let reference = element(&observed,"No operation");
                call(config,"browser_click",json!({"elementRef":reference})).await;
            }
            assert!(execute_action(config,"browser_fill",&json!({"selector":"#name","text":"blocked"})).await.err().is_some_and(|e|e.contains("browser_blocked")));
            TARGET.scope(tab.clone(),async {evaluate(config,"document.querySelector('#name').value='User takeover';true".into()).await.unwrap();}).await;
            let takeover = call(config,"browser_observe",json!({"takeoverComplete":true})).await;
            assert!(takeover["elements"].as_array().unwrap().iter().any(|e|e["value"] == "User takeover"));
            let failed_navigation = call(config,"browser_open",json!({"url":"http://127.0.0.1:1/"})).await;
            assert_eq!(failed_navigation["status"],"failed_or_ambiguous");
            assert!(failed_navigation["error"].as_str().unwrap().contains("navigation_failed"));
        }).await;
    }

    fn script<'a>(
        config: &'a MintConfig,
        expression: &'a str,
    ) -> std::pin::Pin<Box<dyn Future<Output = Value> + Send + 'a>> {
        Box::pin(async move {
            let tab = SESSION.with(Arc::clone).lock().await.tab.clone().unwrap();
            TARGET
                .scope(tab, evaluate(config, expression.into()))
                .await
                .unwrap()
        })
    }

    #[tokio::test]
    #[ignore = "isolated headless Chrome; real AgentInput parsing and tool executor"]
    async fn browser_reliability_tabs_progress_and_focus() {
        let fixture = BrowserFixture::start().await;
        run_session(async {
            let config=&fixture.config;
            for args in [json!({"operation":"open"}),json!({"operation":"open","url":""}),json!({"operation":"open","url":"  \n "})] {
                let opened=call(config,"browser_tabs",args).await;
                let id=opened["selectedTabId"].as_str().unwrap();
                assert!(opened["tabs"].as_array().unwrap().iter().any(|t|t["id"]==id && t["url"]=="about:blank"));
            }
            let before=call(config,"browser_tabs",json!({"operation":"list"})).await;
            for url in ["javascript:alert(1)","http://", "bad url"] {
                assert!(agent_execute(config,"browser_tabs",json!({"operation":"open","url":url})).await.is_err());
                assert_eq!(before,call(config,"browser_tabs",json!({"operation":"list"})).await);
            }
            call(config,"browser_tabs",json!({"operation":"open","url":fixture.url})).await;
            let current=call(config,"browser_observe",json!({})).await["tabId"].as_str().unwrap().to_owned();
            for _ in 0..4 {
                let result=call(config,"browser_open",json!({"url":fixture.url})).await;
                assert!(result["error"].is_null(),"{result}");
            }
            script(config,"document.body.insertAdjacentHTML('beforeend','<input id=check type=checkbox aria-label=Check><span id=clock>0</span>');true").await;
            for _ in 0..4 {
                let result=call(config,"browser_click",json!({"selector":"#check"})).await;
                assert!(result["error"].is_null(),"{result}");
            }
            for _ in 0..4 {
                call(config,"browser_click",json!({"selector":"#search"})).await;
                call(config,"browser_wait",json!({"condition":"text","value":"Matching result"})).await;
            }
            for i in 0..3 {
                // Progress and a wait on another action must not erase noop retries.
                call(config,"browser_click",json!({"selector":"#check"})).await;
                call(config,"browser_wait",json!({"condition":"visible","selector":"#check"})).await;
                script(config,&format!("document.querySelector('#clock').textContent='{i}';true")).await;
                let fresh=call(config,"browser_observe",json!({})).await;
                let reference=element(&fresh,"No operation");
                let result=call(config,"browser_click",json!({"elementRef":reference})).await;
                assert_eq!(result["error"].is_null(),i<2,"{result}");
                agent_execute(config,"browser_read",json!({})).await.unwrap();
            }
            let tabs_before=call(config,"browser_tabs",json!({"operation":"list"})).await;
            for (action,args) in [("browser_tabs",json!({"operation":"open","url":fixture.url})),("browser_tabs",json!({"operation":"close","tabId":current})),("browser_open",json!({"url":fixture.url}))] {
                assert!(agent_execute(config,action,args).await.unwrap_err().contains("browser_blocked"));
                assert_eq!(tabs_before,call(config,"browser_tabs",json!({"operation":"list"})).await);
            }
            let other=before["selectedTabId"].as_str().unwrap();
            call(config,"browser_tabs",json!({"operation":"select","tabId":other})).await;
            assert!(agent_execute(config,"browser_open",json!({"url":fixture.url})).await.unwrap_err().contains("browser_blocked"));
            call(config,"browser_tabs",json!({"operation":"select","tabId":current})).await;
            call(config,"browser_observe",json!({"takeoverComplete":true})).await;
            // Click and key handlers can steal focus or replace the original node.
            for (action,handler,event,replace,text) in [
                ("browser_type","document.querySelector('#query').focus()","onclick",false,"unwanted"),
                ("browser_type","document.querySelector('#query').focus()","onselect",false,"unwanted"),
                ("browser_type","if(event.key==='End')document.querySelector('#query').focus()","onkeydown",false,"unwanted"),
                ("browser_type","this.replaceWith(this.cloneNode())","onclick",true,"unwanted"),
                ("browser_fill","document.querySelector('#query').focus()","onclick",false,"unwanted"),
                ("browser_fill","document.querySelector('#query').focus()","onkeydown",false,"unwanted"),
                ("browser_fill","document.querySelector('#query').focus()","onkeydown",false,""),
                ("browser_type","if(event.key==='End')this.readOnly=true","onkeydown",false,"unwanted"),
                ("browser_type","if(event.key==='End')this.replaceWith(this.cloneNode())","onkeydown",true,"unwanted"),
            ] {
                script(config,"document.querySelector('#name').outerHTML='<input id=name value=old>';document.querySelector('#query').value='other';true").await;
                script(config,&format!("document.querySelector('#name').{event}=function(event){{{handler}}};true")).await;
                if event == "onselect" {
                    script(config,"document.querySelector('#name').onclick=function(){this.setSelectionRange(0,0)};true").await;
                }
                let result=call(config,action,json!({"selector":"#name","text":text})).await;
                let error=result["error"].as_str().unwrap_or_else(||panic!("Expected {event} focus rejection: {result}"));
                assert!(error.contains("focus_changed") || (replace && error.contains("stale_reference")),"{result}");
                assert_eq!(script(config,"[document.querySelector('#name').value,document.querySelector('#query').value]").await,json!(["old","other"]));
                call(config,"browser_observe",json!({"takeoverComplete":true})).await;
            }
            script(config,"document.querySelector('#shadow').shadowRoot.innerHTML='<input aria-label=Shadow value=shadow>';true").await;
            let observed=call(config,"browser_observe",json!({})).await;
            let result=call(config,"browser_type",json!({"elementRef":element(&observed,"Shadow"),"text":"!"})).await;
            assert_eq!(result["verification"],"verified","{result}");
            let observed=call(config,"browser_observe",json!({})).await;
            let result=call(config,"browser_fill",json!({"elementRef":element(&observed,"Shadow"),"text":"replaced"})).await;
            assert_eq!(result["verification"],"verified","{result}");
        }).await;
    }

    #[tokio::test]
    #[ignore = "isolated headless Chrome; target-local screenshot regression checks"]
    async fn browser_reliability_screenshot_targets() {
        let fixture = BrowserFixture::start().await;
        run_session(async {
            let config=&fixture.config;
            call(config,"browser_open",json!({"url":fixture.url})).await;
            // randomUUID is unavailable on some non-secure documents.
            script(config,"Object.defineProperty(crypto,'randomUUID',{value:undefined,configurable:true});delete globalThis.__mintScreenshotDocument;true").await;
            let setup="document.body.innerHTML='<button id=target style=\"position:fixed;left:100px;top:100px;width:100px;height:50px\">Target</button><span id=ticker style=\"position:fixed;left:550px;top:350px\">Clock</span><canvas id=canvas width=100 height=50 style=\"position:fixed;left:250px;top:100px\"></canvas>';window.presses=0;document.onmousedown=()=>++window.presses;window.clicks=0;document.querySelector('#target').onclick=()=>++window.clicks;true";
            for change in [
                "document.body.insertAdjacentHTML('beforeend','<div style=\"position:fixed;inset:0;z-index:100;background:white\">Modal</div>');true",
                "document.querySelector('#target').style.left='120px';true",
                "const b=document.querySelector('#target');b.replaceWith(b.cloneNode(true));true",
                "document.querySelector('#target').onmouseenter=()=>document.body.insertAdjacentHTML('beforeend','<div style=\"position:fixed;inset:0;z-index:100;background:white\">Hover modal</div>');true",
            ] {
                script(config,setup).await;
                agent_execute(config,"browser_screenshot",json!({})).await.unwrap();
                script(config,change).await;
                let result=agent_execute(config,"browser_mouse_click",json!({"x":150,"y":125})).await;
                let report=result.unwrap_or_else(|e|e);
                assert!(report.contains("screenshot_stale"),"{report}");
                assert_eq!(script(config,"window.presses").await,0);
                call(config,"browser_observe",json!({"takeoverComplete":true})).await;
                // Move away before preparing the next hover case.
                let tab=SESSION.with(Arc::clone).lock().await.tab.clone().unwrap();
                TARGET.scope(tab,super::super::mouse_move(config,0.0,0.0)).await.unwrap();
            }
            script(config,setup).await;
            agent_execute(config,"browser_screenshot",json!({})).await.unwrap();
            script(config,"const ctx=document.querySelector('#canvas').getContext('2d');ctx.fillStyle='red';ctx.fillRect(0,0,100,50);true").await;
            assert!(agent_execute(config,"browser_mouse_click",json!({"x":300,"y":125})).await.unwrap_err().contains("screenshot_stale"));
            assert_eq!(script(config,"window.presses").await,0);
            script(config,setup).await;
            agent_execute(config,"browser_screenshot",json!({})).await.unwrap();
            script(config,"document.querySelector('#ticker').textContent='New ticker';true").await;
            let result=call(config,"browser_mouse_click",json!({"x":150,"y":125})).await;
            assert!(result["error"].is_null(),"{result}");
            assert_eq!(script(config,"[window.presses,window.clicks]").await,json!([1,1]));
        }).await;
    }

    #[tokio::test]
    #[ignore = "isolated Chrome; actual Gemini Live task runner and agent executor"]
    async fn browser_reliability_live_session_keeps_state() {
        let fixture = BrowserFixture::start().await;
        let config = fixture.config.clone();
        let url = fixture.url.clone();
        // Use the production background runner, not a test-owned run_session.
        let tab = crate::gemini_live::spawn_live_session(async move {
            let opened = call(&config,"browser_open",json!({"url":url})).await;
            let tab = opened["observation"]["tabId"].as_str().unwrap().to_owned();
            let observed = call(&config,"browser_observe",json!({})).await;
            assert_eq!(observed["tabId"],tab);
            assert_eq!(observed["url"],format!("{url}/"));
            let filled = call(&config,"browser_fill",json!({"elementRef":element(&observed,"Name"),"text":"Live session"})).await;
            assert_eq!(filled["verification"],"verified");
            assert_eq!(filled["observation"]["tabId"],tab);
            let point = script(&config,"(() => {const el=document.querySelector('#noop');el.scrollIntoView({behavior:'instant'});const r=el.getBoundingClientRect();return [r.left+r.width/2,r.top+r.height/2]})()").await;
            agent_execute(&config,"browser_screenshot",json!({})).await.unwrap();
            // A separate observation/tool batch must retain screenshot evidence.
            call(&config,"browser_observe",json!({})).await;
            assert!(script(&config,"scrollY").await.as_f64().unwrap()>0.0);
            let clicked = call(&config,"browser_mouse_click",json!({"x":point[0],"y":point[1]})).await;
            assert!(clicked["error"].is_null(),"{clicked}");
            call(&config,"browser_wait",json!({"condition":"visible","selector":"#noop"})).await;
            for _ in 0..3 {
                call(&config,"browser_click",json!({"selector":"#noop"})).await;
            }
            assert!(agent_execute(&config,"browser_tabs",json!({"operation":"open"})).await.unwrap_err().contains("browser_blocked"));
            assert!(agent_execute(&config,"browser_fill",json!({"selector":"#name","text":"blocked"})).await.unwrap_err().contains("browser_blocked"));
            assert_eq!(call(&config,"browser_observe",json!({})).await["tabId"],tab);
            tab
        }).await.unwrap();
        // Session shutdown releases its leases even when it ends blocked.
        drop(lease(&fixture.config, &tab).unwrap());
        let config = fixture.config.clone();
        let url = fixture.url.clone();
        let next_tab = crate::gemini_live::spawn_live_session(async move {
            let opened = call(&config, "browser_open", json!({"url":url})).await;
            assert!(opened["error"].is_null(), "{opened}");
            let next_tab = opened["observation"]["tabId"].as_str().unwrap().to_owned();
            assert_eq!(
                call(&config, "browser_observe", json!({})).await["tabId"],
                next_tab
            );
            next_tab
        })
        .await
        .unwrap();
        assert_ne!(tab, next_tab);
        drop(lease(&fixture.config, &next_tab).unwrap());
    }

    #[tokio::test]
    #[ignore = "isolated Chrome; continuously changing unrelated DOM and layout"]
    async fn browser_reliability_continuous_ticker_allows_click() {
        let fixture = BrowserFixture::start().await;
        run_session(async {
            let config=&fixture.config;
            call(config,"browser_open",json!({"url":fixture.url})).await;
            script(config,"document.body.innerHTML='<button id=target style=\"position:fixed;left:100px;top:100px;width:100px;height:50px\">Target</button><span id=ticker style=\"position:fixed;left:550px;top:350px\">Clock</span>';window.presses=0;window.clicks=0;document.onmousedown=()=>++window.presses;document.querySelector('#target').onclick=()=>++window.clicks;window.n=0;window.tickerTimer=setInterval(()=>{const t=document.querySelector('#ticker');t.textContent=(++n%2?'Clock':'Changing carousel text');t.style.width=(n%2?80:180)+'px';},1);true").await;
            // Both image capture and revalidation must tolerate continuous
            // unrelated text-node replacement and layout changes.
            agent_execute(config,"browser_screenshot",json!({})).await.unwrap();
            let tick_before = script(config,"window.n").await;
            let clicked=call(config,"browser_mouse_click",json!({"x":150,"y":125})).await;
            assert!(clicked["error"].is_null(),"{clicked}");
            assert!(script(config,"window.n").await.as_u64().unwrap()>tick_before.as_u64().unwrap());
            assert_eq!(script(config,"[window.presses,window.clicks]").await,json!([1,1]));
            // Local movement remains stale even while the distant timer runs.
            agent_execute(config,"browser_screenshot",json!({})).await.unwrap();
            script(config,"document.querySelector('#target').style.left='120px';true").await;
            assert!(agent_execute(config,"browser_mouse_click",json!({"x":150,"y":125})).await.unwrap_err().contains("screenshot_stale"));
            assert_eq!(script(config,"window.presses").await,1);
            script(config,"clearInterval(window.tickerTimer);true").await;
        }).await;
    }

    #[tokio::test]
    #[ignore = "isolated Chrome; target movement across initial capture layout epochs"]
    async fn browser_reliability_target_changes_during_capture() {
        let fixture = BrowserFixture::start().await;
        run_session(async {
            let config=&fixture.config;
            call(config,"browser_open",json!({"url":fixture.url})).await;
            script(config,"document.body.innerHTML='<button id=target style=\"position:fixed;left:100px;top:100px;width:300px;height:50px\">Target</button>';window.presses=0;window.clicks=0;document.onmousedown=()=>++window.presses;document.querySelector('#target').onclick=()=>++window.clicks;window.n=0;window.moveTimer=setInterval(()=>document.querySelector('#target').style.left=(100+(++n))+'px',1);true").await;
            agent_execute(config,"browser_screenshot",json!({})).await.unwrap();
            script(config,"clearInterval(window.moveTimer);true").await;
            assert!(script(config,"window.n").await.as_u64().unwrap()>1);
            // Even when movement stops, the initial image has inconsistent
            // evidence around this coordinate and cannot authorize input.
            assert!(agent_execute(config,"browser_mouse_click",json!({"x":250,"y":125})).await.unwrap_err().contains("screenshot_stale"));
            assert_eq!(script(config,"window.presses").await,0);
            let point=script(config,"(() => {const r=document.querySelector('#target').getBoundingClientRect();return [r.left+r.width/2,r.top+r.height/2]})()").await;
            agent_execute(config,"browser_screenshot",json!({})).await.unwrap();
            let clicked=call(config,"browser_mouse_click",json!({"x":point[0],"y":point[1]})).await;
            assert!(clicked["error"].is_null(),"{clicked}");
            assert_eq!(script(config,"[window.presses,window.clicks]").await,json!([1,1]));
        }).await;
    }

    #[tokio::test]
    #[ignore = "uses local HTTP and WebSocket listeners to test stalled CDP transport"]
    async fn browser_transport_timeout_and_protocol_error() {
        use futures_util::{SinkExt, StreamExt};
        let websocket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let socket_url = format!("ws://{}", websocket.local_addr().unwrap());
        let mut config = MintConfig::default();
        config.extra.insert(
            "browserDebugUrl".into(),
            json!(format!("http://{}/json/list", http.local_addr().unwrap())),
        );
        let body =
            json!([{"type":"page","id":"mock","webSocketDebuggerUrl":socket_url}]).to_string();
        let http_task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = http.accept().await.unwrap();
                let mut buffer = [0; 1024];
                stream.read(&mut buffer).await.unwrap();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            }
        });
        let websocket_task = tokio::spawn(async move {
            let (stream, _) = websocket.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            socket.next().await.unwrap().unwrap();
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    json!({"id":1,"error":{"message":"mock protocol error"}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            drop(socket);
            let (stream, _) = websocket.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            socket.next().await.unwrap().unwrap();
            tokio::time::sleep(Duration::from_secs(12)).await;
            let _ = socket.close(None).await;
        });
        let error = super::super::cdp::cdp_call_raw(&config, "Page.captureScreenshot", json!({}))
            .await
            .unwrap_err();
        assert_eq!(error, "mock protocol error");
        let started = Instant::now();
        let error = super::super::cdp::cdp_call_raw(&config, "Page.captureScreenshot", json!({}))
            .await
            .unwrap_err();
        assert!(error.contains("browser_timeout"));
        assert!(started.elapsed() < Duration::from_secs(12));
        http_task.abort();
        websocket_task.abort();
    }

    #[tokio::test]
    #[ignore = "launches Chrome and performs read-only navigation on public websites"]
    async fn browser_public_read_only_smoke() {
        let fixture = BrowserFixture::start().await;
        run_session(async {
            for (url, expected) in [
                ("https://example.com", "Example Domain"),
                ("https://www.iana.org/domains/reserved", "IANA"),
            ] {
                let result = call(&fixture.config, "browser_open", json!({"url":url})).await;
                assert!(result["error"].is_null(), "{result}");
                assert!(
                    result["observation"]["text"]
                        .as_str()
                        .unwrap()
                        .contains(expected)
                        || result["observation"]["title"]
                            .as_str()
                            .unwrap()
                            .contains(expected),
                    "Public page did not match expected title or text"
                );
                assert!(!result["observation"]["text"].as_str().unwrap().is_empty());
            }
        })
        .await;
    }
}
