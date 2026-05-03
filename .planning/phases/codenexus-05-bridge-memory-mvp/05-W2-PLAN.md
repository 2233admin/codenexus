---
phase: 5
slice: 05-W2
plan_id: 05-W2
title: "W2: A2A op read side -- query_constraints + list_notes + search.rs corpus_scope extension"
wave: 2
depends_on: [05-W0, 05-W1]
status: PLAN-AUTHORED (awaits plan-checker iter)
files_modified:
  - experiments/poc-retrieval/core/src/a2a.rs
  - experiments/poc-retrieval/core/src/server.rs
  - experiments/poc-retrieval/core/src/search.rs
  - experiments/poc-retrieval/core/src/storage.rs
locked_decisions_honored:
  - G2   # query_constraints 3-modality enum; reuse search.rs::search via corpus_scope param; ranked relevance x severity
  - G3   # list_notes ships as separate A2A op (UQ-A3 = 5 ops total)
  - UQ-A3   # 5 public A2A ops (list_notes is one of them)
  - UQ-B2   # NoteView.is_active_leaf default = active leaves only; include_history opt-in
gates:
  - G-A   # build clean
  - G-B   # all unit + integration tests pass
  - G-C   # query_constraints 3 modalities (file / symbol / topic) all dispatchable
  - G-D   # list_notes returns active leaves by default; full chain on include_history=true
  - G-E   # search.rs corpus_scope extension does not break existing Query path
---

> **!! AMENDED 2026-05-03 per CCG round 2 !!** Round-2 amendment below
> SUPERSEDES the original objective + plan_time_decisions for this slice.
> Original sections retained as audit trail. See
> `05-DISCUSS-SUMMARY.md § Round-3 Amendments LANDED`.

## Round-2 Amendment Block (W2 -- CI-1 cascade; corpus_scope -> kind_filter + notes_fts)

W2 is the second-heaviest amended slice. Codex CI-1 surfaced that the
original `corpus_scope` parameter underestimated LOC because search.rs is
symbol-shaped throughout (Hit embeds parser::Symbol; storage accessors are
symbol-specific). Cascade resolution: ADRs become Symbol kind='ADR' (W0
amendment), so search.rs only needs a `kind_filter` parameter, NOT a result-
type abstraction. Notes get a separate dedicated FTS accessor.

### Replaces original Output items

**OUT (original, now superseded):**
- `core/src/search.rs::search` gains `corpus_scope: Option<CorpusScope>`
  parameter
- `pub enum CorpusScope { Symbols, Constraints }`
- `core/src/storage.rs::all_constraint_embeddings()`
- `embed_symbol_note(note_id, vec)` per-row embedding column on symbol_notes

**IN (amended, authoritative):**
- `core/src/search.rs::search` gains `kind_filter: Option<Vec<String>>`
  parameter (~30-50 LOC). Default None preserves existing behavior. When
  Some(["ADR"]), filter both BM25 results (`store.bm25(...)`) AND vector
  results (`store.all_embeddings()`) to allowed Symbol kinds. Filtering
  applied AFTER store calls, BEFORE RRF/scoring -- minimal change to
  search() body.
- `core/src/storage.rs::search_notes_fts(text: &str, top: usize)
  -> Result<Vec<(NoteId, BM25Score)>>` -- BM25-only over notes_fts
  (W0 created). NO vector index for notes in V1.0; if a future eval shows
  recall gap, add vector in V1.1.
- `core/src/server.rs::handle_query_constraints(~80-120 LOC)`:
  - File mode: SQL on symbol_notes for symbols-in-file + Symbol kind='ADR'
    rows WHERE adr_metadata.source_path=path OR adr_symbol_links join
  - Symbol mode: SQL on symbol_notes for fnk + adr_symbol_links join for
    code_symbol_id=target
  - Topic mode: parallel calls to (a) `search::search(kind_filter=Some(vec!["ADR".into()]),...)`
    over symbols_fts and (b) `Store::search_notes_fts(text, top)` over notes_fts;
    merge via simple RRF (k=60 default) in handler.

### LOC re-sizing (CI-1 honest)

Original W2 estimate: ~30-80 LOC for corpus_scope thread + ~30 LOC for
list_notes handler = ~110 LOC. Amended W2 estimate:
- search.rs kind_filter: ~30-50 LOC
- Store::search_notes_fts accessor: ~30-50 LOC
- query_constraints handler with two-stream RRF: ~80-120 LOC
- list_notes handler: ~30 LOC
- a2a.rs new request/response variants: ~50 LOC
- tests (file/symbol/topic dispatch + RRF merge correctness): ~80-100 LOC
- **Total: ~300-400 LOC** (in line with Codex's "100-200 plus tests" estimate
  when tests are split out)

### Removed plan_time_decisions

- D-W2-01 "per-row embedding storage" -- DROP (no embedding on notes in V1.0)
- Any reference to `bm25_constraints` / `all_constraint_embeddings` -- DROP
  (single notes_fts surface; ADRs ride symbols_fts)
- Any reference to `corpus_scope: Option<CorpusScope>` -- REPLACE with
  `kind_filter: Option<Vec<String>>`

### New plan_time_decisions

- **D-W2-01-amended (kind_filter cardinality):** `Option<Vec<String>>` not
  `Option<HashSet<String>>` -- typical filter is 1-2 kinds; Vec linear scan
  is faster than hash setup + lookup for n<=4.
- **D-W2-02-amended (RRF k constant):** k=60 default per literature; expose
  as struct field on QueryConstraints request with `#[serde(default = "default_rrf_k")]`
  if eval requires tuning. Hardcode for V1.0.
- **D-W2-03-amended (notes BM25-only):** No vector embedding on notes in
  V1.0. Notes are short, agent-authored, BM25 captures keyword overlap
  adequately. Revisit V1.1 if eval recall < 0.7 on note-targeted topic
  queries.
- **D-W2-04-amended (graceful W4 degradation):** if extract_adrs (W4) hasn't
  populated kind='ADR' Symbol rows yet, query_constraints Topic mode just
  returns notes-only results (symbols search returns 0 hits naturally; no
  special-case branch needed). Response envelope still sets
  `adr_extracted: false` if `Store::count_symbols_kind('ADR') == 0`.

### W2 acceptance test additions

- [ ] kind_filter=Some(["ADR"]) returns ONLY ADR Symbol rows from
      search::search; no code Symbols leak
- [ ] kind_filter=None preserves existing Query path bytewise (regression)
- [ ] Store::search_notes_fts returns BM25-ranked NoteId list; empty for
      query that matches no notes
- [ ] query_constraints Topic mode with both ADRs and notes present merges
      via RRF; ADR with score=10 + note with score=5 ranks ADR first
- [ ] query_constraints File mode returns notes for all symbols in file
      AND ADRs anchored to file
- [ ] adr_extracted=false when symbols table has no kind='ADR' rows

### W2 unaffected items

- list_notes handler (~30 LOC) unchanged in shape -- amended only inasmuch
  as it queries symbol_notes table whose FK is now against symbols unique
  index (CI-2; W0 deliverable, transparent to W2)
- UQ-A3 (5 ops) and UQ-B2 (active leaves default) unchanged

---


<objective>
Land the READ side of Phase 5 A2A surface. After W2, agents can:

1. **list_notes**: query notes for a (path, name, kind) symbol, with
   active-leaves vs full-history selection. Pure SQL on symbol_notes.
2. **query_constraints**: query constraints (notes + ADRs from W4 if
   landed; notes-only if W4 not yet landed -- adr_extracted=false flag
   in response per G2 graceful-degrade) by file / symbol / topic. Topic
   modality reuses search.rs::search via NEW corpus_scope parameter.
3. **search.rs::search** gains `corpus_scope: Option<CorpusScope>`
   parameter (~30-80 LOC change). Default None preserves existing
   behavior; Some(Constraints) routes through bm25_constraints + per-row
   embedding column on symbol_notes / adrs.

W2 is the heaviest plan in Phase 5 by LOC (G2 backend dispatch + search
extension + 2 A2A ops). Splitting into Task 1 (search.rs extension +
list_notes) and Task 2 (query_constraints) keeps each task in the
2000-line context budget.

Out of scope: get_edit_context (W3 composite), ADR extraction (W4),
MCP wrap (W5).

Reconciliation with W4: query_constraints returns ADR rows ONLY if W4
ships first AND adrs table is populated. If W4 lands AFTER W2, the
response envelope sets `adr_extracted: false` and returns notes-only.
Per G2 line 107-109: do NOT block W2 on W4. The adrs/adr_symbol_links
tables EXIST after W0 (empty); W2 queries them and returns 0 rows
gracefully.

Output:
- `core/src/search.rs::search`: new `corpus_scope: Option<CorpusScope>`
  parameter; new pub enum CorpusScope { Symbols, Constraints }; default
  CorpusScope::Symbols preserves existing behavior at all call sites.
- `core/src/storage.rs`: new `all_constraint_embeddings()` method
  (parallel to all_embeddings); maybe `embed_symbol_note(note_id, vec)`
  to populate per-row embedding column on symbol_notes (W2 adds the
  column via ALTER TABLE if not present in W0).
- `core/src/a2a.rs`: new variants `OperationRequest::QueryConstraints`,
  `OperationRequest::ListNotes`, plus matching responses.
- `core/src/server.rs`: handlers `handle_query_constraints` (~100 LOC)
  + `handle_list_notes` (~30 LOC).
</objective>

<plan_time_decisions>
- **D-W2-01 (per-row embedding storage):** Add `embedding BLOB` column
  to `symbol_notes` and `adrs` tables. W0 schema does NOT include
  these (oversight to fix here; W0 PLAN noted constraints_fts but not
  the vector column needed for hybrid topic search). W2 ALTER TABLE
  ADD COLUMN IF NOT EXISTS pattern (or schema bump in Store::open with
  back-compat migration). Recommended: add to W0 schema in execution
  if W0 not yet shipped; if W0 already shipped, ALTER TABLE here.
- **D-W2-02 (topic embedding population):** When W1 writes a note via
  insert_symbol_note, ALSO call embedder + write the embedding to the
  new column. This requires touching W1's handler -- BUT W2 owns the
  CorpusScope::Constraints path so threading the embedder into the
  handler is W2 work. Decision: W2 EXTENDS handle_remember_symbol_note
  (W1 task) to additionally embed + persist; W1 SUMMARY notes that
  embedding is W2 territory. If embedder is unavailable at write time,
  store NULL and topic search falls back to bm25-only.
- **D-W2-03 (severity ranking):** Per G2 line 87, topic modality
  returns RRF score; file/symbol modes return score=1.0. Severity
  multiplier applied AFTER hybrid score: `final_score = base_score * severity_weight`
  with `severity_weight = {Must: 1.5, MustNot: 1.5, Should: 1.0, Note: 0.7}`.
  Eyeball defaults; G6 W6 eval tunes.
- **D-W2-04 (CorpusScope enum location):** In `search.rs` (new public
  enum). Re-exported via lib.rs if external callers need it; for V1.0
  only server.rs is the caller, and it already imports search via
  `crate::search`.
- **D-W2-05 (ConstraintHit response shape):** Match G2's `ConstraintHit`
  struct (05-discuss-api.md lines 58-65) verbatim. Add to a2a.rs as a
  new pub struct + Severity enum re-exported from types.rs (Severity
  was added in W0).
- **D-W2-06 (bm25_constraints already exists from W0):** No new
  storage.rs API for BM25 over notes; W0 ships it. W2 only adds the
  vector counterpart `all_constraint_embeddings()`.
</plan_time_decisions>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W0-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W1-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md
@CONTEXT.md
@experiments/poc-retrieval/core/src/search.rs
@experiments/poc-retrieval/core/src/a2a.rs
@experiments/poc-retrieval/core/src/storage.rs

<interfaces>
<!-- Existing search.rs (verified 2026-05-03; 164 lines) -->
```rust
pub struct Hit {
    pub id: i64,
    pub bm25_score: f32,
    pub vector_score: f32,
    pub rrf_score: f32,
    pub rerank_score: Option<f32>,
    pub symbol: Symbol,
}

pub fn search(
    store: &Store,
    embedder: &Embedder,
    reranker: Option<&Reranker>,
    query: &str,
    k: usize,
    alpha: f32,
) -> Result<Vec<Hit>>
```

<!-- Target search.rs after W2 -->
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusScope {
    Symbols,        // existing default; queries symbols + symbols_fts
    Constraints,    // NEW; queries symbol_notes + constraints_fts (and adrs + adrs_fts unioned)
}

pub fn search(
    store: &Store,
    embedder: &Embedder,
    reranker: Option<&Reranker>,
    query: &str,
    k: usize,
    alpha: f32,
    corpus_scope: Option<CorpusScope>,    // NEW; None defaults to CorpusScope::Symbols
) -> Result<Vec<Hit>>
```

<!-- W0 storage primitives in scope -->
```rust
Store::insert_symbol_note(...) -> Result<i64>
Store::list_notes_for_symbol(path, name, kind, include_history) -> Result<Vec<SymbolNote>>
Store::bm25_constraints(query, k) -> Result<Vec<(i64, f32)>>     // queries constraints_fts
Store::bm25_adrs(query, k) -> Result<Vec<(i64, f32)>>
Store::list_adrs_for_symbol(symbol_id) -> Result<Vec<Adr>>
Store::list_adrs_for_file(source_path) -> Result<Vec<Adr>>
Store::all_embeddings() -> Result<Vec<(i64, Vec<f32>)>>           // for symbols
// W2 adds:
Store::all_constraint_embeddings() -> Result<Vec<(ConstraintRowRef, Vec<f32>)>>
// where ConstraintRowRef = enum { SymbolNote(i64), Adr(i64) } to disambiguate the row source
```

<!-- Target a2a.rs after W2 -->
```rust
// New OperationRequest variants:
QueryConstraints {
    target: ConstraintTarget,
    #[serde(default = "default_top")] top: usize,
    #[serde(default = "default_alpha")] alpha: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintTarget {
    File { path: String },
    Symbol { id: i64, #[serde(default)] include_callers: bool },
    Topic { text: String },
}

ListNotes {
    symbol_id: i64,
    #[serde(default)] include_history: bool,
}

// New OperationResponse variants:
QueryConstraints { constraints: Vec<ConstraintHit>, adr_extracted: bool }
ListNotes { notes: Vec<NoteView> }

#[derive(Serialize, Deserialize)]
pub struct ConstraintHit {
    pub source: ConstraintSource,
    pub severity: Severity,           // Must | MustNot | Should | Note
    pub text: String,
    pub score: f32,
    pub anchor: Option<HitView>,      // reuse existing HitView from a2a.rs line 119
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintSource {
    Adr { doc_path: String, header_path: Option<String>, line: i64 },
    Note { note_id: i64, symbol_path: String, symbol_name: String, symbol_kind: String },
}

#[derive(Serialize, Deserialize)]
pub struct NoteView {
    pub note_id: i64,
    pub path: String, pub name: String, pub kind: String,
    pub note_text: String,
    pub tags: Vec<String>,
    pub confidence: f32,
    pub source_session: String,
    pub supersedes_note_id: Option<i64>,
    pub created_at: String,
    pub is_active_leaf: bool,         // computed by handler
}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: search.rs CorpusScope extension + Store::all_constraint_embeddings + per-row embedding columns</name>
  <files>experiments/poc-retrieval/core/src/search.rs, experiments/poc-retrieval/core/src/storage.rs, experiments/poc-retrieval/core/src/server.rs</files>

  <read_first>
    - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md G2 backend dispatch table (lines 68-93)
    - experiments/poc-retrieval/core/src/search.rs lines 21-100 (current search function -- the corpus seam at lines 29 + 32)
    - experiments/poc-retrieval/core/src/storage.rs all_embeddings (line 361) + bm25 (line 348)
    - All EXISTING call sites of `search::search(...)` in server.rs Query handler (grep `search::search\|search\(`)
  </read_first>

  <behavior>
    - Test 1 (compile): `cargo build --workspace --release` exits 0 with no NEW warnings
    - Test 2 (CorpusScope enum exists): `grep -nE 'pub enum CorpusScope' core/src/search.rs` returns 1 hit
    - Test 3 (existing call sites pass None or Some(Symbols)): all existing `search::search(...)` invocations updated to add the new arg; behavior unchanged for None default
    - Test 4 (Query path regression): `cargo test -p codenexus-core --lib search -- --test-threads=1` all existing search tests pass
    - Test 5 (embedding column added on symbol_notes + adrs): `PRAGMA table_info(symbol_notes)` includes a row for column `embedding` (BLOB type). Same for adrs.
    - Test 6 (all_constraint_embeddings returns row source disambiguator): given fixture with 2 notes + 1 adr (each with non-NULL embedding BLOB), call returns Vec of 3 (ConstraintRowRef, Vec<f32>) pairs, ConstraintRowRef discriminates between SymbolNote(id) and Adr(id)
    - Test 7 (search with CorpusScope::Constraints): given fixture with constraint corpus populated, `search(..., corpus_scope=Some(Constraints))` returns hits keyed on constraint rows, NOT on Symbol rows
  </behavior>

  <action>

**Step A -- add `embedding BLOB` column to symbol_notes + adrs (D-W2-01).** Update Store::open execute_batch in core/src/storage.rs to ADD `embedding BLOB` to both CREATE TABLE statements. For migration of existing W0 DBs, prepend execute_batch with `ALTER TABLE symbol_notes ADD COLUMN embedding BLOB;` wrapped in a check (catch error if column already exists -- ALTER TABLE is idempotent in newer SQLite via `ADD COLUMN IF NOT EXISTS` syntax which SQLite SUPPORTS as of 3.35; verify rusqlite version supports it; if not, use a try/match on the error).

Alternative (simpler): bump schema_version pragma in Store::open and run ALTER conditionally. Pick whichever pattern keeps W0 DBs compatible. SUMMARY documents choice.

**Step B -- add `pub enum CorpusScope` in `core/src/search.rs`** at the top:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusScope {
    Symbols,
    Constraints,
}
```

**Step C -- modify `search::search` signature** to take `corpus_scope: Option<CorpusScope>` as the new last parameter. Inside the function:
- Default: `let scope = corpus_scope.unwrap_or(CorpusScope::Symbols);`
- Branch on scope:
  - `CorpusScope::Symbols`: existing logic unchanged (store.bm25 + store.all_embeddings + store.fetch).
  - `CorpusScope::Constraints`: new branch using `store.bm25_constraints` + `store.all_constraint_embeddings` + a new `store.fetch_constraint_row(row_ref)` helper that returns a constraint hit (text + source disambiguator).

The `Hit` struct shape today carries `Symbol`. For Constraints scope, this won't fit. Options:
(a) Make `Hit` generic over a `Locator` type.
(b) Branch the function into two: `search_symbols` (existing) and `search_constraints` (new), with `search()` as a thin dispatcher.
(c) Add a `ConstraintHit` parallel struct + `search_constraints` parallel function; Symbols `search` stays unchanged.

**Decision: option (b) with a thin dispatch.** `search()` keeps its public signature (with the new corpus_scope param) but internally branches:
```rust
pub fn search(
    store: &Store, embedder: &Embedder, reranker: Option<&Reranker>,
    query: &str, k: usize, alpha: f32,
    corpus_scope: Option<CorpusScope>,
) -> Result<Vec<Hit>> {
    match corpus_scope.unwrap_or(CorpusScope::Symbols) {
        CorpusScope::Symbols => search_symbols(store, embedder, reranker, query, k, alpha),
        CorpusScope::Constraints => {
            // Constraints route returns Vec<ConstraintHit>, NOT Vec<Hit>.
            // Use a separate public function for the constraints path:
            anyhow::bail!("CorpusScope::Constraints uses search_constraints(); not search()")
        }
    }
}

pub fn search_symbols(...) -> Result<Vec<Hit>> { /* existing body */ }

pub fn search_constraints(
    store: &Store, embedder: &Embedder,
    query: &str, k: usize, alpha: f32,
) -> Result<Vec<crate::storage::ConstraintRowHit>> { /* new */ }
```

**Reconciliation:** the corpus_scope parameter on `search()` is then ALMOST cosmetic; the real entry points are `search_symbols` and `search_constraints`. Per G2's intent, the goal is "one function with a corpus parameter". Honor the spec by keeping `search()` as the dispatcher BUT make it return `enum SearchResult { Symbols(Vec<Hit>), Constraints(Vec<ConstraintRowHit>) }` -- type-safe, no panics, single entry point.

**Final decision (executor implements this):**
```rust
pub enum SearchResult {
    Symbols(Vec<Hit>),
    Constraints(Vec<ConstraintRowHit>),
}

pub fn search(
    store: &Store, embedder: &Embedder, reranker: Option<&Reranker>,
    query: &str, k: usize, alpha: f32,
    corpus_scope: Option<CorpusScope>,
) -> Result<SearchResult>
```

This requires updating all existing call sites (Query handler in server.rs) to destructure `SearchResult::Symbols(hits) => hits`. ~5-10 LOC edit. Plan-checker iter validates.

**Step D -- add `Store::all_constraint_embeddings`** + `ConstraintRowHit` struct + `ConstraintRowRef` enum + `fetch_constraint_row(row_ref)` in storage.rs:
```rust
#[derive(Debug, Clone, Copy)]
pub enum ConstraintRowRef {
    SymbolNote(i64),
    Adr(i64),
}

#[derive(Debug, Clone)]
pub struct ConstraintRowHit {
    pub row_ref: ConstraintRowRef,
    pub bm25_score: f32,
    pub vector_score: f32,
    pub rrf_score: f32,
    pub text: String,
}

pub fn all_constraint_embeddings(&self) -> Result<Vec<(ConstraintRowRef, Vec<f32>)>> {
    let mut out = Vec::new();
    let mut st = self.conn.prepare("SELECT id, embedding FROM symbol_notes WHERE embedding IS NOT NULL")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
    for row in rows {
        let (id, blob) = row?;
        let v = bytes_to_f32_vec(&blob);
        out.push((ConstraintRowRef::SymbolNote(id), v));
    }
    let mut st = self.conn.prepare("SELECT id, embedding FROM adrs WHERE embedding IS NOT NULL")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
    for row in rows {
        let (id, blob) = row?;
        let v = bytes_to_f32_vec(&blob);
        out.push((ConstraintRowRef::Adr(id), v));
    }
    Ok(out)
}

pub fn fetch_constraint_text(&self, row_ref: ConstraintRowRef) -> Result<String> {
    match row_ref {
        ConstraintRowRef::SymbolNote(id) => Ok(self.conn.query_row(
            "SELECT note_text FROM symbol_notes WHERE id = ?", [id], |r| r.get(0))?),
        ConstraintRowRef::Adr(id) => Ok(self.conn.query_row(
            "SELECT paragraph_text FROM adrs WHERE id = ?", [id], |r| r.get(0))?),
    }
}

// Reuse existing bytes_to_f32_vec helper from storage.rs (existing for all_embeddings).
```

**Step E -- write search_constraints function in search.rs** mirroring search_symbols pattern (BM25 via bm25_constraints, vector via all_constraint_embeddings, RRF fusion identical). No reranker for constraints path in V1.0.

**Step F -- update existing call sites of search::search()** in server.rs Query handler. Pattern:
```rust
let result = crate::search::search(&store, &embedder, reranker.as_ref(), &text, top, alpha, None)?;
let hits = match result {
    crate::search::SearchResult::Symbols(h) => h,
    _ => unreachable!("Query handler always uses default Symbols scope"),
};
```

**Step G -- write unit tests** in search.rs `mod tests`. Test 6 + Test 7 above need a fixture; use the `:memory:` Store + a synthetic Embedder mock (or skip if Embedder requires model bytes -- in that case mark Test 7 as `#[ignore]` and provide a manual run command in SUMMARY).

  </action>

  <acceptance_criteria>
    - `grep -nE 'pub enum CorpusScope' experiments/poc-retrieval/core/src/search.rs` exactly 1 hit
    - `grep -nE 'pub enum SearchResult' experiments/poc-retrieval/core/src/search.rs` exactly 1 hit (or pub fn search returns Result<SearchResult>)
    - `grep -nE 'pub enum ConstraintRowRef' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
    - `grep -nE 'pub fn all_constraint_embeddings' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
    - `grep -nE 'pub fn fetch_constraint_text' experiments/poc-retrieval/core/src/storage.rs` exactly 1 hit
    - `grep -nE 'embedding BLOB' experiments/poc-retrieval/core/src/storage.rs` >= 2 hits (one in symbol_notes CREATE, one in adrs CREATE OR ALTER)
    - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -cE '^error'` returns 0 (G-A)
    - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib search -- --test-threads=1` all existing tests + new corpus_scope tests pass (G-B + G-E)
    - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib symbol_notes_tests adrs_tests jsonl_export -- --test-threads=1` exits 0 (W0 + W1 stay green)
  </acceptance_criteria>

  <verify>
    <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core --lib -- --test-threads=1 2>&1 | tail -30</automated>
  </verify>

  <done>
    search.rs has CorpusScope enum + SearchResult enum + search()
    dispatcher + search_symbols (refactored) + search_constraints
    (new). storage.rs has ConstraintRowRef enum + ConstraintRowHit
    struct + all_constraint_embeddings + fetch_constraint_text +
    embedding BLOB column on symbol_notes and adrs. server.rs Query
    handler updated to destructure SearchResult::Symbols. Build clean
    (G-A); all tests green (G-B + G-E regression).
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: A2A list_notes + query_constraints handlers + dispatch arms</name>
  <files>experiments/poc-retrieval/core/src/a2a.rs, experiments/poc-retrieval/core/src/server.rs</files>

  <read_first>
    - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md G2 + G3 (full -- backend dispatch + minimal-plus-3 + active-leaf semantics)
    - experiments/poc-retrieval/core/src/a2a.rs OperationRequest + OperationResponse layout (W1 added RememberSymbolNote)
    - experiments/poc-retrieval/core/src/server.rs handle_remember_symbol_note (W1 -- pattern for new handlers)
    - experiments/poc-retrieval/core/src/storage.rs list_notes_for_symbol + list_adrs_for_file + list_adrs_for_symbol (W0)
  </read_first>

  <behavior>
    - Test 1 (a2a deserialize list_notes): `serde_json::from_str::<OperationRequest>(r#"{"list_notes":{"symbol_id":42}}"#)` succeeds; include_history defaults to false
    - Test 2 (a2a deserialize query_constraints all 3 modalities): File / Symbol / Topic all parse from canonical JSON shapes
    - Test 3 (list_notes returns active leaves only by default): given symbol with 2 notes (n1 superseded by n2), list_notes returns [n2 with is_active_leaf=true]
    - Test 4 (list_notes include_history=true): same fixture returns [n1 with is_active_leaf=false, n2 with is_active_leaf=true]
    - Test 5 (query_constraints scope=symbol returns notes-only when adrs empty): given symbol with 1 note, returns ConstraintHit{source: Note{...}, severity: based on tags, text: note_text, score: 1.0}; adr_extracted=false
    - Test 6 (query_constraints scope=symbol returns notes + adrs when both populated): given symbol with 1 note AND 1 ADR linked via adr_symbol_links, returns 2 hits ranked by severity (MUST > Note baseline)
    - Test 7 (query_constraints scope=file): given file with 2 symbols (each with 1 note) AND 1 ADR anchored to file via list_adrs_for_file, returns 3 hits
    - Test 8 (query_constraints scope=topic): topic="reranker" against fixture with one ADR matching "MUST NOT introduce reranker" returns 1 hit with score from RRF (>0)
    - Test 9 (severity ranking): given two notes with tags=["must"] and tags=["should"], the must note ranks higher (D-W2-03 multiplier)
  </behavior>

  <action>

**Step A -- extend `core/src/a2a.rs`** with new variants per `<interfaces>` block. Add Severity import via `use crate::types::Severity;` (W0 added Severity to types.rs). Add `ConstraintTarget` enum, `ConstraintSource` enum, `ConstraintHit` struct, `NoteView` struct.

**Step B -- handle_list_notes in server.rs**:
```rust
fn handle_list_notes(
    db_path: &str,
    symbol_id: i64,
    include_history: bool,
) -> anyhow::Result<OperationResponse> {
    let store = codenexus_core::storage::Store::open(db_path)?;
    let (path, name, kind) = match store.symbol_by_id(symbol_id)? {
        Some(t) => t,
        None => anyhow::bail!("symbol_id {} not found in this binary", symbol_id),
    };
    let rows = store.list_notes_for_symbol(&path, &name, &kind, include_history)?;
    // Compute is_active_leaf per row: a row is active if no other row's
    // supersedes_note_id == this row's id. When include_history=false, ALL
    // returned rows are active leaves (storage filtered them).
    let leaf_ids: std::collections::HashSet<i64> = rows.iter()
        .filter_map(|r| r.supersedes_note_id)
        .collect();
    let notes: Vec<NoteView> = rows.into_iter().map(|n| {
        let is_active = !leaf_ids.contains(&n.id);
        NoteView { /* convert SymbolNote -> NoteView */ }
    }).collect();
    Ok(OperationResponse::ListNotes { notes })
}
```

**Step C -- handle_query_constraints in server.rs** (~80-100 LOC). Branch on ConstraintTarget:
```rust
fn handle_query_constraints(
    db_path: &str,
    target: ConstraintTarget,
    top: usize,
    alpha: f32,
    embedder: &Embedder,    // pass from dispatch state
) -> anyhow::Result<OperationResponse> {
    let store = codenexus_core::storage::Store::open(db_path)?;
    let mut hits: Vec<ConstraintHit> = Vec::new();
    let adr_extracted = store.has_adrs_table()? && store.list_adrs_for_file_count("/")? >= 0;
    // ^ adr_extracted=true if adrs table exists AND has at least one row globally;
    //   simpler: SELECT COUNT(*) FROM adrs > 0.

    match target {
        ConstraintTarget::Symbol { id, include_callers } => {
            let (path, name, kind) = store.symbol_by_id(id)?
                .ok_or_else(|| anyhow::anyhow!("symbol_id not found"))?;
            // Notes for this symbol:
            for note in store.list_notes_for_symbol(&path, &name, &kind, false)? {
                hits.push(note_to_constraint_hit(&note));
            }
            // ADRs linked to this symbol:
            for adr in store.list_adrs_for_symbol(id)? {
                hits.push(adr_to_constraint_hit(&adr));
            }
            // Optional include_callers: also pull notes for each caller (PPR path).
            if include_callers {
                let callers = /* call existing list_callers logic; reuse server.rs existing fn */;
                for caller in callers {
                    let (cp, cn, ck) = (caller.path, caller.name, caller.kind);
                    for note in store.list_notes_for_symbol(&cp, &cn, &ck, false)? {
                        hits.push(note_to_constraint_hit(&note));
                    }
                }
            }
        }
        ConstraintTarget::File { path } => {
            // All notes for symbols in this file:
            for sym in store.symbols_in_file_full(&path)? {
                for note in store.list_notes_for_symbol(&sym.path, &sym.name, &sym.kind, false)? {
                    hits.push(note_to_constraint_hit(&note));
                }
            }
            // ADRs anchored to this file (via source_path == path):
            for adr in store.list_adrs_for_file(&path)? {
                hits.push(adr_to_constraint_hit(&adr));
            }
        }
        ConstraintTarget::Topic { text } => {
            // Reuse search.rs::search with CorpusScope::Constraints
            let result = codenexus_core::search::search(
                &store, embedder, None, &text, top, alpha,
                Some(codenexus_core::search::CorpusScope::Constraints),
            )?;
            let constraint_hits = match result {
                codenexus_core::search::SearchResult::Constraints(h) => h,
                _ => anyhow::bail!("expected SearchResult::Constraints"),
            };
            for ch in constraint_hits {
                // Re-fetch source row via row_ref to materialize ConstraintHit.
                let hit = constraint_row_hit_to_constraint_hit(&store, &ch)?;
                hits.push(hit);
            }
        }
    }

    // D-W2-03 severity multiplier
    apply_severity_ranking(&mut hits);
    hits.truncate(top);

    Ok(OperationResponse::QueryConstraints { constraints: hits, adr_extracted })
}
```

Helper functions (executor implements):
- `note_to_constraint_hit(note: &SymbolNote) -> ConstraintHit`: map tags (e.g., contains "must") to Severity; ConstraintSource::Note { note_id: note.id, symbol_path: note.path, ... }; score = 1.0 (file/symbol modes per G2 line 88)
- `adr_to_constraint_hit(adr: &Adr) -> ConstraintHit`: severity from adr.keyword (MUST_NOT -> MustNot, MUST -> Must, SHOULD -> Should, MAY -> Note); ConstraintSource::Adr { doc_path: adr.source_path, header_path: adr.heading_anchor, line: adr.source_line }; score = 1.0
- `constraint_row_hit_to_constraint_hit(store, ch: &ConstraintRowHit) -> Result<ConstraintHit>`: re-fetch source row via row_ref, populate severity + source + score=ch.rrf_score
- `apply_severity_ranking(hits: &mut Vec<ConstraintHit>)`: multiply hit.score by severity_weight per D-W2-03; sort descending

**Step D -- dispatch arms in server.rs**:
```rust
OperationRequest::ListNotes { symbol_id, include_history } => {
    handle_list_notes(&db_path, symbol_id, include_history)
}
OperationRequest::QueryConstraints { target, top, alpha } => {
    handle_query_constraints(&db_path, target, top, alpha, &state.embedder)
}
```

(EXECUTOR: state.embedder threading mirrors W1's state.export_dir threading. If embedder is constructed per-request rather than held in state, that pattern continues; W2 does not refactor.)

**Step E -- unit tests** in server.rs `mod tests` (or a new tests/query_constraints.rs):
- Tests 3-9 from `<behavior>`. Use a fixture that pre-seeds symbol_notes + adrs + adr_symbol_links rows directly via raw SQL (skip embedder for non-topic tests; topic test may use mock embedder or `#[ignore]` with manual run note).

  </action>

  <acceptance_criteria>
    - `grep -nE 'OperationRequest::QueryConstraints' experiments/poc-retrieval/core/src/server.rs` >= 1 hit
    - `grep -nE 'OperationRequest::ListNotes' experiments/poc-retrieval/core/src/server.rs` >= 1 hit
    - `grep -nE 'fn handle_query_constraints' experiments/poc-retrieval/core/src/server.rs` exactly 1 hit
    - `grep -nE 'fn handle_list_notes' experiments/poc-retrieval/core/src/server.rs` exactly 1 hit
    - `grep -nE 'pub enum ConstraintTarget' experiments/poc-retrieval/core/src/a2a.rs` exactly 1 hit
    - `grep -nE 'pub enum ConstraintSource' experiments/poc-retrieval/core/src/a2a.rs` exactly 1 hit
    - `grep -nE 'pub struct ConstraintHit' experiments/poc-retrieval/core/src/a2a.rs` exactly 1 hit
    - `grep -nE 'pub struct NoteView' experiments/poc-retrieval/core/src/a2a.rs` exactly 1 hit
    - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -cE '^error'` returns 0 (G-A)
    - `cd experiments/poc-retrieval && cargo test -p codenexus-core query_constraints list_notes -- --test-threads=1` >= 7 tests pass (G-B + G-C + G-D)
    - All W0 + W1 + Task-1 tests still green (G-B regression)
  </acceptance_criteria>

  <verify>
    <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core --lib -- --test-threads=1 2>&1 | tail -30</automated>
  </verify>

  <done>
    a2a.rs has new ListNotes + QueryConstraints request/response
    variants + ConstraintTarget + ConstraintSource + ConstraintHit +
    NoteView types. server.rs has handle_list_notes (~30 LOC) +
    handle_query_constraints (~100 LOC) + dispatch arms. All 3
    modalities (file / symbol / topic) covered; severity ranking
    applied. Active-leaves default + include_history opt-in honored
    (G-D). Build clean (G-A); all tests green (G-B); 3-modality
    coverage (G-C); active-leaf default verified (G-D).
  </done>
</task>

</tasks>

<gates>
- **G-A** (build clean): `cargo build --workspace --release` clean. [Tasks 1, 2]
- **G-B** (regression-green): all W0 + W1 + Task-1 + Task-2 tests pass. [Tasks 1, 2]
- **G-C** (3 modalities): query_constraints dispatchable for File / Symbol / Topic; each returns expected shape. [Task 2]
- **G-D** (active-leaf default): list_notes returns active leaves only by default; include_history=true returns full chain with is_active_leaf flagged correctly. [Task 2]
- **G-E** (search.rs corpus_scope no regression): existing Query handler path unchanged in observed behavior. [Task 1]
</gates>

<must_haves>
truths:
  - "Agent can call list_notes(symbol_id) and receive active-leaf NoteViews; include_history=true returns full supersede chain"
  - "Agent can call query_constraints with File/Symbol/Topic target and receive ranked ConstraintHits with severity (Must > Should > Note) ranking applied"
  - "Topic modality reuses search.rs::search via CorpusScope::Constraints; SearchResult::Constraints variant returns ConstraintRowHit set"
  - "search.rs::search continues to work unchanged for existing Query handler (corpus_scope=None defaults to Symbols)"
  - "When adrs table is empty (W4 not yet shipped), query_constraints returns notes-only with adr_extracted=false flag (graceful degrade per G2 line 107-109)"
  - "Stale rowid in list_notes / query_constraints scope=symbol returns clear error; no crash"
  - "symbol_notes + adrs each have an embedding BLOB column populated either by W2 or by future writes; NULL is acceptable (BM25-only fallback)"
artifacts:
  - path: "experiments/poc-retrieval/core/src/search.rs"
    provides: "CorpusScope enum + SearchResult enum + search dispatcher + search_symbols + search_constraints"
    contains: "pub enum CorpusScope"
  - path: "experiments/poc-retrieval/core/src/storage.rs"
    provides: "ConstraintRowRef + ConstraintRowHit + all_constraint_embeddings + fetch_constraint_text + embedding BLOB columns"
    contains: "pub fn all_constraint_embeddings"
  - path: "experiments/poc-retrieval/core/src/a2a.rs"
    provides: "ListNotes + QueryConstraints A2A variants + ConstraintTarget + ConstraintSource + ConstraintHit + NoteView"
    contains: "pub struct ConstraintHit"
  - path: "experiments/poc-retrieval/core/src/server.rs"
    provides: "handle_list_notes + handle_query_constraints + dispatch arms"
    contains: "fn handle_query_constraints"
key_links:
  - from: "core/src/server.rs::handle_query_constraints"
    to: "core/src/storage.rs::list_notes_for_symbol + list_adrs_for_symbol + list_adrs_for_file"
    via: "scope-specific store lookups"
    pattern: "list_notes_for_symbol|list_adrs_for_symbol|list_adrs_for_file"
  - from: "core/src/server.rs::handle_query_constraints (Topic)"
    to: "core/src/search.rs::search (CorpusScope::Constraints)"
    via: "topic semantic ranking"
    pattern: "CorpusScope::Constraints"
  - from: "core/src/server.rs::handle_list_notes"
    to: "core/src/storage.rs::list_notes_for_symbol"
    via: "active-leaves filtering"
    pattern: "list_notes_for_symbol"
</must_haves>

<verification>
1. `cargo build --workspace --release` clean (G-A)
2. `cargo test -p codenexus-core --lib -- --test-threads=1` all green (G-B)
3. ListNotes + QueryConstraints (3 modalities) tests pass (G-C, G-D)
4. Existing search/Query tests still pass (G-E)
5. `grep -cE 'CorpusScope::Constraints' core/src/server.rs` >= 1 (Topic modality wires to constraint corpus)
</verification>

<open_questions>
- **OQ-W2-01:** D-W2-01 ALTER TABLE timing -- if W0 already shipped without `embedding BLOB` column, decide whether to (a) ALTER TABLE in W2 Step A (compatible with existing W0 DBs) OR (b) bump W0 schema retroactively (cleaner, but requires re-running W0). Recommend (a) for additive safety.
- **OQ-W2-02:** Embedder threading from dispatch state. If existing dispatch() does not have access to an Embedder instance (it constructs per-request), W2's handle_query_constraints follows the same per-request construction. Plan-checker confirms this is acceptable cost (~50ms per query for embedder load).
- **OQ-W2-03:** include_callers branch in handle_query_constraints scope=Symbol calls the existing list_callers logic. If list_callers is not currently a separate function (handler is inlined in dispatch), W2 either (a) factors it into a callable, (b) duplicates the PPR query inline. Plan-checker validates choice.
</open_questions>

<honest_gap_list>
**P1**:
- (none)

**P2**:
- D-W2-02 says W2 extends W1's handle_remember_symbol_note to also write embeddings. This means W2 INDIRECTLY modifies W1's handler. Plan-checker should verify the W1 handler post-W2 still passes its own tests (G-B regression). Risk: embedder unavailability at write time silently writes NULL embedding -> topic search misses that note. Mitigation: fall back to bm25-only via constraints_fts (W0 already populated by trigger). NULL embedding is acceptable.
- search() return type changes from `Result<Vec<Hit>>` to `Result<SearchResult>`. This is a BREAKING change to all existing call sites in server.rs Query handler. Step F covers it but plan-checker should grep for ALL `search::search` calls + verify each is updated.
- Topic modality test (Test 8) requires a real Embedder. If embedder model bytes are not in the test fixture, mark Test 8 #[ignore] and document manual command in SUMMARY. This does NOT compromise G-C because file/symbol modalities are independently tested.

**P3**:
- D-W2-03 severity weights (Must=1.5, MustNot=1.5, Should=1.0, Note=0.7) are eyeball; G6 W6 eval revises.
- adr_extracted boolean in QueryConstraints response is computed via `SELECT COUNT(*) FROM adrs > 0`; this is a global signal not a per-query signal. If a query targets a file/symbol that has no ADRs but the adrs table has rows for OTHER things, the flag returns true (correct: ADR extraction HAS run). UI/agent reads it as "the ADR pipeline has been run, so empty results are real, not pending".
- list_callers reuse for include_callers=true depends on the existing list_callers handler being callable as a library function (not just an HTTP handler). If it's currently inline in dispatch's ListCallers arm, plan-checker should flag this for refactor in W2 OR W3 (W3 also needs callers reuse for get_edit_context).
</honest_gap_list>
</content>
