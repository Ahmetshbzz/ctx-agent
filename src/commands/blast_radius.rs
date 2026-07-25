use super::*;

pub(super) fn cmd_blast_radius(root: &Path, path: &str, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;

    if json_mode {
        let file_id = match db.get_file_id(path)? {
            Some(id) => id,
            None => {
                println!(
                    "{}",
                    json!({
                        "command": "blast-radius",
                        "error": format!("File not found: {}", path),
                    })
                );
                return Ok(());
            }
        };

        let deps = db.get_dependencies_of(file_id)?;
        let dependents = db.get_dependents(file_id)?;

        let dep_list: Vec<_> = deps
            .iter()
            .map(|(_, dep_path)| {
                json!({
                    "target": dep_path,
                })
            })
            .collect();

        let dep_of_list: Vec<_> = dependents
            .iter()
            .map(|(_, dep_path)| {
                json!({
                    "source": dep_path,
                })
            })
            .collect();

        // Transitive blast radius
        let radius = ctx::analyzer::graph::blast_radius(&db, file_id)?;
        let radius_list: Vec<_> = radius
            .iter()
            .map(|(_, rpath, depth)| {
                json!({
                    "path": rpath,
                    "depth": depth,
                })
            })
            .collect();

        let risk = match radius.len() {
            0 => "low",
            1..=5 => "medium",
            6..=20 => "high",
            _ => "critical",
        };

        println!(
            "{}",
            json!({
                "command": "blast-radius",
                "file": path,
                "imports": dep_list,
                "imported_by": dep_of_list,
                "transitive_impact": radius_list,
                "risk": risk,
            })
        );
    } else {
        ctx::query::execute_blast_radius(&db, path)?;
        println!();
    }

    Ok(())
}
