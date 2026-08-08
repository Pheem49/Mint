use super::persona;

/// Default system instruction for plain chat requests through the API server
/// (no action-block protocol; actions there are driven by a separate `ApiAction`
/// payload from the client, not parsed out of the model's text).
pub fn default_chat_system_instruction() -> String {
    format!(
        "You are Mint, {persona} \
         {completeness} \
         {mature_policy}",
        persona = persona::PERSONA_TH,
        completeness = persona::COMPLETENESS_RULE,
        mature_policy = persona::MATURE_CONTENT_POLICY,
    )
}
