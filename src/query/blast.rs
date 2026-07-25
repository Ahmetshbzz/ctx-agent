use crate::analyzer::graph;
use crate::db::Database;
use anyhow::Result;
use colored::*;

/// Execute blast-radius analysis and display results
pub fn execute_blast_radius(db: &Database, file_path: &str) -> Result<()> {
    let file_id = match db.get_file_id(file_path)? {
        Some(id) => id,
        None => {
            println!("  {} File not found: {}", "ERROR".red(), file_path);
            return Ok(());
        }
    };

    // Direct dependencies
    let deps = db.get_dependencies_of(file_id)?.into_iter().fold(
        Vec::<(Option<i64>, String)>::new(),
        |mut acc: Vec<(Option<i64>, String)>, item| {
            if !acc.iter().any(|existing| existing == &item) {
                acc.push(item);
            }
            acc
        },
    );
    let dependents = db.get_dependents(file_id)?.into_iter().fold(
        Vec::<(i64, String)>::new(),
        |mut acc: Vec<(i64, String)>, item| {
            if !acc.iter().any(|existing| existing == &item) {
                acc.push(item);
            }
            acc
        },
    );
    let sibling_modules = if let Some(parent) = file_path.rsplit_once('/') {
        db.get_all_files()?
            .into_iter()
            .filter(|file| {
                file.path != file_path && file.path.starts_with(&format!("{}/", parent.0))
            })
            .map(|file| file.path)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    println!(
        "\n  {} {}\n",
        "Blast Radius:".yellow().bold(),
        file_path.white().bold()
    );

    // Show what this file depends on
    if !deps.is_empty() {
        println!(
            "  {} {} dependencies (this file imports):",
            "←".blue(),
            deps.len().to_string().cyan()
        );
        for (_, path) in &deps {
            println!("    {} {}", "←".dimmed(), path);
        }
        println!();
    }

    // Show direct dependents
    if !dependents.is_empty() {
        println!(
            "  {} {} direct dependents (files that import this):",
            "→".green(),
            dependents.len().to_string().cyan()
        );
        for (_, path) in &dependents {
            println!("    {} {}", "→".dimmed(), path);
        }
        println!();
    }

    if !sibling_modules.is_empty() {
        println!(
            "  {} {} same-directory sibling modules (likely co-change surface):",
            "≈".yellow(),
            sibling_modules.len().to_string().cyan()
        );
        for path in sibling_modules.iter().take(12) {
            println!("    {} {}", "≈".dimmed(), path);
        }
        if sibling_modules.len() > 12 {
            println!(
                "    {} ... and {} more",
                "≈".dimmed(),
                sibling_modules.len() - 12
            );
        }
        println!();
    }

    // Show transitive blast radius
    let radius = graph::blast_radius(db, file_id)?;
    if !radius.is_empty() {
        let max_depth = radius.iter().map(|r| r.2).max().unwrap_or(0);
        println!(
            "  {} {} total files in blast radius (depth {}):",
            "IMPACT".red(),
            radius.len().to_string().red().bold(),
            max_depth.to_string().yellow()
        );
        for (_, path, depth) in &radius {
            let indent = "  ".repeat(*depth);
            let marker = if *depth == 1 { "→" } else { "↳" };
            println!("    {}{} {}", indent, marker.dimmed(), path);
        }
        println!();

        // Risk assessment
        let effective_radius = radius.len() + sibling_modules.len();
        let risk = if effective_radius > 20 {
            "CRITICAL".red().bold()
        } else if effective_radius > 10 {
            "HIGH".red()
        } else if effective_radius > 5 {
            "MEDIUM".yellow()
        } else {
            "LOW".green()
        };
        println!("  Risk: {}", risk);
    } else if dependents.is_empty() {
        if sibling_modules.is_empty() {
            println!(
                "  {} No files depend on this file (leaf node)",
                "OK".green()
            );
        } else {
            println!(
                "  {} No direct import dependents found, but nearby sibling modules suggest a broader co-change surface",
                "INFO".yellow()
            );
        }
    }

    Ok(())
}
