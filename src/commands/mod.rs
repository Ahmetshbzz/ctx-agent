use anyhow::{Context, Result};
use colored::*;
use serde_json::json;
use std::path::Path;

use ctx::db::Database;
use ctx::index::update_index;
use ctx::watcher;

use crate::cli::Commands;

mod blast_radius;
mod decisions;
mod ensure_watch;
mod grep;
mod health;
mod init;
mod learn;
mod map;
mod query;
mod scan;
mod status;
mod warnings;
mod watch;
mod watch_status;

pub fn run(command: Commands, root: &Path, json_mode: bool) -> Result<()> {
    let skip_auto_watch = matches!(
        &command,
        Commands::Watch | Commands::EnsureWatch | Commands::WatchStatus | Commands::Health
    );

    match command {
        Commands::Init => init::cmd_init(root, json_mode)?,
        Commands::Scan => scan::cmd_scan(root, json_mode)?,
        Commands::Map => map::cmd_map(root, json_mode)?,
        Commands::Status => status::cmd_status(root, json_mode)?,
        Commands::Health => health::cmd_health(root, json_mode)?,
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
        Commands::EnsureWatch => ensure_watch::cmd_ensure_watch(root, json_mode)?,
        Commands::WatchStatus => watch_status::cmd_watch_status(root, json_mode)?,
    }

    let auto_watch_disabled = std::env::var_os("CTX_AGENT_NO_WATCH").is_some();
    if !skip_auto_watch && !auto_watch_disabled {
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
