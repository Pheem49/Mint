//! Development checkouts read live notes; standalone builds retain a snapshot.

const SNAPSHOT: &str = include_str!(concat!(env!("OUT_DIR"), "/release-notes.md"));

pub fn text() -> String {
    if env!("MINT_LIVE_RELEASE_NOTES") == "1" {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // Use the checkout this binary was built from, never the user's current
        // workspace (which may have an unrelated Release_Note.md).
        if root.join(".git").exists()
            && let Ok(notes) = std::fs::read_to_string(root.join("Release_Note.md"))
            && !notes.trim().is_empty()
        {
            return notes.trim().to_owned();
        }
    }
    SNAPSHOT.trim().to_owned()
}
