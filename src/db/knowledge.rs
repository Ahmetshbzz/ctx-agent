use anyhow::Result;

use super::core::Database;
use super::models::Knowledge;

impl Database {
    // =================================================================
    // Knowledge operations
    // =================================================================

    /// Insert a knowledge note
    pub fn insert_knowledge(
        &self,
        content: &str,
        source: &str,
        related_file: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO knowledge (content, source, related_file) VALUES (?1, ?2, ?3)",
            rusqlite::params![content, source, related_file],
        )?;
        Ok(())
    }

    /// Get all knowledge notes
    pub fn get_knowledge(&self) -> Result<Vec<Knowledge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, source, related_file, timestamp FROM knowledge ORDER BY timestamp DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Knowledge {
                id: row.get(0)?,
                content: row.get(1)?,
                source: row.get(2)?,
                related_file: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get warnings about knowledge
    pub fn get_warnings_knowledge(&self) -> Result<Vec<Knowledge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, source, related_file, timestamp FROM knowledge
             WHERE source = 'agent' ORDER BY timestamp DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Knowledge {
                id: row.get(0)?,
                content: row.get(1)?,
                source: row.get(2)?,
                related_file: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
