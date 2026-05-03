---
phase: 5
slice: 05-W0
plan_id: 05-W0
title: "W0: Storage layer -- notes + adrs + adr_symbol_links + JSONL export hook scaffold"
wave: 0
depends_on: []
status: PLAN-AUTHORED (awaits plan-checker iter)
files_modified:
 - experiments/poc-retrieval/core/src/storage.rs
 - experiments/poc-retrieval/core/src/types.rs
 - experiments/poc-retrieval/core/src/lib.rs
 - experiments/poc-retrieval/core/src/jsonl_export.rs
locked_decisions_honored:
 - G1  # Mode B self-contained SQLite + JSONL export hook scaffold
 - G3  # minimal-plus-3 notes schema; append-only supersede; fnk persist
 - G5  # adrs + adr_symbol_links separate tables (NOT Symbol kind=ADR); FTS5 contentless
 - UQ-A1  # JSONL export dest = <repo>/.codenexus/notes-export/ with --export-dir override
 - UQ-A4  # ADR supersede = history-preserving append-only (matches G3)
gates:
 - G-A  # build clean; no NEW warnings beyond pre-existing dead-code
 - G-B  # all new W0 unit tests pass
 - G-C  # schema migration loud-error: pre-W0 DB triggers clear "schema not migrated" message
---

> **!! PROVISIONAL !!** This plan was authored 2026-05-03 in parallel with
> CCG round 2 challenge. Codex surfaced 4 critical issues (CI-1 G2 LOC,
> CI-2 G3 SQL FK, CI-3 G4 handler, CI-4 G5 FTS5) plus 3 missed constraints
> that affect this slice. **Do NOT execute this plan as-is.** See
> `.planning/phases/codenexus-05-bridge-memory-mvp/05-CCG-ROUND-2-FINDINGS.md`
> for required amendments before plan-checker iter and execution.


<objective>
Land the W0 storage layer for Phase 5 Bridge memory MVP. After W0, the
SQLite schema knows about three new tables (`symbol_notes`, `adrs`,
`adr_symbol_links`) plus FTS5 virtual tables for the constraints corpus,
plus a JSONL export hook scaffold (per UQ-A1: writes to
`<repo>/.codenexus/notes-export/notes.jsonl` with `--export-dir` CLI
override). The hook is WRITE-ONLY in V1.0 -- no reader; V1.1+ memU
integration replays JSONL into MemoryService.memorize(...) per G1 Mode B.

Storage key policy is locked by drift probe (commit d5e5eb0): notes
persist on `(path, name, kind)` triple per M5_fnk = 1.0 evidence. ADRs
persist on `(source_path, source_line, doc_version_sha)` triple
mirroring the same drift-safe identity discipline (G5 section 3.2 DDL).

W1 / W2 / W3 / W4 all consume the storage primitives created here. W0
ships ZERO A2A surface -- it is pure schema + insert/list/clear APIs +
the JSONL writer. Anything else is out-of-scope for W0.

Purpose: separate the SQL schema migration from the A2A op surface so
each risk surface is isolated. If schema migration breaks something, we
discover it on a poc.db reindex (~30s) before we touch a2a.rs +
server.rs (W1+).

Output:
- `core/src/storage.rs`: 3 new tables (symbol_notes, adrs,
 adr_symbol_links) + 2 FTS5 virtual tables (constraints_fts,
 adrs_fts) + insert/list/clear APIs for each + has_symbol_notes /
 has_adrs migration helpers (parallel to W0/04.5-03's
 has_imports_edges + has_alias_decls pattern).
- `core/src/types.rs`: append `SymbolNote`, `Adr`, `AdrSymbolLink`,
 `Severity` enum (Must/MustNot/Should/Note) per G3 + G5 schema lock.
- `core/src/lib.rs`: re-export `pub mod jsonl_export`.
- `core/src/jsonl_export.rs` (NEW): write-only event log. One event per
 remember_symbol_note / extract_adrs invocation. Append-only NDJSON.
 Default dest = `<repo>/.codenexus/notes-export/notes.jsonl`;
 `--export-dir` override threaded through Cmd::Index / Cmd::Serve.
</objective>

<plan_time_decisions>
- **D-W0-01 (jsonl_export module placement):** New file
 `core/src/jsonl_export.rs` rather than embedded inside storage.rs.
 Rationale: storage.rs is already 456 lines; jsonl_export is a
 side-effect surface (filesystem write) distinct from SQLite CRUD.
 Keeping it separate makes the V1.1+ memU reader easy to bolt on
 (separate consumer, separate file).
- **D-W0-02 (FTS5 mode):** `contentless` for both constraints_fts and
 adrs_fts (per G5 section 3.2 DDL `content='symbol_notes'` /
 `content='adrs'`). Saves disk; text already lives in the source
 table. UPDATE/DELETE triggers added to keep FTS index synchronized
 on supersede inserts (notes superseded -> NEW row inserted, NOT
 UPDATE; FTS5 contentless model handles INSERT-only flow naturally).
- **D-W0-03 (Severity enum):** Lives in types.rs as
 `pub enum Severity { Must, MustNot, Should, Note }` with serde
 rename_all = "snake_case" (matches existing CONTEXT.md vocab
 convention). Stored as TEXT in SQLite (not INTEGER) for grep-ability
 in raw `sqlite3` debugging.
- **D-W0-04 (notes table column name):** `symbol_notes` rather than
 `notes`. Reason: avoids ambiguity with potential V1.1+ "doc notes"
 / "session notes" distinction; matches G3 naming.
- **D-W0-05 (--export-dir wiring scope):** W0 lands the CLI flag
 parsing in main.rs Cmd::Index / Cmd::Serve AND a config struct that
 jsonl_export consumes. The actual write-on-A2A-call is W1 work
 (remember_symbol_note handler). W0 verifies the writer compiles +
 unit-tests open/append a fixture file.
</plan_time_decisions>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/codenexus-05-bridge-memory-mvp/05-DISCUSS-SUMMARY.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-adr.md
@.planning/probes/runs/2026-05-03-drift-evidence.md
@CONTEXT.md
@experiments/poc-retrieval/core/src/storage.rs
@experiments/poc-retrieval/core/src/types.rs
@experiments/poc-retrieval/core/src/lib.rs

<interfaces>
<!-- Existing storage.rs surface (verified 2026-05-03) -->

```rust
// core/src/storage.rs
pub struct Store { conn: Connection }
impl Store {
  pub fn open(path: &str) -> Result<Self>;                    // line 11
  pub fn symbols_in_file_full(...) -> Result<Vec<Symbol>>;            // line 216
  pub fn symbol_by_id(&self, id: i64) -> Result<Option<(String, String, String)>>; // line 255 -- returns (path, name, kind)
  pub fn bm25(&self, query: &str, k: usize) -> Result<Vec<(i64, f32)>>;     // line 348
  pub fn all_embeddings(&self) -> Result<Vec<(i64, Vec<f32>)>>;         // line 361
  pub fn fetch(&self, id: i64) -> Result<Symbol>;                // line 375
  // 04.5-03 W0 additions:
  pub fn insert_alias_decl(file, alias, target_file, target_member) -> Result<()>;
  pub fn list_alias_decls(file) -> Result<Vec<AliasDecl>>;
  pub fn clear_alias_decls() -> Result<()>;
  pub fn has_imports_edges() -> Result<bool>;
  pub fn has_alias_decls() -> Result<bool>;
}
```

```rust
// core/src/types.rs (current state -- 04.5-03 W0)
pub struct AliasDecl {
  pub from_file: String,
  pub alias: String,
  pub target_file: String,
  pub target_member: Option<String>,
}
```

```sql
-- Target schema after Phase 5 W0 (additive; existing tables unchanged):

CREATE TABLE IF NOT EXISTS symbol_notes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  -- fnk identity (drift-probe-vindicated; M5_fnk = 1.0)
  path TEXT NOT NULL,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  -- payload
  note_text TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '[]',     -- JSON array; SQLite has no Vec<T>
  confidence REAL NOT NULL,         -- caller-supplied [0, 1]
  source_session TEXT NOT NULL,       -- caller agent session id
  -- lifecycle
  supersedes_note_id INTEGER REFERENCES symbol_notes(id),
  created_at TEXT NOT NULL         -- ISO 8601, server-set
);
CREATE INDEX IF NOT EXISTS notes_fnk
 ON symbol_notes(path, name, kind);
CREATE INDEX IF NOT EXISTS notes_supersedes
 ON symbol_notes(supersedes_note_id)
 WHERE supersedes_note_id IS NOT NULL;
CREATE VIRTUAL TABLE IF NOT EXISTS constraints_fts USING fts5(
  note_text, tags, content='symbol_notes', content_rowid='id'
);
CREATE TRIGGER IF NOT EXISTS symbol_notes_ai AFTER INSERT ON symbol_notes BEGIN
 INSERT INTO constraints_fts(rowid, note_text, tags)
 VALUES (new.id, new.note_text, new.tags);
END;

CREATE TABLE IF NOT EXISTS adrs (
 id       INTEGER PRIMARY KEY AUTOINCREMENT,
 source_path   TEXT NOT NULL,
 source_line   INTEGER NOT NULL,
 source_end_line INTEGER NOT NULL,
 heading_anchor TEXT,
 keyword     TEXT NOT NULL,      -- MUST_NOT|MUST|SHOULD_NOT|SHOULD|MAY
 confidence   REAL NOT NULL,      -- 1.0 / 0.7 / 0.4 per G5 section 2.2
 paragraph_text TEXT NOT NULL,
 doc_version_sha TEXT NOT NULL,      -- git blob sha of source_path
 extracted_at  INTEGER NOT NULL,
 superseded_by  INTEGER REFERENCES adrs(id),
 UNIQUE (source_path, source_line, doc_version_sha)
);
CREATE INDEX IF NOT EXISTS idx_adrs_active ON adrs(source_path) WHERE superseded_by IS NULL;
CREATE INDEX IF NOT EXISTS idx_adrs_keyword ON adrs(keyword, confidence DESC);
CREATE VIRTUAL TABLE IF NOT EXISTS adrs_fts USING fts5(
 paragraph_text, heading_anchor, content='adrs', content_rowid='id', tokenize='unicode61'
);
CREATE TRIGGER IF NOT EXISTS adrs_ai AFTER INSERT ON adrs BEGIN
 INSERT INTO adrs_fts(rowid, paragraph_text, heading_anchor)
 VALUES (new.id, new.paragraph_text, new.heading_anchor);
END;

CREATE TABLE IF NOT EXISTS adr_symbol_links (
 adr_id   INTEGER NOT NULL REFERENCES adrs(id),
 symbol_id INTEGER NOT NULL REFERENCES symbols(id),
 link_kind TEXT NOT NULL,  -- mention|topic_match|file_overlap
 score   REAL NOT NULL,
 PRIMARY KEY (adr_id, symbol_id, link_kind)
);
CREATE INDEX IF NOT EXISTS idx_adr_links_symbol ON adr_symbol_links(symbol_id, score DESC);
```

```rust
// New types in core/src/types.rs (W0 additions):

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
  Must,
  MustNot,
  Should,
  Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolNote {
  pub id: i64,
  pub path: String,
  pub name: String,
  pub kind: String,
  pub note_text: String,
  pub tags: Vec<String>,
  pub confidence: f32,
  pub source_session: String,
  pub supersedes_note_id: Option<i64>,
  pub created_at: String,        // ISO 8601
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adr {
  pub id: i64,
  pub source_path: String,
  pub source_line: i64,
  pub source_end_line: i64,
  pub heading_anchor: Option<String>,
  pub keyword: String,
  pub confidence: f32,
  pub paragraph_text: String,
  pub doc_version_sha: String,
  pub extracted_at: i64,
  pub superseded_by: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdrSymbolLink {
  pub adr_id: i64,
  pub symbol_id: i64,
  pub link_kind: String,
  pub score: f32,
}
```

```rust
// New module core/src/jsonl_export.rs (W0 scaffold):

pub struct JsonlExporter {
  path: PathBuf,
}

impl JsonlExporter {
  /// Resolve dest dir per UQ-A1: <repo>/.codenexus/notes-export/notes.jsonl
  /// with override from --export-dir CLI flag.
  pub fn for_repo(repo_root: &Path, override_dir: Option<&Path>) -> Result<Self>;

  /// Append one event line (NDJSON). Event is JSON object with at least:
  /// {"event": "remember_symbol_note" | "extract_adrs" | ...,
  /// "ts": "<ISO8601>", "payload": {...}}.
  /// Payload shape is event-specific; W1+ handlers fill them.
  pub fn append(&self, event: &serde_json::Value) -> Result<()>;
}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
 <name>Task 1: Storage schema + Rust types + insert/list/clear APIs</name>
 <files>experiments/poc-retrieval/core/src/types.rs, experiments/poc-retrieval/core/src/storage.rs</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md G3 SQL DDL (lines 177-203)
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-adr.md G5 section 3.2 DDL (lines 130-162)
  - .planning/probes/runs/2026-05-03-drift-evidence.md (M5_fnk = 1.0 -> (path, name, kind) keying lock)
  - experiments/poc-retrieval/core/src/storage.rs Store::open (lines 13-39 schema execute_batch) AND existing helpers (symbol_by_id line 255; insert_alias_decl pattern)
  - experiments/poc-retrieval/core/src/types.rs (current AliasDecl placement; append new types here)
  - CONTEXT.md lines 13-65 (vocab lock; do NOT introduce "node" / "entity" / "score" / "weight")
 </read_first>

 <behavior>
  - Test 1 (compile): `cargo build -p codenexus-core` exits 0 with no NEW warnings
  - Test 2 (3 tables present): `SELECT name FROM sqlite_master WHERE type='table' AND name IN ('symbol_notes','adrs','adr_symbol_links')` returns 3 rows on fresh `Store::open(":memory:")`
  - Test 3 (2 FTS5 virtual tables present): `SELECT name FROM sqlite_master WHERE name IN ('constraints_fts','adrs_fts')` returns 2 rows
  - Test 4 (notes_fnk index present): `SELECT name FROM sqlite_master WHERE type='index' AND name='notes_fnk'` returns 1 row
  - Test 5 (note insert + list roundtrip): insert one note via `insert_symbol_note(path, name, kind, note_text, tags=["must"], confidence=0.9, source_session="s1", supersedes=None)` returning note_id; then `list_notes_for_symbol(path, name, kind, include_history=false)` returns 1 row with is_active_leaf=true
  - Test 6 (supersede append-only): insert note A; insert note B with supersedes_note_id=A.id; `list_notes_for_symbol(..., include_history=false)` returns ONLY B (B is active leaf, A is hidden); `list_notes_for_symbol(..., include_history=true)` returns BOTH A and B with is_active_leaf=false on A and true on B
  - Test 7 (ADR insert + FTS roundtrip): insert one ADR row with paragraph_text="MUST NOT introduce reranker without LLM-judge eval"; `bm25_adrs("reranker", 5)` returns at least 1 hit (FTS index updated by trigger)
  - Test 8 (ADR UNIQUE drift-safe): insert (path="docs/ARCHITECTURE.md", line=508, sha="abc") twice -- second is INSERT OR IGNORE silent drop; SELECT COUNT(*) returns 1
  - Test 9 (adr_symbol_links insert + lookup): insert link (adr_id=1, symbol_id=42, link_kind="mention", score=0.85); `list_adrs_for_symbol(42)` returns 1 ADR
  - Test 10 (clear_symbol_notes): insert 2 notes; `clear_symbol_notes()` empties table; FTS5 trigger keeps constraints_fts in sync via DELETE cascade (or explicit DELETE FROM constraints_fts)
  - Test 11 (existing schema unchanged): `cargo test -p codenexus-core --lib graph_build::tests -- --test-threads=1` exits 0 with all tests still green (additive schema only)
 </behavior>

 <action>

**Step A -- extend `core/src/types.rs`.** Append `Severity` enum + `SymbolNote` + `Adr` + `AdrSymbolLink` structs after the existing `AliasDecl`. Use `#[derive(Debug, Clone, Serialize, Deserialize)]` consistently; `Severity` adds `PartialEq, Eq` for assert_eq! in tests. Required imports: `use serde::{Serialize, Deserialize};` (verify already present).

**Step B -- extend `Store::open` schema** in `core/src/storage.rs::open` execute_batch block. APPEND (do not modify existing CREATE TABLE / INDEX / TRIGGER for symbols / edges / alias_decls) the following SQL exactly as shown in the `<interfaces>` block:
- `CREATE TABLE IF NOT EXISTS symbol_notes (...)`
- `CREATE INDEX IF NOT EXISTS notes_fnk ON symbol_notes(path, name, kind)`
- `CREATE INDEX IF NOT EXISTS notes_supersedes ...`
- `CREATE VIRTUAL TABLE IF NOT EXISTS constraints_fts USING fts5(...)` (contentless mode, content='symbol_notes')
- `CREATE TRIGGER IF NOT EXISTS symbol_notes_ai AFTER INSERT ON symbol_notes ...`
- `CREATE TABLE IF NOT EXISTS adrs (...)` with UNIQUE(source_path, source_line, doc_version_sha)
- `CREATE INDEX IF NOT EXISTS idx_adrs_active` (partial index WHERE superseded_by IS NULL)
- `CREATE INDEX IF NOT EXISTS idx_adrs_keyword`
- `CREATE VIRTUAL TABLE IF NOT EXISTS adrs_fts USING fts5(...)` (contentless mode, content='adrs')
- `CREATE TRIGGER IF NOT EXISTS adrs_ai AFTER INSERT ON adrs ...`
- `CREATE TABLE IF NOT EXISTS adr_symbol_links (...)` PRIMARY KEY (adr_id, symbol_id, link_kind)
- `CREATE INDEX IF NOT EXISTS idx_adr_links_symbol`

**Step C -- add insert/list/clear APIs** in `core/src/storage.rs`. Place after the existing `clear_alias_decls` API (preserves the 04.5-03 W0 grouping convention). Use `INSERT OR IGNORE` for ADR uniqueness; plain INSERT for symbol_notes (notes are append-only, never collide -- separate id PK).

Required APIs (signatures):
```rust
pub fn insert_symbol_note(
  &self,
  path: &str, name: &str, kind: &str,
  note_text: &str,
  tags: &[String],     // serialized to JSON array TEXT
  confidence: f32,
  source_session: &str,
  supersedes_note_id: Option<i64>,
  created_at: &str,     // ISO 8601 (caller passes; W1 handler computes)
) -> Result<i64>;       // returns note_id

pub fn list_notes_for_symbol(
  &self,
  path: &str, name: &str, kind: &str,
  include_history: bool,  // false = active leaves only; true = full chain
) -> Result<Vec<SymbolNote>>;

pub fn get_note_by_id(&self, note_id: i64) -> Result<Option<SymbolNote>>;

pub fn clear_symbol_notes(&self) -> Result<()>; // for test fixtures only

pub fn bm25_constraints(&self, query: &str, k: usize) -> Result<Vec<(i64, f32)>>;
// Same shape as Store::bm25; queries constraints_fts.

pub fn insert_adr(
  &self,
  source_path: &str, source_line: i64, source_end_line: i64,
  heading_anchor: Option<&str>,
  keyword: &str, confidence: f32,
  paragraph_text: &str,
  doc_version_sha: &str,
  extracted_at: i64,
) -> Result<Option<i64>>;   // None if INSERT OR IGNORE dropped duplicate

pub fn supersede_adr(&self, old_id: i64, new_id: i64) -> Result<()>;

pub fn list_adrs_for_symbol(&self, symbol_id: i64) -> Result<Vec<Adr>>;

pub fn list_adrs_for_file(&self, source_path: &str) -> Result<Vec<Adr>>;

pub fn bm25_adrs(&self, query: &str, k: usize) -> Result<Vec<(i64, f32)>>;
// Queries adrs_fts.

pub fn insert_adr_symbol_link(
  &self,
  adr_id: i64, symbol_id: i64, link_kind: &str, score: f32,
) -> Result<()>;

pub fn clear_adrs(&self) -> Result<()>;       // for re-extraction
pub fn clear_adr_symbol_links(&self) -> Result<()>;
```

Active-leaf semantics for `list_notes_for_symbol`:
- Active leaves = notes where `id` does NOT appear in any other note's `supersedes_note_id` column.
- SQL: `WHERE id NOT IN (SELECT supersedes_note_id FROM symbol_notes WHERE supersedes_note_id IS NOT NULL)`.
- `include_history=true` returns ALL rows for the (path, name, kind) triple, with each row's `is_active_leaf` computed from the same NOT IN check.

The `SymbolNote.is_active_leaf` field is COMPUTED at query time (not stored); the struct in types.rs does NOT have this field by itself -- W2 may add a wrapper `NoteView` struct that includes it. For W0 the boolean is computed inline by the caller via the SQL. (Reconciliation note for plan-checker: G3's `NoteView` struct in 05-discuss-api.md line 225-235 includes `is_active_leaf`; W0 ships `SymbolNote` matching the table columns and W2 adds `NoteView` as the API-surface wrapper.)

**Step D -- write unit tests** in `core/src/storage.rs` (new `mod symbol_notes_tests` and `mod adrs_tests`, parallel to 04.5-03's `mod alias_decls_tests`). Implement Tests 2-10 from `<behavior>` above, using `Store::open(":memory:")`. ISO 8601 fixture: `"2026-05-03T12:00:00Z"`.

**Step E -- verify existing tests stay green:**
```bash
cd D:/projects/codenexus/experiments/poc-retrieval
cargo test -p codenexus-core --lib graph_build::tests -- --test-threads=1 2>&1 | tail -15
cargo test -p codenexus-core --lib alias_decls_tests -- --test-threads=1 2>&1 | tail -15
```
Expected: both green; W0 schema additions are pure-additive.

 </action>

 <acceptance_criteria>
  - `grep -nE 'pub enum Severity' experiments/poc-retrieval/core/src/types.rs` exactly 1 hit
  - `grep -nE 'pub struct SymbolNote' experiments/poc-retrieval/core/src/types.rs` exactly 1 hit
  - `grep -nE 'pub struct Adr' experiments/poc-retrieval/core/src/types.rs` exactly 1 hit (do NOT match `AdrSymbolLink` or `AliasDecl` -- use whole-word `\bAdr\b`)
  - `grep -nE 'pub struct AdrSymbolLink' experiments/poc-retrieval/core/src/types.rs` exactly 1 hit
  - `grep -nE 'CREATE TABLE IF NOT EXISTS symbol_notes' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'CREATE TABLE IF NOT EXISTS adrs' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'CREATE TABLE IF NOT EXISTS adr_symbol_links' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'CREATE VIRTUAL TABLE IF NOT EXISTS constraints_fts' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'CREATE VIRTUAL TABLE IF NOT EXISTS adrs_fts' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'pub fn insert_symbol_note' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'pub fn list_notes_for_symbol' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'pub fn insert_adr\b' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'pub fn list_adrs_for_symbol' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'pub fn bm25_constraints' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'pub fn bm25_adrs' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -E '^(error|warning)' | grep -vE '(resolve|personalized_pagerank|count_edges_by_kind_conf)' | wc -l` returns 0 (G-A: no new warnings beyond 3 pre-existing dead-code)
  - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib symbol_notes_tests -- --test-threads=1` exits 0 with >= 5 tests passing
  - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib adrs_tests -- --test-threads=1` exits 0 with >= 4 tests passing
  - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib graph_build::tests -- --test-threads=1` exits 0 with all 7 tests still green (G-B: regression check)
  - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib alias_decls_tests -- --test-threads=1` exits 0 (04.5-03 W0 stays green)
 </acceptance_criteria>

 <verify>
  <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core --lib symbol_notes_tests -- --test-threads=1 && cargo test -p codenexus-core --lib adrs_tests -- --test-threads=1 && cargo test -p codenexus-core --lib graph_build::tests -- --test-threads=1 && cargo test -p codenexus-core --lib alias_decls_tests -- --test-threads=1</automated>
 </verify>

 <done>
  types.rs has Severity + SymbolNote + Adr + AdrSymbolLink. storage.rs
  Store::open creates symbol_notes / adrs / adr_symbol_links tables +
  constraints_fts / adrs_fts virtual tables + AFTER INSERT triggers
  + all required indexes. Insert/list/clear APIs + bm25_constraints +
  bm25_adrs APIs landed. >= 9 new unit tests pass. T1-T7 +
  alias_decls_tests stay green. Build clean (G-A). G-B (regression)
  verified.
 </done>
</task>

<task type="auto" tdd="true">
 <name>Task 2: jsonl_export module + CLI --export-dir wiring + schema migration check</name>
 <files>experiments/poc-retrieval/core/src/jsonl_export.rs, experiments/poc-retrieval/core/src/lib.rs, experiments/poc-retrieval/core/src/main.rs, experiments/poc-retrieval/core/src/storage.rs</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-strategic.md G1 Mode B (lines 83-101) AND open question 1 (UQ-A1 default)
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-DISCUSS-SUMMARY.md UQ-A1 default (CodeNexus-owned dir + --export-dir override)
  - experiments/poc-retrieval/core/src/lib.rs (current pub mod list)
  - experiments/poc-retrieval/core/src/main.rs Cmd::Index + Cmd::Serve match arms (grep `Cmd::`)
  - experiments/poc-retrieval/core/src/storage.rs has_imports_edges / has_alias_decls (template for has_symbol_notes / has_adrs)
 </read_first>

 <behavior>
  - Test 1 (module compiles + exported): `cargo build -p codenexus-core` exits 0; `grep 'pub mod jsonl_export' core/src/lib.rs` returns 1 hit
  - Test 2 (default path resolution): `JsonlExporter::for_repo(Path::new("/tmp/fakerepo"), None)` resolves to `/tmp/fakerepo/.codenexus/notes-export/notes.jsonl` (parent dirs auto-created)
  - Test 3 (override path resolution): `JsonlExporter::for_repo(Path::new("/tmp/fakerepo"), Some(Path::new("/tmp/myexport")))` resolves to `/tmp/myexport/notes.jsonl`
  - Test 4 (append NDJSON): exporter.append(json!({"event": "remember_symbol_note", "ts": "2026-05-03T12:00:00Z", "payload": {"path": "x.rs", "name": "f", "kind": "function"}})) writes one line; second append writes a second line; file contents have 2 lines, each parseable as JSON
  - Test 5 (CLI flag parsed): `target/release/codenexus-core index --help` shows `--export-dir <PATH>` flag in output
  - Test 6 (schema migration check fires): synthetic pre-W0 DB (only symbols + edges + alias_decls tables, NO symbol_notes / adrs) triggers `eprintln!("[codenexus] Phase 5 schema not migrated...")` + non-zero exit code. Fresh DB / post-W0 DB does NOT trigger.
 </behavior>

 <action>

**Step A -- create `core/src/jsonl_export.rs`.** New file:
```rust
//! Write-only JSONL event log for Phase 5 Bridge memU integration scaffold
//! (G1 Mode B). Per UQ-A1: default dest = <repo>/.codenexus/notes-export/
//! notes.jsonl with --export-dir CLI override. V1.0 ships writer-only;
//! V1.1+ adds reader that replays into memU MemoryService.memorize(...).
//!
//! Append-only NDJSON: each line is one event JSON object with at least
//! {"event": "<name>", "ts": "<ISO8601>", "payload": {...}}. Schema is
//! event-specific; W1 (remember_symbol_note) and W4 (extract_adrs) define
//! the payloads.

use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct JsonlExporter {
  pub path: PathBuf,
}

impl JsonlExporter {
  pub fn for_repo(repo_root: &Path, override_dir: Option<&Path>) -> Result<Self> {
    let dir: PathBuf = match override_dir {
      Some(d) => d.to_path_buf(),
      None => repo_root.join(".codenexus").join("notes-export"),
    };
    fs::create_dir_all(&dir).with_context(|| format!("create export dir {}", dir.display()))?;
    Ok(Self { path: dir.join("notes.jsonl") })
  }

  pub fn append(&self, event: &serde_json::Value) -> Result<()> {
    let line = serde_json::to_string(event).context("serialize event")?;
    let mut f = OpenOptions::new()
      .create(true)
      .append(true)
      .open(&self.path)
      .with_context(|| format!("open {}", self.path.display()))?;
    writeln!(f, "{}", line).context("append line")?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
  use tempfile::tempdir;

  #[test]
  fn default_path_under_codenexus_dir() {
    let tmp = tempdir().unwrap();
    let exp = JsonlExporter::for_repo(tmp.path(), None).unwrap();
    assert_eq!(exp.path, tmp.path().join(".codenexus/notes-export/notes.jsonl"));
    assert!(tmp.path().join(".codenexus/notes-export").exists());
  }

  #[test]
  fn override_path_used_when_provided() {
    let tmp = tempdir().unwrap();
    let other = tempdir().unwrap();
    let exp = JsonlExporter::for_repo(tmp.path(), Some(other.path())).unwrap();
    assert_eq!(exp.path, other.path().join("notes.jsonl"));
  }

  #[test]
  fn append_writes_ndjson_lines() {
    let tmp = tempdir().unwrap();
    let exp = JsonlExporter::for_repo(tmp.path(), None).unwrap();
    exp.append(&json!({"event": "test", "ts": "2026-05-03T12:00:00Z", "payload": {}})).unwrap();
    exp.append(&json!({"event": "test2", "ts": "2026-05-03T12:00:01Z", "payload": {"x": 1}})).unwrap();
    let body = std::fs::read_to_string(&exp.path).unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 2);
    let v1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v1["event"], "test");
    let v2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(v2["payload"]["x"], 1);
  }
}
```

If `tempfile` is not in `[dev-dependencies]`, add: `tempfile = "3"` to `experiments/poc-retrieval/core/Cargo.toml` `[dev-dependencies]` section.

**Step B -- export module in `core/src/lib.rs`.** Add `pub mod jsonl_export;` (alphabetical order if existing mods are alphabetical).

**Step C -- add CLI flag in `core/src/main.rs`.** Locate the clap-derived struct(s) for `Cmd::Index` and `Cmd::Serve`. Add:
```rust
/// Override default JSONL export dir (default: <repo>/.codenexus/notes-export/).
/// Per UQ-A1 of Phase 5 Bridge.
#[arg(long, value_name = "PATH")]
export_dir: Option<PathBuf>,
```
to BOTH Cmd::Index and Cmd::Serve variants. Thread the value into a config struct (or pass as parameter) consumed by the W1 remember_symbol_note handler. W0 only verifies the flag PARSES; the actual handler-side `JsonlExporter::for_repo(repo, export_dir.as_deref())?.append(&event)?` call lands in W1.

(EXECUTOR: read main.rs first to identify the actual clap pattern in use; existing `Cmd` enum defines how flags are declared. If the pattern uses positional struct fields rather than clap derive, mirror that pattern.)

**Step D -- add schema migration check helpers** in `core/src/storage.rs`. Place after `has_alias_decls` (parallel to 04.5-03 W0 pattern):
```rust
/// Returns true if the symbol_notes table exists with at least 0 rows
/// reachable. Distinguished from has_symbol_notes_rows; this is a
/// SCHEMA presence check, not a data presence check.
pub fn has_symbol_notes_table(&self) -> Result<bool> {
  let n: i64 = self.conn.query_row(
    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='symbol_notes'",
    [], |r| r.get(0),
  )?;
  Ok(n > 0)
}

/// Same for adrs table.
pub fn has_adrs_table(&self) -> Result<bool> {
  let n: i64 = self.conn.query_row(
    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='adrs'",
    [], |r| r.get(0),
  )?;
  Ok(n > 0)
}
```

(NOTE: because `Store::open` always runs the additive `CREATE TABLE IF NOT EXISTS`, opening a pre-W0 DB through `Store::open` will silently CREATE the new tables -- so the migration check fires at a different layer than 04.5-03 W0's. Instead, this check is informational: it logs if the DB had to be auto-extended. Implementation: in main.rs Cmd::Index, call `has_symbol_notes_table` BEFORE `Store::open`, using a raw rusqlite Connection peek; if false AND has_imports_edges OR has_symbols, log "[codenexus] Phase 5 W0 schema auto-applied (additive)". Non-fatal; informational.)

Simpler alternative (RECOMMENDED for W0): defer the migration check to Task 1 of W1 entirely. W0 ships schema only; W1 handler-entry logs schema state. This task drops Step D's main.rs wiring and only adds the helpers + their unit tests.

**Adopt the simpler alternative.** Step D ships ONLY the two helper functions in storage.rs + unit tests verifying they return correct booleans. The actual main.rs informational log is W1 work; this keeps W0 zero-touch on main.rs entry paths beyond the --export-dir flag (Step C).

 </action>

 <acceptance_criteria>
  - `test -f experiments/poc-retrieval/core/src/jsonl_export.rs` exits 0
  - `grep -nE '^pub mod jsonl_export' experiments/poc-retrieval/core/src/lib.rs` exactly 1 hit
  - `grep -nE 'pub struct JsonlExporter' experiments/poc-retrieval/core/src/jsonl_export.rs` exactly 1 hit
  - `grep -nE 'pub fn for_repo' experiments/poc-retrieval/core/src/jsonl_export.rs` exactly 1 hit
  - `grep -nE 'pub fn append' experiments/poc-retrieval/core/src/jsonl_export.rs` exactly 1 hit
  - `grep -nE '\.codenexus.*notes-export' experiments/poc-retrieval/core/src/jsonl_export.rs` >= 1 hit (default path baked in per UQ-A1)
  - `grep -nE 'export_dir' experiments/poc-retrieval/core/src/main.rs` >= 1 hit (CLI flag declared)
  - `grep -nE 'pub fn has_symbol_notes_table' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `grep -nE 'pub fn has_adrs_table' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
  - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -E '^(error)' | wc -l` returns 0 (G-A: build clean)
  - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib jsonl_export::tests -- --test-threads=1` exits 0 with 3 tests passing
  - `cd experiments/poc-retrieval && target/release/codenexus-core index --help 2>&1 | grep -F 'export-dir'` >= 1 hit
 </acceptance_criteria>

 <verify>
  <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core --lib jsonl_export::tests -- --test-threads=1 && target/release/codenexus-core index --help 2>&1 | grep -F 'export-dir'</automated>
 </verify>

 <done>
  jsonl_export.rs module created with JsonlExporter::for_repo +
  append APIs; lib.rs re-exports it; main.rs Cmd::Index + Cmd::Serve
  accept --export-dir flag (parsed but not yet wired to a handler --
  W1 wires it into remember_symbol_note); storage.rs has
  has_symbol_notes_table + has_adrs_table helpers (informational; W1
  consumes); 3 jsonl_export unit tests pass. Build clean.
 </done>
</task>

</tasks>

<gates>
- **G-A** (build clean): `cargo build --workspace --release` clean, no NEW warnings beyond the 3 pre-existing dead-code (`resolve` / `personalized_pagerank` / `count_edges_by_kind_conf`). [Tasks 1, 2]
- **G-B** (regression-green): all existing tests stay green: graph_build::tests (7 tests), alias_decls_tests (5 tests). T3 / T4 stay PINNED. [Task 1]
- **G-C** (schema additive verified): fresh `Store::open(":memory:")` creates 3 new tables + 2 FTS5 virtual tables + 4 indexes + 2 triggers; all helper APIs callable; >= 9 new unit tests in symbol_notes_tests + adrs_tests pass; jsonl_export tests (3) pass. [Tasks 1, 2]
</gates>

<must_haves>
truths:
 - "symbol_notes table exists in fresh Store::open with notes_fnk index on (path, name, kind) per drift probe M5_fnk = 1.0 lock"
 - "adrs table exists with UNIQUE (source_path, source_line, doc_version_sha) per G5 section 3.2 drift-safe identity"
 - "adr_symbol_links table exists with PRIMARY KEY (adr_id, symbol_id, link_kind) ready for W2/W4 to populate"
 - "constraints_fts + adrs_fts virtual tables exist (contentless mode); AFTER INSERT triggers populate them"
 - "Insert/list/clear APIs for symbol_notes / adrs / adr_symbol_links available; bm25_constraints + bm25_adrs ready for W2 search.rs corpus_scope work"
 - "Append-only supersede semantics enforced: list_notes_for_symbol(include_history=false) returns only active leaves"
 - "JsonlExporter::for_repo defaults to <repo>/.codenexus/notes-export/notes.jsonl per UQ-A1; --export-dir CLI flag parsed in Cmd::Index and Cmd::Serve"
 - "All existing tests stay green: graph_build::tests (7), alias_decls_tests (5)"
artifacts:
 - path: "experiments/poc-retrieval/core/src/types.rs"
  provides: "Severity enum + SymbolNote, Adr, AdrSymbolLink structs"
  contains: "pub enum Severity"
 - path: "experiments/poc-retrieval/core/src/storage.rs"
  provides: "3 new tables + 2 FTS5 virtual tables + insert/list/clear + bm25_constraints + bm25_adrs APIs"
  contains: "CREATE TABLE IF NOT EXISTS symbol_notes"
 - path: "experiments/poc-retrieval/core/src/jsonl_export.rs"
  provides: "JsonlExporter (write-only NDJSON event log per G1 Mode B)"
  contains: "pub struct JsonlExporter"
 - path: "experiments/poc-retrieval/core/src/lib.rs"
  provides: "pub mod jsonl_export re-export"
  contains: "pub mod jsonl_export"
 - path: "experiments/poc-retrieval/core/src/main.rs"
  provides: "--export-dir CLI flag on Cmd::Index and Cmd::Serve"
  contains: "export_dir"
key_links:
 - from: "core/src/storage.rs::list_notes_for_symbol"
  to: "core/src/types.rs::SymbolNote"
  via: "Vec<SymbolNote> return type"
  pattern: "Vec<SymbolNote>"
 - from: "core/src/storage.rs::list_adrs_for_symbol"
  to: "core/src/types.rs::Adr"
  via: "Vec<Adr> return type"
  pattern: "Vec<Adr>"
 - from: "core/src/lib.rs"
  to: "core/src/jsonl_export.rs"
  via: "pub mod jsonl_export"
  pattern: "pub mod jsonl_export"
</must_haves>

<verification>
1. `grep -cE 'CREATE TABLE IF NOT EXISTS (symbol_notes|adrs|adr_symbol_links)' core/src/storage.rs` returns 3
2. `grep -cE 'CREATE VIRTUAL TABLE IF NOT EXISTS (constraints_fts|adrs_fts)' core/src/storage.rs` returns 2
3. `cargo build --workspace --release` clean (G-A)
4. `cargo test -p codenexus-core --lib symbol_notes_tests adrs_tests jsonl_export::tests -- --test-threads=1` exits 0
5. `cargo test -p codenexus-core --lib graph_build::tests alias_decls_tests -- --test-threads=1` exits 0 (G-B regression)
6. `target/release/codenexus-core index --help | grep -F 'export-dir'` >= 1 hit
</verification>

<open_questions>
None blocking. UQ-B1 (min confidence floor) and UQ-B2 (active-leaf default) are settled by W0 schema (no floor; active-leaves default = true) and merely confirmed in W1 handler-time. UQ-B3 (note authorship) deferred to V1.1+ per discuss; W0 schema does NOT include an `agent_model` column (additions are V1.1+ schema migration territory).
</open_questions>

<honest_gap_list>
**P1**:
- (none -- W0 is schema-only; runtime risk is in W1+)

**P2**:
- adr_symbol_links is created EMPTY; W2 (query_constraints) and W4 (extract_adrs) populate it. Plan-checker should re-verify that the `link_kind` enum values (`mention | topic_match | file_overlap`) line up with G2 + G5 expectations during W2 / W4 plan-time.
- main.rs --export-dir flag is PARSED but not wired to any handler in W0; W1 must wire it into the remember_symbol_note handler. Risk: W1 forgets, JSONL never written, V1.1+ memU integration has no data to replay. Mitigation: W1 plan task explicitly lists "thread export_dir into JsonlExporter::for_repo invocation" as a must_haves truth.

**P3**:
- `tempfile` dev-dependency may already be in Cargo.toml; if so, Step A's instruction to add it is a no-op (idempotent). Plan-checker confirms.
- Tests use `:memory:` SQLite DBs; FTS5 virtual table behavior on `:memory:` is identical to file-backed per rusqlite docs. If a future plan-checker round flags this assumption as risky, replace one test with a tempdir file-backed DB sanity check.
- W0's schema-additive design means existing pre-W0 fsc.db / poc.db files will auto-acquire the new tables on first re-open; this is silent (no migration message). 04.5-03 W0's loud-error pattern does NOT apply here because W0 ADDS rather than RENAMES / REMOVES. If user observability is a concern, W1's first call into a new table can log "[codenexus] Phase 5 W0 schema first use".
</honest_gap_list>
</content>
</invoke>