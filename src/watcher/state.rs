use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
pub struct WatchState {
    pub project_path: String,
    pub project_key: String,
    pub pid: u32,
    pub status: String,
    pub started_at_ms: u128,
    pub last_event_at_ms: Option<u128>,
    pub last_scan_at_ms: Option<u128>,
    pub last_scan_reason: Option<String>,
    pub last_error: Option<String>,
    pub dirty_count: usize,
    pub recent_paths: Vec<String>,
}

pub fn project_key(project_path: &Path) -> String {
    blake3::hash(project_path.to_string_lossy().as_bytes())
        .to_hex()
        .to_string()
}

pub fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".ctx-agent").join("watch-state")
}

pub fn state_path(project_path: &Path) -> PathBuf {
    state_dir().join(format!("{}.json", project_key(project_path)))
}

pub fn pid_path(project_path: &Path) -> PathBuf {
    state_dir().join(format!("{}.pid", project_key(project_path)))
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn write_state(project_path: &Path, state: &WatchState) -> Result<()> {
    fs::create_dir_all(state_dir())?;
    fs::write(state_path(project_path), serde_json::to_vec_pretty(state)?)?;
    Ok(())
}

pub fn remove_state(project_path: &Path) {
    let _ = fs::remove_file(state_path(project_path));
    let _ = fs::remove_file(pid_path(project_path));
}

pub fn try_acquire_pid_lock(project_path: &Path) -> Result<bool> {
    fs::create_dir_all(state_dir())?;
    let pid_file = pid_path(project_path);
    let pid = std::process::id().to_string();

    match fs::OpenOptions::new().write(true).create_new(true).open(&pid_file) {
        Ok(_) => {
            fs::write(pid_file, pid)?;
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            if let Some(existing_pid) = read_pid_lock(project_path) {
                let is_running = std::process::Command::new("kill")
                    .arg("-0")
                    .arg(existing_pid.to_string())
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false);
                if !is_running {
                    let _ = fs::remove_file(&pid_file);
                    return try_acquire_pid_lock(project_path);
                }
            } else {
                let _ = fs::remove_file(&pid_file);
                return try_acquire_pid_lock(project_path);
            }
            Ok(false)
        }
        Err(err) => Err(err.into()),
    }
}

pub fn read_pid_lock(project_path: &Path) -> Option<u32> {
    let pid_file = pid_path(project_path);
    let content = fs::read_to_string(pid_file).ok()?;
    content.trim().parse::<u32>().ok()
}
