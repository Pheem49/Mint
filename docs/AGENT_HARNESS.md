# The Mint Agent Harness: Architecture & Engineering Guide

> **"A foundation model provides the reasoning engine; the Agent Harness provides the sensory actuators, safety guardrails, execution loops, and memory that make autonomy reliable."**

---

## 1. What is Harness Engineering?

In modern AI systems engineering, there is a fundamental distinction between the **Foundation Model** (e.g., Claude 3.7 Sonnet, GPT-4o, Gemini 2.5 Flash, DeepSeek-R1) and the **Agent Harness** (also referred to as the *Agent Scaffolding* or *Agent Runtime*):

- **The Foundation Model (The Engine):** Generates next tokens based on statistical inference and reasoning. By itself, it has no sensory perception, cannot read your disk, cannot run bash scripts, cannot verify whether code compiles, and cannot recover when an execution fails.
- **The Agent Harness (The Vehicle & Control Systems):** The deterministic software environment engineered around the model. It interprets model intentions, executes real-world tools, inspects outputs, enforces security boundaries, manages context windows, and provides self-correction feedback loops.

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
│  │   Orchestration Loop     │  │   Safety & Verification  │  │   Memory & Context Engine     │  │
│  │  • ReAct / OODA Cycle    │  │  • Human-in-the-Loop Gate│  │  • Context Compaction         │  │
│  │  • Self-Correction Loop  │  │  • Verification Gating   │  │  • Two-Tier Memory Recall     │  │
│  │  • Read-Only Concurrency │  │  • Process Sandbox/Docker│  │  • Fact Quarantine & Promote  │  │
│  └──────────────────────────┘  └──────────────────────────┘  └───────────────────────────────┘  │
│  ┌──────────────────────────┐  ┌──────────────────────────┐  ┌───────────────────────────────┐  │
│  │   Tool Actuator System   │  │   Subagent DAG Engine    │  │   Protocol & Plugin Bridge    │  │
│  │  • File I/O & Git Diff   │  │  • Parallel Subagents    │  │  • Model Context Protocol     │  │
│  │  • Terminal / Shell Exec │  │  • Scoped Tool Isolation │  │  • Ecosystem Plugins (7+)     │  │
│  │  • Search (Web/Code/KB)  │  │  • DAG State Tracking    │  │  • Messaging Bridges (7+)     │  │
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

## 3. Core Pillars of the Mint Harness

### Pillar 1: Resilient Orchestration & Self-Correction Feedback Loop

Autonomous agents frequently encounter transient failures: an LLM calls a tool with missing arguments, a bash command fails with a non-zero exit code, or a web search query needs refinement. 

- **Closed Feedback Loop:** Rather than aborting, the Mint Harness captures the exact `ToolEnd` stderr/error output and injects it back into the conversation context as a structured observation.
- **Autonomous Recovery (Retry):** The LLM receives the failure message, diagnoses its own mistake, and self-corrects in the subsequent turn (e.g. providing the missing argument or trying an alternative approach).
- **UX Parity & Observability:** The Harness tags intermediate transient errors with a soft `[Retried]` status in telemetry, reserving `[Failed]` strictly for unrecoverable errors where execution halts.
- **Read-Only Concurrency Batching:** Read-only tools (like `read_file`, `search_code`, `list_files`) are automatically executed concurrently in parallel batches, while mutating tools (`write_file`, `run_shell`) execute sequentially to avoid race conditions.

### Pillar 2: Safety, Sandboxing & Verification Gatekeeping

Unchecked AI agents can accidentally delete data or hallucinate broken code. The Mint Harness enforces deterministic security policies:

- **Human-in-the-Loop Interception (`AgentApproval`):**
  - Risky actions (arbitrary shell execution, modifying project files, calling untrusted MCP tools) trigger structured approval gates.
  - The user can approve once, reject with feedback, or whitelist an entire trusted MCP server.
- **The Verification Gate:**
  - In code-agent mode (`/code`), the agent is prohibited from issuing a `finish` command if files were modified without a subsequent verification step (`run_shell` running unit tests, linters, or diagnostics).
  - If the agent attempts to conclude prematurely, the Harness intercepts and reminds the agent to verify its changes.
- **Execution Sandboxing:**
  - Supports configurable execution backends, including standard host sandbox mode or full Docker container isolation (`sandboxBackend: "docker"`).

### Pillar 3: Context Window Management & Continuous Memory

Long-running agent workflows quickly exceed LLM context window limits without careful memory engineering:

- **Dynamic Token Projection:** In CLI and UI, the Harness monitors token velocity in real-time (`tokens_rate`) and projects token counts to prevent hitting hard API limits unexpectedly.
- **Context Compaction:** When message history grows dense, the Harness compacts and summarizes older conversation rounds while preserving crucial system instructions and recent tool outputs.
- **Two-Tier Memory Architecture:**
  1. *Short-Term Working Memory:* Ephemeral session context and DAG tool states.
  2. *Long-Term Semantic Memory:* Persistent SQLite database with local embeddings (`FastEmbed`) for semantic search across past conversations, project knowledge bases (`docs/`), and learned skills (`.agents/skills/`).
- **Fact Quarantine & Promotion:** Subagents can extract observations into a quarantined fact store; only verified facts are promoted to global memory, preventing cross-session hallucination.

### Pillar 4: Subagent DAG Orchestration & Ecosystem Protocols

Complex software tasks cannot be solved linearly by a single prompt:

- **Directed Acyclic Graph (DAG) Subagents:** The primary agent can dispatch specialized subagents (`dispatch_subagent`) to handle isolated sub-tasks (e.g. researching documentation, exploring codebase layout, running long build scripts).
- **Tool Isolation:** Subagents can be granted restricted subsets of tools to limit their blast radius.
- **Model Context Protocol (MCP):** Native integration with Anthropic's open MCP standard allows Mint to dynamically connect to external tool servers (GitHub, SQLite, Filesystem, Memory) with interactive permission management.
- **Dynamic Skill Synthesis:** When the agent discovers a reusable solution, it can synthesize a skill definition (`.agents/skills/<name>/SKILL.md`) that future runs can recall and refine.

### Pillar 5: Cross-Platform Parity & Unified Telemetry

A key design requirement of Mint is the **Platform Parity Rule**:

- The entire core logic resides in Rust (`crates/mint-core`), ensuring that the exact same behavior, security rules, and agent loops run identically across:
  1. **CLI (`crates/mint-cli`)**: Interactive ANSI terminal with live status bars, token counters, and interactive approval cards.
  2. **Desktop UI (`src/renderer/src`)**: Tauri v2 application with real-time Activity Drawer, Subagent DAG Visualization, and Live2D companion.
  3. **Web UI (`src/renderer/src-web`)**: Browser dashboard sharing the same TypeScript components.
  4. **Messaging Bridges**: Headless gateway enabling continuous interaction via Telegram, Discord, Slack, LINE, WhatsApp, Signal, and Email.

---

## 4. Summary

Harness Engineering transforms raw LLMs into dependable software engineers. By handling the complex mechanics of tool dispatch, process sandboxing, self-correcting loops, memory recall, and user approvals, **Mint Agent** provides a robust, transparent, and battle-tested runtime for local-first autonomous development.
