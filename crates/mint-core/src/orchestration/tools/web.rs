use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to web.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    _root: &Path,
    config: &MintConfig,
    _chat_id: &str,
    _approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "knowledge_search" => Ok(serde_json::to_string_pretty(
            &KnowledgeStore::open_default()
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?
                .search(required(&input.query, "query")?, input.limit.unwrap_or(5))
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
        )
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
        "web_search" => {
            let query = required(&input.query, "query")?;
            let limit = input.limit.unwrap_or(5);
            match crate::web_search::search(query, limit, config).await {
                Ok((hits, provider)) => {
                    if hits.is_empty() {
                        Ok("No web search results found.".to_owned())
                    } else {
                        let formatted: String = hits
                            .iter()
                            .enumerate()
                            .map(|(i, h)| {
                                let image_line = h
                                    .image_url
                                    .as_deref()
                                    .map(|img| format!("   Image: {img}\n"))
                                    .unwrap_or_default();
                                format!(
                                    "{}. {}\n   URL: {}\n{}{}\n",
                                    i + 1,
                                    h.title,
                                    h.url,
                                    image_line,
                                    h.snippet
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        Ok(format!(
                            "{formatted}\n\nNote: Web search succeeded using {provider} Search. In your finish summary, you MUST:\n1. Answer the user's question using the information above.\n2. Mention that you found this information via {provider} Search (e.g. \"I found this information using {provider} Search.\").\n3. INLINE IMAGES: For each result that includes an 'Image:' URL, embed it in your summary using standard markdown image syntax:\n   ![result title](image_url)\n   Place the image tag on its OWN LINE, immediately AFTER the bullet point or paragraph that references that result.\n   Only embed images when they add visual value — e.g. food, restaurants, travel, products, people, art, profiles.\n   Do NOT embed images for code snippets, math, API docs, or pure text answers.\nDo NOT list source URLs manually — the UI will display them automatically."
                        ))
                    }
                }
                Err(e) => Ok(format!(
                    "Web search error: {e}. Web search is currently unavailable. \
                     Do not try to search again. You MUST now proceed by calling the 'finish' action. \
                     In your finish summary, explain to the user in Thai that the web search failed (mentioning the search error: {e}), \
                     and then answer their query using your own pre-existing knowledge/database."
                )),
            }
        }
        "image_search" => {
            let query = required(&input.query, "query")?;
            let limit = input.limit.unwrap_or(6);
            match crate::image_search::image_search(query, limit, config).await {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Image search succeeded. In your finish summary, you MUST include the exact ```image_search_json ... ``` code block from above in your response so the user sees the image results UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Image search failed for '{query}': {e}")),
            }
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::web::execute: {action}"
        ),
    }
}
