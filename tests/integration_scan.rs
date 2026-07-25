use anyhow::Result;
use ctx::{analyzer::analyze_project, Database};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempProject {
    base: PathBuf,
    root: PathBuf,
    store: PathBuf,
}

impl TempProject {
    fn new() -> Result<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let base =
            std::env::temp_dir().join(format!("ctx-integration-{}-{nonce}", std::process::id()));
        let root = base.join("project");
        let store = base.join("store");
        fs::create_dir_all(&root)?;
        Ok(Self { base, root, store })
    }

    fn write(&self, relative_path: &str, content: &str) -> Result<()> {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn scans_go_modules_incrementally_and_searches_safely() -> Result<()> {
    let project = TempProject::new()?;
    project.write("go.mod", "module example.test/app\n\ngo 1.26\n")?;
    project.write(
        "internal/models/model.go",
        "package models\n\ntype Customer struct{}\n",
    )?;
    project.write(
        "internal/models/model_test.go",
        "package models\n\nfunc TestCustomer() {}\n",
    )?;
    project.write(
        "cmd/api/main.go",
        r#"package main

import (
    "fmt"
    "example.test/app/internal/models"
)

func BuildCustomer() models.Customer {
    fmt.Println("building")
    return models.Customer{}
}
"#,
    )?;

    let db = Database::open_in(&project.root, project.store.clone())?;
    let first = analyze_project(&db, &project.root)?;
    assert_eq!(first.total_files, 4);
    assert_eq!(first.analyzed_files, 4);

    let source_id = db.get_file_id("cmd/api/main.go")?.expect("source file");
    let production_target = db
        .get_file_id("internal/models/model.go")?
        .expect("production target");
    let dependencies = db.get_dependencies_of(source_id)?;
    assert!(dependencies
        .iter()
        .any(|(target, path)| *target == Some(production_target)
            && path == "example.test/app/internal/models"));
    assert!(dependencies
        .iter()
        .any(|(target, path)| target.is_none() && path == "fmt"));
    assert!(db
        .search("BuildCustomer\"")?
        .iter()
        .any(|(name, _, _, _)| name == "BuildCustomer"));
    assert!(db.search("\"*():-")?.is_empty());
    assert!(db.search("OR NEAR (")?.is_empty());

    let second = analyze_project(&db, &project.root)?;
    assert_eq!(second.analyzed_files, 0);
    assert_eq!(second.skipped_files, second.total_files);

    fs::remove_file(project.root.join("internal/models/model.go"))?;
    let third = analyze_project(&db, &project.root)?;
    assert_eq!(third.removed_files, 1);
    let test_target = db
        .get_file_id("internal/models/model_test.go")?
        .expect("test fallback target");
    assert!(db
        .get_dependencies_of(source_id)?
        .iter()
        .any(|(target, path)| *target == Some(test_target)
            && path == "example.test/app/internal/models"));

    Ok(())
}
