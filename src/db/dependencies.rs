use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::core::Database;

#[derive(Debug, Eq, PartialEq)]
struct GoModule {
    import_path: String,
    relative_root: String,
}

struct FileIndexes {
    by_path: HashMap<String, i64>,
    go_packages: HashMap<String, i64>,
    go_mod_paths: Vec<String>,
}

impl Database {
    pub fn clear_dependencies(&self, file_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM dependencies WHERE from_file_id = ?1",
            [file_id],
        )?;
        Ok(())
    }

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

    pub fn resolve_dependencies(&self) -> Result<()> {
        let unresolved = self.unresolved_dependencies()?;
        if unresolved.is_empty() {
            return Ok(());
        }
        let indexes = self.file_indexes()?;
        let go_modules = discover_go_modules(&self.project_root, &indexes.go_mod_paths)?;

        for (dep_id, to_path, from_path, language) in unresolved {
            let go_target = (language == "go")
                .then(|| resolve_go_import(&to_path, &go_modules, &indexes.go_packages))
                .flatten();
            let target_id = go_target.or_else(|| {
                dependency_path_candidates(&from_path, &to_path)
                    .into_iter()
                    .find_map(|candidate| indexes.by_path.get(&candidate).copied())
            });

            if let Some(target_id) = target_id {
                self.conn.execute(
                    "UPDATE dependencies SET to_file_id = ?1 WHERE id = ?2",
                    rusqlite::params![target_id, dep_id],
                )?;
            }
        }
        Ok(())
    }

    pub fn get_dependents(&self, file_id: i64) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT f.id, f.path FROM dependencies d
             JOIN files f ON f.id = d.from_file_id
             WHERE d.to_file_id = ?1 AND f.id != ?1",
        )?;
        let rows = stmt.query_map([file_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_dependencies_of(&self, file_id: i64) -> Result<Vec<(Option<i64>, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.to_file_id, d.to_path FROM dependencies d WHERE d.from_file_id = ?1",
        )?;
        let rows = stmt.query_map([file_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn count_dependencies(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))?)
    }

    fn unresolved_dependencies(&self) -> Result<Vec<(i64, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.id, d.to_path, f.path, f.language
             FROM dependencies d
             JOIN files f ON f.id = d.from_file_id
             WHERE d.to_file_id IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn file_indexes(&self) -> Result<FileIndexes> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, language FROM files ORDER BY path")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut files_by_path = HashMap::new();
        let mut go_packages: HashMap<String, (i64, bool)> = HashMap::new();
        let mut go_mod_paths = Vec::new();
        for (id, path, language) in rows.collect::<rusqlite::Result<Vec<_>>>()? {
            files_by_path.insert(path.clone(), id);
            if language == "gomod" {
                go_mod_paths.push(path);
                continue;
            }
            if language != "go" {
                continue;
            }

            let package = Path::new(&path)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_string_lossy()
                .replace('\\', "/");
            let is_test = path.ends_with("_test.go");
            match go_packages.get(&package) {
                Some((_, existing_is_test)) if !existing_is_test || is_test => {}
                _ => {
                    go_packages.insert(package, (id, is_test));
                }
            }
        }

        Ok(FileIndexes {
            by_path: files_by_path,
            go_packages: go_packages
                .into_iter()
                .map(|(package, (id, _))| (package, id))
                .collect(),
            go_mod_paths,
        })
    }
}

fn discover_go_modules(project_root: &Path, go_mod_paths: &[String]) -> Result<Vec<GoModule>> {
    let mut modules = Vec::new();
    for relative_path in go_mod_paths {
        let path = project_root.join(relative_path);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let Some(import_path) = parse_go_module_path(&content) else {
            continue;
        };
        let relative_root = Path::new(relative_path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
            .replace('\\', "/");
        modules.push(GoModule {
            import_path,
            relative_root,
        });
    }
    modules.sort_by_key(|module| std::cmp::Reverse(module.import_path.len()));
    Ok(modules)
}

fn parse_go_module_path(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = line.split("//").next()?.trim();
        let mut parts = line.split_whitespace();
        (parts.next()? == "module")
            .then(|| parts.next().map(str::to_owned))
            .flatten()
    })
}

fn resolve_go_import(
    raw_target: &str,
    modules: &[GoModule],
    packages: &HashMap<String, i64>,
) -> Option<i64> {
    let target = raw_target.trim().trim_matches('"');
    let module = modules.iter().find(|module| {
        target == module.import_path
            || target
                .strip_prefix(&module.import_path)
                .is_some_and(|rest| rest.starts_with('/'))
    })?;
    let suffix = target
        .strip_prefix(&module.import_path)?
        .trim_start_matches('/');
    let package = match (module.relative_root.is_empty(), suffix.is_empty()) {
        (true, _) => suffix.to_string(),
        (_, true) => module.relative_root.clone(),
        _ => format!("{}/{}", module.relative_root, suffix),
    };
    packages.get(&package).copied()
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

fn add_parent_module_candidates(
    candidates: &mut Vec<String>,
    seen: &mut HashSet<String>,
    base: String,
) {
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
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();

        if let Some(last) = parts.last().copied() {
            let is_symbol_name = last == "*" || last.chars().next().is_some_and(char::is_uppercase);
            if is_symbol_name && parts.len() > 1 {
                parts.pop();
            }
        }

        (!parts.is_empty()).then(|| parts.join("::"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        dependency_path_candidates, normalize_import_target, parse_go_module_path,
        resolve_go_import, GoModule,
    };
    use std::collections::HashMap;

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
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "src/db/mod.rs"));

        let self_candidates = dependency_path_candidates("src/analyzer/mod.rs", "self::parser");
        assert!(self_candidates
            .iter()
            .any(|candidate| candidate == "src/analyzer/parser/mod.rs"));
    }

    #[test]
    fn resolve_candidates_include_parent_module_for_symbol_imports() {
        let candidates =
            dependency_path_candidates("src/commands/status.rs", "crate::watcher::watch_status");
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "src/watcher/mod.rs"));

        let local_candidates =
            dependency_path_candidates("src/watcher/mod.rs", "state::WatchState");
        assert!(local_candidates
            .iter()
            .any(|candidate| candidate == "src/watcher/state.rs"));
    }

    #[test]
    fn parses_go_module_directive() {
        assert_eq!(
            parse_go_module_path("// comment\nmodule example.test/app // main\n\ngo 1.26\n"),
            Some("example.test/app".to_string())
        );
        assert_eq!(parse_go_module_path("go 1.26\n"), None);
    }

    #[test]
    fn resolves_longest_matching_go_module() {
        let modules = vec![
            GoModule {
                import_path: "example.test/app/internal".to_string(),
                relative_root: "nested".to_string(),
            },
            GoModule {
                import_path: "example.test/app".to_string(),
                relative_root: "backend".to_string(),
            },
        ];
        let packages = HashMap::from([
            ("nested/feature".to_string(), 7),
            ("backend/internal/feature".to_string(), 9),
        ]);

        assert_eq!(
            resolve_go_import("example.test/app/internal/feature", &modules, &packages),
            Some(7)
        );
        assert_eq!(resolve_go_import("fmt", &modules, &packages), None);
    }
}
