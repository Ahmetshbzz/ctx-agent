use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use super::core::Database;

impl Database {
    // =================================================================
    // Dependency operations
    // =================================================================

    /// Clear dependencies for a file
    pub fn clear_dependencies(&self, file_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM dependencies WHERE from_file_id = ?1",
            [file_id],
        )?;
        Ok(())
    }

    /// Insert a dependency
    pub fn insert_dependency(
        &self,
        from_file_id: i64,
        to_path: &str,
        kind: &str,
        imported_names: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO dependencies (from_file_id, to_path, kind, imported_names)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![from_file_id, to_path, kind, imported_names],
        )?;
        Ok(())
    }

    /// Resolve dependency to_file_id based on path matching
    pub fn resolve_dependencies(&self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT d.id, d.to_path, f.path
             FROM dependencies d
             JOIN files f ON f.id = d.from_file_id
             WHERE d.to_file_id IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let unresolved: Vec<(i64, String, String)> = rows.filter_map(|r| r.ok()).collect();
        drop(stmt);

        for (dep_id, to_path, from_path) in unresolved {
            if let Some(target_id) = self.resolve_dependency_target(&from_path, &to_path)? {
                self.conn.execute(
                    "UPDATE dependencies SET to_file_id = ?1 WHERE id = ?2",
                    rusqlite::params![target_id, dep_id],
                )?;
            }
        }
        Ok(())
    }

    /// Get files that depend on the given file
    pub fn get_dependents(&self, file_id: i64) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT f.id, f.path FROM dependencies d
             JOIN files f ON f.id = d.from_file_id
             WHERE d.to_file_id = ?1 AND f.id != ?1",
        )?;
        let rows = stmt.query_map([file_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get files that this file depends on
    pub fn get_dependencies_of(&self, file_id: i64) -> Result<Vec<(Option<i64>, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.to_file_id, d.to_path FROM dependencies d WHERE d.from_file_id = ?1",
        )?;
        let rows = stmt.query_map([file_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count total dependencies
    pub fn count_dependencies(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))?)
    }

    fn resolve_dependency_target(&self, from_file: &str, raw_target: &str) -> Result<Option<i64>> {
        for candidate in dependency_path_candidates(from_file, raw_target) {
            if let Some(file_id) = self.get_file_id(&candidate)? {
                return Ok(Some(file_id));
            }
        }
        Ok(None)
    }
}

fn dependency_path_candidates(from_file: &str, raw_target: &str) -> Vec<String> {
    let Some(target) = normalize_import_target(raw_target) else {
        return vec![];
    };

    let from_dir = Path::new(from_file)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let target_slash = target.replace("::", "/");

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(rest) = target.strip_prefix("crate::") {
        let rel = rest.replace("::", "/");
        add_module_candidates(&mut candidates, &mut seen, format!("src/{rel}"));
        add_parent_module_candidates(&mut candidates, &mut seen, format!("src/{rel}"));
    } else if let Some(rest) = target.strip_prefix("self::") {
        let rel = rest.replace("::", "/");
        let joined = from_dir.join(rel).to_string_lossy().to_string();
        add_module_candidates(&mut candidates, &mut seen, joined.clone());
        add_parent_module_candidates(&mut candidates, &mut seen, joined);
    } else if let Some(rest) = target.strip_prefix("super::") {
        let rel = rest.replace("::", "/");
        let parent = from_dir.parent().unwrap_or_else(|| Path::new(""));
        let joined = parent.join(rel).to_string_lossy().to_string();
        add_module_candidates(&mut candidates, &mut seen, joined.clone());
        add_parent_module_candidates(&mut candidates, &mut seen, joined);
    } else {
        let local_joined = from_dir.join(&target_slash).to_string_lossy().to_string();
        add_module_candidates(&mut candidates, &mut seen, local_joined.clone());
        add_parent_module_candidates(&mut candidates, &mut seen, local_joined);

        let src_joined = format!("src/{target_slash}");
        add_module_candidates(&mut candidates, &mut seen, src_joined.clone());
        add_parent_module_candidates(&mut candidates, &mut seen, src_joined);
    }

    add_candidate(&mut candidates, &mut seen, target.clone());
    add_candidate(&mut candidates, &mut seen, target_slash.clone());
    add_module_candidates(&mut candidates, &mut seen, target_slash.clone());
    add_parent_module_candidates(&mut candidates, &mut seen, target_slash);

    candidates
}

fn add_candidate(candidates: &mut Vec<String>, seen: &mut HashSet<String>, path: String) {
    if path.is_empty() {
        return;
    }
    let normalized = path.replace('\\', "/");
    if seen.insert(normalized.clone()) {
        candidates.push(normalized);
    }
}

fn add_module_candidates(candidates: &mut Vec<String>, seen: &mut HashSet<String>, base: String) {
    if base.is_empty() {
        return;
    }
    for suffix in [
        ".rs",
        "/mod.rs",
        ".ts",
        ".tsx",
        ".js",
        ".jsx",
        ".py",
        ".go",
        ".java",
        ".php",
        ".rb",
        ".cs",
        ".c",
        ".cpp",
        "/index.ts",
        "/index.tsx",
        "/index.js",
        "/index.jsx",
    ] {
        add_candidate(candidates, seen, format!("{base}{suffix}"));
    }
}

fn add_parent_module_candidates(candidates: &mut Vec<String>, seen: &mut HashSet<String>, base: String) {
    let Some((parent, _)) = base.rsplit_once('/') else {
        return;
    };
    add_module_candidates(candidates, seen, parent.to_string());
}

fn normalize_import_target(raw_target: &str) -> Option<String> {
    let mut target = raw_target.trim().trim_end_matches(';').trim();

    if target.is_empty() {
        return None;
    }

    if let Some((left, _)) = target.split_once(" as ") {
        target = left.trim();
    }

    if let Some((left, _)) = target.split_once('{') {
        target = left.trim().trim_end_matches("::").trim();
    }

    if let Some((left, _)) = target.split_once(',') {
        target = left.trim();
    }

    if target.is_empty() {
        None
    } else {
        let mut parts = target
            .split("::")
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>();

        if let Some(last) = parts.last().copied() {
            let is_symbol_name = last == "*" || last.chars().next().is_some_and(char::is_uppercase);
            if is_symbol_name && parts.len() > 1 {
                parts.pop();
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("::"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{dependency_path_candidates, normalize_import_target};

    #[test]
    fn normalize_rust_use_targets() {
        assert_eq!(
            normalize_import_target("anyhow::{Context, Result};"),
            Some("anyhow".to_string())
        );
        assert_eq!(
            normalize_import_target("crate::db::Database"),
            Some("crate::db".to_string())
        );
        assert_eq!(
            normalize_import_target("parser::{parse_file, ExtractedSymbol}"),
            Some("parser".to_string())
        );
    }

    #[test]
    fn resolve_candidates_include_module_forms() {
        let candidates = dependency_path_candidates("src/main.rs", "crate::db::Database");
        assert!(candidates.iter().any(|c| c == "src/db/mod.rs"));

        let self_candidates = dependency_path_candidates("src/analyzer/mod.rs", "self::parser");
        assert!(self_candidates
            .iter()
            .any(|c| c == "src/analyzer/parser/mod.rs"));
    }

    #[test]
    fn resolve_candidates_include_parent_module_for_symbol_imports() {
        let candidates = dependency_path_candidates("src/commands/status.rs", "crate::watcher::watch_status");
        assert!(candidates.iter().any(|c| c == "src/watcher/mod.rs"));

        let local_candidates = dependency_path_candidates("src/watcher/mod.rs", "state::WatchState");
        assert!(local_candidates
            .iter()
            .any(|c| c == "src/watcher/state.rs"));
    }
}
