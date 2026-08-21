//! File-based subagent definitions for the `dispatch_subagent` tool.
//!
//! A subagent is a `---` frontmatter Markdown file describing a focused
//! worker the top-level agent loop can delegate a sub-task to: it runs its
//! own isolated `orchestrate_agent_loop` (see `orchestration.rs`'s
//! `"dispatch_subagent"` arm) and only its final summary is returned to the
//! caller — its own step-by-step trajectory never leaks into the parent's
//! context. This mirrors `skills.rs`'s directory-scanning convention
//! (`.agents/skills/` workspace, a global directory under the user's config
//! dir) rather than inventing a new file layout.

use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};

const MAX_SUBAGENT_FILE_BYTES: u64 = 64 * 1024;

/// A subagent definition loaded from a `---` frontmatter Markdown file under
/// `.agents/subagents/` (workspace) or `~/.config/mint/mint-agents/`
/// (global).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubagentDefinition {
    pub name: String,
    pub description: String,
    /// Tool names this subagent is restricted to. `None` means the full
    /// normal catalog (minus `dispatch_subagent` itself — subagents can't
    /// recurse into further subagents).
    pub tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub provider: Option<String>,
    /// The Markdown body after the frontmatter block — becomes the
    /// subagent's system prompt.
    pub system_prompt: String,
    pub source_path: String,
    /// True for one of [`builtin_subagents`]'s entries rather than a file a
    /// user wrote — `source_path` is the sentinel `"<builtin>"`, not a real
    /// path, so `save_subagent`/`delete_subagent` refuse to touch it.
    #[serde(default)]
    pub builtin: bool,
}

/// Subagents available with zero setup — always present in
/// [`list_subagents`], so `dispatch_subagent` has something to offer the
/// model even on a fresh install. A user-authored file (global or
/// workspace) with the same `name` overrides its entry here, exactly like
/// workspace already overrides global.
pub fn builtin_subagents() -> Vec<SubagentDefinition> {
    let read_only_tools = || {
        Some(
            [
                "list_files",
                "read_file",
                "search_code",
                "symbols",
                "semantic_search",
                "knowledge_search",
                "git_status",
                "git_diff",
                "git_log",
                "git_branch",
                "detect_project",
                "list_tests",
                "read_diagnostics",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        )
    };
    vec![
        SubagentDefinition {
            name: "explorer".to_string(),
            description: "Fast, read-only code search — locate files, grep symbols, and answer \"where is X\" questions without writing or running anything.".to_string(),
            tools: read_only_tools(),
            model: None,
            provider: None,
            system_prompt: "You are a focused, read-only research subagent. Investigate the given \
                question by reading files, searching code, and checking git history, then report a \
                clear, concise answer. Do not write, edit, or delete files, and do not run shell \
                commands — if the task turns out to need changes, say so in your summary instead of \
                attempting them yourself."
                .to_string(),
            source_path: "<builtin>".to_string(),
            builtin: true,
        },
        SubagentDefinition {
            name: "general-purpose".to_string(),
            description: "Catch-all for any task that doesn't fit a more specialized subagent — research, multi-step edits, and running commands with the full toolset.".to_string(),
            tools: None,
            model: None,
            provider: None,
            system_prompt: "You are a general-purpose subagent with access to the full toolset. \
                Investigate and carry out the given task end to end — reading, writing, and \
                running commands as needed — then report a clear, concise summary of what you \
                did and found."
                .to_string(),
            source_path: "<builtin>".to_string(),
            builtin: true,
        },
        SubagentDefinition {
            name: "debugger".to_string(),
            description: "Investigates a bug or failing test — reads code, checks diagnostics, and runs commands to reproduce the failure, then reports the root cause. Does not write or edit files.".to_string(),
            tools: Some(
                [
                    "list_files",
                    "read_file",
                    "search_code",
                    "symbols",
                    "semantic_search",
                    "knowledge_search",
                    "git_status",
                    "git_diff",
                    "git_log",
                    "git_branch",
                    "detect_project",
                    "list_tests",
                    "read_diagnostics",
                    "run_shell",
                    "shell_output",
                    "kill_shell",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            ),
            model: None,
            provider: None,
            system_prompt: "You are a focused debugging subagent. Investigate the given bug or \
                failing test: read the relevant code, check diagnostics, and run commands \
                (tests, reproduction scripts, log inspection) to pin down the root cause. Report \
                a clear, concise explanation of what's wrong and where. Do not write, edit, or \
                delete files — if the task turns out to need a fix, say so in your summary \
                instead of applying it yourself."
                .to_string(),
            source_path: "<builtin>".to_string(),
            builtin: true,
        },
        SubagentDefinition {
            name: "plan".to_string(),
            description: "Software-architect subagent for designing an implementation plan — investigates the codebase read-only and returns a step-by-step plan, not code changes.".to_string(),
            tools: read_only_tools(),
            model: None,
            provider: None,
            system_prompt: "You are a software-architect subagent for designing implementation \
                plans. Investigate the codebase as needed to understand the current state, then \
                return a step-by-step plan: the critical files involved, the order of changes, \
                and any architectural trade-offs. Do not write, edit, or delete files, and do not \
                run shell commands that change state — you are planning the work, not doing it."
                .to_string(),
            source_path: "<builtin>".to_string(),
            builtin: true,
        },
    ]
}

/// Parses a subagent definition file's contents. Returns `None` if there's no
/// `---`-delimited frontmatter block, or no `name:` field (both required —
/// everything else is optional).
pub fn parse_subagent_definition(content: &str, source_path: &str) -> Option<SubagentDefinition> {
    let trimmed = content.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end_idx = rest.find("---")?;
    let frontmatter = &rest[..end_idx];
    let body = rest[end_idx + 3..].trim().to_string();

    let mut name = None;
    let mut description = String::new();
    let mut tools = None;
    let mut model = None;
    let mut provider = None;

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name:") {
            let value = unquote(value.trim());
            if !value.is_empty() {
                name = Some(value);
            }
        } else if let Some(value) = line.strip_prefix("description:") {
            description = unquote(value.trim());
        } else if let Some(value) = line.strip_prefix("tools:") {
            let value = unquote(value.trim());
            if !value.is_empty() {
                let list: Vec<String> = value
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                if !list.is_empty() {
                    tools = Some(list);
                }
            }
        } else if let Some(value) = line.strip_prefix("model:") {
            let value = unquote(value.trim());
            if !value.is_empty() {
                model = Some(value);
            }
        } else if let Some(value) = line.strip_prefix("provider:") {
            let value = unquote(value.trim());
            if !value.is_empty() {
                provider = Some(value);
            }
        }
    }

    Some(SubagentDefinition {
        name: name?,
        description,
        tools,
        model,
        provider,
        system_prompt: body,
        source_path: source_path.to_string(),
        builtin: false,
    })
}

fn unquote(value: &str) -> String {
    let is_quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if is_quoted {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

/// Scans a directory (non-recursive) for `*.md` subagent definition files.
fn load_subagents_from_dir(dir: &Path, list: &mut Vec<SubagentDefinition>) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        if metadata.len() > MAX_SUBAGENT_FILE_BYTES {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(definition) = parse_subagent_definition(&content, &path.to_string_lossy()) {
            list.push(definition);
        }
    }
}

/// Lists all subagent definitions visible for `workspace_root` — global
/// (`~/.config/mint/mint-agents/`) plus workspace (`.agents/subagents/`),
/// with workspace definitions winning on a name collision (loaded second,
/// overwriting in the dedup map).
pub fn list_subagents(workspace_root: Option<&Path>) -> Vec<SubagentDefinition> {
    let mut unique: BTreeMap<String, SubagentDefinition> = BTreeMap::new();
    for definition in builtin_subagents() {
        unique.insert(definition.name.clone(), definition);
    }

    let mut definitions = Vec::new();
    if let Some(home) = dirs::home_dir() {
        load_subagents_from_dir(
            &home.join(".config").join("mint").join("mint-agents"),
            &mut definitions,
        );
    }
    if let Some(root) = workspace_root {
        load_subagents_from_dir(&root.join(".agents").join("subagents"), &mut definitions);
    }
    for definition in definitions {
        unique.insert(definition.name.clone(), definition);
    }
    unique.into_values().collect()
}

/// Finds a single subagent definition by exact name.
pub fn find_subagent(name: &str, workspace_root: Option<&Path>) -> Option<SubagentDefinition> {
    list_subagents(workspace_root)
        .into_iter()
        .find(|definition| definition.name == name)
}

/// A subagent definition as submitted from the Settings UI — the write-side
/// counterpart to [`SubagentDefinition`], which is read-only (parsed straight
/// off disk).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentDraft {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    pub system_prompt: String,
    /// `"workspace"` writes to `.agents/subagents/` under `workspace_root`;
    /// anything else (including missing/empty) falls back to the global
    /// `~/.config/mint/mint-agents/` directory.
    #[serde(default)]
    pub scope: String,
    /// When editing an existing definition, the file it was originally loaded
    /// from — removed after the new file is written successfully so a rename
    /// (or a scope change) doesn't leave a stale duplicate behind.
    #[serde(default)]
    pub previous_source_path: Option<String>,
}

/// Writes `draft` to a `---`-frontmatter Markdown file and returns the
/// resulting definition (parsed back from what was actually written, so the
/// caller sees exactly what `list_subagents` would see next time).
pub fn save_subagent(
    draft: &SubagentDraft,
    workspace_root: Option<&Path>,
) -> Result<SubagentDefinition, String> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err("name is required".into());
    }
    if draft.system_prompt.trim().is_empty() {
        return Err("system prompt is required".into());
    }
    if draft.previous_source_path.as_deref() == Some("<builtin>") {
        return Err("cannot modify a built-in subagent".into());
    }

    let dir = if draft.scope == "workspace" {
        let root = workspace_root.ok_or("a workspace must be open to save a workspace subagent")?;
        root.join(".agents").join("subagents")
    } else {
        dirs::home_dir()
            .ok_or("could not determine the home directory")?
            .join(".config")
            .join("mint")
            .join("mint-agents")
    };
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join(format!("{}.md", slugify(name)));

    let mut frontmatter = format!("---\nname: {name}\n");
    if !draft.description.trim().is_empty() {
        frontmatter.push_str(&format!("description: {}\n", draft.description.trim()));
    }
    if let Some(tools) = draft.tools.as_ref().filter(|tools| !tools.is_empty()) {
        frontmatter.push_str(&format!("tools: {}\n", tools.join(", ")));
    }
    if let Some(model) = draft
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        frontmatter.push_str(&format!("model: {model}\n"));
    }
    if let Some(provider) = draft
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        frontmatter.push_str(&format!("provider: {provider}\n"));
    }
    frontmatter.push_str("---\n\n");
    let content = format!("{frontmatter}{}\n", draft.system_prompt.trim());

    fs::write(&path, &content).map_err(|error| error.to_string())?;

    if let Some(previous) = draft.previous_source_path.as_deref() {
        let previous_path = Path::new(previous);
        if previous_path != path.as_path() && previous_path.is_file() {
            let _ = fs::remove_file(previous_path);
        }
    }

    parse_subagent_definition(&content, &path.to_string_lossy())
        .ok_or_else(|| "failed to parse the subagent definition that was just written".to_string())
}

/// Deletes a subagent definition file by the `sourcePath` a caller got back
/// from `list_subagents`/`save_subagent`.
pub fn delete_subagent(source_path: &str) -> Result<(), String> {
    if source_path == "<builtin>" {
        return Err("cannot delete a built-in subagent".into());
    }
    fs::remove_file(source_path).map_err(|error| error.to_string())
}

fn slugify(name: &str) -> String {
    let normalized: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "subagent".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_frontmatter() {
        let content = "---\nname: code-reviewer\ndescription: Reviews diffs for bugs.\ntools: read_file, search_code, git_diff\nmodel: gemini-2.5-flash\nprovider: gemini\n---\nYou are a focused code-review subagent.\n";
        let definition = parse_subagent_definition(content, "test.md").unwrap();
        assert_eq!(definition.name, "code-reviewer");
        assert_eq!(definition.description, "Reviews diffs for bugs.");
        assert_eq!(
            definition.tools,
            Some(vec![
                "read_file".to_string(),
                "search_code".to_string(),
                "git_diff".to_string()
            ])
        );
        assert_eq!(definition.model.as_deref(), Some("gemini-2.5-flash"));
        assert_eq!(definition.provider.as_deref(), Some("gemini"));
        assert_eq!(
            definition.system_prompt,
            "You are a focused code-review subagent."
        );
    }

    #[test]
    fn parses_minimal_frontmatter_with_only_name() {
        let content = "---\nname: minimal\n---\nBody text.";
        let definition = parse_subagent_definition(content, "test.md").unwrap();
        assert_eq!(definition.name, "minimal");
        assert_eq!(definition.description, "");
        assert_eq!(definition.tools, None);
        assert_eq!(definition.model, None);
        assert_eq!(definition.provider, None);
    }

    #[test]
    fn rejects_missing_frontmatter() {
        assert!(parse_subagent_definition("No frontmatter here.", "test.md").is_none());
    }

    #[test]
    fn rejects_frontmatter_without_name() {
        let content = "---\ndescription: no name field\n---\nBody.";
        assert!(parse_subagent_definition(content, "test.md").is_none());
    }

    #[test]
    fn unquotes_quoted_values() {
        let content = "---\nname: \"quoted-name\"\ndescription: 'single quoted'\n---\nBody.";
        let definition = parse_subagent_definition(content, "test.md").unwrap();
        assert_eq!(definition.name, "quoted-name");
        assert_eq!(definition.description, "single quoted");
    }

    #[test]
    fn lists_and_finds_subagents_from_workspace_directory() {
        let root = std::env::temp_dir().join("mint-subagents-test-list");
        let _ = fs::remove_dir_all(&root);
        let subagents_dir = root.join(".agents").join("subagents");
        fs::create_dir_all(&subagents_dir).unwrap();
        fs::write(
            subagents_dir.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Reviews code.\n---\nReview the given diff.",
        )
        .unwrap();
        fs::write(subagents_dir.join("not-a-subagent.txt"), "ignored").unwrap();

        // `list_subagents` also always includes the built-in `explorer`
        // entry now, so this checks for `reviewer` specifically rather than
        // asserting an exact total count (covered by
        // `list_subagents_always_includes_builtins_and_lets_files_override_them`
        // below).
        let found = list_subagents(Some(&root));
        assert!(found.iter().any(|d| d.name == "reviewer"));

        let single = find_subagent("reviewer", Some(&root)).unwrap();
        assert_eq!(single.description, "Reviews code.");
        assert!(find_subagent("nonexistent", Some(&root)).is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn list_subagents_always_includes_builtins_and_lets_files_override_them() {
        let root = std::env::temp_dir().join("mint-subagents-test-builtin");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        // With no files at all, the builtin `explorer` is still there.
        let found = list_subagents(Some(&root));
        let explorer = found.iter().find(|d| d.name == "explorer").unwrap();
        assert!(explorer.builtin);
        assert_eq!(explorer.source_path, "<builtin>");

        // A user-authored file with the same name overrides it.
        let subagents_dir = root.join(".agents").join("subagents");
        fs::create_dir_all(&subagents_dir).unwrap();
        fs::write(
            subagents_dir.join("explorer.md"),
            "---\nname: explorer\ndescription: Custom override.\n---\nCustom body.",
        )
        .unwrap();
        let overridden = find_subagent("explorer", Some(&root)).unwrap();
        assert!(!overridden.builtin);
        assert_eq!(overridden.description, "Custom override.");

        // Deleting/modifying the builtin sentinel path is rejected.
        assert!(delete_subagent("<builtin>").is_err());
        assert!(
            save_subagent(
                &SubagentDraft {
                    name: "explorer".to_string(),
                    description: String::new(),
                    tools: None,
                    model: None,
                    provider: None,
                    system_prompt: "Body.".to_string(),
                    scope: "workspace".to_string(),
                    previous_source_path: Some("<builtin>".to_string()),
                },
                Some(&root),
            )
            .is_err()
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn saves_and_deletes_a_workspace_subagent() {
        let root = std::env::temp_dir().join("mint-subagents-test-save");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let draft = SubagentDraft {
            name: "Code Reviewer!".to_string(),
            description: "Reviews diffs.".to_string(),
            tools: Some(vec!["read_file".to_string(), "search_code".to_string()]),
            model: Some("gemini-2.5-flash".to_string()),
            provider: Some("gemini".to_string()),
            system_prompt: "You review diffs for bugs.".to_string(),
            scope: "workspace".to_string(),
            previous_source_path: None,
        };
        let saved = save_subagent(&draft, Some(&root)).unwrap();
        assert_eq!(saved.name, "Code Reviewer!");
        assert_eq!(
            saved.tools,
            Some(vec!["read_file".to_string(), "search_code".to_string()])
        );
        assert!(saved.source_path.ends_with("code-reviewer.md"));
        assert_eq!(
            find_subagent("Code Reviewer!", Some(&root)).unwrap().name,
            "Code Reviewer!"
        );

        delete_subagent(&saved.source_path).unwrap();
        assert!(find_subagent("Code Reviewer!", Some(&root)).is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn renaming_a_subagent_removes_the_old_file() {
        let root = std::env::temp_dir().join("mint-subagents-test-rename");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let first = save_subagent(
            &SubagentDraft {
                name: "old-name".to_string(),
                description: String::new(),
                tools: None,
                model: None,
                provider: None,
                system_prompt: "Body.".to_string(),
                scope: "workspace".to_string(),
                previous_source_path: None,
            },
            Some(&root),
        )
        .unwrap();

        let renamed = save_subagent(
            &SubagentDraft {
                name: "new-name".to_string(),
                description: String::new(),
                tools: None,
                model: None,
                provider: None,
                system_prompt: "Body.".to_string(),
                scope: "workspace".to_string(),
                previous_source_path: Some(first.source_path.clone()),
            },
            Some(&root),
        )
        .unwrap();

        assert!(!Path::new(&first.source_path).exists());
        assert!(Path::new(&renamed.source_path).exists());
        // +4 for the always-present builtins (explorer, general-purpose, debugger, plan).
        assert_eq!(list_subagents(Some(&root)).len(), 5);

        let _ = fs::remove_dir_all(&root);
    }
}
