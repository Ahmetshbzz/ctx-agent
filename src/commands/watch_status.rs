use super::*;

pub(super) fn cmd_watch_status(root: &Path, json_mode: bool) -> Result<()> {
    let status = watcher::watch_status(root);

    if json_mode {
        println!("{}", json!(status));
    } else {
        println!("\n  {} — Watch Status\n", "ctx-agent".cyan().bold());
        println!(
            "  Running: {}",
            if status.running {
                "yes".green().to_string()
            } else {
                "no".red().to_string()
            }
        );
        println!(
            "  PID: {}",
            status
                .pid
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
                .cyan()
        );
        println!("  State file: {}", status.state_path.dimmed());
        if let Some(last_scan) = status.last_scan_at_ms {
            println!("  Last scan at ms: {}", last_scan.to_string().cyan());
        }
        if let Some(last_event) = status.last_event_at_ms {
            println!("  Last event at ms: {}", last_event.to_string().cyan());
        }
        if let Some(reason) = status.last_scan_reason {
            println!("  Last reason: {}", reason.cyan());
        }
        println!("  Dirty count: {}", status.dirty_count.to_string().cyan());
        if !status.recent_paths.is_empty() {
            println!("  Recent paths:");
            for path in status.recent_paths.iter().take(5) {
                println!("    - {}", path.dimmed());
            }
        }
        if let Some(error) = status.last_error {
            println!("  Last error: {}", error.red());
        }
        println!();
    }

    Ok(())
}
