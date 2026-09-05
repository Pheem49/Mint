use crate::{MintConfig, SymbolError, build_symbol_index};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoMapSummary {
    pub total_files: usize,
    pub total_symbols: usize,
    pub rendered_files: usize,
    pub rendered_symbols: usize,
    pub truncated: bool,
    pub map: String,
}

pub fn generate_repo_map(
    root: &Path,
    max_tokens: usize,
    config: &MintConfig,
) -> Result<RepoMapSummary, SymbolError> {
    // 1 token ~= 4 characters as standard heuristic
    let char_budget = max_tokens.saturating_mul(4).max(500);

    // Build symbol index with a generous ceiling
    let index = build_symbol_index(root, 2000, config)?;

    // Group symbols by relative file path
    let mut files_map: BTreeMap<PathBuf, Vec<&crate::symbols::CodeSymbol>> = BTreeMap::new();
    for sym in &index.symbols {
        let rel_path = sym
            .file
            .strip_prefix(root)
            .unwrap_or(&sym.file)
            .to_path_buf();
        files_map.entry(rel_path).or_default().push(sym);
    }

    let mut output = String::new();
    let total_files = files_map.len();
    let total_symbols = index.symbol_count;

    output.push_str(&format!(
        "# Repo Map (AST Outline): {} ({} files, {} symbols)\n\n",
        root.display(),
        total_files,
        total_symbols
    ));

    let mut rendered_files = 0;
    let mut rendered_symbols = 0;
    let mut truncated = false;

    for (file_path, syms) in files_map {
        let mut file_block = format!("{}:\n", file_path.display());
        let mut count_in_file = 0;

        for sym in syms {
            let sig = if sym.signature.is_empty() {
                format!("{} {}", sym.kind, sym.name)
            } else {
                sym.signature.clone()
            };
            file_block.push_str(&format!("  │ line {:<4} {}\n", sym.line, sig));
            count_in_file += 1;
        }
        file_block.push('\n');

        if output.len() + file_block.len() > char_budget {
            truncated = true;
            let remaining_files = total_files.saturating_sub(rendered_files);
            output.push_str(&format!(
                "... [{} more files omitted due to token budget limit ({} tokens)]\n",
                remaining_files, max_tokens
            ));
            break;
        }

        output.push_str(&file_block);
        rendered_files += 1;
        rendered_symbols += count_in_file;
    }

    Ok(RepoMapSummary {
        total_files,
        total_symbols,
        rendered_files,
        rendered_symbols,
        truncated,
        map: output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_map_budget() {
        let cfg = MintConfig::default();
        let cur = Path::new(".");
        let res = generate_repo_map(cur, 500, &cfg);
        assert!(res.is_ok());
        let summary = res.unwrap();
        assert!(!summary.map.is_empty());
    }
}
