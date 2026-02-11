use anyhow::Result;
use colored::*;
use serde_json::json;
use std::path::Path;
use std::time::Instant;

use ctx::analyzer;
use ctx::db::Database;
use ctx::git;
use ctx::watcher;

use crate::cli::Commands;

mod blast_radius;
mod decisions;
mod grep;
mod init;
mod learn;
mod map;
mod query;
mod scan;
mod status;
mod warnings;
mod watch;

pub fn run(command: Commands, root: &Path, json_mode: bool) -> Result<()> {
    let is_watch_command = matches!(&command, Commands::Watch);

    match command {
        Commands::Init => init::cmd_init(root, json_mode)?,
        Commands::Scan => scan::cmd_scan(root, json_mode)?,
        Commands::Map => map::cmd_map(root, json_mode)?,
        Commands::Status => status::cmd_status(root, json_mode)?,
        Commands::Query { term } => query::cmd_query(root, &term, json_mode)?,
        Commands::Grep {
            pattern,
            max_results,
        } => grep::cmd_grep(root, &pattern, max_results, json_mode)?,
        Commands::BlastRadius { path } => blast_radius::cmd_blast_radius(root, &path, json_mode)?,
        Commands::Decisions => decisions::cmd_decisions(root, json_mode)?,
        Commands::Learn { note, file } => {
            learn::cmd_learn(root, &note, file.as_deref(), json_mode)?
        }
        Commands::Warnings => warnings::cmd_warnings(root, json_mode)?,
        Commands::Watch => watch::cmd_watch(root)?,
    }

    // Agent-first default: keep context fresh in background unless this invocation is already `watch`.
    if !is_watch_command {
        watcher::ensure_background_watch(root).ok();
    }

    Ok(())
}

fn ensure_initialized(root: &Path) -> Result<Database> {
    if !Database::exists(root) {
        anyhow::bail!(
            "ctx-agent is not initialized in this project.\nRun {} first.",
            "ctx-agent init".cyan()
        );
    }
    Database::open(root)
}
