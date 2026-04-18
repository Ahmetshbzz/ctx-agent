use anyhow::Result;

use super::core::Database;
use super::models::Decision;

impl Database {
    // =================================================================
    // Decision operations
    // =================================================================

    /// Insert a decision
    pub fn insert_decision(
        &self,
        description: &str,
        source: &str,
        commit_hash: Option<&str>,
        related_files: &str,
    ) -> Result<bool> {
        let changed = self.conn.execute(
            "INSERT OR IGNORE INTO decisions (description, source, commit_hash, related_files)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![description, source, commit_hash, related_files],
        )?;
        Ok(changed > 0)
    }

    /// Get all decisions
    pub fn get_decisions(&self) -> Result<Vec<Decision>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, description, source, commit_hash, related_files
             FROM decisions ORDER BY timestamp DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Decision {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                description: row.get(2)?,
                source: row.get(3)?,
                commit_hash: row.get(4)?,
                related_files: row.get(5)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
