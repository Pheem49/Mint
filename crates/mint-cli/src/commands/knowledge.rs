use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use mint_core::{
    Capability, ChatRequest, CodeEdit, CodePatchHunk, KnowledgeStore, MemoryStore, MintConfig,
    apply_code_edits, assert_path_capability, build_code_patch, build_symbol_index,
    fetch_github_repo_summary, index_semantic_code, inspect_code_plan, list_code_files,
    list_subagents, load_config, orchestrate_chat_with_fallback, parse_github_url,
    propose_code_edits, read_code_file, repository_summary, search_code, search_semantic_code,
};

use crate::{ERROR, MINT, RESET, interactive, run_oneshot_agent_task, skills};

#[derive(Debug, Subcommand)]
pub enum SemanticCodeCommand {
    Index {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    Search {
        query: String,
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
pub enum KnowledgeCommand {
    Add {
        path: PathBuf,
    },
    List,
    Search {
        query: String,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodeCommand {
    /// Run the autonomous inspect, act, and verify code-agent loop.
    Agent {
        task: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Investigate read-only and require plan approval before editing files or running commands.
        #[arg(long)]
        plan: bool,
    },
    /// Summarize source files while skipping build and dependency directories.
    Summary {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// List source files while skipping build and dependency directories.
    List {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// List subagent definitions available to `dispatch_subagent`.
    Subagents {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Read a numbered source range.
    Read {
        path: PathBuf,
        #[arg(long, default_value_t = 1)]
        start: usize,
        #[arg(long, default_value_t = 200)]
        end: usize,
    },
    /// Generate an AST-based repo map with token budgeting.
    Repomap {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 2000)]
        limit: usize,
    },
    /// Search source text without invoking a shell command.
    Search {
        query: String,
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Print a bounded inspection-first plan. This never edits files or runs shell commands.
    Plan {
        task: String,
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        file: Vec<PathBuf>,
    },
    /// Preview a full file write and print its content-bound approval token.
    ProposeWrite {
        path: PathBuf,
        #[arg(long, conflicts_with = "from_file")]
        content: Option<String>,
        #[arg(long)]
        from_file: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Apply exactly the full file write that was previously approved.
    ApplyWrite {
        path: PathBuf,
        #[arg(long, conflicts_with = "from_file")]
        content: Option<String>,
        #[arg(long)]
        from_file: Option<PathBuf>,
        #[arg(long)]
        approval_token: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Preview an exact text replacement and print its content-bound approval token.
    ProposePatch {
        path: PathBuf,
        old_text: String,
        new_text: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Apply exactly the text replacement that was previously approved.
    ApplyPatch {
        path: PathBuf,
        old_text: String,
        new_text: String,
        #[arg(long)]
        approval_token: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Preview multiple full file writes. Use TARGET=SOURCE for each edit.
    ProposeEdits {
        #[arg(long, required = true)]
        edit: Vec<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Apply exactly the multi-file write proposal that was previously approved.
    ApplyEdits {
        #[arg(long, required = true)]
        edit: Vec<String>,
        #[arg(long)]
        approval_token: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Fetch GitHub repository metadata and README, then get an overview of the repo.
    GithubOverview {
        /// The GitHub repository URL or name (e.g. "https://github.com/owner/repo" or "owner/repo").
        repo: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    /// Install a skill: a local file/folder path, or a GitHub repo/URL.
    Add {
        source: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra: Vec<String>,
    },
    /// List all skills Mint can currently see (global, workspace, taught).
    List,
}

fn edit_content(
    content: Option<String>,
    from_file: Option<PathBuf>,
    config: &MintConfig,
) -> Result<String> {
    match from_file {
        Some(path) => {
            let path = assert_path_capability(&path, Capability::Read, config)?;
            Ok(fs::read_to_string(path)?)
        }
        None => Ok(content.unwrap_or_default()),
    }
}

fn file_edits(values: &[String], config: &MintConfig) -> Result<Vec<CodeEdit>> {
    values
        .iter()
        .map(|value| {
            let (target, source) = value
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("edit must use TARGET=SOURCE format"))?;
            Ok(CodeEdit {
                path: PathBuf::from(target),
                content: edit_content(None, Some(PathBuf::from(source)), config)?,
            })
        })
        .collect()
}

async fn run_github_overview(repo: &str, config: &MintConfig) -> Result<()> {
    let Some((owner, repo_name)) = parse_github_url(repo) else {
        anyhow::bail!(
            "Invalid GitHub repository URL/format. Please use 'owner/repo' or a full GitHub URL."
        );
    };

    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Duration;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!(
        "Fetching information for {owner}/{repo_name} from GitHub..."
    ));
    spinner.enable_steady_tick(Duration::from_millis(80));

    let summary = match fetch_github_repo_summary(&owner, &repo_name).await {
        Ok(s) => s,
        Err(e) => {
            spinner.finish_and_clear();
            anyhow::bail!(
                "Failed to fetch repository summary: {e}. Check that the repository is public and spelled correctly."
            );
        }
    };

    spinner.set_message("Analyzing repository with AI model...");
    let prompt = format!(
        "Here is the metadata, top-level directory structure, and README.md content for the GitHub repository {owner}/{repo_name}:\n\n{summary}\n\nBased on this information, please provide a high-level overview of what this repository is about, what tech stack it uses, its overall architecture, and how it is organized."
    );

    let (response, _) = match orchestrate_chat_with_fallback(
        config,
        &ChatRequest {
            message: prompt,
            system_instruction: "You are a professional software architect providing a high-level overview of a code repository based on its metadata and README.".to_string(),
            chat_id: Some("github_review".to_string()),
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            pinned_mcp_server: None,
            messages: None,
            tools: None,
        },
    )
    .await {
        Ok(res) => {
            spinner.finish_and_clear();
            res
        }
        Err(e) => {
            spinner.finish_and_clear();
            return Err(e.into());
        }
    };

    println!(
        "\n--- AI Repository Overview for {owner}/{repo_name} ---"
    );
    println!("{}", response.text);
    println!("--------------------------------------------------");
    Ok(())
}

pub fn handle_learn(path: Option<PathBuf>, list: bool, delete: Option<String>) -> Result<()> {
    let memory = MemoryStore::open_default()?;
    if list {
        let mut skills = memory.learned_skills(100)?;

        if let Some(home) = dirs::home_dir() {
            let global_skills_path = home.join(".config").join("mint").join("mint-skills");
            mint_core::skills::load_skills_from_dir(&global_skills_path, &mut skills);
        }
        if let Ok(root) = std::env::current_dir()
            && let Ok(root) = root.canonicalize()
        {
            let workspace_skills_path1 = root.join(".agents").join("skills");
            mint_core::skills::load_skills_from_dir(&workspace_skills_path1, &mut skills);

            let workspace_skills_path2 = root.join("skills");
            mint_core::skills::load_skills_from_dir(&workspace_skills_path2, &mut skills);
        }

        let mut unique_skills = std::collections::BTreeMap::new();
        for skill in skills {
            let loc = if skill.source_path.contains("/.config/mint/mint-skills") {
                "Global"
            } else if skill.source_path.contains("/skills")
                || skill.source_path.contains("/.agents/skills")
            {
                "Workspace"
            } else {
                "Taught"
            };
            unique_skills.insert(skill.name.clone(), (skill, loc));
        }

        if unique_skills.is_empty() {
            println!("No learned skills found.");
        } else {
            println!("Learned AI Skills:");
            for (name, (skill, loc)) in &unique_skills {
                if *loc == "Taught" {
                    println!("  ● [{loc}] {name}");
                } else {
                    println!("  ● [{loc}] {name} (Source: {})", skill.source_path);
                }
            }
        }
    } else if let Some(identifier) = delete {
        println!("{}", memory.delete_learned_skill(&identifier)?);
    } else if let Some(path) = path {
        println!("{}", serde_json::to_string_pretty(&skills::learn(&path)?)?);
    } else {
        anyhow::bail!("use mint learn <path>, --list, or --delete <id|path|name>");
    }
    Ok(())
}

pub fn handle_symbols(root: PathBuf, limit: usize) -> Result<()> {
    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Duration;
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message("Building symbol index...");
    spinner.enable_steady_tick(Duration::from_millis(80));
    let index = build_symbol_index(&root, limit, &load_config()?);
    spinner.finish_and_clear();
    println!("{}", serde_json::to_string_pretty(&index?)?);
    Ok(())
}

pub async fn handle_semantic_code(command: SemanticCodeCommand) -> Result<()> {
    match command {
        SemanticCodeCommand::Index { root } => {
            use indicatif::{ProgressBar, ProgressStyle};
            use std::time::Duration;
            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            spinner.set_message("Indexing semantic code workspace...");
            spinner.enable_steady_tick(Duration::from_millis(80));
            let res = index_semantic_code(&root, &load_config()?).await;
            spinner.finish_and_clear();
            println!("{}", serde_json::to_string_pretty(&res?)?);
        }
        SemanticCodeCommand::Search { query, root, limit } => println!(
            "{}",
            serde_json::to_string_pretty(
                &search_semantic_code(&root, &query, limit, &load_config()?).await?
            )?
        ),
    }
    Ok(())
}

pub fn handle_knowledge(command: KnowledgeCommand) -> Result<()> {
    let store = KnowledgeStore::open_default()?;
    match command {
        KnowledgeCommand::Add { path } => {
            println!("{}", store.index_file(&path, &load_config()?)?)
        }
        KnowledgeCommand::List => {
            println!("{}", serde_json::to_string_pretty(&store.list_sources()?)?)
        }
        KnowledgeCommand::Search { query, limit } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&store.search(&query, limit)?)?
            )
        }
    }
    Ok(())
}

pub async fn handle_code(command: CodeCommand, cli: &crate::Cli, config: &MintConfig) -> Result<()> {
    match command {
        CodeCommand::Agent { task, root, plan } => {
            run_oneshot_agent_task(
                &task,
                &root,
                config,
                cli.fast,
                plan || cli.plan,
                cli.image.as_deref(),
            )
            .await?;
        }
        CodeCommand::Summary { root } => println!(
            "{}",
            serde_json::to_string_pretty(&repository_summary(&root, config)?)?
        ),
        CodeCommand::List { root, limit } => println!(
            "{}",
            serde_json::to_string_pretty(&list_code_files(&root, limit, config)?)?
        ),
        CodeCommand::Subagents { root } => {
            let subagents = list_subagents(Some(&root));
            if subagents.is_empty() {
                println!(
                    "No subagent definitions found under {}/.agents/subagents/ or ~/.config/mint/mint-agents/.",
                    root.display()
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&subagents)?);
            }
        }
        CodeCommand::Read { path, start, end } => {
            println!("{}", read_code_file(&path, start, end, config)?)
        }
        CodeCommand::Search { query, root, limit } => println!(
            "{}",
            serde_json::to_string_pretty(&search_code(&root, &query, limit, config)?)?
        ),
        CodeCommand::Repomap { root, limit } => {
            let summary = mint_core::generate_repo_map(&root, limit, config)?;
            println!("{}", summary.map);
        }
        CodeCommand::Plan { task, root, file } => println!(
            "{}",
            serde_json::to_string_pretty(&inspect_code_plan(
                task, &root, file, config
            )?)?
        ),
        CodeCommand::ProposeWrite {
            path,
            content,
            from_file,
            root,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&propose_code_edits(
                &root,
                &[CodeEdit {
                    path,
                    content: edit_content(content, from_file, config)?,
                }],
                config,
            )?)?
        ),
        CodeCommand::ApplyWrite {
            path,
            content,
            from_file,
            approval_token,
            root,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&apply_code_edits(
                &root,
                &[CodeEdit {
                    path,
                    content: edit_content(content, from_file, config)?,
                }],
                &approval_token,
                config,
            )?)?
        ),
        CodeCommand::ProposePatch {
            path,
            old_text,
            new_text,
            root,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&propose_code_edits(
                &root,
                &[build_code_patch(
                    &root,
                    path,
                    &[CodePatchHunk {
                        old_text,
                        new_text,
                        replace_all: false,
                    }],
                    config,
                )?],
                config,
            )?)?
        ),
        CodeCommand::ApplyPatch {
            path,
            old_text,
            new_text,
            approval_token,
            root,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&apply_code_edits(
                &root,
                &[build_code_patch(
                    &root,
                    path,
                    &[CodePatchHunk {
                        old_text,
                        new_text,
                        replace_all: false,
                    }],
                    config,
                )?],
                &approval_token,
                config,
            )?)?
        ),
        CodeCommand::ProposeEdits { edit, root } => println!(
            "{}",
            serde_json::to_string_pretty(&propose_code_edits(
                &root,
                &file_edits(&edit, config)?,
                config,
            )?)?
        ),
        CodeCommand::ApplyEdits {
            edit,
            approval_token,
            root,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&apply_code_edits(
                &root,
                &file_edits(&edit, config)?,
                &approval_token,
                config,
            )?)?
        ),
        CodeCommand::GithubOverview { repo } => {
            run_github_overview(&repo, config).await?;
        }
    }
    Ok(())
}

pub fn handle_skills(command: SkillsCommand) -> Result<()> {
    match command {
        SkillsCommand::Add { source, extra } => {
            let cwd = std::env::current_dir()?;
            let extra_args: Vec<&str> = extra.iter().map(String::as_str).collect();
            match skills::add(&source, &extra_args, &cwd) {
                Ok(msg) => println!("{MINT}✓{RESET} {msg}"),
                Err(msg) => {
                    eprintln!("{ERROR}✗ {msg}{RESET}");
                    anyhow::bail!("{msg}");
                }
            }
        }
        SkillsCommand::List => {
            let cwd = std::env::current_dir()?;
            let skills = interactive::load_all_available_skills(&cwd);
            if skills.is_empty() {
                println!("No skills found. Use `mint skills add <source>` to add one.");
            } else {
                println!("{MINT}Skills:{RESET}");
                for skill in &skills {
                    let loc = if skill.source_path.contains("/.config/mint/mint-skills") {
                        "Global"
                    } else if skill.source_path.contains("/skills")
                        || skill.source_path.contains("/.agents/skills")
                    {
                        "Workspace"
                    } else {
                        "Taught"
                    };
                    println!("  [{loc}] {} ({})", skill.name, skill.source_path);
                }
            }
        }
    }
    Ok(())
}
