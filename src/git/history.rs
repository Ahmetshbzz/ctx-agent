use anyhow::{Context, Result};
use git2::{Repository, Sort};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::db::Database;

pub fn current_head(project_root: &Path) -> Option<String> {
    Repository::open(project_root)
        .ok()?
        .head()
        .ok()?
        .target()
        .map(|oid| oid.to_string())
}

/// Stats accumulated per file from git history
#[derive(Debug, Default)]
struct GitFileStats {
    commit_count: i64,
    last_modified: Option<String>,
    contributors: HashSet<String>,
}

/// Analyze git history and populate file_stats + decisions
pub fn analyze_git_history(db: &Database, project_root: &Path) -> Result<GitAnalysisResult> {
    let repo = match Repository::open(project_root) {
        Ok(r) => r,
        Err(_) => {
            return Ok(GitAnalysisResult {
                commits_analyzed: 0,
                files_with_stats: 0,
                decisions_found: 0,
                error: Some("Not a git repository".to_string()),
            });
        }
    };

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TIME)?;
    revwalk
        .push_head()
        .context("Failed to push HEAD to revwalk")?;

    let mut file_stats: HashMap<String, GitFileStats> = HashMap::new();
    let mut decisions_found = 0;
    let mut commits_analyzed = 0;

    // Limit to last 1000 commits for performance
    let max_commits = 1000;

    for oid_result in revwalk {
        if commits_analyzed >= max_commits {
            break;
        }

        let oid = match oid_result {
            Ok(o) => o,
            Err(_) => continue,
        };

        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        commits_analyzed += 1;

        let author = commit.author().name().unwrap_or("unknown").to_string();
        let time = commit.time();
        let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string());

        let message = commit.message().unwrap_or("").to_string();

        // Get changed files by diffing with parent
        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let mut changed_files = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    let path_str = path.to_string_lossy().to_string();
                    changed_files.push(path_str.clone());

                    let stats = file_stats.entry(path_str).or_default();
                    stats.commit_count += 1;
                    stats.contributors.insert(author.clone());
                    if stats.last_modified.is_none() {
                        stats.last_modified = timestamp.clone();
                    }
                }
                true
            },
            None,
            None,
            None,
        )
        .ok();

        // Detect decisions from commit messages
        // Conventional commits with "feat:", "fix:", "refactor:", "breaking:" etc.
        let is_decision = message.starts_with("feat:")
            || message.starts_with("feat(")
            || message.starts_with("refactor:")
            || message.starts_with("refactor(")
            || message.contains("BREAKING")
            || message.contains("migration")
            || message.contains("replace")
            || message.contains("switch to")
            || message.contains("switch from");

        if is_decision && !message.is_empty() {
            let related =
                serde_json::to_string(&changed_files).unwrap_or_else(|_| "[]".to_string());
            if db
                .insert_decision(message.trim(), "commit", Some(&oid.to_string()), &related)
                .unwrap_or(false)
            {
                decisions_found += 1;
            }
        }
    }

    // Store file stats
    let mut files_with_stats = 0;
    let max_commit_count = file_stats
        .values()
        .map(|s| s.commit_count)
        .max()
        .unwrap_or(1) as f64;

    for (path, stats) in &file_stats {
        if let Ok(Some(file_id)) = db.get_file_id(path) {
            let churn_score = stats.commit_count as f64 / max_commit_count;
            db.upsert_file_stats(
                file_id,
                stats.commit_count,
                stats.last_modified.as_deref(),
                churn_score,
                stats.contributors.len() as i64,
            )?;
            files_with_stats += 1;
        }
    }

    Ok(GitAnalysisResult {
        commits_analyzed,
        files_with_stats,
        decisions_found,
        error: None,
    })
}

/// Result of git history analysis
#[derive(Debug)]
pub struct GitAnalysisResult {
    pub commits_analyzed: usize,
    pub files_with_stats: usize,
    pub decisions_found: usize,
    pub error: Option<String>,
}
