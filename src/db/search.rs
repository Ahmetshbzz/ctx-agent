use anyhow::Result;

use super::core::Database;

impl Database {
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

    pub fn search(&self, query: &str) -> Result<Vec<(String, String, String, String)>> {
        let Some(fts_query) = fts_prefix_query(query) else {
            return Ok(Vec::new());
        };

        let mut stmt = self.conn.prepare(
            "SELECT name, path, kind, signature
             FROM search_index
             WHERE search_index MATCH ?1
             LIMIT 50",
        )?;
        let rows = stmt.query_map([&fts_query], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn fts_prefix_query(query: &str) -> Option<String> {
    let terms = query
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
mod tests {
    use super::fts_prefix_query;

    #[test]
    fn builds_literal_prefix_terms() {
        assert_eq!(
            fts_prefix_query("ProductFamilyService::Create"),
            Some("\"ProductFamilyService\"* \"Create\"*".to_string())
        );
        assert_eq!(
            fts_prefix_query("OR NEAR foo-bar"),
            Some("\"OR\"* \"NEAR\"* \"foo\"* \"bar\"*".to_string())
        );
        assert_eq!(
            fts_prefix_query("müşteri_ödemesi"),
            Some("\"müşteri_ödemesi\"*".to_string())
        );
    }

    #[test]
    fn ignores_punctuation_only_queries() {
        assert_eq!(fts_prefix_query("\"*():-"), None);
    }
}
