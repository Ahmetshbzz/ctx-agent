use tree_sitter::Node;

use super::{node_text, ExtractedImport, ExtractedSymbol};
use crate::db::models::SymbolKind;

// ===========================================================================
// Python extractor
// ===========================================================================

pub fn extract_python(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "decorated_definition" => {
                // Handle decorated functions/classes
                extract_decorated(child, source, symbols, imports);
            }
            "function_definition" => {
                if let Some(sym) = extract_python_function(child, source) {
                    symbols.push(sym);
                }
            }
            "class_definition" => {
                if let Some(sym) = extract_python_class(child, source) {
                    symbols.push(sym);
                }
            }
            "import_statement" | "import_from_statement" => {
                if let Some(imp) = extract_python_import(child, source) {
                    imports.push(imp);
                }
            }
            // Module-level assignments capture constants/configs such as
            // TOOL_SCHEMAS = [...] or MAX_RETRIES = 3 that agents query for.
            "expression_statement" => {
                if let Some(sym) = extract_python_module_constant(child, source) {
                    symbols.push(sym);
                }
            }
            _ => {}
        }
    }
}

/// Extract a module-level constant from an expression-statement assignment.
///
/// Only ALL_CAPS-style names (optionally with digits/underscores, must contain
/// at least one letter) are treated as constants; lowercase assignments like
/// plain logger/result bindings are ignored to keep the index noise-free.
fn extract_python_module_constant(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let mut cursor = node.walk();
    let assignment = node
        .children(&mut cursor)
        .find(|c| c.kind() == "assignment")?;
    let left = assignment.child_by_field_name("left")?;
    if left.kind() != "identifier" {
        return None;
    }
    let name = node_text(left, source);
    let is_constant = name.chars().any(|c| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !is_constant {
        return None;
    }

    let type_ann = assignment
        .child_by_field_name("type")
        .map(|t| node_text(t, source).to_string())
        .map(|t| if t.starts_with(':') { t } else { format!(": {}", t) })
        .unwrap_or_default();
    let value_node = assignment.child_by_field_name("right")?;
    let value_preview = value_preview(source, &value_node);

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Constant,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("{}{} = {}", name, type_ann, value_preview),
        children: vec![],
    })
}

/// Render a compact one-line preview of an assignment value for signatures.
/// Collections are summarized by their opening token + element count so the
/// signature stays small even for multi-thousand-line literals.
fn value_preview(source: &[u8], node: &Node) -> String {
    const MAX_LEN: usize = 60;
    match node.kind() {
        "list" | "tuple" | "set" | "dictionary" => {
            let (open, close) = match node.kind() {
                "list" => ("[", "]"),
                "dictionary" => ("{", "}"),
                _ => ("(", ")"),
            };
            let count = node.named_child_count();
            format!("{}{} elements{}", open, count, close)
        }
        _ => {
            let raw = node_text(*node, source);
            let one_line: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if one_line.chars().count() > MAX_LEN {
                let truncated: String = one_line.chars().take(MAX_LEN).collect();
                format!("{}...", truncated)
            } else {
                one_line
            }
        }
    }
}

/// Handle decorated definitions (e.g. @property, @staticmethod, @app.route)
fn extract_decorated(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    _imports: &mut Vec<ExtractedImport>,
) {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "decorator" => {
                let dec_text = node_text(child, source).trim_start_matches('@').to_string();
                decorators.push(dec_text);
            }
            "function_definition" => {
                if let Some(mut sym) = extract_python_function(child, source) {
                    // Prepend decorators to signature
                    if !decorators.is_empty() {
                        let dec_str = decorators
                            .iter()
                            .map(|d| format!("@{}", d))
                            .collect::<Vec<_>>()
                            .join(" ");
                        sym.signature = format!("{} {}", dec_str, sym.signature);
                    }
                    // Use full decorated range
                    sym.start_line = node.start_position().row + 1;
                    symbols.push(sym);
                }
            }
            "class_definition" => {
                if let Some(mut sym) = extract_python_class(child, source) {
                    if !decorators.is_empty() {
                        let dec_str = decorators
                            .iter()
                            .map(|d| format!("@{}", d))
                            .collect::<Vec<_>>()
                            .join(" ");
                        sym.signature = format!("{} {}", dec_str, sym.signature);
                    }
                    sym.start_line = node.start_position().row + 1;
                    symbols.push(sym);
                }
            }
            _ => {}
        }
    }
}

fn extract_python_function(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let params = node
        .child_by_field_name("parameters")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "()".to_string());

    // Extract return type annotation if present
    let ret = node
        .child_by_field_name("return_type")
        .map(|n| format!(" -> {}", node_text(n, source)))
        .unwrap_or_default();

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Function,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("def {}{}{}", name, params, ret),
        children: vec![],
    })
}

fn extract_python_class(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    // Extract superclasses
    let superclass = node
        .child_by_field_name("superclasses")
        .map(|n| node_text(n, source))
        .unwrap_or_default();

    let mut methods = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(method) = extract_python_function(child, source) {
                        methods.push(ExtractedSymbol {
                            kind: SymbolKind::Method,
                            ..method
                        });
                    }
                }
                "decorated_definition" => {
                    // Methods with decorators (@property, @staticmethod, etc.)
                    let mut dec_cursor = child.walk();
                    for dec_child in child.children(&mut dec_cursor) {
                        if dec_child.kind() == "function_definition" {
                            if let Some(method) = extract_python_function(dec_child, source) {
                                methods.push(ExtractedSymbol {
                                    kind: SymbolKind::Method,
                                    start_line: child.start_position().row + 1,
                                    ..method
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let sig = if superclass.is_empty() {
        format!("class {}", name)
    } else {
        format!("class {}{}", name, superclass)
    };

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Class,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: sig,
        children: methods,
    })
}

fn extract_python_import(node: Node, source: &[u8]) -> Option<ExtractedImport> {
    let text = node_text(node, source);
    let mut names = Vec::new();

    if node.kind() == "import_from_statement" {
        let mut path = String::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "dotted_name" if path.is_empty() => {
                    path = node_text(child, source);
                }
                "dotted_name" => {
                    names.push(node_text(child, source));
                }
                "aliased_import" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        names.push(node_text(name_node, source));
                    }
                }
                "wildcard_import" => {
                    names.push("*".to_string());
                }
                _ => {}
            }
        }
        if !path.is_empty() {
            return Some(ExtractedImport {
                path,
                kind: "import".to_string(),
                names,
            });
        }
    } else {
        // Regular import
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "dotted_name" {
                let path = node_text(child, source);
                return Some(ExtractedImport {
                    path,
                    kind: "import".to_string(),
                    names: vec![],
                });
            }
        }
    }

    // Fallback: extract from text
    if text.starts_with("import ") || text.starts_with("from ") {
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 2 {
            return Some(ExtractedImport {
                path: parts[1].to_string(),
                kind: "import".to_string(),
                names,
            });
        }
    }

    None
}
