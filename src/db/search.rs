use anyhow::Result;

use super::core::Database;

impl Database {
    // =================================================================
    // Search operations (FTS5)
    // =================================================================

    /// Rebuild the FTS5 search index
    pub fn rebuild_search_index(&self) -> Result<()> {
        self.conn.execute("DELETE FROM search_index", [])?;
        self.conn.execute(
            "INSERT INTO search_index(name, path, kind, signature)
             SELECT s.name, f.path, s.kind, s.signature
             FROM symbols s JOIN files f ON f.id = s.file_id",
            [],
        )?;
        Ok(())
    }

    /// Full-text search across symbols
    pub fn search(&self, query: &str) -> Result<Vec<(String, String, String, String)>> {
        let fts_query = query
            .split_whitespace()
            .map(|w| format!("{}*", w))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = self.conn.prepare(
            "SELECT name, path, kind, signature FROM search_index WHERE search_index MATCH ?1 LIMIT 50"
        )?;
        let rows = stmt.query_map([&fts_query], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
