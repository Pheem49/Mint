//! Coordinate input is authorized by target-local screenshot evidence, not a
//! whole-page hash. Backend node IDs survive reads but not node replacement.
use super::{cdp::cdp_call_raw, session::evaluate};
use crate::MintConfig;
use base64::Engine;
use image::RgbaImage;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) struct ScreenshotEvidence {
    tab: String,
    captured: Instant,
    marker: Value,
    image: RgbaImage,
    snapshot_before: Value,
    snapshot_after: Value,
}

async fn marker(config: &MintConfig) -> Result<Value, String> {
    evaluate(config,"[location.href,globalThis.__mintScreenshotDocument ||= (crypto.randomUUID?.() || String(performance.timeOrigin)+Math.random()),scrollX,scrollY,innerWidth,innerHeight,devicePixelRatio]".into()).await
}

async fn snapshot(config: &MintConfig) -> Result<Value, String> {
    Ok(cdp_call_raw(config,"DOMSnapshot.captureSnapshot",json!({"computedStyles":["pointer-events"],"includePaintOrder":true,"includeDOMRects":true})).await?["result"].clone())
}

pub(super) async fn capture(
    config: &MintConfig,
    tab: &str,
) -> Result<(String, ScreenshotEvidence), String> {
    capture_at(config, tab, None).await
}

/// At initial capture the coordinate is unknown. Retain both layout epochs so
/// validation can check the eventual target without requiring distant DOM to
/// remain unchanged. A coordinate-aware recapture retries target movement once.
async fn capture_at(
    config: &MintConfig,
    tab: &str,
    coordinate: Option<(f64, f64)>,
) -> Result<(String, ScreenshotEvidence), String> {
    // Hide only Mint's two cosmetic surfaces, including their shadows. Restore
    // the stylesheet even if capture fails; never hide website content.
    let token = format!("mint-evidence-{}", uuid::Uuid::new_v4());
    evaluate(config,format!("(() => {{const s=document.createElement('style'); s.id={}; s.textContent='#mint-cursor-overlay,#mint-browser-aura{{display:none!important}}';document.documentElement.appendChild(s);return true;}})()",json!(token))).await?;
    let result = async {
        for _ in 0..2 {
            let before_marker = marker(config).await?;
            let before = snapshot(config).await?;
            let captured = Instant::now();
            let data = super::screenshot(config).await?;
            let after = snapshot(config).await?;
            if before_marker != marker(config).await? {
                continue;
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&data)
                .map_err(|e| e.to_string())?;
            let image = image::load_from_memory(&bytes)
                .map_err(|e| e.to_string())?
                .to_rgba8();
            let evidence = ScreenshotEvidence {
                tab: tab.into(),
                captured,
                marker: before_marker,
                image,
                snapshot_before: before,
                snapshot_after: after,
            };
            if coordinate.is_some_and(|(x, y)| target(&evidence, x, y).is_err()) {
                continue;
            }
            return Ok((data, evidence));
        }
        Err("screenshot_stale: target or viewport changed during capture; capture again".into())
    }
    .await;
    let _ = evaluate(
        config,
        format!("document.getElementById({})?.remove()", json!(token)),
    )
    .await;
    result
}

#[derive(Debug, PartialEq)]
struct Target {
    backend: i64,
    bounds: Value,
}

fn ancestor(nodes: &Value, ancestor: usize, mut child: usize) -> bool {
    loop {
        if ancestor == child {
            return true;
        }
        let Some(parent) = nodes["parentIndex"][child].as_i64().filter(|p| *p >= 0) else {
            return false;
        };
        child = parent as usize;
    }
}

fn target(evidence: &ScreenshotEvidence, x: f64, y: f64) -> Result<Target, String> {
    let before = target_in_snapshot(&evidence.snapshot_before, &evidence.marker, x, y)?;
    let after = target_in_snapshot(&evidence.snapshot_after, &evidence.marker, x, y)?;
    if before != after {
        return Err("screenshot_stale: target changed around image capture".into());
    }
    Ok(before)
}

fn target_in_snapshot(snapshot: &Value, marker: &Value, x: f64, y: f64) -> Result<Target, String> {
    let stale = || {
        "screenshot_stale: ambiguous coordinate target; capture again or use an element".to_string()
    };
    let document = &snapshot["documents"][0];
    let nodes = &document["nodes"];
    let layout = &document["layout"];
    let indices = layout["nodeIndex"].as_array().ok_or_else(stale)?;
    let sx = marker[2].as_f64().ok_or_else(stale)?;
    let sy = marker[3].as_f64().ok_or_else(stale)?;
    let mut candidates = Vec::new();
    for (i, index) in indices.iter().enumerate() {
        let Some(node) = index.as_u64().map(|n| n as usize) else {
            continue;
        };
        if nodes["nodeType"][node] != 1 {
            continue;
        }
        let b = &layout["bounds"][i];
        let Some(rect) = b.as_array().filter(|r| r.len() == 4) else {
            continue;
        };
        let r: Vec<f64> = rect.iter().filter_map(Value::as_f64).collect();
        if r.len() != 4
            || x + sx < r[0]
            || y + sy < r[1]
            || x + sx >= r[0] + r[2]
            || y + sy >= r[1] + r[3]
        {
            continue;
        }
        let style = layout["styles"][i][0].as_u64().map(|v| v as usize);
        if style.is_some_and(|v| snapshot["strings"][v] == "none") {
            continue;
        }
        let paint = layout["paintOrders"][i].as_i64().ok_or_else(stale)?;
        candidates.push((paint, node, i));
    }
    let top = candidates.iter().map(|v| v.0).max().ok_or_else(stale)?;
    let mut chosen: Option<(usize, usize)> = None;
    for (_, node, i) in candidates.into_iter().filter(|v| v.0 == top) {
        if let Some((previous, _)) = chosen {
            if ancestor(nodes, node, previous) {
                continue;
            }
            if !ancestor(nodes, previous, node) {
                return Err(stale());
            }
        }
        chosen = Some((node, i));
    }
    let (node, i) = chosen.ok_or_else(stale)?;
    Ok(Target {
        backend: nodes["backendNodeId"][node].as_i64().ok_or_else(stale)?,
        bounds: layout["bounds"][i].clone(),
    })
}

fn pixels_match(old: &ScreenshotEvidence, new: &ScreenshotEvidence, x: f64, y: f64) -> bool {
    if old.image.dimensions() != new.image.dimensions() {
        return false;
    }
    let scale_x = old.image.width() as f64 / old.marker[4].as_f64().unwrap_or(1.0);
    let scale_y = old.image.height() as f64 / old.marker[5].as_f64().unwrap_or(1.0);
    let cx = (x * scale_x).round() as i64;
    let cy = (y * scale_y).round() as i64;
    for py in (cy - 32).max(0)..(cy + 32).min(old.image.height() as i64) {
        for px in (cx - 32).max(0)..(cx + 32).min(old.image.width() as i64) {
            if old.image.get_pixel(px as u32, py as u32)
                != new.image.get_pixel(px as u32, py as u32)
            {
                return false;
            }
        }
    }
    true
}

pub(super) async fn validate(
    config: &MintConfig,
    evidence: &ScreenshotEvidence,
    tab: &str,
    args: &Value,
) -> Result<(), String> {
    if evidence.tab != tab || evidence.captured.elapsed() >= Duration::from_secs(30) {
        return Err("screenshot_stale: screenshot expired or tab changed".into());
    }
    let x = args["x"].as_f64().ok_or("x required")?;
    let y = args["y"].as_f64().ok_or("y required")?;
    if !x.is_finite()
        || !y.is_finite()
        || x < 0.0
        || y < 0.0
        || x >= evidence.marker[4].as_f64().unwrap_or(0.0)
        || y >= evidence.marker[5].as_f64().unwrap_or(0.0)
    {
        return Err("screenshot_stale: coordinate outside viewport".into());
    }
    let original = target(evidence, x, y)?;
    let (_, current) = capture_at(config, tab, Some((x, y))).await?;
    if evidence.marker != current.marker
        || original != target(&current, x, y)?
        || !pixels_match(evidence, &current, x, y)
    {
        return Err("screenshot_stale: target identity, geometry, or pixels changed".into());
    }
    // DOM location lookup uses main-document coordinates, while screenshots
    // and Input.dispatchMouseEvent use viewport coordinates.
    let document_x = x + current.marker[2]
        .as_f64()
        .ok_or("screenshot_stale: missing scroll geometry")?;
    let document_y = y + current.marker[3]
        .as_f64()
        .ok_or("screenshot_stale: missing scroll geometry")?;
    let hit=cdp_call_raw(config,"DOM.getNodeForLocation",json!({"x":document_x.floor() as i64,"y":document_y.floor() as i64,"includeUserAgentShadowDOM":false,"ignorePointerEventsNone":false})).await?;
    if hit["result"]["backendNodeId"].as_i64() != Some(original.backend) {
        return Err("screenshot_stale: target is obstructed".into());
    }
    Ok(())
}

pub(super) async fn input(
    config: &MintConfig,
    evidence: &ScreenshotEvidence,
    tab: &str,
    action: &str,
    args: &Value,
) -> Result<String, String> {
    let x = args["x"].as_f64().ok_or("x required")?;
    let y = args["y"].as_f64().ok_or("y required")?;
    super::mouse_move(config, x, y).await?;
    tokio::time::sleep(Duration::from_millis(60)).await;
    validate(config, evidence, tab, args).await?;
    if action == "browser_mouse_move" {
        return Ok("mouse moved".into());
    }
    let button = match args["button"].as_str() {
        Some("right") => "right",
        Some("middle") => "middle",
        _ => "left",
    };
    // No second move or cosmetic animation between the final check and press.
    for kind in ["mousePressed", "mouseReleased"] {
        cdp_call_raw(config,"Input.dispatchMouseEvent",json!({"type":kind,"x":x,"y":y,"button":button,"buttons":if kind=="mousePressed" {match button {"right"=>2,"middle"=>4,_=>1}} else {0},"clickCount":1})).await?;
    }
    super::navigate::wait_for_page_load(config).await;
    Ok(format!("clicked at ({x:.0},{y:.0}) with {button} button"))
}
