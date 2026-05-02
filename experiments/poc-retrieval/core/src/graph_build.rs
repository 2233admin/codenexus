use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use streaming_iterator::StreamingIterator;
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
    /// (file_path, alias) -> target_file_path. Built in pass_imports for
    /// `import * as X from "..."` declarations; consumed in pass_relations
    /// when a Calls site is `X.foo()` to resolve `foo` in target file.
    namespace_aliases: HashMap<(String, String), String>,
}

// Calls covers function calls AND constructor invocations (new X(...)).
// Member-call alternates capture both the property-identifier (callee) and the
// object-identifier (ns_obj) to support namespace-import resolution X.foo().
// Pattern overlap (member with vs without object) is fine: storage UNIQUE
// constraint dedupes resulting edges, picking the first-inserted confidence.
const Q_CALLS: &str = r#"
[
  (call_expression function: (identifier) @callee)
  (call_expression function: (member_expression
    object: (identifier) @ns_obj
    property: (property_identifier) @callee))
  (call_expression function: (member_expression
    property: (property_identifier) @callee))
  (new_expression constructor: (identifier) @callee)
  (new_expression constructor: (member_expression
    object: (identifier) @ns_obj
    property: (property_identifier) @callee))
  (new_expression constructor: (member_expression
    property: (property_identifier) @callee))
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
        let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
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
            namespace_aliases: HashMap::new(),
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
        let mut matches = cursor.matches(&self.q_imports, tree.root_node(), src.as_bytes());
        while let Some(m) = matches.next() {
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
                // Track alias → target_file for Calls resolver namespace lookup.
                // Don't insert an Imports edge (no single anchor symbol exists for `* as X`).
                if let Some(target) = self.resolve_import_path(file, &source) {
                    self.namespace_aliases
                        .insert((file.to_string(), name.clone()), target);
                } else {
                    stats.unresolved += 1;
                }
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

        // Calls — namespace-aware: per match, look for ns_obj capture and resolve via
        // namespace_aliases when present. Otherwise fall through to resolve_with_conf.
        {
            let callee_idx = self
                .q_calls
                .capture_index_for_name("callee")
                .ok_or_else(|| anyhow::anyhow!("callee capture missing"))?;
            let ns_obj_idx = self.q_calls.capture_index_for_name("ns_obj");
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.q_calls, root, src.as_bytes());
            while let Some(m) = matches.next() {
                let mut callee_name: Option<(String, usize)> = None;
                let mut ns_obj_name: Option<String> = None;
                for cap in m.captures {
                    if cap.index == callee_idx {
                        callee_name = Some((
                            src[cap.node.byte_range()].to_string(),
                            cap.node.start_position().row + 1,
                        ));
                    } else if Some(cap.index) == ns_obj_idx {
                        ns_obj_name = Some(src[cap.node.byte_range()].to_string());
                    }
                }
                let (name, row) = match callee_name {
                    Some(t) => t,
                    None => continue,
                };
                let from_id = match enclosing_symbol(&symbols_in_file, row) {
                    Some(id) => id,
                    None => continue,
                };
                if from_id == 0 {
                    continue;
                }
                let resolved = if let Some(obj) = ns_obj_name {
                    self.resolve_with_namespace(file, &obj, &name)
                } else {
                    self.resolve_with_conf(file, &name)
                };
                match resolved {
                    Some((to_id, conf)) => {
                        if to_id == from_id {
                            continue;
                        }
                        self.storage.insert_edge_conf(from_id, to_id, "Calls", conf)?;
                        stats.calls += 1;
                    }
                    None => {
                        stats.unresolved += 1;
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
            let mut matches = cursor.matches(&self.q_implements, root, src.as_bytes());
            while let Some(m) = matches.next() {
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
                    match self.resolve_with_conf(file, &name) {
                        Some((to_id, conf)) => {
                            if to_id == from_id {
                                continue;
                            }
                            self.storage
                                .insert_edge_conf(from_id, to_id, "Implements", conf)?;
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
            let mut matches = cursor.matches(&self.q_extends, root, src.as_bytes());
            while let Some(m) = matches.next() {
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
                    match self.resolve_with_conf(file, &name) {
                        Some((to_id, conf)) => {
                            if to_id == from_id {
                                continue;
                            }
                            self.storage
                                .insert_edge_conf(from_id, to_id, "Extends", conf)?;
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
        self.resolve_with_conf(from_file, name).map(|(id, _)| id)
    }

    /// Naive 3-step resolver with per-step confidence:
    ///   Step 1 (same-file)        -> 1.0
    ///   Step 2 (import-file)      -> 0.9
    ///   Step 3 (global-unique)    -> 0.7
    fn resolve_with_conf(&self, from_file: &str, name: &str) -> Option<(i64, f64)> {
        if let Ok(Some(id)) = self.storage.symbol_in_file_by_name(from_file, name) {
            return Some((id, 1.0));
        }
        if let Ok(targets) = self.storage.import_targets_for_file(from_file) {
            for tgt in targets {
                if let Ok(Some(id)) = self.storage.symbol_in_file_by_name(&tgt, name) {
                    return Some((id, 0.9));
                }
            }
        }
        self.storage
            .find_global_unique(name)
            .ok()
            .flatten()
            .map(|id| (id, 0.7))
    }

    /// Namespace-aware resolution for `X.foo()` calls. If `obj` is a tracked
    /// namespace alias for `from_file`, look up `name` directly in the target
    /// file at confidence 0.9 (deterministic post-resolution). Falls back to
    /// regular resolve_with_conf if obj is not a namespace alias.
    fn resolve_with_namespace(
        &self,
        from_file: &str,
        obj: &str,
        name: &str,
    ) -> Option<(i64, f64)> {
        let key = (from_file.to_string(), obj.to_string());
        if let Some(target_file) = self.namespace_aliases.get(&key) {
            if let Ok(Some(id)) = self
                .storage
                .symbol_in_file_by_name(target_file.as_str(), name)
            {
                return Some((id, 0.9));
            }
        }
        self.resolve_with_conf(from_file, name)
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

#[cfg(test)]
mod tests {
    //! Characterization tests pinning EdgeBuilder's current resolver behavior
    //! before Phase 04.5-03 splits this module into edge-find / edge-resolve /
    //! edge-store. These pin today's *actual* behavior including known bugs
    //! (T3 renamed import) and documented gaps (T4 default import). Refactor
    //! must keep these green or update them in the same commit with rationale.
    //!
    //! Confidence values per resolve_with_conf: 1.0 (same-file), 0.9
    //! (import-file via resolve_with_namespace OR step-2 import_targets),
    //! 0.7 (global-unique fallback). resolve_with_namespace returns 0.9 for
    //! `import * as X from "Y"; X.foo()` lookups.
    use super::*;
    use crate::parser::Symbol;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Store) {
        let dir = TempDir::new().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = Store::open(db_path.to_str().unwrap()).expect("Store::open");
        (dir, store)
    }

    fn write_file(dir: &TempDir, rel: &str, content: &str) {
        let p = dir.path().join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content).unwrap();
    }

    fn insert_sym(
        store: &Store,
        kind: &str,
        name: &str,
        path: &str,
        sl: usize,
        el: usize,
    ) -> i64 {
        let s = Symbol {
            kind: kind.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            start_line: sl,
            end_line: el,
            snippet: String::new(),
        };
        store.insert(&s, "", &[]).expect("insert sym")
    }

    /// Dump all edges as (from_name, kind, to_name, confidence) for assertion.
    fn dump_edges(store: &Store) -> Vec<(String, String, String, f64)> {
        let kinds = ["Calls", "Imports", "Implements", "Extends"];
        let mut out = Vec::new();
        for k in &kinds {
            for (from, to, conf) in store.edges_of_kinds(&[k], 0.0).unwrap() {
                let from_name = store
                    .symbol_by_id(from)
                    .unwrap()
                    .map(|(_, n, _)| n)
                    .unwrap_or_default();
                let to_name = store
                    .symbol_by_id(to)
                    .unwrap()
                    .map(|(_, n, _)| n)
                    .unwrap_or_default();
                out.push((from_name, k.to_string(), to_name, conf));
            }
        }
        out
    }

    fn build_graph(store: &Store, dir: &TempDir) -> EdgeStats {
        let mut builder = EdgeBuilder::new(store, dir.path().to_path_buf()).unwrap();
        builder.build_all().unwrap()
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// T1: named import `import { foo } from "./X"; foo()` resolves the call
    /// across files at confidence 0.9 (resolve_with_conf step 2 import-file).
    /// Imports edge written at default conf 1.0.
    #[test]
    fn t1_named_import_resolves_calls_at_conf_0_9() {
        let (dir, store) = setup();
        write_file(
            &dir,
            "A.ts",
            "import { foo } from \"./X\";\nexport function caller() { foo(); }\n",
        );
        write_file(&dir, "X.ts", "export function foo() {}\n");
        insert_sym(&store, "function", "caller", "A.ts", 2, 2);
        insert_sym(&store, "function", "foo", "X.ts", 1, 1);

        let _stats = build_graph(&store, &dir);
        let edges = dump_edges(&store);

        assert!(
            edges.iter().any(|(f, k, t, c)| f == "caller"
                && k == "Imports"
                && t == "foo"
                && approx(*c, 1.0)),
            "expected Imports caller->foo conf=1.0; got {:?}",
            edges
        );
        assert!(
            edges.iter().any(|(f, k, t, c)| f == "caller"
                && k == "Calls"
                && t == "foo"
                && approx(*c, 0.9)),
            "expected Calls caller->foo conf=0.9; got {:?}",
            edges
        );
    }

    /// T2: namespace import `import * as X from "./Y"; X.bar()` does NOT
    /// produce an Imports edge (line 244 explicit skip), only writes
    /// namespace_aliases. Calls edge resolves via resolve_with_namespace
    /// at confidence 0.9.
    #[test]
    fn t2_namespace_import_resolves_calls_via_alias_at_conf_0_9() {
        let (dir, store) = setup();
        write_file(
            &dir,
            "A.ts",
            "import * as X from \"./Y\";\nexport function caller() { X.bar(); }\n",
        );
        write_file(&dir, "Y.ts", "export function bar() {}\n");
        insert_sym(&store, "function", "caller", "A.ts", 2, 2);
        insert_sym(&store, "function", "bar", "Y.ts", 1, 1);

        let _stats = build_graph(&store, &dir);
        let edges = dump_edges(&store);

        assert!(
            !edges.iter().any(|(_, k, _, _)| k == "Imports"),
            "namespace import should NOT produce Imports edge; got {:?}",
            edges
        );
        assert!(
            edges.iter().any(|(f, k, t, c)| f == "caller"
                && k == "Calls"
                && t == "bar"
                && approx(*c, 0.9)),
            "expected Calls caller->bar conf=0.9 via NS; got {:?}",
            edges
        );
    }

    /// T3: renamed import `{ foo as bar }` — KNOWN BUG pinned. Q_IMPORTS
    /// captures the original `name` field (`foo`), so Imports edge points
    /// to X.foo correctly. But at the callsite `bar()`, resolve_with_conf
    /// looks up local binding `bar` in target file (fails — X exports `foo`)
    /// then falls through to global-unique (also fails — no `bar` exists).
    /// Net result: Imports edge OK, Calls edge MISSING. Refactor must
    /// preserve this behavior; fix in a separate slice with explicit tracking.
    #[test]
    fn t3_renamed_import_callsite_silently_unresolved_today() {
        let (dir, store) = setup();
        write_file(
            &dir,
            "A.ts",
            "import { foo as bar } from \"./X\";\nexport function caller() { bar(); }\n",
        );
        write_file(&dir, "X.ts", "export function foo() {}\n");
        insert_sym(&store, "function", "caller", "A.ts", 2, 2);
        insert_sym(&store, "function", "foo", "X.ts", 1, 1);

        let _stats = build_graph(&store, &dir);
        let edges = dump_edges(&store);

        assert!(
            edges.iter().any(|(_, k, t, _)| k == "Imports" && t == "foo"),
            "expected Imports edge to foo (Q_IMPORTS captures original name); got {:?}",
            edges
        );
        assert!(
            !edges.iter().any(|(_, k, _, _)| k == "Calls"),
            "renamed-import bar() should silently fail today (KNOWN BUG); got {:?}",
            edges
        );
    }

    /// T4: default import `import X from "./Y"` — DOCUMENTED GAP. Local
    /// binding `X` does not match exported name in Y (Y exports default
    /// function `foo`). pass_imports lookup fails, NO Imports edge. At
    /// callsite `X()` resolution also fails (no symbol named X anywhere).
    /// Pin this so a refactor that "accidentally fixes" it via different
    /// resolution path triggers a loud test break instead of silent change.
    #[test]
    fn t4_default_import_silently_unresolved_today() {
        let (dir, store) = setup();
        write_file(
            &dir,
            "A.ts",
            "import X from \"./Y\";\nexport function caller() { X(); }\n",
        );
        write_file(&dir, "Y.ts", "export default function foo() {}\n");
        insert_sym(&store, "function", "caller", "A.ts", 2, 2);
        insert_sym(&store, "function", "foo", "Y.ts", 1, 1);

        let _stats = build_graph(&store, &dir);
        let edges = dump_edges(&store);

        assert!(
            !edges.iter().any(|(_, k, _, _)| k == "Imports"),
            "default-import name mismatch produces NO Imports edge; got {:?}",
            edges
        );
        assert!(
            !edges.iter().any(|(_, k, _, _)| k == "Calls"),
            "default-import callsite silently fails today; got {:?}",
            edges
        );
    }

    /// T5: cross-file `class Derived extends Base` resolves via Imports +
    /// import_targets at confidence 0.9 (step 2).
    #[test]
    fn t5_cross_file_extends_resolves_at_conf_0_9() {
        let (dir, store) = setup();
        write_file(
            &dir,
            "A.ts",
            "import { Base } from \"./B\";\nexport class Derived extends Base {}\n",
        );
        write_file(&dir, "B.ts", "export class Base {}\n");
        insert_sym(&store, "class", "Derived", "A.ts", 2, 2);
        insert_sym(&store, "class", "Base", "B.ts", 1, 1);

        let _stats = build_graph(&store, &dir);
        let edges = dump_edges(&store);

        assert!(
            edges.iter().any(|(f, k, t, c)| f == "Derived"
                && k == "Extends"
                && t == "Base"
                && approx(*c, 0.9)),
            "expected Extends Derived->Base conf=0.9; got {:?}",
            edges
        );
    }

    /// T6: same-file call resolves at confidence 1.0 (resolve_with_conf
    /// step 1 same-file).
    #[test]
    fn t6_same_file_call_resolves_at_conf_1_0() {
        let (dir, store) = setup();
        write_file(
            &dir,
            "A.ts",
            "function foo() {}\nfunction bar() { foo(); }\n",
        );
        insert_sym(&store, "function", "foo", "A.ts", 1, 1);
        insert_sym(&store, "function", "bar", "A.ts", 2, 2);

        let _stats = build_graph(&store, &dir);
        let edges = dump_edges(&store);

        assert!(
            edges.iter().any(|(f, k, t, c)| f == "bar"
                && k == "Calls"
                && t == "foo"
                && approx(*c, 1.0)),
            "expected Calls bar->foo conf=1.0 (same-file); got {:?}",
            edges
        );
    }

    /// T7: an unresolvable name produces no Calls edge and increments the
    /// unresolved counter — does NOT silently false-match a coincidental
    /// global symbol via find_global_unique. This guards against a refactor
    /// that loosens fallback resolution.
    #[test]
    fn t7_unresolvable_call_is_not_silently_falsematched() {
        let (dir, store) = setup();
        write_file(
            &dir,
            "A.ts",
            "export function caller() { nonexistent(); }\n",
        );
        insert_sym(&store, "function", "caller", "A.ts", 1, 1);

        let stats = build_graph(&store, &dir);
        let edges = dump_edges(&store);

        assert!(
            !edges.iter().any(|(_, k, _, _)| k == "Calls"),
            "unresolvable call should produce no Calls edge; got {:?}",
            edges
        );
        assert!(
            stats.unresolved >= 1,
            "expected stats.unresolved >= 1, got {}",
            stats.unresolved
        );
    }
}
