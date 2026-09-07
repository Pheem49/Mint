use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeHit {
    pub file_path: String,
    pub title: String,
    pub section: String,
    pub snippet: String,
    pub relevance_score: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TieredDocSearchResult {
    pub tier: String, // "project_docs" | "web_fallback"
    pub hits: Vec<KnowledgeHit>,
    pub suggestion: Option<String>,
}

/// Searches local project documentation across docs/, architecture/, decisions/, etc.
pub fn search_project_knowledge(root: &Path, query: &str) -> Vec<KnowledgeHit> {
    let mut hits = Vec::new();
    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower.split_whitespace().collect();
    if query_terms.is_empty() {
        return hits;
    }

    let search_dirs = [
        root.join("docs"),
        root.join("architecture"),
        root.join("decisions"),
        root.join("api"),
        root.join("troubleshooting"),
        root.to_path_buf(),
    ];

    let mut visited_paths = std::collections::HashSet::new();

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        collect_doc_hits(dir, root, &query_terms, &mut hits, &mut visited_paths, 0);
    }

    hits.sort_by(|a, b| b.relevance_score.cmp(&a.relevance_score));
    hits.truncate(10);
    hits
}

fn collect_doc_hits(
    dir: &Path,
    root: &Path,
    query_terms: &[&str],
    hits: &mut Vec<KnowledgeHit>,
    visited: &mut std::collections::HashSet<PathBuf>,
    depth: usize,
) {
    if depth > 4 || hits.len() >= 50 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if visited.contains(&path) {
            continue;
        }
        visited.insert(path.clone());

        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with('.') && name != "node_modules" && name != "target" && name != "build" {
                collect_doc_hits(&path, root, query_terms, hits, visited, depth + 1);
            }
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(ext, "md" | "markdown" | "mdx" | "txt") {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let mut current_header = String::from("Overview");
        let mut section_lines = Vec::new();

        for line in content.lines() {
            if line.starts_with('#') {
                if !section_lines.is_empty() {
                    score_and_add_section(
                        &rel_path,
                        &current_header,
                        &section_lines.join("\n"),
                        query_terms,
                        hits,
                    );
                    section_lines.clear();
                }
                current_header = line.trim_start_matches('#').trim().to_string();
            } else if !line.trim().is_empty() {
                section_lines.push(line);
            }
        }
        if !section_lines.is_empty() {
            score_and_add_section(
                &rel_path,
                &current_header,
                &section_lines.join("\n"),
                query_terms,
                hits,
            );
        }
    }
}

fn score_and_add_section(
    file_path: &str,
    header: &str,
    body: &str,
    query_terms: &[&str],
    hits: &mut Vec<KnowledgeHit>,
) {
    let header_lower = header.to_lowercase();
    let body_lower = body.to_lowercase();
    let path_lower = file_path.to_lowercase();

    let mut score = 0;
    for term in query_terms {
        if header_lower.contains(term) {
            score += 15;
        }
        if path_lower.contains(term) {
            score += 10;
        }
        score += body_lower.matches(term).count() * 3;
    }

    if score > 0 {
        let snippet = if body.len() > 500 {
            let end = body.floor_char_boundary(500);
            format!("{}...", &body[..end])
        } else {
            body.to_string()
        };

        hits.push(KnowledgeHit {
            file_path: file_path.to_string(),
            title: header.to_string(),
            section: header.to_string(),
            snippet,
            relevance_score: score,
        });
    }
}

/// Creates a standardized Knowledge Base document in the workspace docs directory
pub fn create_project_doc(
    root: &Path,
    category: &str,
    title: &str,
    content: &str,
) -> Result<PathBuf, String> {
    let sanitized_title: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = sanitized_title.trim_matches('-');

    let (dir_name, filename) = match category.to_ascii_lowercase().as_str() {
        "decisions" | "decision" | "adr" => {
            let timestamp = chrono::Local::now().format("%Y%m%d");
            (
                root.join("docs").join("decisions"),
                format!("{timestamp}-{slug}.md"),
            )
        }
        "architecture" | "arch" => (root.join("docs").join("architecture"), format!("{slug}.md")),
        "api" => (root.join("docs").join("api"), format!("{slug}.md")),
        "troubleshooting" | "bugs" | "runbook" => {
            (root.join("docs").join("troubleshooting"), format!("{slug}.md"))
        }
        _ => (root.join("docs"), format!("{slug}.md")),
    };

    fs::create_dir_all(&dir_name)
        .map_err(|e| format!("Failed to create documentation directory: {e}"))?;

    let file_path = dir_name.join(&filename);

    let full_doc = match category.to_ascii_lowercase().as_str() {
        "decisions" | "decision" | "adr" => format!(
            "# ADR: {}\n\n- **Date**: {}\n- **Status**: Proposed / Accepted\n\n## Context\n{}\n\n## Decision\n{}\n\n## Consequences\nPositive and negative tradeoffs of this decision.\n",
            title,
            chrono::Local::now().format("%Y-%m-%d"),
            content,
            content
        ),
        "troubleshooting" => format!(
            "# Troubleshooting: {}\n\n## Symptoms / Problem Description\n{}\n\n## Root Cause\nAnalysis of why this issue occurs.\n\n## Resolution / Fix Steps\nStep-by-step fix guide.\n",
            title,
            content
        ),
        _ => format!("# {}\n\n{}\n", title, content),
    };

    fs::write(&file_path, full_doc)
        .map_err(|e| format!("Failed to write documentation file {}: {e}", file_path.display()))?;

    Ok(file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_search_project_doc() {
        let temp = std::env::temp_dir().join(format!("mint-knowledge-test-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let doc_path = create_project_doc(
            &temp,
            "decisions",
            "Use PostgreSQL For Persistent Storage",
            "We decided to use PostgreSQL for relational transactions and ACID compliance.",
        )
        .unwrap();

        assert!(doc_path.exists());
        let content = fs::read_to_string(&doc_path).unwrap();
        assert!(content.contains("PostgreSQL"));

        let hits = search_project_knowledge(&temp, "PostgreSQL relational");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].title, "ADR: Use PostgreSQL For Persistent Storage");

        let _ = fs::remove_dir_all(&temp);
    }
}
