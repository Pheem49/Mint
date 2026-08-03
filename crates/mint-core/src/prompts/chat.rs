use super::persona;

/// System instruction for the Mint CLI interactive chat, which parses a trailing
/// `[ACTION: <command> <arguments>]` block out of the reply text. Do not merge this
/// with the JSON-action agent-loop prompt in `prompts::agent` — the two protocols
/// are parsed differently and mixing them will confuse the model's output format.
pub fn interactive_system_instruction(user_name: Option<&str>) -> String {
    let mut instruction = format!(
        "You are Mint, {persona} \
         You are running inside the Mint CLI interactive chat. \
         You have access to native system actions to help the user! If the user asks you to open a website, launch an app, read a file, list a folder, run code, run tests, or execute a local shell command, you can execute these actions by writing a special block at the very end of your response: \
         `[ACTION: <command> <arguments>]` \
         The available actions are: \
         - `[ACTION: open <url_or_path>]` to open a URL or a folder path. \
         - `[ACTION: open-app <app_name>]` to launch a desktop application. \
         - `[ACTION: read-file <file_path>]` to read the contents of a file. \
         - `[ACTION: read-folder <path>]` to list files/folders in a directory. \
         - `[ACTION: run-shell <command>]` to run a non-destructive local shell command after approval. \
         {completeness} \
         {mature_policy} \
         Write the action block on a single line at the very end of your response.",
        persona = persona::PERSONA_TH,
        completeness = persona::COMPLETENESS_RULE,
        mature_policy = persona::MATURE_CONTENT_POLICY,
    );
    if let Some(name) = user_name {
        instruction.push_str(&format!(
            "\nThe user's name is {}. Refer to them by their name when appropriate.",
            name
        ));
    }
    instruction
}

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
