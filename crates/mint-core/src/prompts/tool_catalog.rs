//! Per-tool JSON schemas for native function/tool-calling.
//!
//! This is the native-tool-calling counterpart to `prompts::agent::build_system_prompt`'s
//! `input_formats` text: instead of describing each tool's arguments in prose inside the
//! prompt, `tool_catalog` returns a real `ToolSpec` (name + description + JSON schema) per
//! action, which `orchestrate_agent_loop` sends via `ChatRequest.tools` so the provider
//! enforces the shape instead of the model merely being asked to follow it. Action names
//! and per-action semantics are shared with the legacy path via
//! `prompts::agent::base_allowed_actions` and `prompts::agent::PLAN_MODE_ALLOWED_ACTIONS`.

use std::path::Path;

use serde_json::{Value, json};

use super::agent::{PLAN_MODE_ALLOWED_ACTIONS, base_allowed_actions, is_port_9222_open};
use crate::MintConfig;
use crate::chat::ToolSpec;
use crate::subagents::list_subagents;

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

fn tool(name: &str, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

/// Builds the native tool-calling catalog for the current request: the set of
/// actions available given `config.disabled_tools`, whether the automation
/// browser is reachable on port 9222, and whether plan mode is active (which
/// restricts to investigation-only actions plus `exit_plan_mode`).
///
/// Mirrors the exact filtering `build_system_prompt` applies to
/// `base_allowed_actions`, so the two code paths can never offer a different
/// set of tools for the same request.
///
/// `dispatch_subagent` is deliberately **not** part of `base_allowed_actions`
/// (which also drives the legacy JSON-prompt path) — it's native-tool-calling
/// only, same scoping as context compaction. `allow_subagent_dispatch` should
/// be `false` when building the catalog for a subagent's own nested loop, so
/// subagents can never dispatch further subagents (the depth limit is
/// enforced by simply never offering the tool, not by a counter).
pub fn tool_catalog(
    config: &MintConfig,
    plan_mode: bool,
    root: &Path,
    allow_subagent_dispatch: bool,
) -> Vec<ToolSpec> {
    let mut allowed = base_allowed_actions();
    if is_port_9222_open() {
        allowed.push("browser_open");
        allowed.push("browser_click");
        allowed.push("browser_type");
        allowed.push("browser_read");
        allowed.push("browser_mouse_move");
        allowed.push("browser_mouse_click");
        allowed.push("browser_key_press");
        allowed.push("browser_screenshot");
    }
    allowed.retain(|action| !config.disabled_tools.contains(&action.to_string()));
    if plan_mode {
        allowed.retain(|action| PLAN_MODE_ALLOWED_ACTIONS.contains(action));
        allowed.push("exit_plan_mode");
    }

    let mut tools: Vec<ToolSpec> = allowed
        .into_iter()
        .filter_map(|action| all_tools().into_iter().find(|t| t.name == action))
        .collect();

    if allow_subagent_dispatch && !plan_mode {
        let subagents = list_subagents(Some(root));
        if !subagents.is_empty() {
            tools.push(dispatch_subagent_tool(&subagents));
        }
    }

    tools
}

fn dispatch_subagent_tool(subagents: &[crate::subagents::SubagentDefinition]) -> ToolSpec {
    let names: Vec<String> = subagents.iter().map(|s| s.name.clone()).collect();
    let catalog_text = subagents
        .iter()
        .map(|s| format!("- {}: {}", s.name, s.description))
        .collect::<Vec<_>>()
        .join("\n");
    tool(
        "dispatch_subagent",
        &format!(
            "Delegate a focused sub-task to a specialized subagent that runs in its own \
             isolated context and returns only its final result — use this to investigate \
             something without filling up your own context with the intermediate steps. \
             Available subagents:\n{catalog_text}"
        ),
        schema(
            json!({
                "name": { "type": "string", "enum": names, "description": "Which subagent to dispatch." },
                "instruction": { "type": "string", "description": "The task/question to give the subagent." }
            }),
            &["name", "instruction"],
        ),
    )
}

fn all_tools() -> Vec<ToolSpec> {
    vec![
        tool(
            "list_files",
            "List files and directories under a workspace path, ~/path, or an allowed user folder (Downloads, Documents, Desktop, Pictures, Music, Videos).",
            schema(
                json!({
                    "path": { "type": "string", "description": "Directory to list. Defaults to \".\" (workspace root)." },
                    "limit": { "type": "integer", "description": "Max entries to return." }
                }),
                &[],
            ),
        ),
        tool(
            "read_file",
            "Read the contents of a file, optionally a line range. If the file has more lines \
             than fit in the requested range (default: lines 1-240), the result says so \
             explicitly and tells you the exact startLine/endLine to pass to continue reading — \
             re-read with those before assuming you've seen the whole file.",
            schema(
                json!({
                    "path": { "type": "string", "description": "Relative file path." },
                    "startLine": { "type": "integer", "description": "1-indexed first line to read. Defaults to 1." },
                    "endLine": { "type": "integer", "description": "Last line to read. Defaults to startLine + 239." }
                }),
                &["path"],
            ),
        ),
        tool(
            "search_code",
            "Search the workspace for text/regex matches. Prefer this over reading many files when locating a symbol or behavior.",
            schema(
                json!({
                    "query": { "type": "string", "description": "Text or regex to search for." },
                    "path": { "type": "string", "description": "Directory to search within. Defaults to \".\"." },
                    "limit": { "type": "integer", "description": "Max matches to return." }
                }),
                &["query"],
            ),
        ),
        tool(
            "symbols",
            "List code symbols (functions, types, etc.) under a path.",
            schema(
                json!({
                    "path": { "type": "string" },
                    "limit": { "type": "integer" }
                }),
                &[],
            ),
        ),
        tool(
            "semantic_index",
            "Build or refresh the semantic (embedding-based) code index for a path.",
            schema(json!({ "path": { "type": "string" } }), &["path"]),
        ),
        tool(
            "semantic_search",
            "Search code by natural-language behavior description using the semantic index.",
            schema(
                json!({
                    "query": { "type": "string", "description": "Behavior description, not literal text." },
                    "path": { "type": "string" },
                    "limit": { "type": "integer" }
                }),
                &["query"],
            ),
        ),
        tool(
            "knowledge_search",
            "Search the user's local indexed knowledge base (documents, notes).",
            schema(
                json!({ "query": { "type": "string" }, "limit": { "type": "integer" } }),
                &["query"],
            ),
        ),
        tool(
            "web_search",
            "Search the web for current information.",
            schema(
                json!({ "query": { "type": "string" }, "limit": { "type": "integer" } }),
                &["query"],
            ),
        ),
        tool(
            "image_search",
            "Find pictures/photos of something the user explicitly wants to browse or see. Not for decorating a text answer.",
            schema(
                json!({ "query": { "type": "string" }, "limit": { "type": "integer" } }),
                &["query"],
            ),
        ),
        tool(
            "weather",
            "Get current weather conditions or forecast for a city or location.",
            schema(json!({ "city": { "type": "string" } }), &["city"]),
        ),
        tool(
            "stock",
            "Look up real-time stock or crypto prices by name or ticker.",
            schema(json!({ "query": { "type": "string" } }), &["query"]),
        ),
        tool(
            "calculation",
            "Evaluate a math expression, percentage, or unit conversion.",
            schema(json!({ "expression": { "type": "string" } }), &["expression"]),
        ),
        tool(
            "browser_open",
            "Navigate the automation browser to a URL.",
            schema(json!({ "url": { "type": "string" } }), &["url"]),
        ),
        tool(
            "browser_click",
            "Click an element in the automation browser.",
            schema(
                json!({
                    "selector": {
                        "type": "string",
                        "description": "CSS selector, or text=ExactText, contains=PartialText, xpath=//expr."
                    }
                }),
                &["selector"],
            ),
        ),
        tool(
            "browser_type",
            "Type text into a form field or search box in the automation browser.",
            schema(
                json!({
                    "selector": { "type": "string", "description": "Same formats as browser_click." },
                    "text": { "type": "string" }
                }),
                &["selector", "text"],
            ),
        ),
        tool(
            "browser_read",
            "Read the text content of the active browser page.",
            schema(json!({}), &[]),
        ),
        tool(
            "browser_mouse_move",
            "Move the real mouse cursor to absolute (x, y) coordinates.",
            schema(
                json!({ "x": { "type": "number" }, "y": { "type": "number" } }),
                &["x", "y"],
            ),
        ),
        tool(
            "browser_mouse_click",
            "Perform a native mouse click at (x, y). Use browser_screenshot first to find the target position.",
            schema(
                json!({
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "button": { "type": "string", "description": "left | right | middle. Defaults to left." }
                }),
                &["x", "y"],
            ),
        ),
        tool(
            "browser_key_press",
            "Press a keyboard key in the automation browser.",
            schema(
                json!({
                    "key": {
                        "type": "string",
                        "description": "Enter, Tab, Escape, Backspace, Delete, ArrowUp/Down/Left/Right, F1-F12, Space, ..."
                    }
                }),
                &["key"],
            ),
        ),
        tool(
            "browser_screenshot",
            "Capture the current automation browser page as an image.",
            schema(json!({}), &[]),
        ),
        tool(
            "memory_recall",
            "Search past interactions/memory before asking the user to repeat context.",
            schema(json!({ "query": { "type": "string" } }), &["query"]),
        ),
        tool(
            "git_status",
            "Show the workspace git status.",
            schema(json!({}), &[]),
        ),
        tool(
            "git_diff",
            "Show git diff, optionally scoped to a path.",
            schema(json!({ "path": { "type": "string" } }), &[]),
        ),
        tool(
            "git_log",
            "Show recent git log entries.",
            schema(json!({ "limit": { "type": "integer" } }), &[]),
        ),
        tool(
            "git_branch",
            "List git branches.",
            schema(json!({}), &[]),
        ),
        tool(
            "create_plan",
            "Create a multi-step implementation plan for the user to see.",
            schema(
                json!({
                    "summary": { "type": "string", "description": "Overall objective." },
                    "steps": { "type": "array", "items": { "type": "string" } }
                }),
                &["summary", "steps"],
            ),
        ),
        tool(
            "update_plan",
            "Update the status of an existing plan's steps.",
            schema(
                json!({
                    "steps": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "e.g. \"done: step 1\", \"in_progress: step 2\"."
                    }
                }),
                &["steps"],
            ),
        ),
        tool(
            "request_user_approval",
            "Ask the user to approve a proposed action before proceeding.",
            schema(
                json!({ "title": { "type": "string" }, "summary": { "type": "string" } }),
                &["title", "summary"],
            ),
        ),
        tool(
            "ask_user",
            "Ask the user a short question, optionally with up to 3 short pick-a-choice options (the user can still answer freely).",
            schema(
                json!({
                    "query": { "type": "string" },
                    "options": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional, max 3 short choices."
                    }
                }),
                &["query"],
            ),
        ),
        tool(
            "detect_project",
            "Detect the project type/toolchain at a path (package manager, language, etc.).",
            schema(json!({ "path": { "type": "string" } }), &[]),
        ),
        tool(
            "list_tests",
            "List discoverable tests under a path.",
            schema(json!({ "path": { "type": "string" } }), &[]),
        ),
        tool(
            "read_diagnostics",
            "Read compiler/linter diagnostics for a path.",
            schema(json!({ "path": { "type": "string" } }), &[]),
        ),
        tool(
            "view_image",
            "View an image file from the workspace.",
            schema(json!({ "path": { "type": "string" } }), &["path"]),
        ),
        tool(
            "note_write",
            "Save a note to the user's local notes folder.",
            schema(
                json!({
                    "path": { "type": "string", "description": "Note filename, e.g. \"filename.md\"." },
                    "fileContent": { "type": "string" }
                }),
                &["path", "fileContent"],
            ),
        ),
        tool(
            "run_plugin",
            "Run a native plugin the user has explicitly enabled.",
            schema(
                json!({
                    "name": {
                        "type": "string",
                        "enum": ["gmail", "google_calendar", "notion", "docker", "spotify", "obsidian", "system_metrics"]
                    },
                    "instruction": { "type": "string" }
                }),
                &["name", "instruction"],
            ),
        ),
        tool(
            "mcp_tool",
            "Call a tool on a configured MCP server.",
            schema(
                json!({
                    "server": { "type": "string" },
                    "tool": { "type": "string" },
                    "arguments": { "type": "object" }
                }),
                &["server", "tool"],
            ),
        ),
        tool(
            "mcp_list_tools",
            "List the tools available on a configured MCP server.",
            schema(json!({ "server": { "type": "string" } }), &["server"]),
        ),
        tool(
            "run_shell",
            "Run a local shell command. Subject to safety classification and user approval; destructive commands are blocked outright.",
            schema(json!({ "command": { "type": "string" } }), &["command"]),
        ),
        tool(
            "exit_plan_mode",
            "Exit plan mode with a full implementation plan for the user to approve before any mutating action can run.",
            schema(
                json!({
                    "plan": {
                        "type": "string",
                        "description": "Full plan describing investigation findings and the exact implementation steps, in the language the user wrote in."
                    }
                }),
                &["plan"],
            ),
        ),
        tool(
            "verify",
            "Run verification commands (tests, builds, linters) after making changes.",
            schema(
                json!({ "commands": { "type": "array", "items": { "type": "string" } } }),
                &["commands"],
            ),
        ),
        tool(
            "apply_patch",
            "Apply one or more find-and-replace hunks to an existing file. This is the only way to edit existing files (write_file is for new files only). Keep oldText minimal (1-3 exact lines) per hunk; use separate hunks for separate edit locations.",
            schema(
                json!({
                    "patch": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "hunks": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "oldText": { "type": "string", "description": "Exact block of code to replace." },
                                        "newText": { "type": "string", "description": "Replacement block." },
                                        "replaceAll": {
                                            "type": "boolean",
                                            "description": "Replace every occurrence of oldText instead of requiring it to be unique. Defaults to false — if oldText matches more than once, the edit is rejected unless this is true."
                                        }
                                    },
                                    "required": ["oldText", "newText"]
                                }
                            }
                        },
                        "required": ["path", "hunks"]
                    }
                }),
                &["patch"],
            ),
        ),
        tool(
            "write_file",
            "Create a new file with the given content. Do not use this to edit an existing file — use apply_patch instead.",
            schema(
                json!({
                    "path": { "type": "string", "description": "New relative file path." },
                    "fileContent": { "type": "string" }
                }),
                &["path", "fileContent"],
            ),
        ),
        tool(
            "video_trim",
            "Trim a video to a start/end time range.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "output": { "type": "string" },
                    "start": { "type": "number" },
                    "end": { "type": "number" }
                }),
                &["input", "output"],
            ),
        ),
        tool(
            "video_remove_silence",
            "Remove silent segments from a video.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "output": { "type": "string" },
                    "thresholdDb": { "type": "number", "description": "Silence threshold in dB. Default -30.0." },
                    "minSilenceSecs": { "type": "number", "description": "Minimum silence duration to cut. Default 0.5." }
                }),
                &["input", "output"],
            ),
        ),
        tool(
            "video_resize",
            "Resize a video to a target width/height.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "output": { "type": "string" },
                    "width": { "type": "integer" },
                    "height": { "type": "integer" }
                }),
                &["input", "output", "width", "height"],
            ),
        ),
        tool(
            "video_merge",
            "Concatenate multiple video files into one.",
            schema(
                json!({
                    "commands": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Ordered list of input video paths to merge."
                    },
                    "output": { "type": "string" }
                }),
                &["commands", "output"],
            ),
        ),
        tool(
            "video_export",
            "Re-encode/export a video, optionally at a preset resolution.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "output": { "type": "string" },
                    "preset": { "type": "string", "description": "Target resolution/preset, e.g. \"1080p\"." }
                }),
                &["input", "output"],
            ),
        ),
        tool(
            "video_extract_audio",
            "Extract the audio track from a video into a separate file.",
            schema(
                json!({ "input": { "type": "string" }, "output": { "type": "string" } }),
                &["input", "output"],
            ),
        ),
        tool(
            "speech_transcribe",
            "Transcribe speech from an audio/video file.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "language": { "type": "string", "description": "e.g. \"en\". Omit to auto-detect." }
                }),
                &["input"],
            ),
        ),
        // Alias of `speech_transcribe` (same underlying implementation, offered
        // as a separate action name so "generate subtitles" reads more naturally
        // than "transcribe speech" — matches the legacy prompt's alias listing).
        tool(
            "subtitle_generate",
            "Generate subtitles by transcribing speech from an audio/video file.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "language": { "type": "string", "description": "e.g. \"en\". Omit to auto-detect." }
                }),
                &["input"],
            ),
        ),
        tool(
            "subtitle_translate",
            "Translate SRT subtitle content into another language.",
            schema(
                json!({
                    "srtContent": { "type": "string" },
                    "targetLanguage": { "type": "string", "description": "e.g. \"th\"." }
                }),
                &["srtContent", "targetLanguage"],
            ),
        ),
        tool(
            "subtitle_burn",
            "Burn SRT subtitles into a video.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "output": { "type": "string" },
                    "srtContent": { "type": "string" },
                    "preset": { "type": "string", "description": "Style preset, e.g. \"tiktok\"." }
                }),
                &["input", "output", "srtContent"],
            ),
        ),
        tool(
            "timeline_reorder",
            "Reorder and concatenate a list of clips into one output video.",
            schema(
                json!({
                    "inputs": { "type": "array", "items": { "type": "string" } },
                    "order": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "description": "New index order into `inputs`."
                    },
                    "output": { "type": "string" }
                }),
                &["inputs", "order", "output"],
            ),
        ),
        tool(
            "effect_zoom_on_speaker",
            "Apply a zoom effect on the speaker in a video.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "output": { "type": "string" },
                    "zoomFactor": { "type": "number", "description": "Default 1.25." }
                }),
                &["input", "output"],
            ),
        ),
        tool(
            "audio_duck_music",
            "Duck background music volume under a video's speech track.",
            schema(
                json!({
                    "videoInput": { "type": "string" },
                    "musicInput": { "type": "string" },
                    "output": { "type": "string" },
                    "musicVolume": { "type": "number", "description": "0.0-1.0. Default 0.2." }
                }),
                &["videoInput", "musicInput", "output"],
            ),
        ),
        tool(
            "make_shorts",
            "Automatically cut a long video into short highlight clips with burned-in subtitles.",
            schema(
                json!({
                    "input": { "type": "string" },
                    "output": { "type": "string", "description": "Output folder. Optional." },
                    "maxClips": { "type": "integer", "description": "Default 3." },
                    "targetDuration": { "type": "number", "description": "Seconds per clip. Default 60." }
                }),
                &["input"],
            ),
        ),
        tool(
            "generate_image",
            "Generate an image from a text prompt.",
            schema(
                json!({
                    "prompt": { "type": "string" },
                    "aspectRatio": { "type": "string", "description": "e.g. \"1:1\", \"16:9\". Default \"1:1\"." },
                    "provider": { "type": "string", "description": "Optional provider override." }
                }),
                &["prompt"],
            ),
        ),
        tool(
            "generate_video",
            "Generate a short video from a text prompt.",
            schema(
                json!({
                    "prompt": { "type": "string" },
                    "aspectRatio": { "type": "string", "description": "Default \"16:9\"." },
                    "duration": { "type": "number", "description": "Seconds. Default 5." },
                    "provider": { "type": "string" }
                }),
                &["prompt"],
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_every_base_action_and_exit_plan_mode() {
        let names: Vec<String> = all_tools().into_iter().map(|t| t.name).collect();
        for action in base_allowed_actions() {
            assert!(names.iter().any(|n| n == action), "missing schema for {action}");
        }
        assert!(names.iter().any(|n| n == "exit_plan_mode"));
    }

    #[test]
    fn plan_mode_catalog_only_offers_plan_safe_tools_plus_exit_plan_mode() {
        let config = MintConfig::default();
        let tools = tool_catalog(&config, true, Path::new("."), true);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"exit_plan_mode"));
        assert!(!names.contains(&"write_file"));
        assert!(!names.contains(&"apply_patch"));
    }

    #[test]
    fn disabled_tools_are_excluded_from_the_catalog() {
        let config = MintConfig {
            disabled_tools: vec!["web_search".into()],
            ..MintConfig::default()
        };
        let tools = tool_catalog(&config, false, Path::new("."), true);
        assert!(!tools.iter().any(|t| t.name == "web_search"));
        assert!(tools.iter().any(|t| t.name == "read_file"));
    }

    #[test]
    fn non_plan_mode_catalog_never_offers_exit_plan_mode() {
        let config = MintConfig::default();
        let tools = tool_catalog(&config, false, Path::new("."), true);
        assert!(!tools.iter().any(|t| t.name == "exit_plan_mode"));
    }

    fn write_subagent(dir: &std::path::Path, name: &str, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(format!("{name}.md")), content).unwrap();
    }

    #[test]
    fn dispatch_subagent_offered_only_when_allowed_and_subagents_exist() {
        let root = std::env::temp_dir().join("mint-tool-catalog-subagents");
        let _ = std::fs::remove_dir_all(&root);
        write_subagent(
            &root.join(".agents").join("subagents"),
            "reviewer",
            "---\nname: reviewer\ndescription: Reviews code.\n---\nReview the diff.",
        );
        let config = MintConfig::default();

        // No subagents visible at this root -> tool absent even when allowed.
        let empty_root = std::env::temp_dir().join("mint-tool-catalog-no-subagents");
        let _ = std::fs::remove_dir_all(&empty_root);
        std::fs::create_dir_all(&empty_root).unwrap();
        let tools = tool_catalog(&config, false, &empty_root, true);
        assert!(!tools.iter().any(|t| t.name == "dispatch_subagent"));

        // Subagents exist + allowed -> offered.
        let tools = tool_catalog(&config, false, &root, true);
        assert!(tools.iter().any(|t| t.name == "dispatch_subagent"));

        // Subagents exist but not allowed (this is itself a nested subagent
        // call) -> the depth-limit enforcement, never offered.
        let tools = tool_catalog(&config, false, &root, false);
        assert!(!tools.iter().any(|t| t.name == "dispatch_subagent"));

        // Subagents exist but plan mode is active -> never offered either.
        let tools = tool_catalog(&config, true, &root, true);
        assert!(!tools.iter().any(|t| t.name == "dispatch_subagent"));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&empty_root);
    }
}
