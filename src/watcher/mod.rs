use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::db::Database;
use crate::index::update_index;

pub mod state;
use state::{
    now_ms, project_key, read_pid_lock, remove_state, try_acquire_pid_lock, write_state, WatchState,
};

const DEBOUNCE_QUIET_PERIOD: Duration = Duration::from_secs(1);
const DEBOUNCE_MAX_DELAY: Duration = Duration::from_secs(5);
const MAX_DIRTY_PATHS: usize = 1_000;

#[derive(Default)]
struct DebounceState {
    first_event: Option<Instant>,
    last_event: Option<Instant>,
    last_attempt: Option<Instant>,
}

impl DebounceState {
    fn record_event(&mut self, now: Instant) {
        self.first_event.get_or_insert(now);
        self.last_event = Some(now);
    }

    fn should_scan(&self, now: Instant) -> bool {
        let (Some(first_event), Some(last_event)) = (self.first_event, self.last_event) else {
            return false;
        };
        if self
            .last_attempt
            .is_some_and(|attempt| now.duration_since(attempt) < DEBOUNCE_QUIET_PERIOD)
        {
            return false;
        }
        now.duration_since(last_event) >= DEBOUNCE_QUIET_PERIOD
            || now.duration_since(first_event) >= DEBOUNCE_MAX_DELAY
    }

    fn scan_succeeded(&mut self) {
        *self = Self::default();
    }

    fn scan_failed(&mut self, now: Instant) {
        self.last_attempt = Some(now);
    }
}

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
    pub dirty_count: usize,
    pub recent_paths: Vec<String>,
}

/// Start watching for file changes and re-analyze incrementally
pub fn watch_project(project_root: &Path) -> Result<()> {
    let project_root = Database::canonical_project_root(project_root);
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
        dirty_count: 0,
        recent_paths: Vec::new(),
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
    let mut debounce = DebounceState::default();
    let mut dirty_paths = HashSet::new();

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                let dominated_by_ignored = event.paths.iter().all(|path| {
                    let path = path.to_string_lossy();
                    path.contains("/.ctx/")
                        || path.contains("/.git/")
                        || path.contains("/target/")
                        || path.contains("/node_modules/")
                });

                if !dominated_by_ignored
                    && matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                    )
                {
                    debounce.record_event(Instant::now());
                    watch_state.last_event_at_ms = Some(now_ms());
                    watch_state.last_scan_reason = Some("filesystem-event".to_string());
                    let changed_paths = event
                        .paths
                        .iter()
                        .map(|path| {
                            path.strip_prefix(&project_root)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string()
                        })
                        .collect::<Vec<_>>();
                    for path in changed_paths {
                        if dirty_paths.len() < MAX_DIRTY_PATHS {
                            dirty_paths.insert(path.clone());
                        }
                        if let Some(position) = watch_state
                            .recent_paths
                            .iter()
                            .position(|existing| existing == &path)
                        {
                            watch_state.recent_paths.remove(position);
                        }
                        watch_state.recent_paths.push(path);
                        if watch_state.recent_paths.len() > 5 {
                            watch_state.recent_paths.remove(0);
                        }
                    }
                    watch_state.dirty_count = dirty_paths.len();
                    let _ = write_state(&project_root, &watch_state);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let attempt_started = Instant::now();
        if !debounce.should_scan(attempt_started) {
            continue;
        }

        println!("  Change detected, re-analyzing...");
        match update_index(&db, &project_root) {
            Ok(update) => {
                debounce.scan_succeeded();
                dirty_paths.clear();
                watch_state.last_scan_at_ms = Some(now_ms());
                watch_state.status = "watching".to_string();
                watch_state.last_error = None;
                watch_state.dirty_count = 0;
                watch_state.recent_paths.clear();
                let _ = write_state(&project_root, &watch_state);
                println!(
                    "  OK  Updated: {} files, {} symbols",
                    update.analysis.analyzed_files, update.analysis.total_symbols
                );
            }
            Err(error) => {
                debounce.scan_failed(attempt_started);
                watch_state.last_scan_at_ms = Some(now_ms());
                watch_state.status = "error".to_string();
                watch_state.last_error = Some(error.to_string());
                let _ = write_state(&project_root, &watch_state);
                eprintln!("  ERROR  Analysis error: {}", error);
            }
        }
    }

    remove_state(&project_root);
    Ok(())
}

pub fn watch_status(project_root: &Path) -> WatchStatus {
    let project = Database::canonical_project_root(project_root);
    let pid = read_pid_lock(&project);
    let pid_running = pid
        .map(|value| {
            Command::new("kill")
                .arg("-0")
                .arg(value.to_string())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
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
        last_scan_reason: state
            .as_ref()
            .and_then(|value| value.last_scan_reason.clone()),
        last_error: state.as_ref().and_then(|value| value.last_error.clone()),
        status: state.as_ref().map(|value| value.status.clone()),
        dirty_count: state.as_ref().map(|value| value.dirty_count).unwrap_or(0),
        recent_paths: state
            .as_ref()
            .map(|value| value.recent_paths.clone())
            .unwrap_or_default(),
    }
}

/// Ensure a background watcher process is running for this project.
/// Intended for agent-driven workflows where explicit `watch` command is not called.
pub fn ensure_background_watch(project_root: &Path) -> Result<()> {
    if std::env::var("CTX_AGENT_DISABLE_AUTO_WATCH")
        .ok()
        .as_deref()
        == Some("1")
    {
        return Ok(());
    }

    let project = Database::canonical_project_root(project_root);
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
    let output = Command::new("pgrep").arg("-f").arg(&pattern).output();

    match output {
        Ok(out) => out.status.success() && !out.stdout.is_empty(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{DebounceState, DEBOUNCE_MAX_DELAY, DEBOUNCE_QUIET_PERIOD};
    use std::time::{Duration, Instant};

    #[test]
    fn waits_for_quiet_period_after_first_event() {
        let started = Instant::now();
        let mut state = DebounceState::default();
        state.record_event(started);

        assert!(!state.should_scan(started + DEBOUNCE_QUIET_PERIOD - Duration::from_millis(1)));
        assert!(state.should_scan(started + DEBOUNCE_QUIET_PERIOD));
    }

    #[test]
    fn coalesces_bursts_without_starvation() {
        let started = Instant::now();
        let mut state = DebounceState::default();
        state.record_event(started);
        state.record_event(started + DEBOUNCE_MAX_DELAY - Duration::from_millis(100));

        assert!(state.should_scan(started + DEBOUNCE_MAX_DELAY));
    }

    #[test]
    fn preserves_pending_events_after_failed_scan() {
        let started = Instant::now();
        let mut state = DebounceState::default();
        state.record_event(started);
        let first_attempt = started + DEBOUNCE_QUIET_PERIOD;
        assert!(state.should_scan(first_attempt));

        state.scan_failed(first_attempt);
        assert!(!state.should_scan(first_attempt + Duration::from_millis(500)));
        assert!(state.should_scan(first_attempt + DEBOUNCE_QUIET_PERIOD));

        state.scan_succeeded();
        assert!(!state.should_scan(first_attempt + DEBOUNCE_MAX_DELAY));
    }
}
