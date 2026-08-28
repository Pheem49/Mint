//! The shared slash-command catalog.
//!
//! Authored once in `slash-commands.json` at the repo root and read by every
//! surface: the Rust CLI (`crates/mint-cli/src/interactive/`), this engine, and
//! the Web/Desktop renderer (`src/renderer/shared/constants/slashCommands.ts`,
//! which imports the same JSON). Each entry's `surfaces` array says which UIs
//! list it — CLI-only commands (`/bg`, `/jobs`, `/shells`, `/exit`, `/plan`, and
//! the `Ctrl+V` / `↑ / ↓` help rows) carry just `["cli"]`.

#[derive(serde::Deserialize)]
pub struct SlashCommandSpec {
    /// Exact literal the autocomplete dropdown prefix-matches against, e.g.
    /// `"/cron add"`.
    pub token: String,
    /// Appended after `token` in `/help` output only — empty if the bare token
    /// needs no further explanation.
    #[serde(default)]
    pub usage: String,
    pub description: String,
    /// Which UIs list this command: any subset of `"cli"`, `"web"`, `"desktop"`.
    #[serde(default)]
    pub surfaces: Vec<String>,
    /// Optional grouping hint for the Web dropdown; unused by Rust but kept on
    /// the struct so the shared JSON round-trips through serde unchanged.
    #[serde(default)]
    #[allow(dead_code)]
    pub category: Option<String>,
}

impl SlashCommandSpec {
    /// True when this command should appear in the interactive CLI.
    pub fn on_cli(&self) -> bool {
        self.surfaces.iter().any(|s| s == "cli")
    }
}

/// The catalog, deserialized once from `slash-commands.json` at the repo root
/// (`<mint-core>/../../slash-commands.json`). Parse failure is a build-time
/// authoring bug, so panic rather than limp along with an empty list.
pub static SLASH_COMMANDS: std::sync::LazyLock<Vec<SlashCommandSpec>> =
    std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../slash-commands.json"
        )))
        .expect("slash-commands.json is valid JSON matching Vec<SlashCommandSpec>")
    });

/// Look up a catalog entry by its exact `token`.
pub fn find(token: &str) -> Option<&'static SlashCommandSpec> {
    SLASH_COMMANDS.iter().find(|s| s.token == token)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared catalog must parse and every entry must declare a valid,
    /// non-empty `surfaces` set — this file and the Web renderer both depend on
    /// it deserializing cleanly.
    #[test]
    fn manifest_surfaces_are_valid() {
        const VALID: &[&str] = &["cli", "web", "desktop"];
        assert!(
            !SLASH_COMMANDS.is_empty(),
            "slash-commands.json deserialized to an empty list",
        );
        for spec in SLASH_COMMANDS.iter() {
            assert!(
                !spec.surfaces.is_empty(),
                "{} has an empty `surfaces` array",
                spec.token,
            );
            for surface in &spec.surfaces {
                assert!(
                    VALID.contains(&surface.as_str()),
                    "{} has unknown surface {surface:?} (expected one of {VALID:?})",
                    spec.token,
                );
            }
        }
        assert!(
            SLASH_COMMANDS
                .iter()
                .any(|s| s.surfaces.iter().any(|x| x == "web")),
            "no command is surfaced to web — the manifest filter would hide the whole dropdown",
        );
    }
}
