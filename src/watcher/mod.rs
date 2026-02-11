use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::analyzer;
use crate::db::Database;

/// Start watching for file changes and re-analyze incrementally
pub fn watch_project(project_root: &Path) -> Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            tx.send(event).ok();
        }
    })?;

    // Watch the project root (excluding .ctx and .git)
    watcher.watch(project_root, RecursiveMode::Recursive)?;

    println!("  Watching for changes... (Ctrl+C to stop)");

    let db = Database::open(project_root)?;
    let mut debounce_timer = std::time::Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                // Skip events in .ctx, .git, target directories
                let dominated_by_ignored = event.paths.iter().all(|p| {
                    let path_str = p.to_string_lossy();
                    path_str.contains("/.ctx/")
                        || path_str.contains("/.git/")
                        || path_str.contains("/target/")
                        || path_str.contains("/node_modules/")
                });

                if dominated_by_ignored {
                    continue;
                }

                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        // Debounce: wait at least 1 second between re-analyses
                        if debounce_timer.elapsed() > Duration::from_secs(1) {
                            println!("  Change detected, re-analyzing...");
                            match analyzer::analyze_project(&db, project_root) {
                                Ok(result) => {
                                    println!(
                                        "  OK  Updated: {} files, {} symbols",
                                        result.analyzed_files, result.total_symbols
                                    );
                                }
                                Err(e) => {
                                    eprintln!("  ERROR  Analysis error: {}", e);
                                }
                            }
                            debounce_timer = std::time::Instant::now();
                        }
                    }
                    _ => {}
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

/// Ensure a background watcher process is running for this project.
/// Intended for agent-driven workflows where explicit `watch` command is not called.
pub fn ensure_background_watch(project_root: &Path) -> Result<()> {
    if std::env::var("CTX_AGENT_DISABLE_AUTO_WATCH").ok().as_deref() == Some("1") {
        return Ok(());
    }

    let project = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let project_str = project.to_string_lossy().to_string();

    if is_watch_running(&project_str) {
        return Ok(());
    }

    let exe = std::env::current_exe()?;
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let log_dir = Path::new(&home).join(".ctx-agent").join("watch-logs");
    fs::create_dir_all(&log_dir).ok();

    let project_key = blake3::hash(project_str.as_bytes()).to_hex().to_string();
    let log_path = log_dir.join(format!("{project_key}.log"));
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let err_file = log_file.try_clone()?;

    Command::new(exe)
        .arg("-p")
        .arg(&project_str)
        .arg("watch")
        .env("CTX_AGENT_AUTO_WATCH", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()
        .ok();

    Ok(())
}

fn is_watch_running(project_path: &str) -> bool {
    let pattern = format!("ctx -p {} watch", project_path);
    let output = Command::new("pgrep")
        .arg("-f")
        .arg(&pattern)
        .output();

    match output {
        Ok(out) => out.status.success() && !out.stdout.is_empty(),
        Err(_) => false,
    }
}
