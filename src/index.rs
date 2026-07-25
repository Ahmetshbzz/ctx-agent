use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};

use crate::analyzer;
use crate::db::Database;
use crate::git;

pub struct IndexUpdate {
    pub analysis: analyzer::AnalysisResult,
    pub git: git::GitAnalysisResult,
    pub git_skipped: bool,
    pub generation: i64,
}

pub fn update_index(db: &Database, root: &Path) -> Result<IndexUpdate> {
    let started = Instant::now();
    db.with_immediate_transaction(|db| {
        let analysis = analyzer::analyze_project(db, root)?;
        let current_head = git::current_head(root);
        let git_key = current_head.as_deref().unwrap_or("__not_git__");
        let git_skipped = db.get_meta("git_head")?.as_deref() == Some(git_key);
        let git = if git_skipped {
            git::GitAnalysisResult {
                commits_analyzed: 0,
                files_with_stats: 0,
                decisions_found: 0,
                error: None,
            }
        } else {
            let result = git::analyze_git_history(db, root)?;
            db.set_meta("git_head", git_key)?;
            result
        };

        let generation = db
            .get_meta("index_generation")?
            .map(|value| value.parse::<i64>())
            .transpose()
            .context("Invalid index generation metadata")?
            .unwrap_or(0)
            + 1;
        db.set_meta("index_generation", &generation.to_string())?;
        db.set_meta(
            "last_scan_at_ms",
            &chrono::Utc::now().timestamp_millis().to_string(),
        )?;
        db.set_meta(
            "last_scan_duration_ms",
            &started.elapsed().as_millis().to_string(),
        )?;
        db.set_meta("last_scan_total_files", &analysis.total_files.to_string())?;
        db.set_meta(
            "last_scan_analyzed_files",
            &analysis.analyzed_files.to_string(),
        )?;
        db.set_meta(
            "last_scan_removed_files",
            &analysis.removed_files.to_string(),
        )?;

        Ok(IndexUpdate {
            analysis,
            git,
            git_skipped,
            generation,
        })
    })
}
