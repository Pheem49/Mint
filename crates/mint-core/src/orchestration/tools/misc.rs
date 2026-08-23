use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to misc.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    _root: &Path,
    _config: &MintConfig,
    chat_id: &str,
    _approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "weather" => {
            let city = if !input.city.trim().is_empty() {
                input.city.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else if !input.path.trim().is_empty() {
                input.path.trim()
            } else {
                "Thailand"
            };
            match crate::weather::weather(city).await {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Weather lookup succeeded. In your finish summary, you MUST include the exact ```weather_json ... ``` code block from above in your response so the user sees the weather card UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Weather lookup failed for {city}: {e}")),
            }
        }
        "stock" => {
            let symbol = if !input.query.trim().is_empty() {
                input.query.trim()
            } else if !input.name.trim().is_empty() {
                input.name.trim()
            } else if !input.path.trim().is_empty() {
                input.path.trim()
            } else {
                "AAPL"
            };
            match crate::stock::stock(symbol).await {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Stock lookup succeeded. In your finish summary, you MUST include the exact ```stock_json ... ``` code block from above in your response so the user sees the stock card UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Stock lookup failed for {symbol}: {e}")),
            }
        }
        "calculation" => {
            let expr = if !input.expression.trim().is_empty() {
                input.expression.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else if !input.command.trim().is_empty() {
                input.command.trim()
            } else {
                "0"
            };
            match crate::calculation::calculate(expr) {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Calculation succeeded. In your finish summary, you MUST include the exact ```calculation_json ... ``` code block from above in your response so the user sees the calculation card UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Calculation failed for {expr}: {e}")),
            }
        }
        "memory_recall" => {
            let query = required(&input.query, "query")?;
            let query_lower = query.to_ascii_lowercase();
            let mut results = Vec::new();

            if let Ok(memory) = MemoryStore::open_default() {
                if let Ok(interactions) = memory.recent_interactions_for_chat(chat_id, 50) {
                    for item in interactions.iter().rev() {
                        if item.user_text.to_ascii_lowercase().contains(&query_lower)
                            || item.ai_text.to_ascii_lowercase().contains(&query_lower)
                        {
                            results.push(format!(
                                "[{}] You: {}\nMint: {}",
                                // `created_at` is a SQLite DATETIME string
                                // (always ASCII digits/dashes/colons), so a
                                // byte-index slice here is safe.
                                &item.created_at[..16.min(item.created_at.len())],
                                truncate_for_context(&item.user_text, 200),
                                truncate_for_context(&item.ai_text, 200),
                            ));
                            if results.len() >= 5 {
                                break;
                            }
                        }
                    }
                }

                if let Ok(skills) = memory.learned_skills(20) {
                    for skill in &skills {
                        if skill.content.to_ascii_lowercase().contains(&query_lower)
                            || skill.name.to_ascii_lowercase().contains(&query_lower)
                        {
                            results.push(format!(
                                "[Skill: {}]\n{}",
                                skill.name,
                                truncate_for_context(&skill.content, 300)
                            ));
                        }
                    }
                }
            }

            if results.is_empty() {
                Ok(format!("No memory found matching: {query}"))
            } else {
                Ok(results.join("\n\n"))
            }
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::misc::execute: {action}"
        ),
    }
}
