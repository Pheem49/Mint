# Workspace Rules for Mint Agent

## Platform Parity Rule (CLI, Desktop, and Web)
Whenever implementing a new feature, modifying behavior, or adding slash commands/UI options in this codebase, you MUST audit and ensure complete feature parity across ALL THREE interfaces:
1. **CLI (`crates/mint-cli`)**
2. **Desktop UI (`src/renderer/src`)**
3. **Web UI (`src/renderer/src-web`)**

Always verify all three entry points (CLI, Desktop, and Web) before concluding any task so no platform is left out.
