use super::*;
use anyhow::anyhow;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::SearcherBuilder;
use ignore::WalkBuilder;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct GrepHit {
    file: String,
    line: u64,
    text: String,
}

pub(super) fn cmd_grep(
    root: &Path,
    pattern: &str,
    max_results: usize,
    json_mode: bool,
) -> Result<()> {
    let _db = ensure_initialized(root)?;
    let limit = max_results.clamp(1, 200);

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(false)
        .build(pattern)
        .map_err(|e| anyhow!("invalid grep pattern: {e}"))?;

    let mut hits: Vec<GrepHit> = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
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
        if hits.len() >= limit {
            break;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let mut searcher = SearcherBuilder::new()
            .line_number(true)
            .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
            .build();

        let mut sink = UTF8(|line_number: u64, line: &str| {
            if hits.len() >= limit {
                return Ok(false);
            }
            hits.push(GrepHit {
                file: rel.clone(),
                line: line_number,
                text: line.trim_end().to_string(),
            });
            Ok(hits.len() < limit)
        });

        let _ = searcher.search_path(&matcher, path, &mut sink);
    }

    if json_mode {
        println!(
            "{}",
            json!({
                "command": "grep",
                "pattern": pattern,
                "count": hits.len(),
                "limit": limit,
                "results": hits,
            })
        );
        return Ok(());
    }

    println!();
    if hits.is_empty() {
        println!("  {} No text matches found.", "INFO".cyan().bold());
    } else {
        println!(
            "  {} {} matches for \"{}\"",
            "OK".green().bold(),
            hits.len().to_string().cyan().bold(),
            pattern.white()
        );
        for h in &hits {
            println!("    {}:{}  {}", h.file.cyan(), h.line, h.text);
        }
        if hits.len() >= limit {
            println!("  {} result limit reached ({limit})", "INFO".yellow().bold());
        }
    }
    println!();

    Ok(())
}
