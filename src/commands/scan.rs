use std::time::Instant;

use super::*;

pub(super) fn cmd_scan(root: &Path, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let start = Instant::now();

    if !json_mode {
        print!("  Scanning...");
    }

    let update = update_index(&db, root)?;
    let result = &update.analysis;
    let git_result = &update.git;
    let total_symbols = db.count_symbols()?;
    let total_dependencies = db.count_dependencies()?;
    let elapsed = start.elapsed();

    if json_mode {
        println!(
            "{}",
            json!({
                "command": "scan",
                "total_files": result.total_files,
                "analyzed_files": result.analyzed_files,
                "skipped_files": result.skipped_files,
                "removed_files": result.removed_files,
                "parsed_symbols": result.total_symbols,
                "parsed_dependencies": result.total_imports,
                "symbols": total_symbols,
                "dependencies": total_dependencies,
                "commits_analyzed": git_result.commits_analyzed,
                "git_skipped": update.git_skipped,
                "index_generation": update.generation,
                "elapsed_ms": elapsed.as_millis(),
            })
        );
    } else {
        println!(" {}", "done".green());
        println!(
            "    {} files ({} analyzed, {} unchanged, {} removed)",
            result.total_files.to_string().cyan(),
            result.analyzed_files.to_string().green(),
            result.skipped_files.to_string().dimmed(),
            result.removed_files.to_string().red(),
        );
        println!(
            "    {} symbols parsed, {} dependencies parsed",
            result.total_symbols.to_string().cyan(),
            result.total_imports.to_string().cyan(),
        );
        println!(
            "    {} total symbols, {} total dependencies",
            total_symbols.to_string().cyan(),
            total_dependencies.to_string().cyan(),
        );
        if git_result.error.is_none() {
            println!(
                "    {} git commits analyzed",
                git_result.commits_analyzed.to_string().cyan()
            );
        }
        println!(
            "  {} Completed in {:.1}s\n",
            "OK".green(),
            elapsed.as_secs_f64()
        );
    }

    Ok(())
}
