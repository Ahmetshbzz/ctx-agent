use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Fixture {
    root: PathBuf,
    storage: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(format!("ctx-cli-project-{suffix}"));
        let storage = std::env::temp_dir().join(format!("ctx-cli-storage-{suffix}"));
        std::fs::create_dir_all(&root).expect("create project fixture");
        std::fs::write(root.join("main.rs"), "fn main() {}\n").expect("write fixture source");
        Self { root, storage }
    }

    fn run(&self, command: &str) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_ctx"))
            .args(["--project", path_text(&self.root), "--json", command])
            .env("CTX_AGENT_DATA_DIR", &self.storage)
            .env("CTX_AGENT_NO_WATCH", "1")
            .output()
            .expect("run ctx command");
        assert!(
            output.status.success(),
            "ctx {command} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("parse ctx JSON output")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        let _ = std::fs::remove_dir_all(&self.storage);
    }
}

#[test]
fn reports_health_and_advances_generation_without_reanalyzing_git() {
    let fixture = Fixture::new();

    let before = fixture.run("health");
    assert_eq!(before["initialized"], false);

    let init = fixture.run("init");
    assert_eq!(init["index_generation"], 1);
    assert_eq!(init["git_skipped"], false);

    let health = fixture.run("health");
    assert_eq!(health["initialized"], true);
    assert_eq!(health["index_generation"], 1);
    assert_eq!(health["files"], 1);

    let scan = fixture.run("scan");
    assert_eq!(scan["index_generation"], 2);
    assert_eq!(scan["git_skipped"], true);
    assert_eq!(scan["analyzed_files"], 0);
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("fixture path must be UTF-8")
}
