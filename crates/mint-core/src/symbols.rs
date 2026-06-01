use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use serde::Serialize;
use thiserror::Error;

use crate::{CodeInspectionError, MintConfig, list_code_files};

static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        (
            "function",
            r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(",
        ),
        (
            "function",
            r"^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\(",
        ),
        (
            "function",
            r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(",
        ),
        (
            "class",
            r"^\s*(?:export\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)\b",
        ),
        (
            "struct",
            r"^\s*(?:pub\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)\b",
        ),
        ("enum", r"^\s*(?:pub\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)\b"),
        (
            "trait",
            r"^\s*(?:pub\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)\b",
        ),
        (
            "interface",
            r"^\s*(?:export\s+)?interface\s+([A-Za-z_$][A-Za-z0-9_$]*)\b",
        ),
        (
            "type",
            r"^\s*(?:export\s+)?type\s+([A-Za-z_$][A-Za-z0-9_$]*)\b",
        ),
    ]
    .into_iter()
    .map(|(kind, pattern)| (kind, Regex::new(pattern).unwrap()))
    .collect()
});

#[derive(Debug, Error)]
pub enum SymbolError {
    #[error(transparent)]
    Inspect(#[from] CodeInspectionError),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeSymbol {
    pub name: String,
    pub kind: String,
    pub file: PathBuf,
    pub line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolIndex {
    pub root: PathBuf,
    pub file_count: usize,
    pub symbol_count: usize,
    pub kind_counts: BTreeMap<String, usize>,
    pub symbols: Vec<CodeSymbol>,
}

pub fn build_symbol_index(
    root: &Path,
    limit: usize,
    config: &MintConfig,
) -> Result<SymbolIndex, SymbolError> {
    let files = list_code_files(root, usize::MAX, config)?;
    let mut symbols = Vec::new();
    for file in &files {
        if symbols.len() >= limit {
            break;
        }
        let Some(extension) = file.path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !matches!(
            extension,
            "rs" | "js" | "jsx" | "ts" | "tsx" | "py" | "cjs" | "mjs"
        ) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file.path) else {
            continue;
        };
        for (index, line) in content.lines().enumerate() {
            for (kind, pattern) in PATTERNS.iter() {
                let Some(found) = pattern.captures(line).and_then(|captures| captures.get(1))
                else {
                    continue;
                };
                symbols.push(CodeSymbol {
                    name: found.as_str().into(),
                    kind: (*kind).into(),
                    file: file.path.clone(),
                    line: index + 1,
                    signature: line.trim().into(),
                });
                break;
            }
            if symbols.len() >= limit {
                break;
            }
        }
    }
    let mut kind_counts = BTreeMap::new();
    for symbol in &symbols {
        *kind_counts.entry(symbol.kind.clone()).or_insert(0) += 1;
    }
    Ok(SymbolIndex {
        root: root.to_path_buf(),
        file_count: files.len(),
        symbol_count: symbols.len(),
        kind_counts,
        symbols,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_rust_and_typescript_symbols() {
        let root = std::env::temp_dir().join("mint-symbol-index");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("main.rs"), "pub struct Mint;\npub fn run() {}\n").unwrap();
        fs::write(root.join("ui.ts"), "export interface Widget {}\n").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            ..MintConfig::default()
        };
        let index = build_symbol_index(&root, 20, &config).unwrap();
        assert_eq!(index.symbol_count, 3);
        let _ = fs::remove_dir_all(root);
    }
}
