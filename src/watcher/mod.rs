use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::analyzer;
use crate::db::Database;

pub mod state;
use state::{now_ms, project_key, read_pid_lock, remove_state, try_acquire_pid_lock, write_state, WatchState};

#[derive(Serialize)]
pub struct WatchStatus {
    pub project_path: String,
    pub project_key: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub state_path: String,
    pub pid_path: String,
    pub last_event_at_ms: Option<u128>,
    pub last_scan_at_ms: Option<u128>,
    pub last_scan_reason: Option<String>,
    pub last_error: Option<String>,
    pub status: Option<String>,
}

/// Start watching for file changes and re-analyze incrementally
pub fn watch_project(project_root: &Path) -> Result<()> {
    let project_root = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let project_root_str = project_root.to_string_lossy().to_string();
    let project_key = project_key(&project_root);

    if !try_acquire_pid_lock(&project_root)? {
        return Ok(());
    }

    let started_at_ms = now_ms();
    let mut watch_state = WatchState {
        project_path: project_root_str.clone(),
        project_key: project_key.clone(),
        pid: std::process::id(),
        status: "watching".to_string(),
        started_at_ms,
        last_event_at_ms: Some(started_at_ms),
        last_scan_at_ms: Some(started_at_ms),
        last_scan_reason: Some("watch-start".to_string()),
        last_error: None,
    };
    write_state(&project_root, &watch_state)?;

    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            tx.send(event).ok();
        }
    })?;

    // Watch the project root (excluding .ctx and .git)
    watcher.watch(&project_root, RecursiveMode::Recursive)?;

    println!("  Watching for changes... (Ctrl+C to stop)");

    let db = Database::open(&project_root)?;
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
                        watch_state.last_event_at_ms = Some(now_ms());
                        watch_state.last_scan_reason = Some("filesystem-event".to_string());
                        let _ = write_state(&project_root, &watch_state);

                        // Debounce: wait at least 1 second between re-analyses
                        if debounce_timer.elapsed() > Duration::from_secs(1) {
                            println!("  Change detected, re-analyzing...");
                            match analyzer::analyze_project(&db, &project_root) {
                                Ok(result) => {
                                    watch_state.last_scan_at_ms = Some(now_ms());
                                    watch_state.status = "watching".to_string();
                                    watch_state.last_error = None;
                                    let _ = write_state(&project_root, &watch_state);
                                    println!(
                                        "  OK  Updated: {} files, {} symbols",
                                        result.analyzed_files, result.total_symbols
                                    );
                                }
                                Err(e) => {
                                    watch_state.last_scan_at_ms = Some(now_ms());
                                    watch_state.status = "error".to_string();
                                    watch_state.last_error = Some(e.to_string());
                                    let _ = write_state(&project_root, &watch_state);
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

    remove_state(&project_root);
    Ok(())
}

pub fn watch_status(project_root: &Path) -> WatchStatus {
    let project = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let pid = read_pid_lock(&project);
    let pid_running = pid
        .map(|value| Command::new("kill").arg("-0").arg(value.to_string()).status().map(|s| s.success()).unwrap_or(false))
        .unwrap_or(false);

    let state_path = state::state_path(&project);
    let pid_path = state::pid_path(&project);
    let state = std::fs::read_to_string(&state_path)
        .ok()
        .and_then(|content| serde_json::from_str::<WatchState>(&content).ok());

    WatchStatus {
        project_path: project.to_string_lossy().to_string(),
        project_key: project_key(&project),
        running: pid_running,
        pid,
        state_path: state_path.to_string_lossy().to_string(),
        pid_path: pid_path.to_string_lossy().to_string(),
        last_event_at_ms: state.as_ref().and_then(|value| value.last_event_at_ms),
        last_scan_at_ms: state.as_ref().and_then(|value| value.last_scan_at_ms),
        last_scan_reason: state.as_ref().and_then(|value| value.last_scan_reason.clone()),
        last_error: state.as_ref().and_then(|value| value.last_error.clone()),
        status: state.as_ref().map(|value| value.status.clone()),
    }
}

/// Ensure a background watcher process is running for this project.
/// Intended for agent-driven workflows where explicit `watch` command is not called.
pub fn ensure_background_watch(project_root: &Path) -> Result<()> {
    if std::env::var("CTX_AGENT_DISABLE_AUTO_WATCH").ok().as_deref() == Some("1") {
        return Ok(());
    }

    let project = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let project_str = project.to_string_lossy().to_string();

    let status = watch_status(&project);
    if status.running || is_watch_running(&project_str) {
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
