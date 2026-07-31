#![allow(clippy::module_inception)]

#[cfg(test)]
mod tests {
    use crate::analyzer::parser::parse_file;
    use crate::db::models::SymbolKind;

    // =====================================================================
    // Rust parser tests
    // =====================================================================

    #[test]
    fn test_parse_rust_function() {
        let source = r#"fn hello(name: &str) -> String { format!("Hello, {}", name) }"#;
        let result = parse_file(source, "rust").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "hello");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Function));
        assert!(result.symbols[0].signature.contains("fn hello"));
        assert!(result.symbols[0].signature.contains("-> String"));
    }

    #[test]
    fn test_parse_rust_struct() {
        let source = r#"
struct Config {
    name: String,
    port: u16,
}
"#;
        let result = parse_file(source, "rust").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Config");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Struct));
    }

    #[test]
    fn test_parse_rust_enum() {
        let source = r#"
enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let result = parse_file(source, "rust").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Color");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Enum));
    }

    #[test]
    fn test_parse_rust_impl_methods() {
        let source = r#"
struct Foo;

impl Foo {
    fn new() -> Self { Foo }
    fn bar(&self) -> i32 { 42 }
}
"#;
        let result = parse_file(source, "rust").unwrap();
        // struct + 2 methods
        let methods: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| matches!(s.kind, SymbolKind::Method))
            .collect();
        assert_eq!(methods.len(), 2);
        assert!(methods[0].signature.contains("impl Foo"));
    }

    #[test]
    fn test_parse_rust_use_imports() {
        let source = r#"
use std::path::PathBuf;
use anyhow::{Context, Result};
"#;
        let result = parse_file(source, "rust").unwrap();
        assert_eq!(result.imports.len(), 2);
        assert_eq!(result.imports[0].path, "std::path::PathBuf");
        assert_eq!(result.imports[0].kind, "use");
    }

    #[test]
    fn test_parse_rust_trait() {
        let source = r#"
trait Drawable {
    fn draw(&self);
}
"#;
        let result = parse_file(source, "rust").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Drawable");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Interface));
        assert!(result.symbols[0].signature.contains("trait Drawable"));
    }

    #[test]
    fn test_parse_rust_const() {
        let source = r#"const MAX_SIZE: usize = 1024;"#;
        let result = parse_file(source, "rust").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "MAX_SIZE");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Constant));
    }

    #[test]
    fn test_parse_rust_mod() {
        let source = r#"mod utils;"#;
        let result = parse_file(source, "rust").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "utils");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Module));
    }

    // =====================================================================
    // TypeScript / JavaScript parser tests
    // =====================================================================

    #[test]
    fn test_parse_ts_function() {
        let source = r#"function greet(name: string): string { return "Hello, " + name; }"#;
        let result = parse_file(source, "typescript").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "greet");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Function));
    }

    #[test]
    fn test_parse_ts_class_with_methods() {
        let source = r#"
class UserService {
    constructor(db) {}
    getUser(id) { return null; }
    deleteUser(id) {}
}
"#;
        let result = parse_file(source, "javascript").unwrap();
        let class = result
            .symbols
            .iter()
            .find(|s| s.name == "UserService")
            .unwrap();
        assert!(matches!(class.kind, SymbolKind::Class));
        assert!(class.children.len() >= 2); // constructor + getUser + deleteUser
    }

    #[test]
    fn test_parse_ts_imports() {
        let source = r#"
import { readFile, writeFile } from 'fs';
import path from 'path';
"#;
        let result = parse_file(source, "javascript").unwrap();
        assert_eq!(result.imports.len(), 2);
        assert_eq!(result.imports[0].path, "fs");
        assert!(result.imports[0].names.contains(&"readFile".to_string()));
        assert_eq!(result.imports[1].path, "path");
    }

    #[test]
    fn test_parse_ts_arrow_function() {
        let source = r#"const add = (a, b) => a + b;"#;
        let result = parse_file(source, "javascript").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "add");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Function));
    }

    #[test]
    fn test_parse_ts_export_function() {
        let source = r#"export function getData() { return []; }"#;
        let result = parse_file(source, "javascript").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert!(result.symbols[0].signature.contains("export"));
    }

    #[test]
    fn test_parse_ts_interface() {
        let source = r#"interface Config { port: number; host: string; }"#;
        let result = parse_file(source, "typescript").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Config");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Interface));
    }

    #[test]
    fn test_parse_ts_enum() {
        let source = r#"enum Direction { Up, Down, Left, Right }"#;
        let result = parse_file(source, "typescript").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Direction");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Enum));
    }

    #[test]
    fn test_parse_ts_type_alias() {
        let source = r#"type ID = string | number;"#;
        let result = parse_file(source, "typescript").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "ID");
        assert!(matches!(result.symbols[0].kind, SymbolKind::TypeAlias));
    }

    // =====================================================================
    // Python parser tests
    // =====================================================================

    #[test]
    fn test_parse_python_function() {
        let source = r#"
def greet(name):
    return f"Hello, {name}"
"#;
        let result = parse_file(source, "python").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "greet");
        assert!(matches!(result.symbols[0].kind, SymbolKind::Function));
        assert!(result.symbols[0].signature.contains("def greet"));
    }

    #[test]
    fn test_parse_python_class_with_methods() {
        let source = r#"
class UserService:
    def __init__(self, db):
        self.db = db

    def get_user(self, id):
        return None
"#;
        let result = parse_file(source, "python").unwrap();
        let class = result
            .symbols
            .iter()
            .find(|s| s.name == "UserService")
            .unwrap();
        assert!(matches!(class.kind, SymbolKind::Class));
        assert_eq!(class.children.len(), 2); // __init__ + get_user
    }

    #[test]
    fn test_parse_python_imports() {
        let source = r#"
import os
from pathlib import Path
"#;
        let result = parse_file(source, "python").unwrap();
        assert_eq!(result.imports.len(), 2);
        assert_eq!(result.imports[0].path, "os");
        assert_eq!(result.imports[1].path, "pathlib");
    }

    #[test]
    fn test_parse_python_decorated_function() {
        let source = r#"
@app.route('/api/users')
def get_users():
    return []
"#;
        let result = parse_file(source, "python").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "get_users");
        assert!(result.symbols[0].signature.contains("@app.route"));
    }

    #[test]
    fn test_parse_python_return_type() {
        let source = r#"
def add(a: int, b: int) -> int:
    return a + b
"#;
        let result = parse_file(source, "python").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert!(result.symbols[0].signature.contains("-> int"));
    }

    #[test]
    fn test_parse_python_module_constant() {
        let source = r#"
MAX_RETRIES = 3

TOOL_SCHEMAS = [
    {"type": "function"},
    {"type": "function"},
]

TIMEOUT_SECONDS: int = 30
"#;
        let result = parse_file(source, "python").unwrap();
        assert_eq!(result.symbols.len(), 3);

        let retries = &result.symbols[0];
        assert_eq!(retries.name, "MAX_RETRIES");
        assert!(matches!(retries.kind, SymbolKind::Constant));
        assert!(retries.signature.contains("MAX_RETRIES = 3"));

        let schemas = &result.symbols[1];
        assert_eq!(schemas.name, "TOOL_SCHEMAS");
        assert!(matches!(schemas.kind, SymbolKind::Constant));
        assert!(schemas.signature.contains("[2 elements]"));

        let config = &result.symbols[2];
        assert_eq!(config.name, "TIMEOUT_SECONDS");
        assert!(config.signature.contains("TIMEOUT_SECONDS: int = 30"));
    }

    #[test]
    fn test_parse_python_ignores_lowercase_assignments() {
        let source = r#"
logger = structlog.get_logger()
_log = structlog.get_logger()
result = compute()
"#;
        let result = parse_file(source, "python").unwrap();
        assert_eq!(result.symbols.len(), 0);
    }

    #[test]
    fn test_parse_python_ignores_class_and_local_assignments() {
        let source = r#"
class Service:
    LIMIT = 10

    def run(self):
        LOCAL = 5
        return LOCAL
"#;
        let result = parse_file(source, "python").unwrap();
        // Only the class + its method; LIMIT/LOCAL are not module-level.
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Service");
        assert_eq!(result.symbols[0].children.len(), 1);
    }

    // =====================================================================
    // Edge cases
    // =====================================================================

    #[test]
    fn test_parse_unsupported_language() {
        let source = "def main\n  puts 'hello'\nend";
        let result = parse_file(source, "elixir").unwrap();
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
    }

    #[test]
    fn test_parse_empty_file() {
        let result = parse_file("", "rust").unwrap();
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
    }

    #[test]
    fn test_parse_invalid_syntax_no_crash() {
        let source = "fn { { { unclosed brackets";
        let result = parse_file(source, "rust");
        // Should not crash — tree-sitter is error-tolerant
        assert!(result.is_ok());
    }

    #[test]
    fn test_line_numbers_are_1_indexed() {
        let source = r#"
fn first() {}

fn second() {}
"#;
        let result = parse_file(source, "rust").unwrap();
        assert!(result.symbols.len() >= 2);
        // Line numbers should be 1-indexed (not 0-indexed)
        for sym in &result.symbols {
            assert!(
                sym.start_line >= 1,
                "start_line should be >= 1, got {}",
                sym.start_line
            );
        }
    }

    // =====================================================================
    // Go parser tests
    // =====================================================================

    #[test]
    fn test_parse_go_function() {
        let source = r#"
package main
import "fmt"

func greet(name string) string {
    return fmt.Sprintf("Hello, %s", name)
}
"#;
        let result = parse_file(source, "go").unwrap();

        let func_sym = result.symbols.iter().find(|s| s.name == "greet").unwrap();
        assert!(matches!(func_sym.kind, SymbolKind::Function));
        assert!(func_sym.signature.contains("func greet"));

        let pkg_sym = result.symbols.iter().find(|s| s.name == "main").unwrap();
        assert!(matches!(pkg_sym.kind, SymbolKind::Module));

        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].path, "fmt");
    }

    #[test]
    fn test_parse_go_struct_and_methods() {
        let source = r#"
package main

type User struct {
    Name  string
    Email string
}

func (u *User) GetName() string {
    return u.Name
}
"#;
        let result = parse_file(source, "go").unwrap();

        let struct_sym = result.symbols.iter().find(|s| s.name == "User").unwrap();
        assert!(matches!(struct_sym.kind, SymbolKind::Struct));
        // Fields are extracted as children
        assert!(struct_sym.children.len() >= 2);

        let method_sym = result.symbols.iter().find(|s| s.name == "GetName").unwrap();
        assert!(matches!(method_sym.kind, SymbolKind::Method));
        assert!(method_sym.signature.contains("(u *User)"));
    }

    #[test]
    fn test_parse_go_interface() {
        let source = r#"
package main

type Reader interface {
    Read(p []byte) (n int, err error)
    Close() error
}
"#;
        let result = parse_file(source, "go").unwrap();

        let iface_sym = result.symbols.iter().find(|s| s.name == "Reader").unwrap();
        assert!(matches!(iface_sym.kind, SymbolKind::Interface));
        assert_eq!(iface_sym.children.len(), 2); // Read, Close
    }

    #[test]
    fn test_parse_go_variables_and_constants() {
        let source = r#"
package main

const Version = "1.0.0"
var Debug = false
"#;
        let result = parse_file(source, "go").unwrap();

        let const_sym = result.symbols.iter().find(|s| s.name == "Version").unwrap();
        assert!(matches!(const_sym.kind, SymbolKind::Constant));

        let var_sym = result.symbols.iter().find(|s| s.name == "Debug").unwrap();
        assert!(matches!(var_sym.kind, SymbolKind::Constant));
    }

    #[test]
    fn test_parse_go_imports() {
        let source = r#"
package main

import (
    "fmt"
    t "time"
    _ "github.com/lib/pq"
    . "context"
)
"#;
        let result = parse_file(source, "go").unwrap();
        assert_eq!(result.imports.len(), 4);

        let fmt = result.imports.iter().find(|i| i.path == "fmt").unwrap();
        assert_eq!(fmt.names[0], "fmt");

        let time = result.imports.iter().find(|i| i.path == "time").unwrap();
        assert_eq!(time.names[0], "t");

        let pq = result
            .imports
            .iter()
            .find(|i| i.path == "github.com/lib/pq")
            .unwrap();
        assert_eq!(pq.names[0], "_");

        let ctx = result.imports.iter().find(|i| i.path == "context").unwrap();
        assert_eq!(ctx.names[0], "*");
    }
}
