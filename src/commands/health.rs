use super::*;

const HEALTH_SCHEMA_VERSION: u32 = 1;

pub(super) fn cmd_health(root: &Path, json_mode: bool) -> Result<()> {
    let project_root = Database::canonical_project_root(root);

    if !Database::exists(&project_root) {
        let health = json!({
            "command": "health",
            "schema_version": HEALTH_SCHEMA_VERSION,
            "project_root": project_root,
            "initialized": false,
        });
        println!("{health}");
        return Ok(());
    }

    let db = Database::open(&project_root)?;
    let generation = meta_i64(&db, "index_generation")?.unwrap_or(0);
    let last_scan_at_ms = meta_i64(&db, "last_scan_at_ms")?;
    let last_scan_duration_ms = meta_i64(&db, "last_scan_duration_ms")?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let age_ms = last_scan_at_ms.map(|timestamp| now_ms.saturating_sub(timestamp));
    let files = db.count_files()?;
    let symbols = db.count_symbols()?;
    let dependencies = db.count_dependencies()?;
    let health = json!({
        "command": "health",
        "schema_version": HEALTH_SCHEMA_VERSION,
        "project_root": project_root,
        "initialized": generation > 0,
        "index_generation": generation,
        "last_scan_at_ms": last_scan_at_ms,
        "last_scan_duration_ms": last_scan_duration_ms,
        "age_ms": age_ms,
        "git_head": db.get_meta("git_head")?,
        "files": files,
        "symbols": symbols,
        "dependencies": dependencies,
    });

    if json_mode {
        println!("{health}");
    } else {
        println!(
            "  {} generation {}, {} files, {} symbols, {} dependencies",
            "OK".green(),
            generation,
            files,
            symbols,
            dependencies
        );
    }

    Ok(())
}

fn meta_i64(db: &Database, key: &str) -> Result<Option<i64>> {
    db.get_meta(key)?
        .map(|value| {
            value
                .parse::<i64>()
                .with_context(|| format!("Invalid integer metadata for {key}"))
        })
        .transpose()
}
