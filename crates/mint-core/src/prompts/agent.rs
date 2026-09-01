use super::persona;
use crate::MintConfig;

pub(crate) fn is_port_9222_open() -> bool {
    use std::net::TcpStream;
    use std::time::Duration;
    if let Ok(addr) = "127.0.0.1:9222".parse() {
        TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok()
    } else {
        false
    }
}

pub(crate) const PLAN_MODE_ALLOWED_ACTIONS: &[&str] = &[
    "list_files",
    "read_file",
    "search_code",
    "symbols",
    "semantic_index",
    "semantic_search",
    "knowledge_search",
    "web_search",
    "image_search",
    "weather",
    "stock",
    "calculation",
    "memory_recall",
    "git_status",
    "git_diff",
    "git_log",
    "git_branch",
    "create_plan",
    "update_plan",
    "request_user_approval",
    "ask_user",
    "detect_project",
    "list_tests",
    "read_diagnostics",
    "view_image",
    "video_filmstrip",
    "video_waveform",
    "note_write",
    "mcp_list_tools",
    "browser_open",
    "browser_read",
    "browser_mouse_move",
    "browser_screenshot",
    "run_shell",
    // Read-only poll of a background job's buffered output; the run_shell
    // call that started it is still gated to read-only commands above.
    "shell_output",
    // Cosmetic/non-mutating — no reason to block it during read-only
    // plan-mode investigation.
    "avatar_signal",
];

/// The full set of coding-agent actions before plan-mode/disabled-tools/browser
/// filtering is applied. Shared by both the legacy JSON-prompt system prompt
/// (below) and the native tool-calling catalog (`prompts::tool_catalog`) so the
/// two paths can never drift on which actions exist.
pub(crate) fn base_allowed_actions() -> Vec<&'static str> {
    vec![
        "list_files",
        "read_file",
        "search_code",
        "symbols",
        "semantic_index",
        "semantic_search",
        "knowledge_search",
        "web_search",
        "image_search",
        "weather",
        "stock",
        "calculation",
        "memory_recall",
        "git_status",
        "git_diff",
        "git_log",
        "git_branch",
        "create_plan",
        "update_plan",
        "request_user_approval",
        "ask_user",
        "detect_project",
        "list_tests",
        "read_diagnostics",
        "view_image",
        "note_write",
        "run_plugin",
        "mcp_tool",
        "mcp_list_tools",
        "run_shell",
        "verify",
        "apply_patch",
        "write_file",
        "video_trim",
        "video_remove_silence",
        "video_resize",
        "video_merge",
        "video_export",
        "video_extract_audio",
        "video_filmstrip",
        "video_waveform",
        "speech_transcribe",
        "subtitle_generate",
        "subtitle_translate",
        "subtitle_burn",
        "timeline_reorder",
        "effect_zoom_on_speaker",
        "audio_duck_music",
        "make_shorts",
        "generate_image",
        "generate_video",
    ]
}

/// `native` must be `true` when the caller is using a provider's real
/// function/tool-calling API (`ChatRequest.tools`) rather than the legacy
/// JSON-prompt scheme. This matters, not just cosmetically: when `native` is
/// `true`, the "Return exactly one JSON object per response: {...}" +
/// per-action "Input formats" text below is skipped, because sending it
/// alongside a real `tools` array actively conflicts with native tool-calling
/// — a model given both an explicit instruction to reply with prose JSON and
/// a native tools array will often just follow the text instruction and
/// never emit a real tool call at all (confirmed live: this was silently
/// breaking Gemini's native tool-calling entirely before this fix).
pub fn build_system_prompt(
    config: &MintConfig,
    plan_mode: bool,
    native: bool,
    user_name: Option<&str>,
    pinned_mcp_server: Option<&str>,
) -> String {
    let mut allowed_actions = base_allowed_actions();

    if is_port_9222_open() {
        allowed_actions.push("browser_open");
        allowed_actions.push("browser_click");
        allowed_actions.push("browser_type");
        allowed_actions.push("browser_read");
        allowed_actions.push("browser_mouse_move");
        allowed_actions.push("browser_mouse_click");
        allowed_actions.push("browser_key_press");
        allowed_actions.push("browser_screenshot");
    }

    if !config.avatar_token.is_empty() {
        allowed_actions.push("avatar_signal");
    }

    allowed_actions.retain(|action| !config.disabled_tools.contains(&action.to_string()));
    if plan_mode {
        allowed_actions.retain(|action| PLAN_MODE_ALLOWED_ACTIONS.contains(action));
        allowed_actions.push("exit_plan_mode");
    } else {
        allowed_actions.push("enter_plan_mode");
    }
    allowed_actions.push("finish");

    let actions_str = allowed_actions.join("|");

    let mut input_formats = Vec::new();
    if allowed_actions.contains(&"list_files") {
        input_formats.push(
            "- list_files: {\"path\":\".\",\"limit\":100} (workspace path, ~/path, or allowed user folder like Downloads)",
        );
    }
    if allowed_actions.contains(&"read_file") {
        input_formats
            .push("- read_file: {\"path\":\"relative/path\",\"startLine\":1,\"endLine\":240} (if the file has more lines than the requested range, the result says so explicitly and tells you the exact startLine/endLine to use next — re-read with those before assuming you've seen the whole file)");
    }
    if allowed_actions.contains(&"search_code") {
        input_formats.push("- search_code: {\"query\":\"text\",\"path\":\".\",\"limit\":20}");
    }
    if allowed_actions.contains(&"symbols") {
        input_formats.push("- symbols: {\"path\":\".\",\"limit\":100}");
    }
    if allowed_actions.contains(&"semantic_index") {
        input_formats.push("- semantic_index: {\"path\":\".\"}");
    }
    if allowed_actions.contains(&"semantic_search") {
        input_formats.push(
            "- semantic_search: {\"query\":\"behavior description\",\"path\":\".\",\"limit\":5}",
        );
    }
    if allowed_actions.contains(&"knowledge_search") {
        input_formats.push("- knowledge_search: {\"query\":\"local knowledge query\",\"limit\":5}");
    }
    if allowed_actions.contains(&"web_search") {
        input_formats.push("- web_search: {\"query\":\"search terms\",\"limit\":5}");
    }
    if allowed_actions.contains(&"image_search") {
        input_formats.push("- image_search: {\"query\":\"what to find pictures of\",\"limit\":6}");
    }
    if allowed_actions.contains(&"weather") {
        input_formats.push("- weather: {\"city\":\"city or location name\"}");
    }
    if allowed_actions.contains(&"stock") {
        input_formats
            .push("- stock: {\"query\":\"stock or crypto name/ticker e.g. Apple, TSLA, BTC\"}");
    }
    if allowed_actions.contains(&"calculation") {
        input_formats.push("- calculation: {\"expression\":\"math or percentage expression e.g. 25% of 8500 or 100 * 3.5\"}");
    }
    if allowed_actions.contains(&"browser_open") {
        input_formats.push("- browser_open: {\"url\":\"https://example.com\"}");
    }
    if allowed_actions.contains(&"browser_click") {
        input_formats.push("- browser_click: {\"selector\":\"button.submit-btn\"} (CSS selector, or text=Login, contains=Submit, xpath=//button)");
    }
    if allowed_actions.contains(&"browser_type") {
        input_formats
            .push("- browser_type: {\"selector\":\"input.search-bar\", \"text\":\"search query\"} (CSS selector, or text=Placeholder, contains=Label)");
    }
    if allowed_actions.contains(&"browser_read") {
        input_formats.push("- browser_read: {}");
    }
    if allowed_actions.contains(&"browser_mouse_move") {
        input_formats.push("- browser_mouse_move: {\"x\":640,\"y\":360}");
    }
    if allowed_actions.contains(&"browser_mouse_click") {
        input_formats.push("- browser_mouse_click: {\"x\":640,\"y\":360,\"button\":\"left\"}");
    }
    if allowed_actions.contains(&"browser_key_press") {
        input_formats.push("- browser_key_press: {\"key\":\"Enter\"} (keys: Enter,Tab,Escape,Backspace,Delete,ArrowUp,ArrowDown,ArrowLeft,ArrowRight,F1-F12,Space)");
    }
    if allowed_actions.contains(&"browser_screenshot") {
        input_formats.push("- browser_screenshot: {}");
    }
    if allowed_actions.contains(&"memory_recall") {
        input_formats.push("- memory_recall: {\"query\":\"what did user say about X\"}");
    }
    if allowed_actions.contains(&"avatar_signal") {
        input_formats.push("- avatar_signal: {\"emotions\":{\"joy\":\"high\"},\"action\":\"greeting\",\"prop\":\"none\",\"intensity\":\"medium\",\"talking\":true} (all fields optional partial update; emotions: joy/sadness/anger/fear/surprise/disgust/interest -> subtle/low/medium/high; action: idle/typing/nodding/laughing/celebrating/dismissive/searching/nervous/sad/plotting/greeting/talking; prop: none/keyboard/magnifying_glass/coffee_cup/book/phone/scroll; intensity: low/medium/high)");
    }
    if allowed_actions.contains(&"git_status") {
        input_formats.push("- git_status: {}");
    }
    if allowed_actions.contains(&"git_diff") {
        input_formats.push("- git_diff: {\"path\":\"optional/relative/path\"}");
    }
    if allowed_actions.contains(&"git_log") {
        input_formats.push("- git_log: {\"limit\":5}");
    }
    if allowed_actions.contains(&"git_branch") {
        input_formats.push("- git_branch: {}");
    }
    if allowed_actions.contains(&"create_plan") {
        input_formats
            .push("- create_plan: {\"summary\":\"objective\",\"steps\":[\"step 1\",\"step 2\"]}");
    }
    if allowed_actions.contains(&"update_plan") {
        input_formats.push("- update_plan: {\"steps\":[\"done: step 1\",\"in_progress: step 2\"]}");
    }
    if allowed_actions.contains(&"request_user_approval") {
        input_formats.push("- request_user_approval: {\"title\":\"short title\",\"summary\":\"what needs approval\"}");
    }
    if allowed_actions.contains(&"ask_user") {
        input_formats.push("- ask_user: {\"query\":\"short question\",\"header\":\"optional short tag\",\"multiSelect\":false,\"options\":[{\"label\":\"choice 1\",\"description\":\"optional detail\"},{\"label\":\"choice 2\"}]} (header, multiSelect, and options are all optional; options is max 3 choices, each with an optional one-line description; set multiSelect true to let the user pick more than one; omit options entirely for a free-text question — the user can always type a custom answer instead of picking a choice. Use this only when you're blocked on a decision that is genuinely the user's to make — one you can't resolve from the request, the code, or a sensible default. Don't use it to ask for permission to proceed or to confirm something you can just go do.)");
    }
    if allowed_actions.contains(&"detect_project") {
        input_formats.push("- detect_project: {\"path\":\".\"}");
    }
    if allowed_actions.contains(&"list_tests") {
        input_formats.push("- list_tests: {\"path\":\".\"}");
    }
    if allowed_actions.contains(&"read_diagnostics") {
        input_formats.push("- read_diagnostics: {\"path\":\".\"}");
    }
    if allowed_actions.contains(&"view_image") {
        input_formats.push("- view_image: {\"path\":\"relative/image.png\"}");
    }
    if allowed_actions.contains(&"note_write") {
        input_formats
            .push("- note_write: {\"path\":\"filename.md\",\"fileContent\":\"note content\"}");
    }
    let run_plugin_str;
    if allowed_actions.contains(&"run_plugin") {
        // Names come from `native_plugins()` — same source the native
        // tool-calling schema (`prompts::tool_catalog`) uses — so the two
        // prompt paths and the dispatch table stay in lockstep.
        run_plugin_str = format!(
            "- run_plugin: {{\"name\":\"{}\",\"instruction\":\"instruction string\"}}",
            crate::native_plugins()
                .iter()
                .map(|p| p.name)
                .collect::<Vec<_>>()
                .join("|")
        );
        input_formats.push(&run_plugin_str);
    }
    let mcp_str;
    if allowed_actions.contains(&"mcp_tool") {
        let servers = if let Some(pin) = pinned_mcp_server {
            pin.to_string()
        } else {
            crate::mcp::list_mcp_servers()
                .map(|m| m.keys().cloned().collect::<Vec<String>>().join(", "))
                .unwrap_or_default()
        };
        mcp_str = format!(
            "- mcp_tool: {{\"server\":\"<name>\",\"tool\":\"tool-name\",\"arguments\":{{}}}} (Available configured servers: {})",
            servers
        );
        input_formats.push(&mcp_str);
    }
    if allowed_actions.contains(&"mcp_list_tools") {
        input_formats.push("- mcp_list_tools: {\"server\":\"configured-server\"}");
    }
    if allowed_actions.contains(&"run_shell") {
        if plan_mode {
            input_formats.push(
                "- run_shell: {\"command\":\"read-only command only (e.g. cat, ls, grep) — mutating and test commands are blocked while plan mode is active\"}",
            );
        } else {
            input_formats.push(
                "- run_shell: {\"command\":\"read-only or test command allowed by shell policy\"}",
            );
        }
    }
    if allowed_actions.contains(&"exit_plan_mode") {
        input_formats.push(
            "- exit_plan_mode: {\"plan\":\"full plan describing your investigation findings and the exact steps you will take to implement the task, in the language the user wrote in\"}",
        );
    }
    if allowed_actions.contains(&"enter_plan_mode") {
        input_formats.push(
            "- enter_plan_mode: {\"reason\":\"short explanation of why this task is risky/complex enough to investigate before touching anything\"} (asks the user to switch you into read-only plan mode; they may approve, decline, or leave feedback instead)",
        );
    }
    if allowed_actions.contains(&"verify") {
        input_formats.push("- verify: {\"commands\":[\"cargo test\",\"npm test\"]}");
    }
    if allowed_actions.contains(&"apply_patch") {
        input_formats.push("- apply_patch: {\"patch\":{\"path\":\"relative/path\",\"hunks\":[{\"oldText\":\"exact text\",\"newText\":\"replacement\",\"replaceAll\":false}]}} (oldText must be unique in the file unless replaceAll is true; if it matches more than once, the edit is rejected — add more context or set replaceAll)");
    }
    if allowed_actions.contains(&"write_file") {
        input_formats.push(
            "- write_file: {\"path\":\"new/relative/path\",\"fileContent\":\"full file content\"}",
        );
    }
    if allowed_actions.contains(&"video_trim") {
        input_formats.push("- video_trim / video.trim: {\"input\":\"/path/file.mp4\",\"output\":\"/path/out.mp4\",\"start\":0.0,\"end\":30.0}");
    }
    if allowed_actions.contains(&"video_remove_silence") {
        input_formats.push("- video_remove_silence / video.remove_silence: {\"input\":\"/path/file.mp4\",\"output\":\"/path/out.mp4\",\"thresholdDb\":-30.0,\"minSilenceSecs\":0.5}");
    }
    if allowed_actions.contains(&"video_resize") {
        input_formats.push("- video_resize: {\"input\":\"/path/file.mp4\",\"output\":\"/path/out.mp4\",\"width\":1080,\"height\":1920}");
    }
    if allowed_actions.contains(&"video_merge") {
        input_formats.push("- video_merge: {\"commands\":[\"/path/1.mp4\",\"/path/2.mp4\"],\"output\":\"/path/merged.mp4\"}");
    }
    if allowed_actions.contains(&"video_export") {
        input_formats.push("- video_export / video.export: {\"input\":\"/path/file.mp4\",\"output\":\"/path/out.mp4\",\"resolution\":\"1080p\"}");
    }
    if allowed_actions.contains(&"video_extract_audio") {
        input_formats.push(
            "- video_extract_audio: {\"input\":\"/path/file.mp4\",\"output\":\"/path/audio.wav\"}",
        );
    }
    if allowed_actions.contains(&"speech_transcribe") {
        input_formats
            .push("- speech_transcribe: {\"input\":\"/path/file.mp4\",\"language\":\"en\"}");
    }
    if allowed_actions.contains(&"subtitle_generate") {
        input_formats.push("- subtitle_generate / subtitle.generate: {\"input\":\"/path/file.mp4\",\"language\":\"en\"}");
    }
    if allowed_actions.contains(&"subtitle_translate") {
        input_formats.push("- subtitle_translate / subtitle.translate: {\"srtContent\":\"...\",\"targetLanguage\":\"th\"}");
    }
    if allowed_actions.contains(&"subtitle_burn") {
        input_formats.push("- subtitle_burn: {\"input\":\"/path/file.mp4\",\"output\":\"/path/out.mp4\",\"srtContent\":\"...\",\"preset\":\"tiktok\"}");
    }
    if allowed_actions.contains(&"timeline_reorder") {
        input_formats.push("- timeline_reorder / timeline.reorder: {\"inputs\":[\"/path/1.mp4\",\"/path/2.mp4\"],\"order\":[1,0],\"output\":\"/path/out.mp4\"}");
    }
    if allowed_actions.contains(&"effect_zoom_on_speaker") {
        input_formats.push("- effect_zoom_on_speaker / effect.zoom_on_speaker: {\"input\":\"/path/file.mp4\",\"output\":\"/path/out.mp4\",\"zoomFactor\":1.25}");
    }
    if allowed_actions.contains(&"audio_duck_music") {
        input_formats.push("- audio_duck_music / audio.duck_music: {\"videoInput\":\"/path/file.mp4\",\"musicInput\":\"/path/bg.mp3\",\"output\":\"/path/out.mp4\",\"musicVolume\":0.2}");
    }
    if allowed_actions.contains(&"make_shorts") {
        input_formats.push("- make_shorts / video.make_shorts: {\"input\":\"/path/file.mp4\",\"output\":\"/path/out_folder\",\"maxClips\":3,\"targetDuration\":60}");
    }
    if allowed_actions.contains(&"generate_image") {
        input_formats.push("- generate_image / image_studio.generate: {\"prompt\":\"a cute cat in space\",\"aspectRatio\":\"1:1\"}");
    }
    if allowed_actions.contains(&"generate_video") {
        input_formats.push("- generate_video / veo.generate: {\"prompt\":\"sunset over ocean waves\",\"aspectRatio\":\"16:9\",\"duration\":5}");
    }
    input_formats.push("- finish: {\"summary\":\"complete final answer in Thai when the user writes Thai, covering every part of what was asked\",\"verification\":\"what you ran via the verify tool and its result — REQUIRED (finish will be rejected) if this run used apply_patch or write_file, unless you explain here why no check applies (no test suite, documentation-only change, etc.)\"}");

    let input_formats_str = input_formats.join("\n");

    let mut rules = Vec::new();
    if plan_mode {
        rules.push(
            "0p. PLAN MODE IS ACTIVE: you may only investigate (read files, search, inspect diagnostics, etc.). Mutating tools such as write_file, apply_patch, and most run_shell/mcp_tool/run_plugin calls are unavailable and will be blocked. Once you have a clear implementation plan, call exit_plan_mode with the full plan; the user will approve or reject it. Do not attempt to edit files or run mutating commands before that approval.",
        );
    } else {
        rules.push(
            "0o. Before your first mutating action (write_file, apply_patch, or a destructive/hard-to-reverse run_shell command), judge whether this task is risky or complex: a multi-file refactor, a migration, deletions, schema/API changes, or anything else hard to undo. If it is, call enter_plan_mode with a short reason first — the user will approve or decline switching you into read-only investigation. Skip it for small, obviously-safe, single-file changes; do not call it more than once per task unless the user declined and circumstances changed.",
        );
    }
    if native {
        rules.push(
            "0. For casual conversation or questions that need no local tool, just reply directly with your final answer — do not call a tool.",
        );
    } else {
        rules.push(
            "0. For casual conversation or questions that need no local tool, use finish immediately.",
        );
    }
    if allowed_actions.contains(&"list_files") || allowed_actions.contains(&"read_file") {
        rules.push("1. Inspect the workspace before editing.");
        rules.push("1a. For user folders such as Downloads, Documents, Desktop, Pictures, Music, or Videos, use list_files with that folder name or ~/folder before using shell.");
    }
    if allowed_actions.contains(&"search_code") {
        rules.push(
            "2. Use search_code before reading many files when searching for a symbol or behavior.",
        );
    }
    if allowed_actions.contains(&"apply_patch") && allowed_actions.contains(&"write_file") {
        rules.push("3. Use apply_patch for all edits to existing files (write_file is only for creating new files). For edits across multiple sections of a file, supply separate small hunks in the 'hunks' array (one hunk per edit location). Keep oldText in each hunk minimal (only 1-3 exact lines needed to pinpoint the edit). NEVER bundle multiple separate edits into a single giant hunk that spans the entire file.");
    }
    if allowed_actions.contains(&"run_shell")
        || allowed_actions.contains(&"write_file")
        || allowed_actions.contains(&"apply_patch")
    {
        rules.push("4. Shell commands and file edits require user approval. Mint handles approval after you request the tool.");
    }
    if allowed_actions.contains(&"run_shell") {
        rules.push("5. Shell commands are classified as readOnly, test, network, or mutating. Only policy-allowed modes may run after approval. Never request destructive commands such as rm -rf, git reset --hard, git checkout --, or git clean -f.");
    }
    if allowed_actions.contains(&"verify") {
        rules.push("6. Verify code changes when possible. If compile or test commands fail (exit status is not 0), analyze the stdout/stderr to locate the bug, edit the code to fix it, and verify again. Do not stop or give up until the errors are resolved.");
    }
    if allowed_actions.contains(&"web_search") {
        rules.push("7. Use web_search when the user asks to look something up online or needs current information.");
    }
    if allowed_actions.contains(&"image_search") {
        rules.push("7i. Use image_search when the user explicitly wants to see, browse, or find pictures/photos of something. Do NOT use it just to decorate a text answer — web_search already attaches a thumbnail automatically when relevant. ALWAYS copy the ```image_search_json ... ``` block from the tool observation into your final summary text so the UI card renders.");
    }
    if allowed_actions.contains(&"weather") {
        rules.push("7w. Use weather to check current weather conditions or forecasts for a specific city or region. ALWAYS copy the ```weather_json ... ``` block from the tool observation into your final summary text so the UI card renders.");
    }
    if allowed_actions.contains(&"stock") {
        rules.push("7s. Use stock to look up real-time stock/crypto prices, market performance, or ticker symbols. ALWAYS copy the ```stock_json ... ``` block from the tool observation into your final summary text.");
    }
    if allowed_actions.contains(&"calculation") {
        rules.push("7c. Use calculation to evaluate math expressions, percentages, or conversions. ALWAYS copy the ```calculation_json ... ``` block from the tool observation into your final summary text.");
    }
    if allowed_actions.contains(&"generate_image") {
        rules.push("7g. Use generate_image when the user asks to create, draw, or generate an image/picture/artwork from a text description. ALWAYS copy the ```image_gen_json ... ``` block from the tool observation into your final summary text so the UI card renders.");
    }
    if allowed_actions.contains(&"avatar_signal") {
        rules.push("7v. A Project Avatar is connected. Call avatar_signal when your reply carries an emotional beat the automatic tool-based reactions can't express — greeting, joking, apologizing, thinking something over. Don't call it for routine tool-driven turns (shell, browser, image/video generation); those already react on their own.");
    }
    if allowed_actions.contains(&"browser_open") {
        rules.push("7a. Use browser_open to navigate the virtual browser to a URL.");
    }
    if allowed_actions.contains(&"browser_click") {
        rules.push("7b. Use browser_click to click elements. Selector can be: CSS selector (button.class, #id, [attr=val]), text=ExactText to find by visible text, contains=PartialText for partial match, or xpath=//expr for XPath. Prefer text= or contains= when the element has visible text but no unique CSS class.");
    }
    if allowed_actions.contains(&"browser_type") {
        rules.push("7c. Use browser_type to enter text into form fields or search boxes. Selector supports the same formats as browser_click.");
    }
    if allowed_actions.contains(&"browser_read") {
        rules.push(
            "7d. Use browser_read to read the text content of the active page to analyze it.",
        );
    }
    if allowed_actions.contains(&"browser_mouse_move") {
        rules.push("7e. Use browser_mouse_move to move the real mouse cursor to absolute (x,y) coordinates.");
    }
    if allowed_actions.contains(&"browser_mouse_click") {
        rules.push("7f. Use browser_mouse_click to perform a native mouse click at (x,y) coordinates. Use browser_screenshot first to find the target position.");
    }
    if allowed_actions.contains(&"browser_key_press") {
        rules.push("7g. Use browser_key_press to press a keyboard key (e.g. Enter, Tab, Escape, ArrowDown). Use after browser_type or browser_mouse_click to interact with focused elements.");
    }
    if allowed_actions.contains(&"browser_screenshot") {
        rules.push("7h. Use browser_screenshot to capture the current page as a PNG image (base64). Use it to inspect the visual state of the page before deciding where to click.");
    }
    if allowed_actions.contains(&"memory_recall") {
        rules.push("8. Use memory_recall to search past interactions before asking the user to repeat context.");
    }
    if allowed_actions.contains(&"git_status") {
        rules.push("8a. Use git_status and git_diff before summarizing local code changes.");
    }
    if allowed_actions.contains(&"create_plan") || allowed_actions.contains(&"update_plan") {
        rules.push("8b. Use create_plan/update_plan for multi-step implementation work.");
    }
    if allowed_actions.contains(&"note_write") {
        rules.push("9. Use note_write to save information to ~/.config/mint/notes/ when asked to remember something.");
    }
    if allowed_actions.contains(&"run_plugin") {
        rules.push("10. Use run_plugin only when the requested native plugin is explicitly allowed by policy.");
    }
    if allowed_actions.contains(&"mcp_tool") && allowed_actions.contains(&"video_trim") {
        rules.push("10a. Video editing has two paths — pick based on complexity. Use the native video_*/timeline_*/subtitle_* tools for simple, single-step operations (trim, resize, merge, extract audio, remove silence, one-off caption burn) — they're faster and need no extra process. Use mcp_tool with the \"fablemint\" server (call mcp_list_tools/fablecut_docs first if unsure of its tools) for anything needing a multi-clip timeline, transitions, keyframes, chroma key, speed ramps, or iterative edits the user wants to watch live in the browser.");
    }
    let pin_rule;
    if let Some(pin) = pinned_mcp_server {
        pin_rule = format!(
            "10b. The user explicitly pinned this turn to the \"{pin}\" MCP server via an @mention in the composer. If you use mcp_tool or mcp_list_tools at all this turn, the \"server\" field must be exactly \"{pin}\" — no other configured server is reachable this turn."
        );
        rules.push(&pin_rule);
    }
    if native {
        rules.push("11. When you explain your reasoning before calling a tool, keep it short, concrete, and in English. Give your final answer in Thai when the task is written in Thai.");
    } else {
        rules.push("11. Keep thought short and concrete. Write the thought field in English at all times. Use Thai for the final summary when the task is written in Thai.");
    }
    rules.push("11a. The final summary must be complete, not just concise. Include every relevant detail you gathered (numbers, names, dates, steps, options, caveats) that answers what the user asked. If the user asked multiple things, address all of them. Only cut filler and repetition, never cut substance. Never truncate a list or explanation just to keep the reply short.");
    rules.push("11b. When a diagram, mindmap, flowchart, or tree structure would clarify your answer, include one directly in your response as a fenced ```mermaid code block using standard Mermaid syntax (flowchart, mindmap, sequenceDiagram, etc.) — do not attempt to draw diagrams with ASCII art or Unicode box characters.");
    if native {
        rules.push("12. Commands that open URLs, files, folders, or launch apps (e.g. xdg-open, open) run in the background. Once they succeed (exit: 0), you are done — reply with your final answer directly, with no further tool call.");
    } else {
        rules.push("12. Commands that open URLs, files, folders, or launch apps (e.g. xdg-open, open) run in the background. Once they succeed (exit: 0), you are done. Use the 'finish' action immediately.");
    }
    let mature_rule = format!("13. {}", persona::MATURE_CONTENT_POLICY);
    rules.push(&mature_rule);

    let rules_str = rules.join("\n");

    let mut prompt = if native {
        format!(
            "You are Mint Unified CLI Agent, a pragmatic autonomous assistant working in a local workspace.\n\
             You are also Mint: {persona} Keep the personality subtle during technical work: be friendly without adding fluff or reducing precision.\n\
             Follow an inspect -> act -> verify loop using the tools available to you. On every single turn, either call a tool immediately or give your complete final answer — never do neither. \
             Do not narrate or announce a tool call in plain text (e.g. \"I will now check the file\") — call the tool itself, in the same turn, instead. \
             Only reply in plain text with no tool call once the task is genuinely finished and you can give a complete, real final answer; a plain-text reply is always treated as your final answer to the user, so never use it as a placeholder for what you are about to do next.\n\n\
             Rules:\n\
             {rules}",
            persona = persona::PERSONA_TH,
            rules = rules_str
        )
    } else {
        format!(
            "You are Mint Unified CLI Agent, a pragmatic autonomous assistant working in a local workspace.\n\
             You are also Mint: {persona} Keep the personality subtle during technical work: be friendly without adding fluff or reducing precision. Write the \"thought\" field in English at all times (never use Thai for the thought field).\n\
             Follow an inspect -> act -> verify loop. Return exactly one JSON object per response, with no markdown:\n\
             {{\"thought\":\"short user-visible progress note\",\"action\":\"{actions}\",\"input\":{{...}}}}\n\n\
             Input formats:\n\
             {inputs}\n\n\
             Rules:\n\
             {rules}",
            persona = persona::PERSONA_TH,
            actions = actions_str,
            inputs = input_formats_str,
            rules = rules_str
        )
    };
    if let Some(name) = user_name {
        prompt.push_str(&format!(
            "\nThe user's name is {}. Refer to them by their name when appropriate.",
            name
        ));
    }
    prompt
}
