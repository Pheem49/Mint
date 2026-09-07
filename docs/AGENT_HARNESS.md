# The Mint Agent Harness: Architecture & Engineering Guide

> **"A foundation model provides the reasoning engine; the Agent Harness provides the sensory actuators, safety guardrails, execution loops, and memory that make autonomy reliable."**

---

## 1. What is Harness Engineering?

In modern AI systems engineering, there is a fundamental distinction between the **Foundation Model** (e.g., Claude 3.7 Sonnet, GPT-4o, Gemini 2.5 Flash, DeepSeek-R1) and the **Agent Harness** (also referred to as the *Agent Scaffolding* or *Agent Runtime*):

- **The Foundation Model (The Engine):** Generates next tokens based on statistical inference and reasoning. By itself, it has no sensory perception, cannot inspect repository file trees, cannot run compilers or test suites, cannot verify whether code compiles, and cannot self-recover when an execution fails.
- **The Agent Harness (The Vehicle & Control Systems):** The deterministic software environment engineered around the model. It interprets model intentions, executes real-world tools, inspects outputs, enforces security boundaries, manages context windows, isolates tasks in git checkpoints, and provides self-correction feedback loops.

In real-world benchmarks (such as SWE-bench) and production environments, **over 70–80% of an agent's success is dictated by the quality of its Harness Engineering**, rather than prompt adjustments alone.

`Mint` is built in **Rust** as a production-grade, local-first **Agent Execution Harness** (`crates/mint-core`) designed to turn any standard LLM into an autonomous, safe, and resilient software engineering assistant.

---

## 2. High-Level Architecture

The Mint Harness sits between Foundation Models and real-world execution environments:

```text
                     ┌────────────────────────────────────────────────────────┐
                     │                   Foundation Models                    │
                     │          (Claude, GPT-4o, Gemini, Ollama, DeepSeek)    │
                     └───────────────────────────▲────────────────────────────┘
                                                 │ (Reasoning / Tool Invocations)
┌────────────────────────────────────────────────▼────────────────────────────────────────────────┐
│                                   MINT AGENT HARNESS (mint-core)                                │
│                                                                                                 │
│  ┌──────────────────────────┐  ┌──────────────────────────┐  ┌───────────────────────────────┐  │
│  │   Orchestration Loop     │  │   Safety & Git Checkpoint│  │   Memory & Knowledge Engine   │  │
│  │  • ReAct / OODA Cycle    │  │  • Git Snapshots/Rollback│  │  • Two-Tier Memory Recall     │  │
│  │  • Plan State Machine    │  │  • Task-Isolated Branches│  │  • Tiered Repo Documentation  │  │
│  │  • Self-Correction Loop  │  │  • Zero-Prompt Safe Tools│  │  • Dynamic ADR & Playbook Gen │  │
│  │  • Balanced Log Truncate │  │  • Human-in-the-Loop Gate│  │  • Fact Quarantine & Promote  │  │
│  └──────────────────────────┘  └──────────────────────────┘  └───────────────────────────────┘  │
│  ┌──────────────────────────┐  ┌──────────────────────────┐  ┌───────────────────────────────┐  │
│  │   Code Intelligence      │  │   Subagent DAG Engine    │  │   Observability & Evaluation  │  │
│  │  • AST Symbol Extraction │  │  • Parallel Subagents    │  │  • Run Telemetry & Profiling  │  │
│  │  • find_definition       │  │  • Scoped Tool Isolation │  │  • Per-Tool Latency Metrics   │  │
│  │  • find_references       │  │  • DAG State Tracking    │  │  • SWE Benchmark Suite (eval) │  │
│  │  • Monorepo Architecture │  │  • Model Context Protocol│  │  • Real-Time Themed Dashboards│  │
│  └──────────────────────────┘  └──────────────────────────┘  └───────────────────────────────┘  │
└────────────────────────────────────────────────┬────────────────────────────────────────────────┘
                                                 │ Unified Telemetry & State Stream
                     ┌───────────────────────────┼────────────────────────────┐
                     │                           │                            │
        ┌────────────▼────────────┐ ┌────────────▼────────────┐ ┌─────────────▼────────────┐
        │        Mint CLI         │ │       Desktop App       │ │     Web UI & Messaging    │
        │   (Terminal TUI / ANSI) │ │      (Tauri v2 + React) │ │     (Vite + Bot Bridges)  │
        └─────────────────────────┘ └─────────────────────────┘ └───────────────────────────┘
```

---

## 3. The 10 Pillars of the Mint Harness

### Pillar 1: Deep Project & Monorepo Architecture Detection

Autonomous code agents fail when they blindly attempt to run commands without understanding the workspace topology.

- **Automated Ecosystem & Monorepo Scanner (`mint_core::system::project_detector`)**:
  - Automatically identifies monorepo structures: Cargo workspaces, pnpm workspaces, Turborepo, Lerna, and Nx.
  - Detects primary project ecosystems: Rust, Node/TypeScript, Python, Go, and Java.
  - Extracts workspace roots, sub-crates/packages, build scripts, package managers, and test runner configurations.
- **Architectural System Prompt Briefing**:
  - Injects a compact architectural summary directly into the Agent's system prompt before Turn 1.
  - The agent immediately knows which package manager to use, which subdirectories house crates/modules, and how tests are invoked without wasting turns guessing.
- **Project Rule Auto-Ingestion**:
  - Automatically crawls and injects workspace governance files: `CLAUDE.md`, `.cursorrules`, `.github/copilot-instructions.md`, `.agents/rules/`, and `.agents/AGENTS.md`.

### Pillar 2: Code Intelligence & AST Symbol Navigation

Grep-based text searches hallucinate when identifiers appear in comments, imports, tests, or across similarly named functions.

- **AST & Regex Symbol Extraction Engine (`mint_core::search::symbols`)**:
  - Zero-config symbol table extraction without requiring external Language Server Protocol (LSP) daemons.
  - Supports Rust, TypeScript, JavaScript, Python, and Go.
- **Dedicated Navigation Actuators**:
  - **`find_definition`**: Locates definitions of functions, structs, classes, enums, interfaces, types, and traits with precise line numbers and symbol signatures.
  - **`find_references`**: Locates call-sites, usages, and implementations across workspace files.

### Pillar 3: Safe Automated Tooling & Zero-Prompt Pre-Approval Policy

Security guardrails must balance safety with autonomy. Asking the user to confirm read-only or harmless validation commands stalls the agent workflow.

- **Specialized Validation Tools (`mint_core::orchestration::tools::safe_tools`)**:
  - **`run_tests`**: Auto-detects project test framework (`cargo test`, `npm test`, `pytest`, `go test`) or executes targeted test paths.
  - **`run_typecheck`**: Invokes project compiler checks (`tsc`, `cargo check`, `pyright`, `mypy`) to catch typing errors early.
  - **`run_linter`**: Runs code linters (`clippy`, `eslint`, `flake8`/`ruff`, `golangci-lint`) to enforce styling and correctness.
- **Zero-Prompt Pre-Approval Policy**:
  - Non-destructive validation operations run autonomously without user confirmation prompts, while file mutations and shell executions remain guarded by human-in-the-loop policies.

### Pillar 4: Task Planning & Checklist State Machine

Complex multi-step engineering tasks require deterministic state tracking to prevent goal drift or skipping requirements.

- **Plan State Machine (`mint_core::orchestration::tools::planning`)**:
  - Introduces `plan_task` and `update_plan_step` tools.
  - Tracks individual step states: `Pending`, `InProgress`, `Completed`, `Failed`, and `Skipped`.
- **Live Terminal & UI Checklist Cards**:
  - **CLI**: Live ANSI progress cards with status glyphs (`[ ]`, `[-]`, `[x]`, `[!]`, `[~]`) in `mint-cli`.
  - **Desktop & Web UI**: Dedicated `PlanChecklistWidget` rendering step progress, state transitions, and step timing.

### Pillar 5: Active Verification Loop & Balanced Log Engineering

An agent that modifies code without running tests is unreliable. Large compiler logs also risk blowing up LLM context windows.

- **Active Verification Gate**:
  - In code-agent mode (`/code`), the agent is prohibited from declaring task completion (`finish`) if code files were modified without a subsequent verification step (`run_tests`, `run_typecheck`, or `run_shell`).
- **Balanced Log Truncation Engine (`Head + Tail Windowing`)**:
  - Traditional truncation chops off the end of output, which is precisely where compilers and test runners print failure stack traces.
  - Mint implements a 3 KB Head + 12 KB Tail balanced buffer window:
    ```text
    ┌────────────────────────────────────────────────────────┐
    │ Head Window (3 KB): Invocation, flags & build headers   │
    ├────────────────────────────────────────────────────────┤
    │ [... 45,210 bytes truncated by Mint Harness ...]       │
    ├────────────────────────────────────────────────────────┤
    │ Tail Window (12 KB): Stack traces, compile errors & loc │
    └────────────────────────────────────────────────────────┘
    ```
  - Preserves diagnostic integrity while protecting the LLM context window.

### Pillar 6: Git Safety Harness & Rollback Checkpoint System

A production agent must never destroy uncommitted user work or leave dirty worktrees upon failure.

- **Working Tree Snapshots (`mint_core::git::checkpoint`)**:
  - **`git_checkpoint`**: Automatically captures stash and commit refs prior to performing risky edits.
  - **`git_rollback`**: Instantly rolls back uncommitted changes or restores the pre-task git state if tests fail or the agent goes astray.
  - **`git_restore_file`**: Precision rollbacks of specific modified files (`git checkout -- <file>`).
  - **`git_create_branch`**: Automatically creates an isolated task branch (`mint/<task-id>-<slug>`).
  - **`git_commit`**: Generates context-aware commit messages based on staged diff analysis and commits verified changes.

### Pillar 7: Context Window Management, Compaction & Continuous Memory

Long-running agent workflows quickly exceed LLM context window limits without continuous memory engineering:

- **Dynamic Token Velocity & Projection**: Tracks token consumption rate (`tokens_rate`) in real-time to avoid unexpected hard context window crashes.
- **Context Compaction**: Summarizes older conversation rounds when context approaches capacity while pinning architectural instructions and recent observations.
- **Two-Tier Memory Architecture**:
  1. *Short-Term Working Memory:* Ephemeral session context and DAG tool states.
  2. *Long-Term Semantic Memory:* Persistent SQLite database with local embeddings (`FastEmbed`) for semantic search across past conversations and learned skills.
- **Fact Quarantine & Promotion**: Subagents write observations into a quarantined fact store; only verified facts are promoted to global memory.

### Pillar 8: Autonomous Knowledge Engine & Documentation System

Agents must leverage existing project documentation and persist architectural learnings back to the codebase.

- **Tiered Documentation Search (`mint_core::system::knowledge_engine`)**:
  - Automatically indexes repository knowledge directories: `docs/`, `architecture/`, `decisions/` (ADRs), `api/`, and `troubleshooting/`.
  - Prioritizes project-specific documentation before falling back to global skills or web searches.
  - **`search_docs` Tool**: High-speed keyword and semantic search across local project documentation.
- **Autonomous Documentation Authoring (`create_project_doc`)**:
  - Allows the agent to author, persist, and update Architecture Decision Records (ADRs), API guides, and troubleshooting runbooks directly in the workspace.

### Pillar 9: Full-Stack Run Observability & Telemetry

Enterprise engineering requires full visibility into agent execution duration, token cost, and tool latency.

- **Telemetry Collection Engine (`mint_core::orchestration`)**:
  - `RunTelemetrySummary`: Captures start time, end time, duration, token usage, step counts, and test outcomes.
  - `ToolExecutionRecord`: Records per-tool execution latency, invocation parameters, and success/retry status.
  - Emits `AgentProgress::RunCompleted` event upon task finalization.
- **Cross-Platform Telemetry Dashboards**:
  - **CLI**: Rich ANSI terminal summary card rendered upon agent turn completion.
  - **Desktop & Web UI**: Themed `RunSummaryDashboard` component featuring execution statistics, status indicators, and an expandable tool-call latency drawer.
  - Styled with semantic CSS tokens (`src/renderer/shared/css/agent-observability.css`) supporting Dark, Light, and Midnight themes.

### Pillar 10: Benchmark Evaluation Suite (`mint eval`)

To improve harnesses and models, performance must be quantifiable against reproducible evaluation benchmarks.

- **SWE-Style Evaluation Engine (`mint_core::eval`)**:
  - Evaluates models and harnesses against benchmark suites defined in JSON (`benchmarks/mint_eval.json`).
  - Evaluates prompt execution, file modifications, and unit test pass/fail criteria.
- **CLI Subcommand**:
  ```bash
  mint eval --suite benchmarks/mint_eval.json --concurrency 2 --output results.json
  ```
- **Automated Scorecards**: Produces structured JSON reports and terminal scorecards summarizing pass rates, duration, and token consumption.

---

## 4. End-to-End Lifecycle of an Agent Task

The complete flow from initial prompt to verified completion:

```text
User Request ("Fix auth bug")
  │
  ▼
[Pillar 1] Project & Monorepo Detection (Rust/Cargo, test runner: cargo test)
  │
  ▼
[Pillar 6] Git Safety Checkpoint (Snapshot working tree, create task branch)
  │
  ▼
[Pillar 4] Plan State Machine (Generate step checklist via plan_task)
  │
  ▼
[Pillar 2] Code Intelligence (find_definition, find_references on auth token)
  │
  ▼
[Pillar 8] Knowledge Search (search_docs for auth architecture ADRs)
  │
  ▼
File Modifications (Edit code files)
  │
  ▼
[Pillar 3 & 5] Safe Automated Verification (run_tests, balanced log truncate)
  ├── Tests Fail ──► [Pillar 1 & 6] Self-Correction or Git Rollback (Retry)
  │
  └── Tests Pass
        │
        ▼
[Pillar 6] Git Commit (Auto-generate commit message from verified diff)
        │
        ▼
[Pillar 9] Run Observability Dashboard (Display telemetry, tool latency & tokens)
```

---

## 5. The Platform Parity Rule

A fundamental architectural rule of Mint is **100% Platform Parity**:
All 10 pillars operate identically across:
1. **CLI (`crates/mint-cli`)**
2. **Desktop UI (`src/renderer/src`)**
3. **Web UI (`src/renderer/src-web`)**
4. **Messaging Bridges (Telegram, Discord, Slack, LINE, WhatsApp, Signal, Email)**

By keeping the core harness logic centralized in `mint-core`, no feature, safety policy, or tool capability is ever exclusive to a single platform.

---

## 6. Summary

Harness Engineering transforms raw LLMs into dependable software engineers. By combining deep workspace awareness, precise AST symbol intelligence, automated verification gates, Git safety checkpoints, autonomous documentation, and full observability, **Mint Agent** provides a robust, transparent, and battle-tested runtime for local-first autonomous development.
