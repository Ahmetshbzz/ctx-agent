use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ctx-agent",
    version,
    about = "Agent Context Protocol — Structured codebase intelligence for AI agents",
    long_about = "Live codebase intelligence for AI agents. Zero dependencies, local-first, offline-capable."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Project root directory (defaults to current directory)
    #[arg(short, long, global = true)]
    pub project: Option<PathBuf>,

    /// Output in JSON format (for agent consumption)
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize ctx-agent in the current project
    Init,

    /// Scan/re-scan the project
    Scan,

    /// Display codebase map with structure and stats
    Map,

    /// Show project status dashboard
    Status,

    /// Search symbols and files
    Query {
        /// Search term
        term: String,
    },

    /// Search raw text in repository files (ripgrep-like, built-in)
    Grep {
        /// Text or regex pattern
        pattern: String,

        /// Maximum results to return
        #[arg(short, long, default_value_t = 60)]
        max_results: usize,
    },

    /// Show blast radius of changing a file
    BlastRadius {
        /// File path (relative to project root)
        path: String,
    },

    /// Show recorded decisions
    Decisions,

    /// Add a knowledge note
    Learn {
        /// Note content
        note: String,

        /// Related file (optional)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Show warnings (fragile files, dead code)
    Warnings,

    /// Watch for file changes and re-analyze
    Watch,
}
