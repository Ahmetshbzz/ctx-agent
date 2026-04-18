use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use super::schema;

/// Main database handle
pub struct Database {
    pub(crate) conn: Connection,
    pub ctx_dir: PathBuf,
}

impl Database {
    /// Open or create the project database in the global ctx store
    pub fn open(project_root: &Path) -> Result<Self> {
        let (ctx_dir, db_path) = Self::storage_paths(project_root)?;
        std::fs::create_dir_all(&ctx_dir).context("Failed to create project ctx directory")?;

        let conn = Connection::open(&db_path).context("Failed to open database")?;

        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA foreign_keys=ON;
        ",
        )?;

        schema::run_migrations(&conn)?;
        Self::bind_project_root(&conn, project_root)?;

        Ok(Self { conn, ctx_dir })
    }

    /// Check if the database exists for the project
    pub fn exists(project_root: &Path) -> bool {
        Self::storage_paths(project_root)
            .map(|(_, db_path)| db_path.exists())
            .unwrap_or(false)
    }

    fn bind_project_root(conn: &Connection, project_root: &Path) -> Result<()> {
        let canonical_root = std::fs::canonicalize(project_root)
            .unwrap_or_else(|_| project_root.to_path_buf())
            .to_string_lossy()
            .to_string();

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

    fn storage_paths(project_root: &Path) -> Result<(PathBuf, PathBuf)> {
        let canonical_root = std::fs::canonicalize(project_root)
            .unwrap_or_else(|_| project_root.to_path_buf())
            .to_string_lossy()
            .to_string();

        let home = std::env::var("HOME").context("HOME environment variable is not set")?;
        let global_root = PathBuf::from(home).join(".ctx-agent").join("projects");
        let project_key = blake3::hash(canonical_root.as_bytes()).to_hex().to_string();

        let ctx_dir = global_root.join(project_key);
        let db_path = ctx_dir.join("ctx.db");
        Ok((ctx_dir, db_path))
    }
}
