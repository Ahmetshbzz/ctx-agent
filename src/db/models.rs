use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// A tracked file in the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFile {
    pub id: i64,
    pub path: String,
    pub language: String,
    pub size_bytes: i64,
    pub hash: String,
    pub line_count: i64,
    pub last_analyzed: String,
}

/// Kind of symbol extracted from source code
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Enum,
    Constant,
    TypeAlias,
    Module,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Interface => "interface",
            Self::Enum => "enum",
            Self::Constant => "constant",
            Self::TypeAlias => "type_alias",
            Self::Module => "module",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "function" => Self::Function,
            "method" => Self::Method,
            "class" => Self::Class,
            "struct" => Self::Struct,
            "interface" => Self::Interface,
            "enum" => Self::Enum,
            "constant" => Self::Constant,
            "type_alias" => Self::TypeAlias,
            "module" => Self::Module,
            _ => Self::Function,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Function | Self::Method => "ƒ",
            Self::Class => "C",
            Self::Struct => "S",
            Self::Interface => "I",
            Self::Enum => "E",
            Self::Constant => "K",
            Self::TypeAlias => "T",
            Self::Module => "M",
        }
    }
}

impl FromStr for SymbolKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "method" => Ok(Self::Method),
            "class" => Ok(Self::Class),
            "struct" => Ok(Self::Struct),
            "interface" => Ok(Self::Interface),
            "enum" => Ok(Self::Enum),
            "constant" => Ok(Self::Constant),
            "type_alias" => Ok(Self::TypeAlias),
            "module" => Ok(Self::Module),
            _ => Err(()),
        }
    }
}

/// A code symbol (function, class, struct, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: i64,
    pub file_id: i64,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: i64,
    pub end_line: i64,
    pub signature: String,
    pub parent_symbol_id: Option<i64>,
}

/// A dependency between two files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub id: i64,
    pub from_file_id: i64,
    pub to_path: String,
    pub to_file_id: Option<i64>,
    pub kind: String,
    pub imported_names: String,
}

/// A recorded decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: i64,
    pub timestamp: String,
    pub description: String,
    pub source: String,
    pub commit_hash: Option<String>,
    pub related_files: String,
}

/// A knowledge note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Knowledge {
    pub id: i64,
    pub content: String,
    pub source: String,
    pub related_file: Option<String>,
    pub timestamp: String,
}

/// Git file stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub file_id: i64,
    pub commit_count: i64,
    pub last_modified: Option<String>,
    pub churn_score: f64,
    pub contributors: i64,
}

/// File health metrics for warnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHealth {
    pub path: String,
    pub language: String,
    pub line_count: i64,
    pub commit_count: i64,
    pub churn_score: f64,
    pub dependents_count: i64,
    pub symbol_count: i64,
    pub outgoing_dependencies_count: i64,
    pub is_fragile: bool,
    pub is_dead: bool,
}
