pub mod analyzer;
pub mod db;
pub mod git;
pub mod index;
pub mod query;
pub mod watcher;

// Re-export core types
pub use db::Database;
