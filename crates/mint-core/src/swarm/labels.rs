//! Unique, filesystem/chat-id-safe labels for agents within one batch.
use std::collections::HashSet;

/// Reduces arbitrary model-supplied text to a label that is safe inside
/// `{parent}::subagent::{label}` — ASCII letters/digits/`-`/`_` only, no `::`.
/// May return an empty string; callers fall back to the profile name.
pub fn sanitize_label(raw: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    out.truncate(MAX_LABEL_LEN);
    while out.ends_with('-') {
        out.pop();
    }
    out
}

const MAX_LABEL_LEN: usize = 40;

/// Hands out labels that never repeat within a batch: `explorer`,
/// `explorer-2`, `explorer-3`, ...
#[derive(Debug, Default)]
pub struct LabelAllocator {
    used: HashSet<String>,
}

impl LabelAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&mut self, base: &str) -> String {
        let mut candidate = base.to_string();
        let mut n = 2;
        while self.used.contains(&candidate) {
            candidate = format!("{base}-{n}");
            n += 1;
        }
        self.used.insert(candidate.clone());
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_label_is_plain_and_repeats_get_numeric_suffixes() {
        let mut labels = LabelAllocator::new();
        assert_eq!(labels.allocate("explorer"), "explorer");
        assert_eq!(labels.allocate("explorer"), "explorer-2");
        assert_eq!(labels.allocate("explorer"), "explorer-3");
        assert_eq!(labels.allocate("debugger"), "debugger");
    }

    #[test]
    fn an_explicit_suffixed_label_is_never_handed_out_twice() {
        let mut labels = LabelAllocator::new();
        assert_eq!(labels.allocate("explorer-2"), "explorer-2");
        assert_eq!(labels.allocate("explorer"), "explorer");
        assert_eq!(labels.allocate("explorer"), "explorer-3");
    }

    #[test]
    fn sanitize_keeps_ascii_words_joined_by_dashes() {
        assert_eq!(
            sanitize_label("Stripe Webhook Tracer!"),
            "stripe-webhook-tracer"
        );
        assert_eq!(sanitize_label("  a__b  "), "a__b");
    }

    #[test]
    fn sanitize_removes_chat_id_separators() {
        let label = sanitize_label("evil::subagent::name");
        assert!(!label.contains(':'), "got {label}");
    }

    #[test]
    fn sanitize_returns_empty_when_nothing_usable_remains() {
        assert_eq!(sanitize_label("ค้นหาโค้ด"), "");
        assert_eq!(sanitize_label("  !!  "), "");
    }

    #[test]
    fn sanitize_caps_length() {
        assert!(sanitize_label(&"a".repeat(200)).len() <= 40);
    }
}
