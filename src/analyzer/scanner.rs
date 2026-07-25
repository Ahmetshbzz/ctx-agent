use anyhow::Result;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents a discovered source file
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub language: String,
    pub size_bytes: u64,
    pub content: String,
    pub line_count: usize,
    pub hash: String,
}

/// Map file extension to language name
fn detect_language(ext: &str) -> Option<&'static str> {
    match ext {
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" | "pyw" => Some("python"),
        "rs" => Some("rust"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "kt" | "kts" => Some("kotlin"),
        "cs" => Some("csharp"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "md" => Some("markdown"),
        "html" | "htm" => Some("html"),
        "css" | "scss" | "sass" | "less" => Some("css"),
        "sql" => Some("sql"),
        "sh" | "bash" | "zsh" => Some("shell"),
        "dockerfile" => Some("dockerfile"),
        _ => None,
    }
}

/// Languages we can parse with tree-sitter
pub fn is_parseable(language: &str) -> bool {
    matches!(
        language,
        "typescript"
            | "javascript"
            | "python"
            | "rust"
            | "go"
            | "c"
            | "cpp"
            | "csharp"
            | "java"
            | "php"
            | "ruby"
            | "shell"
            | "bash"
    )
}

/// Compute a simple hash of file content
fn hash_content(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

/// Scan a project directory and return all source files
pub fn scan_project(root: &Path) -> Result<Vec<ScannedFile>> {
    let mut files = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(true) // skip hidden files
        .git_ignore(true) // respect .gitignore
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            // Skip common non-source directories
            !matches!(
                name.as_ref(),
                "node_modules"
                    | ".git"
                    | ".ctx"
                    | "target"
                    | "__pycache__"
                    | ".next"
                    | "dist"
                    | "build"
                    | ".venv"
                    | "venv"
                    | ".tox"
                    | "vendor"
                    | "coverage"
                    | ".cache"
            )
        })
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let language = if file_name == "go.mod" {
            "gomod"
        } else if file_name.eq_ignore_ascii_case("dockerfile") {
            "dockerfile"
        } else {
            match detect_language(ext) {
                Some(lang) => lang,
                None => continue, // skip unknown file types
            }
        };

        // Read file content
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // skip binary/unreadable files
        };

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let size_bytes = content.len() as u64;
        let line_count = content.lines().count();
        let hash = hash_content(&content);

        files.push(ScannedFile {
            relative_path,
            absolute_path: path.to_path_buf(),
            language: language.to_string(),
            size_bytes,
            content,
            line_count,
            hash,
        });
    }

    Ok(files)
}

/// Get project stats summary
pub fn project_stats(files: &[ScannedFile]) -> HashMap<String, (usize, usize)> {
    let mut stats: HashMap<String, (usize, usize)> = HashMap::new(); // lang -> (file_count, line_count)
    for f in files {
        let entry = stats.entry(f.language.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += f.line_count;
    }
    stats
}
