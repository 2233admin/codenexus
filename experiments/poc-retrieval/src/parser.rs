use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Symbol {
    pub kind: String,
    pub name: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub snippet: String,
}

const QUERY_SRC: &str = r#"
(function_declaration name: (identifier) @name) @body
(class_declaration name: (type_identifier) @name) @body
(method_definition name: (property_identifier) @name) @body
(interface_declaration name: (type_identifier) @name) @body
(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function))) @body
"#;

pub fn parse_repo(root: &Path) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    let mut parser = Parser::new();
    let lang = tree_sitter_typescript::language_typescript();
    parser.set_language(&lang).context("set ts lang")?;
    let query = Query::new(&lang, QUERY_SRC).context("compile query")?;
    let name_idx = query.capture_index_for_name("name").unwrap();
    let body_idx = query.capture_index_for_name("body").unwrap();

    for entry in WalkBuilder::new(root).build().flatten() {
        let path = entry.path();
        if !is_ts_file(path) {
            continue;
        }
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let tree = match parser.parse(&src, None) {
            Some(t) => t,
            None => continue,
        };
        let mut cursor = QueryCursor::new();
        let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned();
        for m in cursor.matches(&query, tree.root_node(), src.as_bytes()) {
            let mut name = String::new();
            let mut body_node = None;
            for cap in m.captures {
                if cap.index == name_idx {
                    name = src[cap.node.byte_range()].to_string();
                } else if cap.index == body_idx {
                    body_node = Some(cap.node);
                }
            }
            if let Some(bn) = body_node {
                let snippet = clip(&src[bn.byte_range()], 500);
                symbols.push(Symbol {
                    kind: bn.kind().to_string(),
                    name,
                    path: rel.clone(),
                    start_line: bn.start_position().row + 1,
                    end_line: bn.end_position().row + 1,
                    snippet,
                });
            }
        }
    }
    Ok(symbols)
}

fn is_ts_file(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("ts") | Some("tsx")
    ) && !p.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("node_modules") | Some(".git") | Some("dist") | Some("build")
        )
    })
}

fn clip(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

#[allow(dead_code)]
pub fn _unused_pathbuf_keepalive() -> PathBuf {
    PathBuf::new()
}
