//! Slash-command metadata for the interactive chat — the `/help` listing
//! (`slash_commands.rs`) and the `/` autocomplete dropdown (`input_box.rs`).
//!
//! The catalog now lives in `mint_core::slash::catalog`, deserialized from the
//! shared `slash-commands.json` at the repo root (also imported by the
//! Web/Desktop renderer, `src/renderer/shared/constants/slashCommands.ts`). This
//! module just re-exports it so the CLI call sites are unchanged. See the
//! `dispatcher_tokens_are_documented` test in `slash_commands.rs`.

pub use mint_core::slash::catalog::SLASH_COMMANDS;
