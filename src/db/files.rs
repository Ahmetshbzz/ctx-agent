use anyhow::Result;

use super::core::Database;
use super::models::TrackedFile;

impl Database {
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
            Ok(file) => Ok(Some(file)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Get file ID by path
    pub fn get_file_id(&self, path: &str) -> Result<Option<i64>> {
        let mut stmt = self.conn.prepare("SELECT id FROM files WHERE path = ?1")?;
        let result = stmt.query_row(rusqlite::params![path], |row| row.get(0));
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
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
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    /// Remove files not in the given list (for detecting deleted files)
    pub fn remove_files_not_in(&self, paths: &[String]) -> Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }
        let placeholders: Vec<String> = paths
            .iter()
            .enumerate()
            .map(|(index, _)| format!("?{}", index + 1))
            .collect();
        let sql = format!(
            "DELETE FROM files WHERE path NOT IN ({})",
            placeholders.join(",")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = paths
            .iter()
            .map(|path| path as &dyn rusqlite::types::ToSql)
            .collect();
        let count = stmt.execute(params.as_slice())?;
        Ok(count)
    }
}
