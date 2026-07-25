use super::{node_text, ExtractedImport, ExtractedSymbol};
use crate::db::models::SymbolKind;
use tree_sitter::Node;

pub fn extract_scripting(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
    language: &str, // "php", "ruby", "bash"
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            // ── PHP ────────────────────────────────────────────────────
            "class_declaration" | "interface_declaration" | "trait_declaration" => {
                if language == "php" {
                    if let Some(sym) = extract_php_class(child, source) {
                        symbols.push(sym);
                    }
                }
            }
            "function_definition" => {
                if language == "php" {
                    if let Some(sym) = extract_php_func(child, source) {
                        symbols.push(sym);
                    }
                } else if language == "bash" {
                    if let Some(sym) = extract_bash_func(child, source) {
                        symbols.push(sym);
                    }
                }
            }
            "namespace_definition" => {
                if language == "php" {
                    extract_php_namespace(child, source, symbols, imports);
                }
            }

            // ── Ruby ───────────────────────────────────────────────────
            "class" | "module" => {
                if language == "ruby" {
                    if let Some(sym) = extract_ruby_class(child, source) {
                        symbols.push(sym);
                    }
                }
            }
            "method" | "singleton_method" => {
                if language == "ruby" {
                    if let Some(sym) = extract_ruby_method(child, source) {
                        symbols.push(sym);
                    }
                }
            }
            "call" if language == "ruby" => {
                extract_ruby_require(child, source, imports);
            }

            // ── Bash ───────────────────────────────────────────────────
            // function_definition handled above
            _ => {}
        }
    }
}

// ── PHP Helpers ────────────────────────────────────────────────────────

fn extract_php_class(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    let kind = match node.kind() {
        "class_declaration" => SymbolKind::Class,
        "interface_declaration" => SymbolKind::Interface,
        "trait_declaration" => SymbolKind::Class, // Trait kind? Use Class for now
        _ => SymbolKind::Class,
    };

    let signature = format!("{} {}", node.kind().replace("_declaration", ""), name);

    let mut children = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "method_declaration" {
                if let Some(method) = extract_php_func(child, source) {
                    // Reusing func extractor for methods roughly
                    let mut m = method;
                    m.kind = SymbolKind::Method;
                    children.push(m);
                }
            }
        }
    }

    Some(ExtractedSymbol {
        name,
        kind,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children,
    })
}

fn extract_php_func(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let signature = format!("function {}", name);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Function,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn extract_php_namespace(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let name_node = node.child_by_field_name("name");
    let name = name_node
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "namespace".to_string());

    let mut inner_symbols = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        extract_scripting(body, source, &mut inner_symbols, imports, "php");
    }

    symbols.push(ExtractedSymbol {
        name,
        kind: SymbolKind::Module,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: "namespace".to_string(),
        children: inner_symbols,
    });
}

// ── Ruby Helpers ───────────────────────────────────────────────────────

fn extract_ruby_class(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    let kind = if node.kind() == "module" {
        SymbolKind::Module
    } else {
        SymbolKind::Class
    };

    let signature = format!("{} {}", node.kind(), name);

    let mut children = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "method" | "singleton_method" => {
                    if let Some(m) = extract_ruby_method(child, source) {
                        children.push(m);
                    }
                }
                "class" | "module" => {
                    if let Some(inner) = extract_ruby_class(child, source) {
                        children.push(inner);
                    }
                }
                _ => {}
            }
        }
    }

    Some(ExtractedSymbol {
        name,
        kind,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children,
    })
}

fn extract_ruby_method(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let signature = format!("def {}", name);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Method,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn extract_ruby_require(node: Node, source: &[u8], imports: &mut Vec<ExtractedImport>) {
    let method = node.child_by_field_name("method");
    if let Some(m) = method {
        let name = node_text(m, source);
        if name == "require" || name == "require_relative" || name == "include" || name == "extend"
        {
            if let Some(args) = node.child_by_field_name("arguments") {
                let path = node_text(args, source)
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
                imports.push(ExtractedImport {
                    path,
                    kind: name,
                    names: vec![],
                })
            }
        }
    }
}

// ── Bash Helpers ───────────────────────────────────────────────────────

fn extract_bash_func(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let signature = format!("function {}", name);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Function,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}
