use super::*;

impl Database {
    // =================================================================
    // File stats operations
    // =================================================================

    /// Upsert file stats
    pub fn upsert_file_stats(
        &self,
        file_id: i64,
        commit_count: i64,
        last_modified: Option<&str>,
        churn_score: f64,
        contributors: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO file_stats (file_id, commit_count, last_modified, churn_score, contributors)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_id) DO UPDATE SET
                commit_count = ?2, last_modified = ?3, churn_score = ?4, contributors = ?5",
            rusqlite::params![file_id, commit_count, last_modified, churn_score, contributors],
        )?;
        Ok(())
    }

    /// Get file health overview (for warnings)
    pub fn get_file_health(&self) -> Result<Vec<FileHealth>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.path, f.language, f.line_count,
                    COALESCE(fs.commit_count, 0),
                    COALESCE(fs.churn_score, 0.0),
                    (SELECT COUNT(*) FROM dependencies d WHERE d.to_file_id = f.id),
                    (SELECT COUNT(*) FROM symbols s WHERE s.file_id = f.id),
                    (SELECT COUNT(*) FROM dependencies d WHERE d.from_file_id = f.id)
             FROM files f
             LEFT JOIN file_stats fs ON fs.file_id = f.id
             ORDER BY fs.churn_score DESC NULLS LAST",
        )?;
        let rows = stmt.query_map([], |row| {
            let churn_score: f64 = row.get(4)?;
            let commit_count: i64 = row.get(3)?;
            let dependents_count: i64 = row.get(5)?;
            let symbol_count: i64 = row.get(6)?;
            let outgoing_dependencies_count: i64 = row.get(7)?;
            Ok(FileHealth {
                path: row.get(0)?,
                language: row.get(1)?,
                line_count: row.get(2)?,
                commit_count,
                churn_score,
                dependents_count,
                symbol_count,
                outgoing_dependencies_count,
                is_fragile: churn_score > 0.7 && dependents_count > 3,
                is_dead: commit_count == 0
                    && dependents_count == 0
                    && symbol_count == 0
                    && outgoing_dependencies_count == 0,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // =================================================================
    // Aggregate stats
    // =================================================================

    /// Count total files
    pub fn count_files(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?)
    }

    /// Total lines of code
    pub fn total_lines(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(line_count), 0) FROM files",
            [],
            |row| row.get(0),
        )?)
    }

    /// Language breakdown
    pub fn language_stats(&self) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT language, COUNT(*), SUM(line_count) FROM files GROUP BY language ORDER BY SUM(line_count) DESC"
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
