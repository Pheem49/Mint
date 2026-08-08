//! The green aura + animated cursor overlay injected into the automated page
//! so a person watching the browser can see what Mint is doing.

use crate::MintConfig;
use serde_json::json;

use super::cdp::cdp_call_raw;

/// Inject the green aura + animated cursor overlay into the current page.
/// Best-effort: errors are silently ignored.
pub(super) async fn inject_overlay(config: &MintConfig) {
    let script = build_overlay_script();
    let _ = cdp_call_raw(
        config,
        "Runtime.evaluate",
        json!({ "expression": script, "returnByValue": false }),
    )
    .await;
}

pub(super) fn build_overlay_script() -> &'static str {
    r##"
    (() => {
        // ── Green aura border ────────────────────────────────────────────────
        if (!document.getElementById('mint-browser-aura')) {
            const style = document.createElement('style');
            style.id = 'mint-browser-aura-style';
            style.textContent = `
                @keyframes mint-pulse {
                    0%   { box-shadow: inset 0 0 15px rgba(16,185,129,0.4); border-color: rgba(16,185,129,0.6); }
                    100% { box-shadow: inset 0 0 30px rgba(16,185,129,0.8); border-color: rgba(16,185,129,1);   }
                }
            `;
            document.head.appendChild(style);

            const aura = document.createElement('div');
            aura.id = 'mint-browser-aura';
            aura.style.cssText = `
                position:fixed; top:0; left:0; width:100vw; height:100vh;
                border:5px solid rgba(16,185,129,0.6);
                pointer-events:none; z-index:2147483646;
                box-sizing:border-box;
                animation:mint-pulse 1.5s infinite alternate;
            `;
            document.body.appendChild(aura);
        }

        // ── Animated mouse cursor overlay ─────────────────────────────────────
        if (!document.getElementById('mint-cursor-overlay')) {
            const cursor = document.createElement('div');
            cursor.id = 'mint-cursor-overlay';
            cursor.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 28 28">
                <defs>
                    <filter id="mint-cs" x="-30%" y="-30%" width="160%" height="160%">
                        <feDropShadow dx="1" dy="2" stdDeviation="2.5" flood-color="rgba(0,0,0,0.5)"/>
                    </filter>
                </defs>
                <path d="M5 2 L5 22 L9.5 17 L13.5 24.5 L16 23 L12 15.5 L19 15.5 Z"
                      fill="white" stroke="#10b981" stroke-width="1.6"
                      stroke-linejoin="round" filter="url(#mint-cs)"/>
            </svg>`;
            cursor.style.cssText = `
                position:fixed; left:0; top:0; pointer-events:none;
                z-index:2147483647;
                transition:left 0.06s ease-out, top 0.06s ease-out, transform 0.08s ease, opacity 0.08s ease;
                transform-origin:5px 2px;
            `;
            document.body.appendChild(cursor);
        }
    })()
    "##
}
