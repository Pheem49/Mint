# Workspace Rules for Mint Agent

## CLI Interface Priority
The primary Mint CLI experience is the new Full-Screen TUI. For CLI changes, implement and verify behavior in the Full-Screen TUI first; do not assume the Classic CLI is the main entry point. Update Classic CLI only when compatibility or explicit user direction calls for it.

## Platform Parity Rule (CLI, Desktop, and Web)
Whenever implementing a new feature, modifying behavior, or adding slash commands/UI options in this codebase, you MUST audit and ensure complete feature parity across ALL THREE interfaces:
1. **CLI (`crates/mint-cli`)**
2. **Desktop UI (`src/renderer/src`)**
3. **Web UI (`src/renderer/src-web`)**

Always verify all three entry points (CLI, Desktop, and Web) before concluding any task so no platform is left out.

## Codebase Architecture & Reuse Rule
Before implementing new features, utilities, or modifying existing systems:
1. **Inspect Existing Modules First**: Always search the codebase first to verify whether similar functionality or helpers already exist across `mint-core`, `mint-cli`, or `src/renderer` before writing new code. Never create redundant parallel systems.
2. **Deep Modules Over Shallow Wrappers**: Invoke and follow the `codebase-design` skill to place clean seams and build deep modules (small, simple interfaces hiding comprehensive behavior) rather than proliferating shallow helpers or scattered duplicate logic.
3. **Architectural Audits**: For repository-wide refactoring, identifying shallow module friction, or proposing systemic structural changes, run the `improve-codebase-architecture` workflow.

