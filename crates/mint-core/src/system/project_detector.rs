use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectArchitectureSummary {
    pub root: PathBuf,
    pub languages: Vec<String>,
    pub package_managers: Vec<String>,
    pub frameworks: Vec<String>,
    pub test_runners: Vec<String>,
    pub diagnostics: Vec<String>,
    pub workspace_packages: Vec<String>,
    pub test_files_count: usize,
    pub summary_sentence: String,
}

pub fn detect_project_deep(root: &Path) -> ProjectArchitectureSummary {
    let mut languages = BTreeSet::new();
    let mut package_managers = BTreeSet::new();
    let mut frameworks = BTreeSet::new();
    let mut test_runners = BTreeSet::new();
    let mut diagnostics = BTreeSet::new();
    let mut workspace_packages = Vec::new();

    // 1. Node / JavaScript / TypeScript ecosystem
    let pkg_json_path = root.join("package.json");
    if pkg_json_path.exists() {
        languages.insert("JavaScript/TypeScript".to_string());
        if root.join("pnpm-lock.yaml").exists() || root.join("pnpm-workspace.yaml").exists() {
            package_managers.insert("pnpm".to_string());
        } else if root.join("yarn.lock").exists() {
            package_managers.insert("yarn".to_string());
        } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
            package_managers.insert("bun".to_string());
        } else {
            package_managers.insert("npm".to_string());
        }

        if let Ok(content) = fs::read_to_string(&pkg_json_path) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                let mut all_deps = HashSet::new();
                if let Some(deps) = json.get("dependencies").and_then(Value::as_object) {
                    all_deps.extend(deps.keys().cloned());
                }
                if let Some(dev_deps) = json.get("devDependencies").and_then(Value::as_object) {
                    all_deps.extend(dev_deps.keys().cloned());
                }

                // Check frameworks & libraries
                if all_deps.contains("react") {
                    frameworks.insert("React".to_string());
                }
                if all_deps.contains("vue") {
                    frameworks.insert("Vue".to_string());
                }
                if all_deps.contains("svelte") {
                    frameworks.insert("Svelte".to_string());
                }
                if all_deps.contains("next") {
                    frameworks.insert("Next.js".to_string());
                }
                if all_deps.contains("nuxt") {
                    frameworks.insert("Nuxt".to_string());
                }
                if all_deps.contains("vite") {
                    frameworks.insert("Vite".to_string());
                }
                if all_deps.contains("zustand") {
                    frameworks.insert("Zustand".to_string());
                }
                if all_deps.contains("redux") || all_deps.contains("@reduxjs/toolkit") {
                    frameworks.insert("Redux".to_string());
                }
                if all_deps.contains("tailwindcss") {
                    frameworks.insert("TailwindCSS".to_string());
                }
                if all_deps.contains("express") {
                    frameworks.insert("Express".to_string());
                }
                if all_deps.contains("fastify") {
                    frameworks.insert("Fastify".to_string());
                }
                if all_deps.contains("electron") {
                    frameworks.insert("Electron".to_string());
                }
                if all_deps.contains("@tauri-apps/api") {
                    frameworks.insert("Tauri".to_string());
                }

                // Check test runners & linters
                if all_deps.contains("vitest") {
                    test_runners.insert("Vitest".to_string());
                }
                if all_deps.contains("jest") {
                    test_runners.insert("Jest".to_string());
                }
                if all_deps.contains("playwright") || all_deps.contains("@playwright/test") {
                    test_runners.insert("Playwright".to_string());
                }
                if all_deps.contains("typescript") || root.join("tsconfig.json").exists() {
                    diagnostics.insert("tsc --noEmit".to_string());
                }
                if all_deps.contains("eslint") {
                    diagnostics.insert("eslint".to_string());
                }
                if all_deps.contains("prettier") {
                    diagnostics.insert("prettier".to_string());
                }

                // Check monorepo workspaces
                if let Some(workspaces) = json.get("workspaces").and_then(Value::as_array) {
                    for ws in workspaces {
                        if let Some(ws_str) = ws.as_str() {
                            workspace_packages.push(ws_str.to_string());
                        }
                    }
                }
            }
        }
    }

    // 2. Rust ecosystem
    let cargo_toml_path = root.join("Cargo.toml");
    if cargo_toml_path.exists() {
        languages.insert("Rust".to_string());
        package_managers.insert("cargo".to_string());
        test_runners.insert("cargo test".to_string());
        diagnostics.insert("cargo check".to_string());
        diagnostics.insert("cargo clippy".to_string());

        if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
            let lower = content.to_ascii_lowercase();
            if lower.contains("tokio") {
                frameworks.insert("Tokio".to_string());
            }
            if lower.contains("axum") {
                frameworks.insert("Axum".to_string());
            }
            if lower.contains("actix") {
                frameworks.insert("Actix".to_string());
            }
            if lower.contains("tauri") {
                frameworks.insert("Tauri".to_string());
            }
            if lower.contains("diesel") || lower.contains("sqlx") {
                frameworks.insert("SQL/DB".to_string());
            }
            if lower.contains("[workspace]") {
                workspace_packages.push("cargo-workspace".to_string());
            }
        }
    }

    // 3. Python ecosystem
    if root.join("pyproject.toml").exists()
        || root.join("requirements.txt").exists()
        || root.join("setup.py").exists()
    {
        languages.insert("Python".to_string());
        if root.join("uv.lock").exists() {
            package_managers.insert("uv".to_string());
        } else if root.join("poetry.lock").exists() {
            package_managers.insert("poetry".to_string());
        } else {
            package_managers.insert("pip".to_string());
        }
        test_runners.insert("pytest".to_string());
        diagnostics.insert("pytest".to_string());
    }

    // 4. Go ecosystem
    if root.join("go.mod").exists() {
        languages.insert("Go".to_string());
        package_managers.insert("go".to_string());
        test_runners.insert("go test".to_string());
        diagnostics.insert("go vet".to_string());
    }

    // 5. Test files discovery
    let test_files_count = count_test_files(root);

    // 6. Assemble human-readable summary sentence
    let lang_str = if languages.is_empty() {
        "Generic".to_string()
    } else {
        languages.iter().cloned().collect::<Vec<_>>().join(" / ")
    };

    let framework_str = if frameworks.is_empty() {
        String::new()
    } else {
        format!(" (using {})", frameworks.iter().cloned().collect::<Vec<_>>().join(", "))
    };

    let test_info = if test_runners.is_empty() {
        if test_files_count > 0 {
            format!(", {} test files detected", test_files_count)
        } else {
            String::new()
        }
    } else {
        format!(
            ", tested with {} ({} test files found)",
            test_runners.iter().cloned().collect::<Vec<_>>().join(", "),
            test_files_count
        )
    };

    let workspace_info = if !workspace_packages.is_empty() {
        format!(" [Monorepo: {} packages]", workspace_packages.len())
    } else {
        String::new()
    };

    let summary_sentence = format!(
        "{lang_str} project{framework_str}{workspace_info}{test_info}."
    );

    ProjectArchitectureSummary {
        root: root.to_path_buf(),
        languages: languages.into_iter().collect(),
        package_managers: package_managers.into_iter().collect(),
        frameworks: frameworks.into_iter().collect(),
        test_runners: test_runners.into_iter().collect(),
        diagnostics: diagnostics.into_iter().collect(),
        workspace_packages,
        test_files_count,
        summary_sentence,
    }
}

fn count_test_files(root: &Path) -> usize {
    let mut count = 0;
    count_test_files_recursive(root, 0, &mut count);
    count
}

fn count_test_files_recursive(dir: &Path, depth: usize, count: &mut usize) {
    if depth > 8 || *count > 1000 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "dist"
            || name == "build"
            || name == "vendor"
        {
            continue;
        }
        if path.is_dir() {
            count_test_files_recursive(&path, depth + 1, count);
        } else if path.is_file() {
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".test.ts")
                || lower.ends_with(".test.tsx")
                || lower.ends_with(".spec.ts")
                || lower.ends_with(".spec.tsx")
                || lower.ends_with(".test.js")
                || lower.ends_with(".spec.js")
                || lower.ends_with("_test.rs")
                || lower.ends_with("_test.py")
                || lower.starts_with("test_")
                || path.to_string_lossy().contains("/tests/")
                || path.to_string_lossy().contains("/test/")
            {
                *count += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("mint_test_{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_detect_project_deep_package_json() {
        let dir = TempDir::new();
        let pkg_path = dir.path().join("package.json");
        let mut file = File::create(pkg_path).unwrap();
        writeln!(
            file,
            r#"{{
                "dependencies": {{ "react": "^18.0.0", "zustand": "^4.0.0" }},
                "devDependencies": {{ "vite": "^5.0.0", "vitest": "^1.0.0", "typescript": "^5.0.0" }}
            }}"#
        )
        .unwrap();

        let summary = detect_project_deep(dir.path());
        assert!(summary.languages.contains(&"JavaScript/TypeScript".to_string()));
        assert!(summary.frameworks.contains(&"React".to_string()));
        assert!(summary.frameworks.contains(&"Zustand".to_string()));
        assert!(summary.frameworks.contains(&"Vite".to_string()));
        assert!(summary.test_runners.contains(&"Vitest".to_string()));
        assert!(summary.summary_sentence.contains("React"));
        assert!(summary.summary_sentence.contains("Zustand"));
    }
}
