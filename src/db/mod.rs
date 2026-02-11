mod decisions;
mod dependencies;
mod knowledge;
pub mod models;
pub mod schema;
mod search;
mod stats;

use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use self::models::*;

/// Main database handle
pub struct Database {
    conn: Connection,
    pub ctx_dir: PathBuf,
}

impl Database {
    /// Open or create the project database in the global ctx store
    pub fn open(project_root: &Path) -> Result<Self> {
        let (ctx_dir, db_path) = Self::storage_paths(project_root)?;
        std::fs::create_dir_all(&ctx_dir).context("Failed to create project ctx directory")?;

        let conn = Connection::open(&db_path).context("Failed to open database")?;

        // Enable WAL mode for better concurrent access
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

    // =================================================================
    // File operations
    // =================================================================

    /// Insert or update a file record
    pub fn upsert_file(
        &self,
        path: &str,
        language: &str,
        size_bytes: i64,
        hash: &str,
        line_count: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO files (path, language, size_bytes, hash, line_count, last_analyzed)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
             ON CONFLICT(path) DO UPDATE SET
                language = ?2, size_bytes = ?3, hash = ?4, line_count = ?5,
                last_analyzed = CURRENT_TIMESTAMP",
            rusqlite::params![path, language, size_bytes, hash, line_count],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get file by path
    pub fn get_file_by_path(&self, path: &str) -> Result<Option<TrackedFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, language, size_bytes, hash, line_count, last_analyzed FROM files WHERE path = ?1"
        )?;
        let result = stmt.query_row(rusqlite::params![path], |row| {
            Ok(TrackedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
                size_bytes: row.get(3)?,
                hash: row.get(4)?,
                line_count: row.get(5)?,
                last_analyzed: row.get(6)?,
            })
        });
        match result {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get file ID by path
    pub fn get_file_id(&self, path: &str) -> Result<Option<i64>> {
        let mut stmt = self.conn.prepare("SELECT id FROM files WHERE path = ?1")?;
        let result = stmt.query_row(rusqlite::params![path], |row| row.get(0));
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all files
    pub fn get_all_files(&self) -> Result<Vec<TrackedFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, language, size_bytes, hash, line_count, last_analyzed FROM files ORDER BY path"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TrackedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
                size_bytes: row.get(3)?,
                hash: row.get(4)?,
                line_count: row.get(5)?,
                last_analyzed: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Remove files not in the given list (for detecting deleted files)
    pub fn remove_files_not_in(&self, paths: &[String]) -> Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }
        let placeholders: Vec<String> = paths
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "DELETE FROM files WHERE path NOT IN ({})",
            placeholders.join(",")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = paths
            .iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();
        let count = stmt.execute(params.as_slice())?;
        Ok(count)
    }

    // =================================================================
    // Symbol operations
    // =================================================================

    /// Clear all symbols for a file
    pub fn clear_symbols(&self, file_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM symbols WHERE file_id = ?1", [file_id])?;
        Ok(())
    }

    /// Insert a symbol
    #[allow(clippy::too_many_arguments)]
    pub fn insert_symbol(
        &self,
        file_id: i64,
        name: &str,
        kind: &SymbolKind,
        start_line: i64,
        end_line: i64,
        signature: &str,
        parent_id: Option<i64>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO symbols (file_id, name, kind, start_line, end_line, signature, parent_symbol_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![file_id, name, kind.as_str(), start_line, end_line, signature, parent_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get all symbols for a file
    pub fn get_symbols_for_file(&self, file_id: i64) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, name, kind, start_line, end_line, signature, parent_symbol_id
             FROM symbols WHERE file_id = ?1 ORDER BY start_line",
        )?;
        let rows = stmt.query_map([file_id], |row| {
            let kind_str: String = row.get(3)?;
            Ok(Symbol {
                id: row.get(0)?,
                file_id: row.get(1)?,
                name: row.get(2)?,
                kind: SymbolKind::from_db_str(&kind_str),
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                signature: row.get(6)?,
                parent_symbol_id: row.get(7)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count total symbols
    pub fn count_symbols(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?)
    }

    /// Count symbols by kind
    pub fn count_symbols_by_kind(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM symbols GROUP BY kind ORDER BY COUNT(*) DESC")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
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
