---
phase: 5
slice: 05-W4
plan_id: 05-W4
title: "W4: ADR extraction harness -- extract_adrs op + tree-sitter-markdown integration"
wave: 4
depends_on: [05-W0, 05-W3]
status: PLAN-AUTHORED (awaits plan-checker iter)
files_modified:
 - experiments/poc-retrieval/core/Cargo.toml
 - experiments/poc-retrieval/core/src/adr_extractor.rs
 - experiments/poc-retrieval/core/src/lib.rs
 - experiments/poc-retrieval/core/src/a2a.rs
 - experiments/poc-retrieval/core/src/server.rs
 - experiments/poc-retrieval/core/src/main.rs
 - server/internal/mcpsrv/server.go
 - server/internal/proxy/a2a.go
locked_decisions_honored:
 - G5  # ADR extraction: docs/**/*.md + .planning/*.md (one-level) + .planning/phases/**/*-PLAN.md + README.md; RFC 2119 PRIMARY + ## ADR SECONDARY; separate adrs + adr_symbol_links tables; on-demand extract_adrs auto-coupled to index_repo; FTS5 contentless
 - UQ-A4  # ADR supersede = history-preserving append-only via superseded_by column (matches G3)
 - UQ-A3  # extract_adrs is the 4th public A2A op
 - UQ-B5  # markdown parser = tree-sitter-markdown (consistent with lang_extractor candidate set)
 - UQ-B6  # excluded-dirs handled silently; logger trace at debug verbosity
gates:
 - G-A  # build clean (Rust + Go); tree-sitter-markdown dep added cleanly
 - G-B  # all unit tests pass; >= 5 ADR extractor tests + dispatch test
 - G-C  # extract_adrs dispatchable; auto-coupled to index_repo path
 - G-D  # adr_symbol_links populated by query_constraints during W2 lookup OR by extract_adrs file-overlap heuristic
 - G-E  # supersede semantics: re-running extract_adrs on changed source DOES NOT delete prior rows; updates superseded_by chain
---

> **!! AMENDED 2026-05-03 per CCG round 2 !!** Round-2 amendment below
> SUPERSEDES the storage-layer specifics in the original plan. Extraction
> logic (sources / patterns / paragraph segmentation / supersede semantics)
> is unchanged. See `05-DISCUSS-SUMMARY.md § Round-3 Amendments LANDED`.

## Round-2 Amendment Block (W4 -- CI-1 cascade; storage targets shift)

W4 inherits W0's amended schema. Original W4 wrote ADRs to a separate `adrs`
table. Amended W4 writes to `symbols` (kind='ADR') + `adr_metadata` sidecar.
Extraction logic unchanged; persistence path different.

### Storage write targets (amended)

For each extracted paragraph, executor calls TWO inserts in a single
transaction:

```rust
// 1. Insert into symbols (W0 created the schema)
let symbol_id = store.insert_adr_symbol(
    source_path,                        // e.g. "docs/ARCHITECTURE.md"
    heading_anchor,                     // e.g. "9.4-cross-encoder" or None
    source_line,                        // paragraph start line
    body_text,                          // full paragraph text
)?;
// (insert_adr_symbol creates a Symbol row with kind='ADR',
//  name='{heading_anchor || "anon"}#{source_line}', body_text=paragraph,
//  start_line=source_line, end_line=source_end_line)

// 2. Insert sidecar metadata
store.insert_adr_metadata(symbol_id, AdrMeta {
    keyword,                            // MUST_NOT|MUST|SHOULD|...
    confidence,                         // 1.0 / 0.7 / 0.4
    source_line, source_end_line,
    heading_anchor,
    doc_version_sha,                    // git blob sha
    extracted_at: Utc::now().timestamp(),
    superseded_by_symbol_id: None,      // populated on next supersede
})?;
```

### Supersede semantics (amended)

Same append-only discipline as G3 notes (UQ-A4 honored). On re-extraction:
- If paragraph at `(source_path, source_line)` has SAME body_text + SAME
  doc_version_sha -> SKIP (idempotent re-run)
- If paragraph has SAME source_line but NEW doc_version_sha + DIFFERENT
  body_text -> INSERT new Symbol row + new adr_metadata row, then UPDATE
  old `adr_metadata.superseded_by_symbol_id = new_symbol_id`. Old rows
  stay intact. `idx_adr_active` filters to leaves.

Idempotency check uses `(source_path, source_line, doc_version_sha)` triple
as logical-but-not-DB-enforced uniqueness. Executor implements de-dupe in
extract_from_paths logic, not via SQL constraint (avoids brittle CHECK
on body_text equality).

### FTS indexing (amended)

NO new FTS table. ADR Symbol rows are indexed automatically by
`symbols_fts` (W0 rebuilt symbols_fts to include body_text in indexed
columns + triggers). No special W4 FTS work needed.

### adr_symbol_links (unchanged scope, FK target shifted)

`adr_symbol_links` table created empty in W0. Columns:
- `adr_symbol_id INTEGER REFERENCES symbols(id)` -- the ADR Symbol row
- `code_symbol_id INTEGER REFERENCES symbols(id)` -- the code Symbol row
- `link_kind TEXT` -- mention | topic_match | file_overlap
- `score REAL`

V1.0 W4 leaves this table empty. V1.1+ populates lazily from text-mention
heuristic (e.g. parser.rs scans body_text for `Symbol::name` substrings).

### Removed plan_time_decisions

- Any reference to `Store::insert_adr` writing to a separate adrs table
  -- REPLACE with `Store::insert_adr_symbol` + `Store::insert_adr_metadata`
- Any reference to `adrs_fts` virtual table -- DROP (CI-4 dissolved; symbols_fts
  handles ADR FTS via W0's body_text column)

### New plan_time_decisions

- **D-W4-amended-01 (Symbol name encoding):** ADR Symbol `name` field =
  `"{heading_anchor || \"anon\"}#{source_line}"`. Stable across runs given
  same heading + line. Drift on heading rename caught at supersede check
  (different name -> new Symbol; old marked superseded only if doc_version_sha
  matches OLD line position; otherwise both rows coexist as "anon move" --
  acceptable for V1.0).
- **D-W4-amended-02 (kind='ADR' uppercase):** SQL string literal `'ADR'`
  uppercase to match Symbol.kind enum convention (Function, Class, ADR).
  Case-sensitive comparisons throughout.
- **D-W4-amended-03 (transactional insert):** symbols + adr_metadata inserts
  wrapped in `Connection::transaction()`. If adr_metadata insert fails, both
  rollback. Avoids dangling kind='ADR' Symbol with no metadata.

### W4 acceptance test additions

- [ ] Extract a single paragraph from a fixture .md file -> verify Symbol
      row with kind='ADR' exists + adr_metadata row joined by symbol_id
- [ ] Extract with heading anchor -> verify Symbol.name format matches
      `{heading}#{line}`
- [ ] Extract without heading anchor -> verify Symbol.name = `anon#{line}`
- [ ] Re-extract idempotent (same SHA + same line + same body) -> 0 new rows
- [ ] Re-extract with new SHA + same line + diff body -> NEW Symbol + NEW
      adr_metadata + OLD metadata.superseded_by_symbol_id = new_id
- [ ] symbols_fts MATCH 'NOT introduce reranker' -> returns the kind='ADR'
      Symbol row (verifies W0 body_text indexing)
- [ ] adr_metadata insert failure -> both inserts rolled back (no orphan
      kind='ADR' Symbol)

### W4 unaffected items

- Source globs (docs/**, .planning/*.md one-level, README.md) -- unchanged
- RFC 2119 keyword scan PRIMARY + ## ADR SECONDARY -- unchanged
- Confidence levels (1.0 / 0.7 / 0.4) -- unchanged
- tree-sitter-markdown for paragraph segmentation (UQ-B5) -- unchanged
- Auto-coupling to index_repo -- unchanged
- Manual escape hatch via standalone extract_adrs op -- unchanged
- All G5 ranking signals (keyword strength, confidence, text relevance,
  source-doc weight, recency) -- column sources shift to adr_metadata
  (was adrs.keyword, now adr_metadata.keyword) but ranking weights unchanged

---


<objective>
Land the ADR extraction harness. After W4, agents calling
query_constraints (W2) get ADR results from project markdown docs in
addition to per-symbol notes (W1). Per G5 section 1.1: default sources are
`docs/**/*.md` + `.planning/*.md` (one-level) +
`.planning/phases/**/*-PLAN.md` + `README.md`. Excluded by default:
`.planning/audits/**`, `.planning/probes/**`, `.planning/research/**`.
Configurable via plugin.toml `[adr]` section. CLI override via
`--adr-include` / `--adr-exclude`.

Per G5 section 2.1: PRIMARY signal is RFC 2119 keyword scan (MUST / MUST NOT
/ SHOULD / SHOULD NOT / SHALL / REQUIRED / RECOMMENDED / MAY); SECONDARY
is `## ADR` heading anchor scan. Confidence scale: PRIMARY=1.0,
SECONDARY=0.7, WEAK (MAY/OPTIONAL)=0.4 per G5 section 2.2. Granularity:
per-paragraph via tree-sitter-markdown (UQ-B5).

Per G5 section 3.2: storage in `adrs` + `adr_symbol_links` tables (W0 created
empty). UNIQUE (source_path, source_line, doc_version_sha) prevents
duplicate inserts; superseded_by enables append-only audit trail
(UQ-A4).

Per G5 section 4.1: `extract_adrs(scope?)` triggered on-demand (CLI flag /
A2A op) AND auto-coupled to `index_repo` (incremental walk extends to
matching .md files; full re-index triggers full ADR re-extraction).

Out of scope: ranking weight tuning (W6 eval); file-watcher /
scheduled re-extraction (V1.1+ per G5 section 4.2); shared-PG / cross-repo ADR
sourcing (V1.1+ per G5 section 1.3); adr_symbol_links auto-population from
text mentions of symbol names (V1.1+; W2 covers file_overlap implicitly
via list_adrs_for_file).

Output:
- `core/Cargo.toml`: add `tree-sitter-md = "0.3"` (or whatever the
 current crate name is; UQ-B5 -- executor confirms version).
- `core/src/adr_extractor.rs` (NEW): markdown paragraph segmentation +
 RFC 2119 keyword scan + ## ADR heading scan + Adr struct construction
 + Store::insert_adr persistence. `pub fn extract_from_paths(paths:
 &[PathBuf]) -> Result<Vec<Adr>>`.
- `core/src/lib.rs`: re-export `pub mod adr_extractor`.
- `core/src/a2a.rs`: ExtractAdrs request/response variants.
- `core/src/server.rs::handle_extract_adrs`: dispatcher walks include
 globs, calls adr_extractor, persists via Store::insert_adr +
 emits JSONL events (`{"event": "extract_adrs", ...}`).
- `core/src/main.rs::Cmd::Index`: auto-coupled hook -- after symbol
 indexing, walk ADR include globs and run extract_adrs.
- `server/internal/mcpsrv/server.go`: replace W3 stub with real
 dispatch.
- `server/internal/proxy/a2a.go::ExtractAdrs`: real implementation
 (W3 ships method; W4 fills body).
</objective>

<plan_time_decisions>
- **D-W4-01 (markdown parser dep):** Use `tree-sitter-md` (the modern
 crate name; was `tree-sitter-markdown`). Verify version at execution
 time via Context7 / cargo search. Per UQ-B5 default leaning. Per
 G5 OQ1 the alternative was pulldown-cmark; locked in favor of
 tree-sitter for consistency with lang_extractor stack.
- **D-W4-02 (paragraph extraction algorithm):** Use tree-sitter
 `paragraph` node enumeration. For each paragraph node: extract
 source byte span; check if it contains code-fence enclosing context
 (skip if entirely inside a fenced_code_block); strip inline
 backticks; scan stripped text for RFC 2119 keywords (whole-word,
 case-sensitive, in non-code-fence regions). One paragraph -> at
 most one ADR row (highest-strength keyword wins; report severity
 per D-W4-04).
- **D-W4-03 (heading_anchor extraction):** Walk markdown AST upward
 from each paragraph node; collect ATX headings (## / ### / ####);
 format as "## Section / ### Subsection" path string. Stored in
 Adr.heading_anchor.
- **D-W4-04 (severity precedence within paragraph):** If paragraph
 contains both MUST and SHOULD, the row's keyword = MUST (PRIMARY
 wins). Confidence = 1.0. SECONDARY heading anchor co-occurrence
 bumps confidence in OQ4 of G5; locked: confidence stays at PRIMARY=1.0
 even with co-occurrence (avoid >1.0 sentinel; downstream can
 multiply).
- **D-W4-05 (doc_version_sha resolution):** Use `git ls-files -s`
 blob sha from the source path (call out to git via std::process::
 Command). If git is unavailable OR file is untracked, use a
 content sha (sha256 of file bytes). Loud-warn once per
 extract_adrs run if git fallback fires, per UQ-B6 trace.
- **D-W4-06 (re-extract behavior):** `extract_adrs(scope=None)` walks
 ALL include globs. For each file:
 (a) compute current doc_version_sha
 (b) for each paragraph -> attempt INSERT OR IGNORE on adrs table
   with UNIQUE (source_path, source_line, doc_version_sha)
 (c) for any prior adr row at same source_path with DIFFERENT
   doc_version_sha that is NOT yet superseded, find its match in
   the new extraction (same source_line if unchanged; nearest line
   if shifted) and set its superseded_by = new_id.
   If no clear match (paragraph deleted), set superseded_by =
   NULL but mark superseded_by = -1 sentinel? NO: per G5 section 3.1
   append-only discipline, leave superseded_by = NULL when the
   paragraph is GONE (the row stays as historical, no replacement).
 (d) emit JSONL event per file processed:
   `{"event": "extract_adrs", "ts": ..., "payload": {"source_path":
   "...", "doc_version_sha": "...", "adrs_inserted": N, "adrs_superseded": M}}`
- **D-W4-07 (auto-coupled to index_repo):** main.rs Cmd::Index
 handler, AFTER existing indexing logic completes successfully,
 calls extract_adrs(scope=None). Failure of ADR extraction is
 NON-FATAL (eprintln warning + continue with exit 0). Per G5 section 4.1:
 "ADRs go stale exactly when docs go stale -- which is exactly when
 re-indexing already runs."
- **D-W4-08 (dedupe meta-mentions OQ7):** When the same paragraph_text
 appears at TWO different source_paths (e.g., PROJECT.md quotes
 ARCHITECTURE.md verbatim), KEEP both rows but MARK the
 lower-source-weight one with superseded_by = higher-weight row's
 id. Source weights per G5 section 5.3 default table; computed at insert
 time.
</plan_time_decisions>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W0-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W3-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-adr.md
@CONTEXT.md
@experiments/poc-retrieval/core/src/storage.rs
@docs/ARCHITECTURE.md

<interfaces>
<!-- W0 storage primitives in scope -->
```rust
Store::insert_adr(source_path, source_line, source_end_line, heading_anchor, keyword, confidence, paragraph_text, doc_version_sha, extracted_at) -> Result<Option<i64>>
Store::supersede_adr(old_id, new_id) -> Result<()>
Store::list_adrs_for_file(source_path) -> Result<Vec<Adr>>
Store::clear_adrs() -> Result<()>
Store::has_adrs_table() -> Result<bool>
```

<!-- Target a2a.rs additions -->
```rust
ExtractAdrs {
  #[serde(default)] scope: Option<Vec<String>>,  // None = use config defaults; Some(paths) = override
  #[serde(default)] dry_run: bool,         // true = scan but do not persist
}

// OperationResponse::ExtractAdrs:
ExtractAdrs {
  files_scanned: usize,
  adrs_inserted: usize,
  adrs_superseded: usize,
  adrs_skipped_duplicate: usize,
  warnings: Vec<String>,
}
```

<!-- Target adr_extractor.rs surface -->
```rust
pub struct ExtractedParagraph {
  pub source_path: String,
  pub source_line: i64,
  pub source_end_line: i64,
  pub heading_anchor: Option<String>,
  pub keyword: Option<String>,      // None if no RFC 2119 keyword found
  pub confidence: f32,          // 0.0 if no keyword
  pub paragraph_text: String,
}

pub struct AdrExtractorConfig {
  pub include: Vec<String>,        // glob patterns
  pub exclude: Vec<String>,
  pub patterns: Vec<&'static str>,    // RFC 2119 keywords
}

impl AdrExtractorConfig {
  pub fn defaults() -> Self;       // per G5 section 1.1
  pub fn from_plugin_toml(repo_root: &Path) -> Result<Self>;
}

pub fn extract_from_path(path: &Path) -> Result<Vec<ExtractedParagraph>>;

pub fn walk_and_extract(repo_root: &Path, cfg: &AdrExtractorConfig)
  -> Result<Vec<ExtractedParagraph>>;
```

<!-- Existing main.rs Cmd::Index pattern -->
<!-- Read at execution time; auto-coupled hook lives at the END of the index path -->
```
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
 <name>Task 1: tree-sitter-md dep + adr_extractor module + config + paragraph scan + RFC 2119 keyword extraction</name>
 <files>experiments/poc-retrieval/core/Cargo.toml, experiments/poc-retrieval/core/src/adr_extractor.rs, experiments/poc-retrieval/core/src/lib.rs</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-adr.md G5 sections 1, 2, 3 (full -- source scope + extraction pattern + storage)
  - experiments/poc-retrieval/core/Cargo.toml (existing tree-sitter deps; locate `tree-sitter-typescript` or `tree-sitter-python` for version pinning convention)
  - docs/ARCHITECTURE.md lines 305 + 508 (RFC 2119 inline-prose live examples; harness should extract these)
  - experiments/poc-retrieval/core/src/lib.rs (current pub mod list)
 </read_first>

 <behavior>
  - Test 1 (compile + dep added): `cargo build -p codenexus-core` exits 0 with tree-sitter-md as a transitive dep
  - Test 2 (defaults config): `AdrExtractorConfig::defaults()` returns include including `docs/**/*.md`, exclude including `.planning/audits/**`, patterns including `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, `SHALL`, `REQUIRED`, `RECOMMENDED`, `MAY`
  - Test 3 (paragraph segmentation): given a fixture markdown file with 3 paragraphs separated by blank lines, extract_from_path returns Vec of 3 ExtractedParagraphs with non-overlapping (source_line, source_end_line) ranges
  - Test 4 (RFC 2119 keyword detection): paragraph "Cross-encoder reranker MUST NOT be introduced in Phase 3 MVP until LLM-as-judge eval pipeline exists." returns ExtractedParagraph with keyword=Some("MUST_NOT") (or "MUST NOT" -- spec D-W4-04 says PRIMARY wins; document chosen serialization), confidence=1.0
  - Test 5 (heading_anchor extraction): paragraph nested under `## 9 Locked decisions` -> `### 9.4 Reranker policy` returns heading_anchor=Some("## 9 Locked decisions / ### 9.4 Reranker policy") (or similar joined format; document)
  - Test 6 (code-fence skip): paragraph entirely inside ```bash ... ``` block with `# MUST list:` returns ExtractedParagraph WITHOUT keyword (or excluded entirely; D-W4-02 skip behavior)
  - Test 7 (inline backtick skip): paragraph `Use \`MUST NOT\` keyword to mark constraints.` is NOT extracted as a constraint (literal reference, OQ2 from G5)
  - Test 8 (live ARCHITECTURE.md self-test): walk_and_extract on the actual repo with default config returns >= 1 ExtractedParagraph with keyword="MUST_NOT" matching docs/ARCHITECTURE.md lines around 305 and 508
 </behavior>

 <action>

**Step A -- Cargo.toml dep.** Add `tree-sitter-md = "<latest 0.x>"` (or `tree-sitter-markdown`; executor confirms current crate name via cargo search at execution time per UQ-B5). If lang_extractor framework already has a markdown grammar entry from 04.5 lift, reuse it. Document chosen crate + version in SUMMARY.

**Step B -- create `core/src/adr_extractor.rs`.** New file with structure per `<interfaces>` block. Implementation outline:

```rust
//! Phase 5 W4: ADR extraction harness per G5 lock (05-discuss-adr.md).
//!
//! Scans markdown docs for RFC 2119 keywords (PRIMARY: MUST/MUST NOT/SHOULD/...)
//! and `## ADR` heading anchors (SECONDARY). Per-paragraph granularity using
//! tree-sitter-md. Default include/exclude per G5 section 1.1.
//!
//! Outputs `Vec<ExtractedParagraph>` for handler to persist via Store::insert_adr.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct ExtractedParagraph {
  pub source_path: String,
  pub source_line: i64,
  pub source_end_line: i64,
  pub heading_anchor: Option<String>,
  pub keyword: Option<String>,
  pub confidence: f32,
  pub paragraph_text: String,
}

pub struct AdrExtractorConfig {
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub patterns_primary: Vec<&'static str>,
  pub patterns_secondary: Vec<&'static str>,
  pub patterns_weak: Vec<&'static str>,
}

impl AdrExtractorConfig {
  pub fn defaults() -> Self {
    Self {
      include: vec![
        "docs/**/*.md".into(),
        ".planning/*.md".into(),            // one-level only
        ".planning/phases/**/*-PLAN.md".into(),
        "README.md".into(),
      ],
      exclude: vec![
        ".planning/audits/**".into(),
        ".planning/probes/**".into(),
        ".planning/research/**".into(),
        "target/**".into(),
        "node_modules/**".into(),
      ],
      patterns_primary: vec!["MUST NOT", "SHALL NOT", "REQUIRED NOT", "MUST", "SHALL", "REQUIRED"],
      patterns_secondary: vec!["SHOULD NOT", "SHOULD", "RECOMMENDED", "NOT RECOMMENDED"],
      patterns_weak: vec!["MAY", "OPTIONAL"],
    }
  }

  pub fn from_plugin_toml(repo_root: &Path) -> Result<Self> {
    // Read <repo_root>/plugin.toml [adr] section if present;
    // else return defaults(). Optional in V1.0 (config can be omitted).
    ...
  }

  fn keyword_strength(&self, kw: &str) -> Option<(String, f32)> {
    // Returns (canonical_keyword, confidence) for the highest-strength
    // pattern present in `kw`. Whole-word, case-sensitive.
    for p in &self.patterns_primary {
      if matches_whole_word(kw, p) {
        return Some((p.replace(' ', "_"), 1.0));
      }
    }
    for p in &self.patterns_secondary {
      if matches_whole_word(kw, p) {
        return Some((p.replace(' ', "_"), 0.7));
      }
    }
    for p in &self.patterns_weak {
      if matches_whole_word(kw, p) {
        return Some((p.replace(' ', "_"), 0.4));
      }
    }
    None
  }
}

pub fn extract_from_path(path: &Path) -> Result<Vec<ExtractedParagraph>> {
  let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
  let cfg = AdrExtractorConfig::defaults();
  extract_from_bytes(&bytes, path, &cfg)
}

pub fn extract_from_bytes(bytes: &[u8], path: &Path, cfg: &AdrExtractorConfig) -> Result<Vec<ExtractedParagraph>> {
  // Parse with tree-sitter-md
  let mut parser = tree_sitter::Parser::new();
  parser.set_language(&tree_sitter_md::LANGUAGE.into())?;
  let tree = parser.parse(bytes, None).context("parse markdown")?;
  let root = tree.root_node();

  let mut out = Vec::new();
  walk_paragraphs(&root, bytes, path, cfg, &mut Vec::new(), &mut out)?;
  Ok(out)
}

fn walk_paragraphs(
  node: &tree_sitter::Node,
  bytes: &[u8],
  path: &Path,
  cfg: &AdrExtractorConfig,
  heading_stack: &mut Vec<String>,  // tracks ## / ### heading hierarchy
  out: &mut Vec<ExtractedParagraph>,
) -> Result<()> {
  // For each child: if heading, push/pop stack; if paragraph, extract.
  // Skip subtrees rooted at fenced_code_block (D-W4-02).
  ...
}

pub fn walk_and_extract(repo_root: &Path, cfg: &AdrExtractorConfig) -> Result<Vec<ExtractedParagraph>> {
  use globset::{Glob, GlobSetBuilder};
  let mut inc = GlobSetBuilder::new();
  for g in &cfg.include { inc.add(Glob::new(g)?); }
  let inc = inc.build()?;

  let mut exc = GlobSetBuilder::new();
  for g in &cfg.exclude { exc.add(Glob::new(g)?); }
  let exc = exc.build()?;

  let mut out = Vec::new();
  for entry in walkdir::WalkDir::new(repo_root).into_iter().filter_map(|e| e.ok()) {
    let p = entry.path();
    let rel = p.strip_prefix(repo_root).unwrap_or(p);
    if !inc.is_match(rel) { continue; }
    if exc.is_match(rel) { continue; }
    if !p.is_file() { continue; }
    match extract_from_path(p) {
      Ok(mut paras) => out.append(&mut paras),
      Err(e) => eprintln!("[adr_extractor] skip {}: {}", p.display(), e), // UQ-B6 silent ignore
    }
  }
  Ok(out)
}

fn matches_whole_word(text: &str, pattern: &str) -> bool {
  // Case-sensitive whole-word match avoiding sub-string false positives
  // (e.g., "MUST" inside "JUSTIFICATION" must NOT match)
  let text_chars: Vec<char> = text.chars().collect();
  let pat_chars: Vec<char> = pattern.chars().collect();
  if pat_chars.is_empty() { return false; }
  for i in 0..text_chars.len().saturating_sub(pat_chars.len() - 1) {
    if text_chars[i..i+pat_chars.len()].iter().zip(pat_chars.iter()).all(|(a, b)| a == b) {
      let before_ok = i == 0 || !text_chars[i-1].is_alphanumeric();
      let after_idx = i + pat_chars.len();
      let after_ok = after_idx >= text_chars.len() || !text_chars[after_idx].is_alphanumeric();
      if before_ok && after_ok { return true; }
    }
  }
  false
}
```

(EXECUTOR: tree-sitter-md API may differ; adapt walk_paragraphs to the actual node names. Check tree-sitter-md docs for `paragraph` / `atx_heading` / `fenced_code_block` node kinds. If the crate name is `tree-sitter-markdown` rather than `tree-sitter-md`, adjust use statement.)

**Step C -- re-export in lib.rs.** Add `pub mod adr_extractor;`.

**Step D -- write unit tests** in adr_extractor.rs `mod tests`. Tests 2-7 from `<behavior>`. Test 8 uses the actual repo path; mark `#[ignore]` if running outside the repo root, OR use a fixture that is a copy of docs/ARCHITECTURE.md trimmed to relevant lines.

**Step E -- verify build:**
```bash
cd D:/projects/codenexus/experiments/poc-retrieval
cargo build --workspace --release 2>&1 | tail -10
cargo test -p codenexus-core --lib adr_extractor -- --test-threads=1 2>&1 | tail -20
```

 </action>

 <acceptance_criteria>
  - `grep -nF 'tree-sitter-md' experiments/poc-retrieval/core/Cargo.toml` >= 1 hit (or tree-sitter-markdown if that's the actual crate name)
  - `test -f experiments/poc-retrieval/core/src/adr_extractor.rs` exits 0
  - `grep -nE 'pub fn extract_from_path' experiments/poc-retrieval/core/src/adr_extractor.rs` exactly 1 hit
  - `grep -nE 'pub fn walk_and_extract' experiments/poc-retrieval/core/src/adr_extractor.rs` exactly 1 hit
  - `grep -nE 'pub fn defaults' experiments/poc-retrieval/core/src/adr_extractor.rs` >= 1 hit (AdrExtractorConfig::defaults)
  - `grep -nE 'pub mod adr_extractor' experiments/poc-retrieval/core/src/lib.rs` exactly 1 hit
  - `grep -nF 'MUST NOT' experiments/poc-retrieval/core/src/adr_extractor.rs` >= 1 hit (RFC 2119 PRIMARY pattern declared)
  - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -cE '^error'` returns 0 (G-A)
  - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib adr_extractor -- --test-threads=1` >= 5 tests pass
  - All W0-W3 + 04.5-03 tests still green (G-B)
 </acceptance_criteria>

 <verify>
  <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core --lib adr_extractor -- --test-threads=1 2>&1 | tail -20</automated>
 </verify>

 <done>
  tree-sitter-md (or equivalent crate) added to Cargo.toml.
  adr_extractor.rs has AdrExtractorConfig + extract_from_path +
  walk_and_extract + matches_whole_word. RFC 2119 patterns +
  code-fence skip + inline-backtick skip + heading anchor stacking
  all implemented. >= 5 unit tests pass. lib.rs re-exports module.
  Build clean (G-A); regression-green (G-B).
 </done>
</task>

<task type="auto" tdd="true">
 <name>Task 2: extract_adrs A2A op + handler + persistence + auto-couple to index_repo + MCP wiring</name>
 <files>experiments/poc-retrieval/core/src/a2a.rs, experiments/poc-retrieval/core/src/server.rs, experiments/poc-retrieval/core/src/main.rs, server/internal/mcpsrv/server.go, server/internal/proxy/a2a.go</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-adr.md section 3 + section 4 (storage + re-extraction trigger)
  - experiments/poc-retrieval/core/src/main.rs Cmd::Index (auto-couple insertion point AFTER existing index logic)
  - experiments/poc-retrieval/core/src/server.rs handle_remember_symbol_note (pattern for new handler + JSONL emit)
  - server/internal/mcpsrv/server.go extract_adrs stub (W3) -- replace stub with real dispatch
 </read_first>

 <behavior>
  - Test 1 (a2a deserialize): `serde_json::from_str::<OperationRequest>(r#"{"extract_adrs":{}}"#)` succeeds; scope=None, dry_run=false
  - Test 2 (a2a deserialize with scope): `serde_json::from_str::<OperationRequest>(r#"{"extract_adrs":{"scope":["docs/ARCHITECTURE.md"],"dry_run":true}}"#)` succeeds
  - Test 3 (handler dry_run): given fixture repo with 3 ADRs in 1 markdown file, dispatch ExtractAdrs{scope=Some([fixture_path]), dry_run=true} returns ExtractAdrs{files_scanned=1, adrs_inserted=0, adrs_superseded=0, ...}; adrs table count unchanged
  - Test 4 (handler persist): same fixture with dry_run=false returns adrs_inserted=3; SELECT COUNT(*) FROM adrs returns 3
  - Test 5 (idempotent re-run): re-run same handler -- adrs_inserted=0, adrs_skipped_duplicate=3 (UNIQUE constraint INSERT OR IGNORE)
  - Test 6 (supersede on doc change): mutate fixture (change one paragraph text); doc_version_sha changes; re-run handler -- adrs_inserted=1 (new), adrs_superseded=1 (old paragraph at same source_line gets superseded_by set to new id); SELECT COUNT(*) FROM adrs returns 4 (3 original + 1 new); SELECT COUNT(*) FROM adrs WHERE superseded_by IS NOT NULL returns 1 (G-E append-only)
  - Test 7 (auto-couple): `target/release/codenexus-core index --repo <fixture-repo>` runs symbol indexing AND ADR extraction; SELECT COUNT(*) FROM adrs > 0 after run completes
  - Test 8 (JSONL event written): after handler, <export-dir>/notes.jsonl contains 1+ lines with event=="extract_adrs" + payload.adrs_inserted field
  - Test 9 (MCP dispatch): Go MCP server's extract_adrs tool returns the success result, NOT the W3 "not yet implemented" stub error
 </behavior>

 <action>

**Step A -- a2a.rs variants** per `<interfaces>` block. ExtractAdrs request + response.

**Step B -- handle_extract_adrs in server.rs:**
```rust
fn handle_extract_adrs(
  db_path: &str,
  repo_root: &std::path::Path,
  scope: Option<Vec<String>>,
  dry_run: bool,
  export_dir: Option<&std::path::Path>,
) -> anyhow::Result<OperationResponse> {
  let store = codenexus_core::storage::Store::open(db_path)?;
  let cfg = codenexus_core::adr_extractor::AdrExtractorConfig::from_plugin_toml(repo_root)
    .unwrap_or_else(|_| codenexus_core::adr_extractor::AdrExtractorConfig::defaults());

  // Build path list: either explicit scope or walk via cfg.include/exclude
  let extracted: Vec<_> = match scope {
    Some(paths) => {
      let mut out = Vec::new();
      for p in paths {
        let path = std::path::PathBuf::from(&p);
        let abs = if path.is_absolute() { path } else { repo_root.join(p) };
        match codenexus_core::adr_extractor::extract_from_path(&abs) {
          Ok(mut v) => out.append(&mut v),
          Err(e) => eprintln!("[extract_adrs] skip {}: {}", abs.display(), e),
        }
      }
      out
    }
    None => codenexus_core::adr_extractor::walk_and_extract(repo_root, &cfg)?,
  };

  let mut files_scanned = std::collections::HashSet::new();
  let mut inserted = 0usize;
  let mut superseded = 0usize;
  let mut skipped = 0usize;
  let mut warnings = Vec::new();

  for para in extracted {
    files_scanned.insert(para.source_path.clone());
    if para.keyword.is_none() { continue; } // no constraint detected

    let sha = doc_version_sha(&para.source_path).unwrap_or_else(|e| {
      warnings.push(format!("git sha unavailable for {}: {}", para.source_path, e));
      content_sha(&para.source_path).unwrap_or_default()
    });
    let now = chrono::Utc::now().timestamp();

    if dry_run {
      // count + skip persistence
      inserted += 1;
      continue;
    }

    // D-W4-06 supersede dance
    let prior_at_line = store.adr_at_line(&para.source_path, para.source_line)?;
    let new_id = store.insert_adr(
      &para.source_path,
      para.source_line, para.source_end_line,
      para.heading_anchor.as_deref(),
      para.keyword.as_deref().unwrap_or(""),
      para.confidence,
      &para.paragraph_text,
      &sha,
      now,
    )?;
    match new_id {
      Some(id) => {
        inserted += 1;
        // If a prior row at this (source_path, source_line) had a different
        // doc_version_sha and is not yet superseded, supersede it.
        if let Some(prior) = prior_at_line {
          if prior.doc_version_sha != sha && prior.superseded_by.is_none() {
            store.supersede_adr(prior.id, id)?;
            superseded += 1;
          }
        }
      }
      None => {
        skipped += 1; // INSERT OR IGNORE collision (same path/line/sha)
      }
    }
  }

  // JSONL emit (G1 Mode B parity with W1)
  if !dry_run {
    let exporter = codenexus_core::jsonl_export::JsonlExporter::for_repo(repo_root, export_dir)?;
    let event = serde_json::json!({
      "event": "extract_adrs",
      "ts": chrono::Utc::now().to_rfc3339(),
      "payload": {
        "files_scanned": files_scanned.len(),
        "adrs_inserted": inserted,
        "adrs_superseded": superseded,
        "adrs_skipped_duplicate": skipped,
        "warnings": warnings.clone(),
      }
    });
    if let Err(e) = exporter.append(&event) {
      warnings.push(format!("jsonl export failed: {}", e));
    }
  }

  Ok(OperationResponse::ExtractAdrs {
    files_scanned: files_scanned.len(),
    adrs_inserted: inserted,
    adrs_superseded: superseded,
    adrs_skipped_duplicate: skipped,
    warnings,
  })
}

fn doc_version_sha(path: &str) -> Result<String> {
  use std::process::Command;
  let out = Command::new("git").args(["ls-files", "-s", path]).output()?;
  if !out.status.success() {
    anyhow::bail!("git ls-files failed");
  }
  // git ls-files -s output: "<mode> <sha> <stage>\t<path>"
  let s = String::from_utf8_lossy(&out.stdout);
  let token = s.split_whitespace().nth(1).ok_or_else(|| anyhow::anyhow!("no sha"))?;
  Ok(token.to_string())
}

fn content_sha(path: &str) -> Result<String> {
  use sha2::{Digest, Sha256};
  let bytes = std::fs::read(path)?;
  let mut hasher = Sha256::new();
  hasher.update(&bytes);
  Ok(format!("{:x}", hasher.finalize()))
}
```

`Store::adr_at_line(source_path, source_line) -> Result<Option<Adr>>` is a small new helper added to storage.rs (parallel to insert_adr). Returns the most recent NON-SUPERSEDED row at the (source_path, source_line) coordinate.

**Step C -- dispatch arm in server.rs:**
```rust
OperationRequest::ExtractAdrs { scope, dry_run } => {
  handle_extract_adrs(&db_path, &state.repo_root, scope, dry_run, state.export_dir.as_deref())
}
```

**Step D -- auto-couple in main.rs Cmd::Index** (D-W4-07). After existing index logic completes successfully:
```rust
// Phase 5 W4: auto-couple ADR extraction to index_repo per G5 section 4.1.
match handle_extract_adrs(&db_path, &repo_root, None, false, export_dir.as_deref()) {
  Ok(OperationResponse::ExtractAdrs { adrs_inserted, adrs_superseded, .. }) => {
    eprintln!("[codenexus] ADR extraction: {} inserted, {} superseded", adrs_inserted, adrs_superseded);
  }
  Err(e) => {
    // NON-FATAL per D-W4-07
    eprintln!("[codenexus] WARN: ADR extraction failed: {}", e);
  }
  _ => {}
}
```

**Step E -- Go MCP server real dispatch** in server/internal/mcpsrv/server.go. Replace the W3 stub `mcp.NewToolResultError(...)` for extract_adrs with real dispatch via `client.ExtractAdrs(scope)`. `client.ExtractAdrs` in proxy/a2a.go was a stub in W3; fill body now.

**Step F -- proxy.a2a.go fill ExtractAdrs body:**
```go
func (c *Client) ExtractAdrs(scope []string) (json.RawMessage, error) {
  op := map[string]interface{}{
    "extract_adrs": map[string]interface{}{
      "scope": scope, // omitempty if nil
    },
  }
  body := map[string]interface{}{"operation": op}
  // POST /tasks/send + poll, mirroring existing patterns
  return c.dispatch(body)
}
```

**Step G -- Cargo.toml dev-deps.** Add `sha2 = "0.10"` if not already present (for content_sha fallback). Verify `walkdir` + `globset` are in deps (likely already from existing parser work; if not, add).

**Step H -- verify Rust + Go builds + run tests:**
```bash
cd D:/projects/codenexus/experiments/poc-retrieval && cargo build --workspace --release
cd D:/projects/codenexus/experiments/poc-retrieval && cargo test -p codenexus-core extract_adrs -- --test-threads=1
cd D:/projects/codenexus/server && go build ./... && go vet ./...
```

 </action>

 <acceptance_criteria>
  - `grep -nE 'OperationRequest::ExtractAdrs' experiments/poc-retrieval/core/src/server.rs` >= 1 hit
  - `grep -nE 'fn handle_extract_adrs' experiments/poc-retrieval/core/src/server.rs` exactly 1 hit
  - `grep -nE 'pub fn adr_at_line' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nF 'handle_extract_adrs' experiments/poc-retrieval/core/src/main.rs` >= 1 hit (auto-couple path)
  - `grep -nF '"event":"extract_adrs"' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (or with spaces; verify formatting)
  - `grep -nF 'extract_adrs not yet implemented' server/internal/mcpsrv/server.go` returns 0 hits (W3 stub removed; W4 replaces)
  - `grep -nE 'client\.ExtractAdrs' server/internal/mcpsrv/server.go` >= 1 hit (real dispatch)
  - `grep -nE 'func \(c \*Client\) ExtractAdrs' server/internal/proxy/a2a.go` exactly 1 hit (real body)
  - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -cE '^error'` returns 0 (G-A Rust)
  - `cd server && go build ./... && go vet ./...` exits 0 (G-A Go)
  - `cd experiments/poc-retrieval && cargo test -p codenexus-core extract_adrs adr_extractor -- --test-threads=1` >= 8 tests pass (G-B + G-C + G-E)
  - JSONL event side-effect verified by Test 8 (G-D parity with W1's JSONL discipline)
  - All W0-W3 + 04.5-03 tests still green (G-B)
 </acceptance_criteria>

 <verify>
  <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core -- --test-threads=1 2>&1 | tail -30 && cd ../../server && go build ./... && go vet ./...</automated>
 </verify>

 <done>
  a2a.rs has ExtractAdrs request/response. server.rs has
  handle_extract_adrs (~120 LOC including dry_run, supersede,
  JSONL emit) + dispatch arm + storage.rs adr_at_line helper.
  main.rs Cmd::Index auto-couples to extract_adrs (NON-FATAL on
  failure). Go MCP server replaces W3 stub with real dispatch;
  proxy.Client.ExtractAdrs body filled. >= 8 tests pass covering
  a2a parse + dry_run + persist + idempotent re-run + supersede
  on doc change + auto-couple via Cmd::Index + JSONL side-effect
  + MCP dispatch. Build clean (G-A); supersede semantics correct
  (G-E); auto-couple verified (G-C).
 </done>
</task>

</tasks>

<gates>
- **G-A** (build clean): Rust + Go both build clean; tree-sitter-md added cleanly. [Tasks 1, 2]
- **G-B** (regression-green): all W0-W3 + 04.5-03 tests pass; new tests >= 13 added (5 extractor + 8 handler/dispatch). [Tasks 1, 2]
- **G-C** (extract_adrs auto-coupled): `target/release/codenexus-core index --repo <fixture>` triggers ADR extraction after symbol indexing; adrs table populated. [Task 2]
- **G-D** (adr_symbol_links populated): query_constraints scope=symbol returns ADR rows when ADRs exist for the symbol's file (via list_adrs_for_file overlap). Note: explicit text-mention link insertion is V1.1+ (G5 section 5.1 file mode uses file_overlap fallback today). [Task 2]
- **G-E** (supersede append-only): re-running extract_adrs on changed source preserves prior rows + sets superseded_by; SELECT COUNT(*) FROM adrs increases over runs. [Task 2]
</gates>

<must_haves>
truths:
 - "Agent can call extract_adrs(scope?, dry_run?) and receive {files_scanned, adrs_inserted, adrs_superseded, adrs_skipped_duplicate, warnings}"
 - "Default include = docs/**/*.md + .planning/*.md (one-level) + .planning/phases/**/*-PLAN.md + README.md per G5 section 1.1"
 - "Default exclude = .planning/audits + probes + research per G5 section 1.1; silent ignore per UQ-B6"
 - "RFC 2119 keywords (PRIMARY: MUST/MUST NOT/SHALL/REQUIRED; SECONDARY: SHOULD/SHOULD NOT/RECOMMENDED; WEAK: MAY/OPTIONAL) extracted with confidence 1.0/0.7/0.4 per G5 section 2.2"
 - "Code-fence regions skipped (D-W4-02); inline backticks stripped (OQ2 from G5)"
 - "Per-paragraph granularity via tree-sitter-md (UQ-B5)"
 - "Storage: adrs table UNIQUE (source_path, source_line, doc_version_sha) prevents duplicate inserts"
 - "Supersede append-only: re-running on changed source inserts new + sets old.superseded_by (UQ-A4)"
 - "doc_version_sha = git blob sha (preferred) OR content sha2 (fallback when git unavailable)"
 - "Auto-coupled: codenexus-core index --repo X also runs ADR extraction; failure NON-FATAL"
 - "JSONL event emitted per extract_adrs invocation (G1 Mode B parity)"
 - "MCP server dispatches extract_adrs through proxy.Client; W3 stub error gone"
artifacts:
 - path: "experiments/poc-retrieval/core/src/adr_extractor.rs"
  provides: "AdrExtractorConfig + extract_from_path + walk_and_extract + matches_whole_word"
  contains: "pub fn walk_and_extract"
 - path: "experiments/poc-retrieval/core/Cargo.toml"
  provides: "tree-sitter-md (or tree-sitter-markdown) dep + sha2 dep + walkdir + globset"
  contains: "tree-sitter"
 - path: "experiments/poc-retrieval/core/src/server.rs"
  provides: "handle_extract_adrs + dispatch arm"
  contains: "fn handle_extract_adrs"
 - path: "experiments/poc-retrieval/core/src/main.rs"
  provides: "Cmd::Index auto-couple to extract_adrs"
  contains: "handle_extract_adrs"
 - path: "server/internal/mcpsrv/server.go"
  provides: "extract_adrs real dispatch (replaces W3 stub)"
  contains: "client.ExtractAdrs"
 - path: "server/internal/proxy/a2a.go"
  provides: "ExtractAdrs Client method body"
  contains: "func (c *Client) ExtractAdrs"
key_links:
 - from: "core/src/server.rs::handle_extract_adrs"
  to: "core/src/adr_extractor.rs::walk_and_extract"
  via: "scope=None walks default include globs"
  pattern: "walk_and_extract"
 - from: "core/src/server.rs::handle_extract_adrs"
  to: "core/src/storage.rs::insert_adr + supersede_adr + adr_at_line"
  via: "persistence + supersede chain"
  pattern: "insert_adr|supersede_adr|adr_at_line"
 - from: "core/src/main.rs::Cmd::Index"
  to: "core/src/server.rs::handle_extract_adrs"
  via: "auto-couple post-index"
  pattern: "handle_extract_adrs"
</must_haves>

<verification>
1. Rust + Go builds clean (G-A)
2. Extractor + dispatch tests >= 13 green (G-B + G-C + G-E)
3. `target/release/codenexus-core index --repo D:/projects/codenexus` after fresh DB -> SELECT COUNT(*) FROM adrs > 0 (G-C)
4. Mutate docs/ARCHITECTURE.md line 305 paragraph + re-index -> SELECT COUNT(*) FROM adrs WHERE superseded_by IS NOT NULL >= 1 (G-E)
5. Go MCP `extract_adrs` no longer returns "not yet implemented" (G-D)
</verification>

<open_questions>
- **OQ-W4-01:** tree-sitter-md vs tree-sitter-markdown crate naming -- executor verifies current crate name + version at execution time per UQ-B5 + D-W4-01.
- **OQ-W4-02:** plugin.toml `[adr]` section parsing -- if no plugin.toml exists in repo root, defaults() is used silently. Should W4 emit a one-line "[codenexus] using default ADR config" on first run? Recommend: yes, debug-level only (UQ-B6).
- **OQ-W4-03:** adr_symbol_links auto-population -- W4 ships with file_overlap as the only link kind (via list_adrs_for_file in W2 query handler). text_mention (paragraph mentions a symbol name) and topic_match (vector similarity) are V1.1+. Plan-checker confirms G-D scope.
</open_questions>

<honest_gap_list>
**P1**:
- (none)

**P2**:
- tree-sitter-md crate API may differ from the sketched code in Step B. Executor adapts walk_paragraphs to actual node names. SUMMARY documents API delta.
- doc_version_sha via `git ls-files` requires git as a runtime dep. Per PROJECT.md fat-binary constraint, this is acceptable because users running codenexus on a non-git directory get the content_sha fallback. SUMMARY documents both paths.
- D-W4-08 (meta-mention dedupe via source-weight) is sketched but not fully implemented in Step B's code outline. If W4 ships without it, OQ7 from G5 stays open and gets fixed in W5 polish. P2 because: query_constraints results may contain duplicate paragraph_text rows from PROJECT.md quoting ARCHITECTURE.md verbatim. UX consequence: agent sees the same constraint twice in get_edit_context. Acceptable for V1.0; W6 eval will surface if it's a real problem.

**P3**:
- Tests 6 + 7 (live ARCHITECTURE.md self-test) depend on the file containing the expected MUST NOT statements at expected lines. If those move, test breaks. Mitigation: test asserts EXISTENCE of >= 1 MUST_NOT extraction, not exact line.
- AdrExtractorConfig::patterns are 'static slices for V1.0; runtime config from plugin.toml shadows them but cannot ADD beyond the curated list (DoS surface per G5 section 1.2). Plan-checker confirms acceptable.
- adr_symbol_links text_mention link kind is V1.1+. W4 ships with empty link table for new ADRs; query_constraints scope=symbol returns ADRs only via list_adrs_for_symbol (which queries adr_symbol_links and returns []) AND via list_adrs_for_file (file_overlap fallback). G-D verified via the file_overlap path; explicit symbol mention links are deferred.
</honest_gap_list>
</content>
