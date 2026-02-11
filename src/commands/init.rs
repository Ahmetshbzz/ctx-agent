use super::*;

pub(super) fn cmd_init(root: &Path, json_mode: bool) -> Result<()> {
    if !json_mode {
        println!(
            "\n  {} — Agent Context Protocol\n",
            "ctx-agent".cyan().bold()
        );
    }

    if Database::exists(root) {
        if !json_mode {
            println!(
                "  {} Already initialized. Running re-scan...\n",
                "INFO".yellow()
            );
        }
        return super::scan::cmd_scan(root, json_mode);
    }

    let start = Instant::now();
    let db = Database::open(root)?;
    let db_path = db.ctx_dir.join("ctx.db");

    if !json_mode {
        println!(
            "  {} Created {}",
            "OK".green(),
            db_path.display().to_string().dimmed()
        );
        print!("  Scanning project...");
    }

    let result = analyzer::analyze_project(&db, root)?;

    if !json_mode {
        println!(" {}", "done".green());
        println!(
            "    {} files discovered",
            result.total_files.to_string().cyan()
        );
        println!(
            "    {} symbols extracted",
            result.total_symbols.to_string().cyan()
        );
        println!(
            "    {} dependencies mapped",
            result.total_imports.to_string().cyan()
        );
        print!("  Analyzing git history...");
    }

    let git_result = git::analyze_git_history(&db, root)?;

    if json_mode {
        let elapsed = start.elapsed();
        println!(
            "{}",
            json!({
                "command": "init",
                "files": result.total_files,
                "symbols": result.total_symbols,
                "dependencies": result.total_imports,
                "commits_analyzed": git_result.commits_analyzed,
                "decisions_found": git_result.decisions_found,
                "elapsed_ms": elapsed.as_millis(),
            })
        );
    } else {
        println!(" {}", "done".green());
        if let Some(err) = &git_result.error {
            println!("    {} {}", "WARN".yellow(), err.dimmed());
        } else {
            println!(
                "    {} commits analyzed",
                git_result.commits_analyzed.to_string().cyan()
            );
            println!(
                "    {} decisions extracted",
                git_result.decisions_found.to_string().cyan()
            );
        }

        let elapsed = start.elapsed();
        println!(
            "\n  {} Initialized in {:.1}s\n",
            "OK".green().bold(),
            elapsed.as_secs_f64()
        );
    }

    Ok(())
}
