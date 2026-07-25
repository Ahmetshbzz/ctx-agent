use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::schema;

/// Main database handle
pub struct Database {
    pub(crate) conn: Connection,
    pub(crate) project_root: PathBuf,
    pub ctx_dir: PathBuf,
}

impl Database {
    /// Open or create the project database in the global ctx store
    pub fn open(project_root: &Path) -> Result<Self> {
        Self::open_in(project_root, Self::default_storage_root()?)
    }

    pub fn open_in(project_root: &Path, global_root: PathBuf) -> Result<Self> {
        let project_root = Self::canonical_project_root(project_root);
        let (ctx_dir, db_path) = Self::storage_paths(&project_root, global_root);
        std::fs::create_dir_all(&ctx_dir).context("Failed to create project ctx directory")?;

        let conn = Connection::open(&db_path).context("Failed to open database")?;
        conn.busy_timeout(Duration::from_secs(5))?;

        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA foreign_keys=ON;
        ",
        )?;

        schema::run_migrations(&conn)?;
        Self::bind_project_root(&conn, &project_root)?;

        Ok(Self {
            conn,
            project_root,
            ctx_dir,
        })
    }

    /// Check if the database exists for the project
    pub fn exists(project_root: &Path) -> bool {
        let Ok(global_root) = Self::default_storage_root() else {
            return false;
        };
        let project_root = Self::canonical_project_root(project_root);
        let (_, db_path) = Self::storage_paths(&project_root, global_root);
        db_path.exists()
    }

    fn bind_project_root(conn: &Connection, project_root: &Path) -> Result<()> {
        let canonical_root = project_root.to_string_lossy().to_string();

        let existing: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'project_root' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        match existing {
            Some(value) if value != canonical_root => {
                bail!(
                    "This .ctx database belongs to a different project root: {}\nCurrent root: {}",
                    value,
                    canonical_root
                );
            }
            Some(_) => {}
            None => {
                conn.execute(
                    "INSERT INTO meta (key, value) VALUES ('project_root', ?1)",
                    rusqlite::params![canonical_root],
                )?;
            }
        }
        Ok(())
    }

    pub fn canonical_project_root(project_root: &Path) -> PathBuf {
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    pub fn with_immediate_transaction<T>(
        &self,
        operation: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .context("Failed to acquire ctx-agent scan lock")?;

        match operation(self) {
            Ok(value) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(value)
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn default_storage_root() -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("CTX_AGENT_DATA_DIR") {
            return Ok(PathBuf::from(path));
        }

        let home = std::env::var("HOME").context("HOME environment variable is not set")?;
        Ok(Path::new(&home).join(".ctx-agent").join("projects"))
    }

    fn storage_paths(project_root: &Path, global_root: PathBuf) -> (PathBuf, PathBuf) {
        let project_key = blake3::hash(project_root.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();
        let ctx_dir = global_root.join(project_key);
        let db_path = ctx_dir.join("ctx.db");
        (ctx_dir, db_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_back_failed_immediate_transaction() -> Result<()> {
        let root = temporary_directory("transaction-project");
        let storage = temporary_directory("transaction-storage");
        std::fs::create_dir_all(&root)?;
        let db = Database::open_in(&root, storage.clone())?;
        db.set_meta("state", "before")?;

        let result: Result<()> = db.with_immediate_transaction(|db| {
            db.set_meta("state", "during")?;
            bail!("forced failure")
        });

        assert!(result.is_err());
        assert_eq!(db.get_meta("state")?.as_deref(), Some("before"));
        std::fs::remove_dir_all(root)?;
        std::fs::remove_dir_all(storage)?;
        Ok(())
    }

    fn temporary_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ctx-agent-{label}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }
}
