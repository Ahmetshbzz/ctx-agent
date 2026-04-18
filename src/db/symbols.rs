use anyhow::Result;

use super::core::Database;
use super::models::{Symbol, SymbolKind};

impl Database {
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
        Ok(rows.filter_map(|row| row.ok()).collect())
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
        Ok(rows.filter_map(|row| row.ok()).collect())
    }
}
