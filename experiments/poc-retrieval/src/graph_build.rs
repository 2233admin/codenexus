use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Instant;
use tree_sitter::{Parser, Query, QueryCursor};

use crate::storage::Store;

#[derive(Debug, Default)]
pub struct EdgeStats {
    pub calls: usize,
    pub imports: usize,
    pub implements: usize,
    pub extends: usize,
    pub unresolved: usize,
}

pub struct EdgeBuilder<'a> {
    storage: &'a Store,
    parser: Parser,
    repo_root: PathBuf,
    q_calls: Query,
    q_imports: Query,
    q_implements: Query,
    q_extends: Query,
}

const Q_CALLS: &str = r#"
[
  (call_expression function: (identifier) @callee)
  (call_expression function: (member_expression property: (property_identifier) @callee))
]
"#;

// Imports: capture three import-clause variants + the source string fragment.
// Run match-by-match; per match find which capture name was used.
const Q_IMPORTS: &str = r#"
(import_statement
  (import_clause
    [
      (named_imports
        (import_specifier name: (identifier) @name))
      (namespace_import (identifier) @ns_name)
      (identifier) @default_name
    ])
  source: (string (string_fragment) @source))
"#;

const Q_IMPLEMENTS: &str = r#"
(class_heritage
  (implements_clause
    [
      (type_identifier) @impl
      (generic_type (type_identifier) @impl)
    ]))
"#;

// Extends covers class extends + interface extends, each may have generic_type
const Q_EXTENDS: &str = r#"
[
  (class_heritage
    (extends_clause
      value: [
        (identifier) @ext
        (generic_type (type_identifier) @ext)
      ]))
  (interface_declaration
    (extends_type_clause
      [
        (type_identifier) @ext
        (generic_type (type_identifier) @ext)
      ]))
]
"#;

impl<'a> EdgeBuilder<'a> {
    pub fn new(storage: &'a Store, repo_root: PathBuf) -> Result<Self> {
        let mut parser = Parser::new();
        let lang = tree_sitter_typescript::language_typescript();
        parser.set_language(&lang).context("set ts lang")?;
        let q_calls = Query::new(&lang, Q_CALLS).context("compile Calls query")?;
        let q_imports = Query::new(&lang, Q_IMPORTS).context("compile Imports query")?;
        let q_implements =
            Query::new(&lang, Q_IMPLEMENTS).context("compile Implements query")?;
        let q_extends = Query::new(&lang, Q_EXTENDS).context("compile Extends query")?;
        Ok(Self {
            storage,
            parser,
            repo_root,
            q_calls,
            q_imports,
            q_implements,
            q_extends,
        })
    }

    pub fn build_all(&mut self) -> Result<EdgeStats> {
        let t0 = Instant::now();
        let mut stats = EdgeStats::default();

        self.storage.clear_edges()?;
        let files = self.storage.list_files()?;
        eprintln!("graph_build: {} files", files.len());

        // Sanity: warn if some indexed paths missing on disk
        let mut missing = 0usize;
        for f in &files {
            let abs = self.abs_path(f);
            if !abs.exists() {
                missing += 1;
            }
        }
        if missing > 0 {
            eprintln!(
                "WARN: {}/{} indexed files not found under --repo {} (paths may not match index time)",
                missing,
                files.len(),
                self.repo_root.display()
            );
        }

        // PASS 1: Imports
        for f in &files {
            if let Err(e) = self.pass_imports(f, &mut stats) {
                eprintln!("imports error in {}: {}", f, e);
            }
        }
        eprintln!(
            "pass1 imports: {} edges, unresolved so far {}",
            stats.imports, stats.unresolved
        );

        // PASS 2: Calls + Implements + Extends
        for f in &files {
            if let Err(e) = self.pass_relations(f, &mut stats) {
                eprintln!("relations error in {}: {}", f, e);
            }
        }

        let elapsed = t0.elapsed();
        eprintln!("graph_build done in {} ms", elapsed.as_millis());
        Ok(stats)
    }

    fn abs_path(&self, rel: &str) -> PathBuf {
        // symbols.path may use backslashes on Windows; normalise to system PathBuf via
        // forward-slash split then push.
        let mut p = self.repo_root.clone();
        for seg in rel.split(['/', '\\']) {
            if !seg.is_empty() {
                p.push(seg);
            }
        }
        p
    }

    fn read_source(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.abs_path(rel)).ok()
    }

    fn pass_imports(&mut self, file: &str, stats: &mut EdgeStats) -> Result<()> {
        let debug = std::env::var("POC_DEBUG_IMPORTS").is_ok();
        let src = match self.read_source(file) {
            Some(s) => s,
            None => {
                if debug {
                    eprintln!("[debug] read_source FAIL: {}", file);
                }
                return Ok(());
            }
        };
        let tree = match self.parser.parse(&src, None) {
            Some(t) => t,
            None => return Ok(()),
        };

        let symbols_in_file = self.storage.symbols_in_file_full(file)?;
        if symbols_in_file.is_empty() {
            return Ok(());
        }
        let from_id = symbols_in_file[0].0; // first-symbol-of-file stand-in for File node

        let name_idx = self.q_imports.capture_index_for_name("name");
        let ns_idx = self.q_imports.capture_index_for_name("ns_name");
        let default_idx = self.q_imports.capture_index_for_name("default_name");
        let source_idx = self
            .q_imports
            .capture_index_for_name("source")
            .ok_or_else(|| anyhow::anyhow!("source capture missing"))?;

        let mut cursor = QueryCursor::new();
        for m in cursor.matches(&self.q_imports, tree.root_node(), src.as_bytes()) {
            // Each named-import match corresponds to ONE import_specifier inside one import_statement.
            // We only get one source per match (the string_fragment under the same import_statement).
            let mut source_str: Option<String> = None;
            let mut imported_name: Option<String> = None;
            // Track namespace_import case so we can skip it (resolver can't find namespace target)
            let mut is_namespace = false;
            let mut is_default = false;

            for cap in m.captures {
                let txt = src[cap.node.byte_range()].to_string();
                if Some(cap.index) == name_idx {
                    imported_name = Some(txt);
                } else if Some(cap.index) == ns_idx {
                    imported_name = Some(txt);
                    is_namespace = true;
                } else if Some(cap.index) == default_idx {
                    imported_name = Some(txt);
                    is_default = true;
                } else if cap.index == source_idx {
                    source_str = Some(txt);
                }
            }

            let (Some(name), Some(source)) = (imported_name, source_str) else {
                continue;
            };

            if is_namespace {
                // Skip — resolver can't anchor to a single symbol in target file
                stats.unresolved += 1;
                continue;
            }

            let target_file = match self.resolve_import_path(file, &source) {
                Some(t) => t,
                None => {
                    if debug {
                        eprintln!(
                            "[debug] resolve_import_path FAIL from='{}' source='{}'",
                            file, source
                        );
                    }
                    stats.unresolved += 1;
                    continue;
                }
            };

            // For default imports, the local binding name typically does not match an
            // exported symbol named the same. We still try lookup by name (often will miss).
            let to_id = match self.storage.symbol_in_file_by_name(&target_file, &name)? {
                Some(id) => id,
                None => {
                    if debug {
                        eprintln!(
                            "[debug] sym_in_file FAIL target='{}' name='{}'",
                            target_file, name
                        );
                    }
                    stats.unresolved += 1;
                    if is_default {
                        // documented gap: default-import name mismatch
                    }
                    continue;
                }
            };

            self.storage.insert_edge(from_id, to_id, "Imports")?;
            stats.imports += 1;
        }
        Ok(())
    }

    fn pass_relations(&mut self, file: &str, stats: &mut EdgeStats) -> Result<()> {
        let src = match self.read_source(file) {
            Some(s) => s,
            None => return Ok(()),
        };
        let tree = match self.parser.parse(&src, None) {
            Some(t) => t,
            None => return Ok(()),
        };
        let root = tree.root_node();
        let symbols_in_file = self.storage.symbols_in_file_full(file)?;
        if symbols_in_file.is_empty() {
            return Ok(());
        }

        // Calls
        {
            let cap_idx = self
                .q_calls
                .capture_index_for_name("callee")
                .ok_or_else(|| anyhow::anyhow!("callee capture missing"))?;
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&self.q_calls, root, src.as_bytes()) {
                for cap in m.captures {
                    if cap.index != cap_idx {
                        continue;
                    }
                    let name = src[cap.node.byte_range()].to_string();
                    let row = cap.node.start_position().row + 1;
                    let from_id = match enclosing_symbol(&symbols_in_file, row) {
                        Some(id) => id,
                        None => continue, // top-level call outside indexed symbols
                    };
                    if from_id == 0 {
                        continue;
                    }
                    match self.resolve(file, &name) {
                        Some(to_id) => {
                            if to_id == from_id {
                                // self-reference — skip to keep noise down
                                continue;
                            }
                            self.storage.insert_edge(from_id, to_id, "Calls")?;
                            stats.calls += 1;
                        }
                        None => {
                            stats.unresolved += 1;
                        }
                    }
                }
            }
        }

        // Implements
        {
            let cap_idx = self
                .q_implements
                .capture_index_for_name("impl")
                .ok_or_else(|| anyhow::anyhow!("impl capture missing"))?;
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&self.q_implements, root, src.as_bytes()) {
                for cap in m.captures {
                    if cap.index != cap_idx {
                        continue;
                    }
                    let name = src[cap.node.byte_range()].to_string();
                    let row = cap.node.start_position().row + 1;
                    let from_id = match enclosing_symbol(&symbols_in_file, row) {
                        Some(id) => id,
                        None => continue,
                    };
                    match self.resolve(file, &name) {
                        Some(to_id) => {
                            if to_id == from_id {
                                continue;
                            }
                            self.storage.insert_edge(from_id, to_id, "Implements")?;
                            stats.implements += 1;
                        }
                        None => {
                            stats.unresolved += 1;
                        }
                    }
                }
            }
        }

        // Extends
        {
            let cap_idx = self
                .q_extends
                .capture_index_for_name("ext")
                .ok_or_else(|| anyhow::anyhow!("ext capture missing"))?;
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&self.q_extends, root, src.as_bytes()) {
                for cap in m.captures {
                    if cap.index != cap_idx {
                        continue;
                    }
                    let name = src[cap.node.byte_range()].to_string();
                    let row = cap.node.start_position().row + 1;
                    let from_id = match enclosing_symbol(&symbols_in_file, row) {
                        Some(id) => id,
                        None => continue,
                    };
                    match self.resolve(file, &name) {
                        Some(to_id) => {
                            if to_id == from_id {
                                continue;
                            }
                            self.storage.insert_edge(from_id, to_id, "Extends")?;
                            stats.extends += 1;
                        }
                        None => {
                            stats.unresolved += 1;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn resolve(&self, from_file: &str, name: &str) -> Option<i64> {
        // Step 1: same-file
        if let Ok(Some(id)) = self.storage.symbol_in_file_by_name(from_file, name) {
            return Some(id);
        }
        // Step 2: import-file
        if let Ok(targets) = self.storage.import_targets_for_file(from_file) {
            for tgt in targets {
                if let Ok(Some(id)) = self.storage.symbol_in_file_by_name(&tgt, name) {
                    return Some(id);
                }
            }
        }
        // Step 3: global unique
        self.storage.find_global_unique(name).ok().flatten()
    }

    /// Resolve a relative import source string ("./foo", "../bar/baz") to an indexed
    /// symbols.path. Returns None for bare/external imports.
    fn resolve_import_path(&self, from_file: &str, source: &str) -> Option<String> {
        // Bare import ("foo") → external dep, skip
        if !source.starts_with('.') {
            return None;
        }

        // Build dir = parent(from_file) using forward-slash semantics
        let from_norm = from_file.replace('\\', "/");
        let dir: &str = match from_norm.rsplit_once('/') {
            Some((d, _)) => d,
            None => "",
        };

        // Compose dir + source, normalise away "./" and "../"
        let combined = if dir.is_empty() {
            source.to_string()
        } else {
            format!("{}/{}", dir, source)
        };
        let normalised = normalize_relative_path(&combined);

        // TS convention: source `./foo.js` actually maps to `./foo.ts`. Strip
        // `.js`/`.jsx` so the suffix walk can re-attach the right TS extension.
        let stripped = if let Some(s) = normalised.strip_suffix(".js") {
            s.to_string()
        } else if let Some(s) = normalised.strip_suffix(".jsx") {
            s.to_string()
        } else {
            normalised.clone()
        };

        for base in &[stripped.as_str(), normalised.as_str()] {
            for suffix in &["", ".ts", ".tsx", "/index.ts", "/index.tsx"] {
                let candidate = format!("{}{}", base, suffix);
                // The indexed paths use OS-native separators because parser uses
                // to_string_lossy on relative path. On Windows that's backslash.
                // Try both forward-slash and backslash forms.
                let cand_fwd = candidate.clone();
                let cand_bsl = candidate.replace('/', "\\");
                for c in &[cand_fwd, cand_bsl] {
                    if let Ok(rows) = self.storage.symbols_in_file_full(c) {
                        if !rows.is_empty() {
                            return Some(c.clone());
                        }
                    }
                }
            }
        }
        None
    }
}

/// Binary-search-friendly: walk the (sorted by start_line) symbol list and pick the
/// smallest interval [start_line, end_line] covering `row`. Naive linear pass is fine
/// at < 100 symbols per file.
fn enclosing_symbol(
    symbols: &[(i64, String, usize, usize)],
    row: usize,
) -> Option<i64> {
    let mut best: Option<(i64, usize)> = None; // (id, span)
    for (id, _name, sl, el) in symbols {
        if *sl <= row && row <= *el {
            let span = el.saturating_sub(*sl);
            match best {
                None => best = Some((*id, span)),
                Some((_, prev)) if span < prev => best = Some((*id, span)),
                _ => {}
            }
        }
    }
    best.map(|(id, _)| id)
}

/// Normalise `a/b/./c/../d` → `a/b/d`. Forward-slash only. Does not touch leading
/// `./` segments at start (drops them).
fn normalize_relative_path(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => continue,
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.join("/")
}
