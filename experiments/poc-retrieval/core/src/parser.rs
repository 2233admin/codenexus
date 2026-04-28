use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;
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

// Phase 4 group 2 spike (04-08): per-language tree-sitter dispatch.
// Single-language hardcoded TS path (pre-04-08) is replaced by
// `detect_language` + `LangCtx`. Adding a new grammar = (1) add crate
// dep, (2) extend `Language` enum + `detect_language` map, (3) add a
// QUERY_SRC_<LANG> const, (4) add a LangCtx::new line in parse_repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    Typescript,
    Python,
}

const QUERY_SRC_TS: &str = r#"
(function_declaration name: (identifier) @name) @body
(class_declaration name: (type_identifier) @name) @body
(method_definition name: (property_identifier) @name) @body
(interface_declaration name: (type_identifier) @name) @body
(type_alias_declaration name: (type_identifier) @name) @body
(enum_declaration name: (identifier) @name) @body
(lexical_declaration
  (variable_declarator
    name: (identifier) @name)) @body
"#;

// Spike scope: def / class / methods nested in class. Async fn uses the
// same `function_definition` node (with an `async` keyword child) so the
// query catches it without a separate clause. `decorated_definition`
// wraps without altering the inner node structure, so decorated fns are
// also matched at the inner `function_definition` level.
const QUERY_SRC_PY: &str = r#"
(function_definition name: (identifier) @name) @body
(class_definition name: (identifier) @name) @body
"#;

fn detect_language(p: &Path) -> Option<Language> {
    if p.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("node_modules")
                | Some(".git")
                | Some("dist")
                | Some("build")
                | Some("__pycache__")
                | Some(".venv")
                | Some("venv")
        )
    }) {
        return None;
    }
    match p.extension().and_then(|e| e.to_str())? {
        "ts" | "tsx" => Some(Language::Typescript),
        "py" => Some(Language::Python),
        _ => None,
    }
}

struct LangCtx {
    lang: tree_sitter::Language,
    query: Query,
    name_idx: u32,
    body_idx: u32,
}

impl LangCtx {
    fn new(lang: tree_sitter::Language, query_src: &str) -> Result<Self> {
        let query = Query::new(&lang, query_src).context("compile query")?;
        let name_idx = query
            .capture_index_for_name("name")
            .context("name capture missing")?;
        let body_idx = query
            .capture_index_for_name("body")
            .context("body capture missing")?;
        Ok(Self {
            lang,
            query,
            name_idx,
            body_idx,
        })
    }
}

pub fn parse_repo(root: &Path) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    let mut parser = Parser::new();

    let ts_ctx = LangCtx::new(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        QUERY_SRC_TS,
    )
    .context("init TS LangCtx")?;
    let py_ctx = LangCtx::new(tree_sitter_python::LANGUAGE.into(), QUERY_SRC_PY)
        .context("init Python LangCtx")?;

    for entry in WalkBuilder::new(root).build().flatten() {
        let path = entry.path();
        let Some(lang_kind) = detect_language(path) else {
            continue;
        };
        let ctx = match lang_kind {
            Language::Typescript => &ts_ctx,
            Language::Python => &py_ctx,
        };
        parser
            .set_language(&ctx.lang)
            .context("set parser lang")?;
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let tree = match parser.parse(&src, None) {
            Some(t) => t,
            None => continue,
        };
        let mut cursor = QueryCursor::new();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        let mut matches = cursor.matches(&ctx.query, tree.root_node(), src.as_bytes());
        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut body_node = None;
            for cap in m.captures {
                if cap.index == ctx.name_idx {
                    name = src[cap.node.byte_range()].to_string();
                } else if cap.index == ctx.body_idx {
                    body_node = Some(cap.node);
                }
            }
            if let Some(bn) = body_node {
                if name.is_empty() {
                    continue;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 4 group 2 spike acceptance: parse a Python fixture with
    /// def + async def + class + nested methods, assert the basic shape
    /// of multi-language extraction. Locked acceptance bars (per
    /// pre-spike commitment in session start): >=3 symbols extracted,
    /// `hello` + `fetch_data` + `Greeter` all in the name set.
    #[test]
    fn parse_python_fixture_extracts_def_and_class() {
        let uid = uuid::Uuid::new_v4();
        // NOTE: Use target/test-tmp/ (not std::env::temp_dir()) because on
        // Windows, %TEMP% lives under AppData/Local which has the HIDDEN
        // file attribute, and `ignore` crate's WalkBuilder default
        // hidden(true) filters the whole subtree -- yielding 0 symbols.
        // target/ is gitignored and not hidden.
        let dir = std::path::PathBuf::from("target/test-tmp").join(format!("codenexus_py_spike_{}", uid));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let py = dir.join("sample.py");
        std::fs::write(
            &py,
            r#"
def hello(name):
    return f"hi {name}"

async def fetch_data(url):
    return await get(url)

class Greeter:
    def __init__(self, name):
        self.name = name
    def greet(self):
        return f"hello {self.name}"
"#,
        )
        .expect("write py fixture");

        let symbols = parse_repo(&dir).expect("parse_repo");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            symbols.len() >= 3,
            "expected >=3 symbols, got {}: {:?}",
            symbols.len(),
            names
        );
        assert!(
            names.contains(&"hello"),
            "missing fn 'hello': {:?}",
            names
        );
        assert!(
            names.contains(&"fetch_data"),
            "missing async fn 'fetch_data': {:?}",
            names
        );
        assert!(
            names.contains(&"Greeter"),
            "missing class 'Greeter': {:?}",
            names
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cross-language smoke: a repo with both .ts AND .py files yields
    /// symbols from BOTH languages, not just the first one parsed.
    /// Guards against accidental short-circuit when adding language #3.
    #[test]
    fn parse_repo_handles_mixed_ts_and_py() {
        let uid = uuid::Uuid::new_v4();
        let dir = std::path::PathBuf::from("target/test-tmp").join(format!("codenexus_mixed_spike_{}", uid));
        std::fs::create_dir_all(&dir).expect("mkdir");

        std::fs::write(
            dir.join("a.ts"),
            "function tsOnlyFn() { return 1; }\nclass TsOnlyClass {}\n",
        )
        .expect("write ts");
        std::fs::write(
            dir.join("b.py"),
            "def py_only_fn():\n    pass\nclass PyOnlyClass:\n    pass\n",
        )
        .expect("write py");

        let symbols = parse_repo(&dir).expect("parse_repo");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"tsOnlyFn"),
            "missing TS fn: {:?}",
            names
        );
        assert!(
            names.contains(&"TsOnlyClass"),
            "missing TS class: {:?}",
            names
        );
        assert!(
            names.contains(&"py_only_fn"),
            "missing PY fn: {:?}",
            names
        );
        assert!(
            names.contains(&"PyOnlyClass"),
            "missing PY class: {:?}",
            names
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
