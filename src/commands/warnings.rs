use super::*;

pub(super) fn cmd_warnings(root: &Path, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let health = db.get_file_health()?;
    let knowledge = db.get_warnings_knowledge()?;

    let fragile: Vec<_> = health.iter().filter(|h| h.is_fragile).collect();
    let dead: Vec<_> = health.iter().filter(|h| h.is_dead).collect();
    let large: Vec<_> = health.iter().filter(|h| h.line_count > 500).collect();

    if json_mode {
        let fragile_entries: Vec<_> = fragile
            .iter()
            .map(|f| {
                json!({
                    "path": f.path,
                    "commit_count": f.commit_count,
                    "dependents": f.dependents_count,
                    "churn_score": f.churn_score,
                })
            })
            .collect();

        let large_entries: Vec<_> = large
            .iter()
            .map(|f| {
                json!({
                    "path": f.path,
                    "lines": f.line_count,
                    "language": f.language,
                })
            })
            .collect();

        let dead_entries: Vec<_> = dead
            .iter()
            .map(|f| {
                json!({
                    "path": f.path,
                    "language": f.language,
                })
            })
            .collect();

        let knowledge_entries: Vec<_> = knowledge
            .iter()
            .map(|k| {
                json!({
                    "content": k.content,
                    "file": k.related_file,
                })
            })
            .collect();

        println!(
            "{}",
            json!({
                "command": "warnings",
                "total_warnings": fragile.len() + dead.len() + large.len() + knowledge.len(),
                "fragile_files": fragile_entries,
                "large_files": large_entries,
                "dead_files": dead_entries,
                "knowledge_warnings": knowledge_entries,
            })
        );
    } else {
        let total_warnings = fragile.len() + dead.len() + large.len() + knowledge.len();

        if total_warnings == 0 {
            println!("\n  {} No warnings - looking good!\n", "OK".green().bold());
            return Ok(());
        }

        println!(
            "\n  {} {} warnings\n",
            "WARN".yellow().bold(),
            total_warnings.to_string().yellow().bold()
        );

        if !fragile.is_empty() {
            println!("  Fragile files (high churn + many dependents):");
            for f in &fragile {
                println!(
                    "    {} {} — {} commits, {} dependents, churn {:.1}",
                    "WARN".yellow(),
                    f.path.red(),
                    f.commit_count.to_string().cyan(),
                    f.dependents_count.to_string().cyan(),
                    f.churn_score,
                );
            }
            println!();
        }

        if !large.is_empty() {
            println!("  Large files (>500 lines):");
            for f in &large {
                println!(
                    "    {} {} — {} lines ({})",
                    "·".dimmed(),
                    f.path.yellow(),
                    f.line_count.to_string().cyan(),
                    f.language.dimmed(),
                );
            }
            println!();
        }

        if !dead.is_empty() {
            println!("  Potentially dead files (no git history, symbols, or dependencies):");
            for f in dead.iter().take(10) {
                println!(
                    "    {} {} ({})",
                    "·".dimmed(),
                    f.path.dimmed(),
                    f.language.dimmed()
                );
            }
            if dead.len() > 10 {
                println!("    {} ... and {} more", "·".dimmed(), dead.len() - 10);
            }
            println!();
        }

        if !knowledge.is_empty() {
            println!("  Agent-discovered issues:");
            for k in &knowledge {
                let file_str = k.related_file.as_deref().unwrap_or("");
                println!(
                    "    {} {} {}",
                    "WARN".yellow(),
                    k.content,
                    if !file_str.is_empty() {
                        format!("({})", file_str).dimmed().to_string()
                    } else {
                        String::new()
                    },
                );
            }
            println!();
        }
    }

    Ok(())
}
