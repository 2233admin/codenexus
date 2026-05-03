---
phase: 5
title: "Phase 5 Bridge -- A2A API Surface Discussion (G2/G3/G4)"
status: AMENDED 2026-05-03 per CCG round 2 (CI-1/CI-2/CI-3 cascade applied; see Round-2 Amendment Block below)
authority: BETA-V1-SPEC section 8 (proposed scope) + drift probe M5_fnk = 1.0 (storage key locked) + audit synthesis lines 60-67 (3 ops surface) + 05-CCG-ROUND-2-FINDINGS.md (Codex challenge 2026-05-03)
parent_artifacts:
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-PRE-PLAN-NOTES.md (G2/G3/G4 specs)
  - .planning/BETA-V1-SPEC.md section 8
  - .planning/probes/runs/2026-05-03-drift-evidence.md (M5_fnk = 1.0 -> (path, name, kind) primary)
  - CONTEXT.md (Symbol/Edge/AliasDecl/EdgeKind/ResolutionMethod/Confidence vocab)
  - experiments/poc-retrieval/core/src/{a2a.rs, server.rs, search.rs, storage.rs}
scope: API surface only -- W0 storage schema (notes table SQL beyond the sketch here),
  W4 ADR harness, W5 MCP wrap, W6 eval are OUT of this discuss; landed in 05-PLAN.md.
---

# Phase 5 Bridge -- A2A API Surface Discussion

Three new A2A operations land in Phase 5: query_constraints, remember_symbol_note,
get_edit_context. Each has gray-area shape decisions that must be locked before
plan-phase carves W0-W3. This doc resolves those shape decisions with opinions
plus concrete Rust struct sketches that match the existing OperationRequest /
OperationResponse style at core/src/a2a.rs:54-116.

---

## Round-2 Amendment Block (2026-05-03, CCG round 2 cascade)

Codex round-2 challenge surfaced 4 critical issues; this block supersedes
specific subsections below where flagged. Original text retained for audit
trail. Authoritative resolution path: this block first, original below as
historical record.

### A-CI-1: G2 backend dispatch -- replace `corpus_scope` with `kind_filter` + notes_fts

**Supersedes:** § "G2 query_constraints / Backend dispatch (per modality)" Topic
row + § "G2 / Reuse vs new path -- decision" + § "Cross-coupling / G2 + G3"
corpus_scope sentence.

**New decision:** ADRs become Symbols with `kind='ADR'` (G5 cascade per A-CI-1
in 05-discuss-adr.md). search.rs gains `kind_filter: Option<Vec<String>>` (NOT
a corpus abstraction) -- ~30-50 LOC parameter thread through search() ->
hybrid scoring. Notes get a separate dedicated FTS accessor
`Store::search_notes_fts(text, top)` -- BM25-only, no vector index in V1.0.
query_constraints Topic mode merges two result streams via RRF in the handler.

**Backend dispatch (revised):**

| Modality | Backend |
|---|---|
| File { path } | Store::symbols_in_file_full(path) -> per-symbol list_notes + (Symbol kind='ADR' WHERE adr_metadata.source_path = path OR adr_symbol_links join) |
| Symbol { id, include_callers=false } | Store::symbol_by_id(id) -> list_notes for fnk + adr_symbol_links join for ADR Symbols linked to this code Symbol |
| Symbol { id, include_callers=true } | Above + list_callers (PPR) -> per-caller notes + ADR links |
| Topic { text } | search::search(store, embedder, kind_filter=Some(vec!["ADR"]), text, top, alpha) over symbols_fts (existing, with body_text indexed via W0 ALTER) **+** Store::search_notes_fts(text, top) BM25-only over notes_fts (W0 creates) -> RRF-merged in handler |

**LOC re-sizing (honest):** ~30-50 LOC kind_filter on search.rs + ~30-50 LOC
notes FTS accessor + ~80-120 LOC query_constraints handler with two-stream
fusion = ~150-220 LOC total. In line with Codex's 100-200 estimate.

### A-CI-2: G3 notes table FK target -- unique index on symbols(path,name,kind)

**Supersedes:** § "G3 / SQL schema (notes table)" sketch + § "Honest gap" P1.

**New decision:** Pick **(a) unique index on symbols** per Codex CI-2. W0
storage migration adds:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_symbols_fnk
  ON symbols (path, name, kind);
```

Then `symbol_notes` declares the FK against the unique index target:

```sql
CREATE TABLE IF NOT EXISTS symbol_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- fnk identity (drift-probe-vindicated; FK targets symbols unique index)
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    note_text TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    confidence REAL NOT NULL,
    source_session TEXT NOT NULL,
    supersedes_note_id INTEGER REFERENCES symbol_notes(id),
    created_at TEXT NOT NULL,
    FOREIGN KEY (path, name, kind) REFERENCES symbols(path, name, kind)
);
CREATE INDEX IF NOT EXISTS idx_notes_fnk ON symbol_notes(path, name, kind);
-- prevent supersede fork (concurrent supersede on same note rejected at DB layer):
CREATE UNIQUE INDEX IF NOT EXISTS idx_notes_no_double_supersede
  ON symbol_notes(supersedes_note_id) WHERE supersedes_note_id IS NOT NULL;
-- notes FTS (BM25-only; external-content + triggers, mirrors symbols_fts):
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    note_text, tags, content='symbol_notes', content_rowid='id'
);
-- triggers symbol_notes_ai/ad/au mirror existing symbols_fts pattern (W0 spec)
```

Note: `constraints_fts` from the original sketch is REMOVED (no separate
constraints corpus exists under A-CI-1=(b) cascade). The notes_fts above is
the only new FTS table; ADRs reuse symbols_fts via the W0 `body_text` column
addition.

**Transaction discipline:** supersede operation is a single INSERT (new row
points at old via supersedes_note_id; old row never mutated). The unique
index `idx_notes_no_double_supersede` prevents fork at DB layer rather than
requiring application-level locks. No multi-step BEGIN/COMMIT needed for
remember_symbol_note write. (Original Codex CI-2 transaction concern dissolves.)

### A-CI-3: G4 get_edit_context -- internal-fn prerequisite + partial-failure contract

**Supersedes:** § "G4 / Composite vs independent backend decision" + § "G4 /
Signature" EditContextBrief + § "G4 / Recommendation" LOC.

**New decision:**

1. **Internal-fn extraction is a W3 prerequisite:** Before composite handler
   is written, refactor server.rs match arms (server.rs:100-197) to extract
   `handle_query_internal()`, `handle_get_symbol_internal()`,
   `handle_list_callers_internal()` -- all returning typed result structs.
   Composite handler then calls those internals directly, NOT the A2A
   endpoint recursively.

2. **Partial-failure contract: partial brief with warnings.** Composite
   returns the brief with whatever sub-calls succeeded; failed sub-calls
   surface in a new `warnings: Vec<String>` field (e.g., "list_callers
   timeout: 250ms exceeded"). Only validation errors on the request itself
   (bad target shape, unknown symbol_id) cause TaskState::Failed. This is
   per Codex CI-3 recommendation -- never return zero brief if symbol body
   was retrievable.

   ```rust
   pub struct EditContextBrief {
       pub symbol: SymbolView,
       pub callers: Vec<CallerView>,
       pub constraints: Vec<ConstraintHit>,
       pub notes: Vec<NoteView>,
       pub edges_in: Vec<EdgeView>,
       pub edges_out: Vec<EdgeView>,
       pub warnings: Vec<String>,    // NEW per A-CI-3: missing-data flags
   }
   ```

3. **LOC re-sizing (honest):** ~120 LOC internal-fn extraction (server.rs
   refactor) + ~80 LOC composite handler + ~40 LOC partial-failure tests =
   **~240 LOC total**, not the original 80. Plan-checker should sanity-check
   against the actual server.rs match-arm size.

### A-MC-1: Imports edges handling (mixed-schema DBs)

**Supersedes:** § "G4 / Recommendation" "NOT Imports" footnote.

**Decision:** EdgeView builders MUST handle pre-W0 DBs containing legacy
`Imports` edges. On encountering an Imports edge during edges_in/out load:
log warning + skip the edge + add `"skipped Imports edge: {edge_id}"` to
the brief's `warnings` field (per A-CI-3). Do NOT crash. CHECK constraint
in storage.rs:29-35 still permits Imports per CONTEXT.md ambiguity #1 lift
to AliasDecl being a pre-W0 schema state.

### Cross-coupling (revised)

**G2 + G3 share infrastructure (revised):** They share the W0 storage layer
(symbols unique index + notes table) but use separate FTS surfaces. G2 Topic
mode drives BOTH symbols_fts (kind='ADR' filter) AND notes_fts (BM25-only).
search.rs gets a `kind_filter` parameter, NOT a corpus abstraction.

**G4 calls G2 + G3 internally (revised, explicit):** Per A-CI-3, server.rs
internals are extracted into `handle_*_internal()` functions in W3 BEFORE
the composite handler. The composite calls those internals directly.

**G3's note vs G5's ADR (revised):** Both end up indexed in FTS, but
- ADRs = Symbol rows with kind='ADR' (per A-CI-1=(b) cascade in
  05-discuss-adr.md), text in symbols.body_text, indexed via symbols_fts
- Notes = symbol_notes rows, text in note_text, indexed via notes_fts
- Lifecycle: both append-only with supersede; ADRs use adr_metadata.superseded_by_symbol_id, notes use symbol_notes.supersedes_note_id

This separation is preserved -- the amendment only changes the storage
backend, not the lifecycle.

### Amendment status

After A-CI-1/2/3 + A-MC-1 land in W0/W1/W2/W3 PLANs, original CI/MC issues
in 05-CCG-ROUND-2-FINDINGS.md are resolved. CI-4 (FTS5 contentless terminology)
dissolves entirely under A-CI-1=(b) cascade -- ADR FTS reuses symbols_fts
which is already external-content + triggers (storage.rs:22-28).

---

Storage key policy is already locked by drift probe (commit d5e5eb0): (path, name,
kind) is the persistence identity (M5_fnk = 1.0); symbol_id (rowid) is the
caller-convenience input, resolved server-side at write time via existing
Store::symbol_by_id (core/src/storage.rs:255-269).

---

## G2 query_constraints

Find constraints (ADR MUST / MUST-NOT / SHOULD statements + per-symbol notes
flagged as constraint-class) attached to a file, a symbol + its callers, or
matching a NL topic.

### Signature (Rust struct sketch)

```rust
// In a2a.rs OperationRequest variants:
QueryConstraints {
    target: ConstraintTarget,                        // 3-modality enum (below)
    #[serde(default = "default_top")] top: usize,    // reuse default = 5
    #[serde(default = "default_alpha")] alpha: f32,  // reuse default = 0.6
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintTarget {
    File { path: String },
    Symbol { id: i64, include_callers: bool },  // default false; true expands
    Topic { text: String },
}

// In OperationResponse:
QueryConstraints { constraints: Vec<ConstraintHit> }

pub struct ConstraintHit {
    pub source: ConstraintSource, // Adr {doc_path, header_path} | Note {note_id, symbol_path, symbol_name, symbol_kind}
    pub severity: Severity,       // Must | MustNot | Should | Note (Note = G3-sourced, no severity tag)
    pub text: String,             // verbatim constraint text
    pub score: f32,               // RRF score for Topic mode; 1.0 for File / Symbol direct hits
    pub anchor: Option<HitView>,  // reuse existing HitView when symbol-bound
}
```

### Backend dispatch (per modality)

| Modality | Backend path | Reuse vs new |
|---|---|---|
| File { path } | Store::symbols_in_file_full(path) (storage.rs:216-232) -> for each symbol id, list_notes + ADR-anchored-to-file lookup | Reuse symbols_in_file_full + new Store::list_notes_for_symbol. ADR lookup needs new adr_anchors table query (W4 territory; if W4 not landed, return Notes only and surface adr_extracted=false flag). |
| Symbol { id, include_callers=false } | Store::symbol_by_id(id) (storage.rs:255-269) -> list_notes for that one (path, name, kind) + ADRs anchored to its file | Reuse symbol_by_id + new list_notes. |
| Symbol { id, include_callers=true } | Above + run existing list_callers (server.rs:136-197 PPR path) -> list_notes for each caller | Reuse list_callers PPR backend wholesale, then per-caller note lookup. |
| Topic { text } | search::search(store, embedder, None, text, top, alpha) (search.rs:21-100) over a constraint-text-only FTS+vec corpus, NOT the symbols corpus | NEW search index needed: constraints_fts virtual table + per-row embedding column, mirroring the symbols / symbols_fts pattern at storage.rs:14-28. Reuse search::search signature shape but new Store accessor (Store::bm25_constraints / Store::all_constraint_embeddings). |

### Reuse vs new path -- decision

**Reuse search.rs::search shape, NEW backend index for constraints.** Topic mode
must NOT search the symbols corpus -- agents querying "what are the constraints
on auth?" do not want function-name hits, they want constraint-text hits. The
symbols index returns wrong-type matches (BM25 over name + kind + snippet +
search_blob will not surface "MUST NOT log raw passwords" usefully).

Two-corpus design: symbols_fts (existing) + constraints_fts (new). Embeddings
populated at note-insert and at ADR-extract time (W0 + W4). Same RRF + cosine
math as search.rs:29-60; only the row source changes.

File:line evidence: search.rs:29 (store.bm25(...)) and search.rs:32
(store.all_embeddings()) are the two store-coupling seams. A second corpus
needs a parallel Store::bm25_constraints and Store::all_constraint_embeddings
to keep search::search corpus-agnostic OR a corpus: Corpus enum parameter
threaded through. Recommend the latter -- keeps one search function, branch
inside on corpus.

### Recommendation

- Lock 3-modality enum as above.
- Symbol mode default include_callers = false; explicit opt-in for the
  PPR-expansion variant (it is expensive; agents should not accidentally pull
  20 callers when they wanted one symbol notes).
- Topic mode is its own corpus with its own FTS + embedding column.
- Severity is a first-class enum, not a string; ADR rows carry MUST/MUST_NOT/
  SHOULD per BETA-V1-SPEC section 8 line 218; G3 notes carry Note (sentinel for
  "user-flagged but not ADR-grade").
- File mode + Symbol mode both return score = 1.0 (direct attachment, no
  ranking); Topic mode returns RRF score from search::search.
- If W4 ADR harness ships AFTER W2 query_constraints, return Notes only and
  set adr_extracted = false on the response envelope -- do not block W2 on W4.

---

## G3 remember_symbol_note

Persist per-symbol agent annotations keyed on (path, name, kind) per drift
probe. Caller passes symbol_id for convenience; server resolves to fnk at
write time.

### Schema decision: minimal-plus-3

Pre-plan notes pose minimal vs rich. Recommended: **minimal-plus-3**.

Minimal (4 fields per audit synthesis): symbol_id, note_text, source_session,
confidence.

**Add 3** (cost ~zero, value high):
- tags: Vec<String> -- enables severity = "must_not" style retrieval and
  cheap filtering without a join. Empty default. Reserved tag values:
  must, must_not, should, note (default). G2 File/Symbol modes return
  these as the severity field.
- supersedes_note_id: Option<i64> -- enables append-only supersede (below).
- created_at: DateTime<Utc> -- table-level audit + recency tie-break in G2
  ranking. Server-set on insert; not caller-supplied.

**Drop from rich**: author (use source_session -- caller supplies the agent
session id, and that IS the author), last_accessed (read-tracking has zero
business value at MVP; bloats writes; skip), updated_at (append-only design
forbids in-place mutation -- see lifecycle below).

### Storage key resolution (rowid input -> fnk persist)

Caller-API takes symbol_id: i64 (matches GetSymbol { id } ergonomics at
a2a.rs:83-85). Server immediately resolves via Store::symbol_by_id(id) ->
(path, name, kind) (storage.rs:255-269) and persists those three columns
+ note_text + auxiliaries. Note rowid (note_id) is the note OWN PK,
never the symbol rowid.

If symbol_by_id(id) returns None (caller passed stale id from a different
binary version), return OperationResponse::Failed-style error -- do NOT
silently write an orphan note. The drift probe shows rowid is stable
WITHIN a binary; cross-binary callers (rare today, possible at V1.1+
upgrade) get a clear error and should re-fetch the id via Query first.

### Supersede semantics: append-only

**Lock: append-only history, NEVER in-place mutation.**

- Supersede = INSERT a new note row + set its supersedes_note_id to the old
  row id. Old row stays intact.
- "Active note for this symbol" = the leaf of the supersede chain (rows where
  no other row points to them via supersedes_note_id).
- list_notes(symbol_id) returns active leaves by default; pass include_history
  = true to surface superseded ancestors.
- NO delete_note op exists. NO_DELETE_WITHOUT_AUDIT is the lifecycle invariant
  per BETA-V1-SPEC section 8 line 220. If a future op surfaces (e.g.
  redact_note for compliance), it MUST be a separate op with explicit
  audit-row insertion, not a back-door on remember_symbol_note.

Why append-only: (1) memory layer is the agent external long-term memory per
audit reframe (BETA-V1-SPEC section 5.5 row L5) -- destroying old notes
destroys context value; (2) eval reproducibility requires that an agent W6
eval-task output be re-derivable by re-running with the same notes-state-at-
time-T; (3) SQLite append-only is trivial vs in-place CRUD with audit-shadow
tables.

### SQL schema (notes table)

```sql
CREATE TABLE IF NOT EXISTS symbol_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- fnk identity (drift-probe-vindicated)
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    -- payload
    note_text TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT "[]",          -- JSON array; SQLite has no Vec<T>
    confidence REAL NOT NULL,                 -- caller-supplied [0, 1]
    source_session TEXT NOT NULL,             -- caller agent session id
    -- lifecycle
    supersedes_note_id INTEGER REFERENCES symbol_notes(id),
    created_at TEXT NOT NULL                  -- ISO 8601, server-set
);
CREATE INDEX IF NOT EXISTS notes_fnk
  ON symbol_notes(path, name, kind);
CREATE INDEX IF NOT EXISTS notes_supersedes
  ON symbol_notes(supersedes_note_id)
  WHERE supersedes_note_id IS NOT NULL;
-- Constraint corpus FTS for G2 Topic mode (W2 dependency, declared here for cross-ref):
CREATE VIRTUAL TABLE IF NOT EXISTS constraints_fts USING fts5(
    note_text, tags, content="symbol_notes", content_rowid="id"
);
-- ADR rows live in a separate adr_anchors table (W4 territory, OUT of this discuss).
```

### Rust struct sketch

```rust
RememberSymbolNote {
    symbol_id: i64,
    note_text: String,
    source_session: String,
    confidence: f32,
    #[serde(default)] tags: Vec<String>,
    #[serde(default)] supersedes_note_id: Option<i64>,
}

ListNotes {
    symbol_id: i64,
    #[serde(default)] include_history: bool,  // false = active leaves only
}

// OperationResponse::RememberSymbolNote { note_id: i64 }
// OperationResponse::ListNotes { notes: Vec<NoteView> }

pub struct NoteView {
    pub note_id: i64,
    pub path: String, pub name: String, pub kind: String,
    pub note_text: String,
    pub tags: Vec<String>,
    pub confidence: f32,
    pub source_session: String,
    pub supersedes_note_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub is_active_leaf: bool,
}
```

### Recommendation

Lock minimal-plus-3 schema + append-only supersede + fnk persistence with rowid
caller convenience. Two A2A ops actually ship in W1: remember_symbol_note
(write, returns note_id) and list_notes (read, with history flag).
**There is NO read_note(note_id) op** -- list_notes filtered by symbol covers
all read access; a single-note GET is dead weight for agent workflows that
always want all notes for a symbol.

---

## G4 get_edit_context

Composite read op: one A2A call returns the symbol body + callers + applicable
constraints + notes + edges in/out, replacing the manual sequence
`get_symbol` + `list_callers` + `query_constraints` + (hypothetical) `list_notes`.

### Composite vs independent backend decision

**Composite (chosen).** get_edit_context internally calls the same backends
as the 4 underlying ops (`get_symbol` -> Store::symbol_by_id, `list_callers`
-> Store::callers_of, `query_constraints scope=symbol` -> ConstraintsBackend
from G2, list_notes -> Store::list_notes from G3). Zero new backend code;
all aggregation in the handler.

Rationale: ships in W3 against W0/W1/W2 deliverables with zero new storage
or search code. Per CodeCompass affordance research (G6), the value is
having ONE call agents can reach for pre-edit, not optimizing the
aggregation. Optimization (parallel fan-out vs serial calls vs cached
sub-results) is a Phase 5 V1.1+ concern.

### Signature (Rust struct sketch)

```rust
// In a2a.rs OperationRequest variants:
GetEditContext {
    target: EditContextTarget,
    #[serde(default = "default_caller_depth")] caller_depth: usize,  // default 1, max 3
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditContextTarget {
    Symbol { symbol_id: i64 },
    File { path: String },        // V1.1+ per G6 open question 2; V1.0 returns Err if used
}

// OperationResponse::GetEditContext { brief: EditContextBrief }

pub struct EditContextBrief {
    pub symbol: SymbolView,                        // reuse get_symbol's response shape
    pub callers: Vec<CallerView>,                  // reuse list_callers's response shape
    pub constraints: Vec<ConstraintHit>,           // from G2 ConstraintsResponse
    pub notes: Vec<NoteView>,                      // from G3 list_notes (active leaves only)
    pub edges_in: Vec<EdgeView>,                   // file:line + EdgeKind + Confidence
    pub edges_out: Vec<EdgeView>,
}

pub struct EdgeView {
    pub other_symbol: SymbolView,
    pub kind: EdgeKind,                            // CONTEXT.md vocab: Calls/Implements/Extends
    pub confidence: f32,
}
```

### V1.0 cut: symbol-only target

Per G6 open question 2: V1.0 supports `Symbol { symbol_id }` only. File-
scope multiplies output size 20-50x (a 500-line file's symbols all need
full context blocks); cost-benefit is unproven without eval evidence. V1.0
returns `Err("file scope deferred to V1.1")` if `File { path }` is sent;
spec the deprecation path in MCP description so agents know when it'll
land.

### Pagination policy

V1.0: NO pagination. Single JSON blob. Limits enforced by caller_depth
(default 1, max 3) and by lists' inherent caps:
- callers: list_callers's existing default (top 50)
- constraints: G2's `top` parameter (default 5)
- notes: all active leaves for the symbol (typically <=10 per Pareto)
- edges_in/out: ALL edges of the symbol (capped by parser's per-symbol
  edge production; rarely exceeds 30)

If response payload exceeds 256KB in W3 testing, add `caller_depth=0` mode
(skip callers, useful for read-only briefs) before adding pagination.
Pagination is a V1.1+ concern.

### Output format

Single JSON blob, NOT paginated, NOT streamed. Matches existing A2A op
shape conventions (e.g., Query returns `{hits: [...]}` as one blob even
when top=50). MCP wrap (G6) presents as one tool call returning one
structured JSON; agent sees coherent brief, not 4 separate calls to glue.

### Recommendation

Composite handler at server.rs:handle_get_edit_context, ~80 LOC of
sequential calls + aggregation. No new storage paths. Symbol-only target
in V1.0; file-scope deferred to V1.1 with eval evidence as gate. Single-blob
response, no pagination. EdgeView reuses CONTEXT.md EdgeKind variants
verbatim (Calls / Implements / Extends; **NOT Imports** -- those are
AliasDecls per CONTEXT.md flagged ambiguity #1).

---

## Cross-coupling

### G2 + G3 share search infrastructure?

**Partially.** G2's ConstraintsBackend dispatches across 3 modalities; the
`scope=topic` modality reuses search.rs::search() for NL semantic ranking
over (constraints corpus = ADR rows + constraint-class notes). The
`scope=file` and `scope=symbol` modalities are SQL-only joins on the new
`adrs` + `adr_symbol_links` tables (G5) and `notes` table (G3). G3's
list_notes is pure SQL on `notes` table, no search reuse.

Implication: search.rs needs a small extension to accept a corpus filter
("search this subset of symbols/rows"), OR G2 builds a parallel mini-
search over constraints. Recommend: extend search.rs with a `corpus_scope:
Option<CorpusScope>` parameter rather than duplicating; ~30 LOC change,
plan-phase verifies.

### G4 calls G2 + G3 internally -- explicit or implicit?

**Implicit.** get_edit_context handler at server.rs calls the same internal
functions that QueryConstraints and ListNotes handlers call (refactor
those handlers to expose `query_constraints_internal()` and
`list_notes_internal()` -> shared by both A2A handler + composite). Agent
sees ONE op call (get_edit_context); server runs 4 internal calls + aggregates.

Anti-pattern to avoid: making get_edit_context call the A2A endpoint
recursively (server -> server). The internal-function refactor keeps the
call graph straight + testable.

### G3's note vs G5's ADR -- two separate things or one?

**Two.** G5's ADR is doc-extracted (markdown scan of ARCHITECTURE.md /
planning files); G3's note is agent-written runtime annotation on a Symbol.
Both surface in G2's query_constraints results with `source.kind = "ADR" |
"note"` discriminator (per G2 response shape line ~95). They share the
RANKING infrastructure (G2's BM25+vector hybrid) but NOT the storage
(separate tables: `adrs` + `notes`).

This separation matters: ADRs are immutable per markdown source-of-truth;
notes are append-only with supersede. Mixing them in one table forces a
worst-of-both-worlds lifecycle (ADRs would inherit supersede semantics they
don't need; notes would inherit doc-version-sha tracking they don't need).

---

## Open questions for Curry

1. **search.rs corpus_scope parameter** -- 30 LOC extension to filter search
   to a subset, OR build parallel mini-search? Recommend extension. Curry's
   call: are there pending search.rs refactors that'd conflict?

2. **NoteView's `is_active_leaf` flag** -- list_notes default returns
   active leaves only OR full history? Recommend: default = active leaves;
   `?include_history=true` query param returns full chain. Curry's call
   on whether history-by-default better serves agent debugging workflows.

3. **EdgeView depth** -- get_edit_context returns edges_in / edges_out for
   the target symbol only (depth=1), OR include callers' edges too
   (depth=caller_depth)? Recommend: depth=1 for target symbol, callers
   shown WITHOUT their own edges (lighter payload). If user wants caller
   context, they call get_edit_context on the caller separately.

4. **Confidence threshold filter on get_edit_context** -- should
   constraints/notes with `confidence < 0.5` be hidden from agent by
   default, or returned with a lower rank? Recommend: returned but ranked
   by `relevance * confidence`; agent sees full picture. Mirrors G3 open
   question 5.

5. **A2A op count for V1.0** -- counting ops PRE-PLAN-NOTES designated:
   query_constraints, remember_symbol_note, get_edit_context = 3. After
   G3 split (write + list separate ops), G5's extract_adrs (per G5
   recommendation), and Note: list_notes hidden inside get_edit_context
   means it MAY also need its own A2A op for direct queries. Net could be
   3 -> 5 ops. Curry: re-amend BETA-V1-SPEC sec 8 line 213 to name 4 or 5
   public ops?

6. **OperationResponse variant naming** -- existing a2a.rs uses
   `OperationResponse::Query { hits: Vec<...> }` style. Should new ops use
   bare-named-field shape (`OperationResponse::GetEditContext { brief: ...
   }`) OR introduce `OperationResponse::EditContext(EditContextBrief)`
   tuple variant? Recommend: bare-named-field for consistency. Plan-phase
   verifies.

---

## Self-check (analysis-triforce)

1. **Precision**: file:line citations for a2a.rs / storage.rs / search.rs;
   "M5_fnk = 1.0" sourced from drift probe SUMMARY commit d5e5eb0; "30 LOC
   change" for search.rs corpus_scope is a one-author guess (flagged P3).
2. **Framework adaptation**: composite-handler decision depends on G2's
   ConstraintsBackend existing as designed. If G5's `extract_adrs` op
   becomes async-only (file-watcher polling), G2's `scope=file` modality
   needs different shape; flagged.
3. **Feasibility**: G4's ~80 LOC composite handler + G2's 30 LOC search.rs
   extension is W3-sized work. Plan-checker iter 2 should re-validate
   against the W2 backend implementation surface (not just spec).

## Honest gap (rule 18)

**P1**: cross-coupling assumes G3's `notes` table primary key includes
`(path, name, kind)` triple via `symbol_fnk_id INTEGER REFERENCES
symbols_fnk(id)` join table -- the SQL DDL was sketched but not validated
against an actual SQLite migration. W0 plan-phase resolves.

**P2**: G4's "no pagination" decision is a one-author bet. If real-world
symbols (e.g. a heavily-called utility function with 200 callers) blow
the 256KB threshold, V1.0 needs an emergency `caller_depth=0` mode added
in W3 testing. Plan-checker should run a worst-case payload sim against
fsc.db to validate before W4.

**P3**: search.rs `corpus_scope` extension is a 30 LOC eyeball estimate.
Actual change may need to thread the parameter through search() ->
hybrid_score() -> scoring helpers; could be 50-80 LOC. Plan-phase verifies.
