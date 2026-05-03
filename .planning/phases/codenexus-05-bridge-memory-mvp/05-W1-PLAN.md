---
phase: 5
slice: 05-W1
plan_id: 05-W1
title: "W1: A2A op write side -- remember_symbol_note + JSONL export wiring"
wave: 1
depends_on: [05-W0]
status: PLAN-AUTHORED (awaits plan-checker iter)
files_modified:
  - experiments/poc-retrieval/core/src/a2a.rs
  - experiments/poc-retrieval/core/src/server.rs
locked_decisions_honored:
  - G3   # remember_symbol_note minimal-plus-3 schema; rowid input + fnk persist; append-only
  - G1   # JSONL export hook fires on every remember_symbol_note write (write-only)
  - UQ-A1   # JSONL writer uses the --export-dir from W0 CLI flag
gates:
  - G-A   # build clean
  - G-B   # all unit tests + handler integration tests pass
  - G-C   # A2A roundtrip: POST /tasks/send with remember_symbol_note + GET /tasks/{id} returns note_id
  - G-D   # JSONL side-effect: every successful write appends one NDJSON line
---

> **!! AMENDED 2026-05-03 per CCG round 2 !!** Round-2 amendment below
> SUPERSEDES specifics in the original plan_time_decisions for this slice.
> See `05-DISCUSS-SUMMARY.md § Round-3 Amendments LANDED` for cross-doc
> context.

## Round-2 Amendment Block (W1 -- light; CI-2 supersede semantics)

W1 inherits W0's amended schema (per `05-W0-PLAN.md § Round-2 Amendment
Block`). Two W1-specific amendments:

1. **Supersede via unique-index DB layer (CI-2 follow-on).** W0 creates
   `idx_notes_no_double_supersede ON symbol_notes(supersedes_note_id)
   WHERE supersedes_note_id IS NOT NULL`. W1 handler relies on this
   constraint -- second concurrent supersede on the same note row fails
   with SQLITE_CONSTRAINT_UNIQUE. Handler maps the error to a clear
   `OperationResponse::Failed { reason: "note already superseded by
   {existing_id}" }` for the caller. NO application-level lock needed;
   NO multi-step BEGIN/COMMIT needed for the write path.

2. **Supersede write is a single INSERT.** Per A-CI-2 amendment in
   05-discuss-api.md: supersede = INSERT new row with
   `supersedes_note_id = old_id`. Old row is NEVER mutated. The unique
   index above is the only fork-prevention mechanism.

Pre-write resolution remains: caller passes `symbol_id` (rowid); server
calls `Store::symbol_by_id(id)` -> (path, name, kind); writes those + payload
into `symbol_notes`. If `symbol_by_id` returns None, reject (no orphan
write; matches G3 lock).

### W1 acceptance test additions

- [ ] Insert note A on symbol X. Insert note B with supersedes=A. Verify
      idx_notes_no_double_supersede allows it.
- [ ] Attempt second supersede C with supersedes=A. Verify SQLITE_CONSTRAINT
      raised; handler returns Failed with the expected reason string.
- [ ] Insert note with stale symbol_id (no row). Verify Failed; verify
      symbol_notes table contains zero rows after the failed call.

### W1 unaffected items (still authoritative below)

- JSONL export hook wiring (G1 Mode B) -- unchanged
- minimal-plus-3 schema (note_text/tags/confidence/source_session/supersedes/
  created_at) -- unchanged in payload shape; FK targets the W0 unique index
  (this is the CI-2 cascade and lives in W0, not W1)
- A2A op shape (RememberSymbolNote with rowid input) -- unchanged

---


<objective>
Land the WRITE side of the Phase 5 A2A surface. After W1, agents can
POST a `remember_symbol_note` operation that:

1. Resolves the caller-supplied `symbol_id` (rowid) to (path, name,
   kind) via Store::symbol_by_id (existing).
2. Persists a row in `symbol_notes` via Store::insert_symbol_note (W0).
3. Appends one event to the JSONL export log per G1 Mode B (W0
   JsonlExporter, --export-dir flag from W0 wired here).
4. Returns the new `note_id` in the OperationResponse.

If symbol_by_id returns None (stale rowid from cross-binary upgrade),
the handler returns an error -- no orphan note is written. Per G3
discuss, this is the correct semantic; cross-binary callers should
re-fetch ids first.

Out of scope for W1: read side (W2: list_notes / query_constraints),
composite (W3: get_edit_context), ADR extraction (W4), MCP wrap (W5).

Output:
- `core/src/a2a.rs`: new `OperationRequest::RememberSymbolNote`
  variant + `OperationResponse::RememberSymbolNote { note_id }`
  variant per G3 discuss-api.md lines 207-235.
- `core/src/server.rs::dispatch`: new match arm calling
  `handle_remember_symbol_note(...)`.
- `core/src/server.rs::handle_remember_symbol_note`: new handler
  ~50-80 LOC. Resolves rowid -> fnk; calls Store::insert_symbol_note;
  fires JsonlExporter::append; returns note_id.
- Integration tests: A2A roundtrip via the existing test infrastructure
  pattern (if present) OR direct dispatch() calls in unit tests if no
  HTTP test infra exists yet (verify plan-time).
</objective>

<plan_time_decisions>
- **D-W1-01 (rowid resolution failure semantic):** Return
  `OperationResponse::Failed { error: "symbol_id <id> not found in
  this binary; re-fetch via Query" }` style error rather than a panic.
  Existing `dispatch()` returns `anyhow::Result<OperationResponse>`;
  `Err` propagates through to the A2A Task `error` field. Match the
  pattern used by GetSymbol when id is invalid (executor: read
  server.rs::dispatch GetSymbol arm to confirm pattern).
- **D-W1-02 (JSONL event payload shape):** Match memU Resource +
  MemoryItem shape per G1 OQ5 + 05-discuss-strategic.md lines 226-233:
  ```json
  {
    "event": "remember_symbol_note",
    "ts": "<ISO8601>",
    "payload": {
      "resource": {
        "kind": "symbol",
        "path": "<from fnk resolution>",
        "name": "<from fnk resolution>",
        "symbol_kind": "<from fnk resolution>"
      },
      "memory_item": {
        "note_id": <i64>,
        "text": "<note_text>",
        "tags": [...],
        "confidence": <f32>,
        "source_session": "<...>",
        "supersedes_note_id": null | <i64>
      }
    }
  }
  ```
  This makes V1.1+ memU replay a near-trivial mapping; deviation cost
  is documented in 05-discuss-strategic.md line 226-233.
- **D-W1-03 (created_at timestamp):** Server-set via `chrono::Utc::now().to_rfc3339()`. Caller does NOT supply -- prevents clock-skew confusion + enforces audit-trail integrity.
- **D-W1-04 (config plumbing for --export-dir):** Pass via the
  TaskStore state struct (Arc<TaskStore> already passed to router).
  Add an `export_dir: Option<PathBuf>` field on TaskStore; populate at
  startup from the CLI flag; handler reads from `state.export_dir`.
  Avoids per-request env var lookups + threads cleanly.
- **D-W1-05 (max note_text length):** Soft-enforce 2000 chars at the
  handler entry per G6 description prose ("max 2000 chars"). Returns
  Failed error if exceeded; SQL has no length constraint (TEXT is
  unlimited in SQLite). This protects against accidental 100KB note
  pastes.
</plan_time_decisions>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W0-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-strategic.md
@CONTEXT.md
@experiments/poc-retrieval/core/src/a2a.rs
@experiments/poc-retrieval/core/src/server.rs
@experiments/poc-retrieval/core/src/storage.rs

<interfaces>
<!-- Existing a2a.rs OperationRequest enum (verified 2026-05-03; lines 56-91) -->

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationRequest {
    IndexRepo { repo: String, max_consecutive_fail: Option<usize> },
    Query { text: String, top: usize, alpha: f32, rerank: bool },
    GetSymbol { id: i64 },
    ListCallers { name: String, top: usize },
    // W1 adds:
    // RememberSymbolNote {
    //     symbol_id: i64,
    //     note_text: String,
    //     source_session: String,
    //     confidence: f32,
    //     tags: Vec<String>,                       // serde default = []
    //     supersedes_note_id: Option<i64>,         // serde default = None
    // }
}
```

```rust
// Existing dispatch (server.rs line 98):
fn dispatch(db_path: String, op: OperationRequest) -> anyhow::Result<OperationResponse>

// W0 storage APIs available:
Store::symbol_by_id(id) -> Result<Option<(String, String, String)>>   // (path, name, kind)
Store::insert_symbol_note(path, name, kind, note_text, tags, confidence, source_session, supersedes, created_at) -> Result<i64>

// W0 jsonl_export available:
JsonlExporter::for_repo(repo_root, override_dir) -> Result<Self>
JsonlExporter::append(event: &serde_json::Value) -> Result<()>
```

```rust
// Target W1 additions to a2a.rs OperationRequest:
RememberSymbolNote {
    symbol_id: i64,
    note_text: String,
    source_session: String,
    confidence: f32,
    #[serde(default)] tags: Vec<String>,
    #[serde(default)] supersedes_note_id: Option<i64>,
}

// Target W1 additions to a2a.rs OperationResponse:
RememberSymbolNote {
    note_id: i64,
    stored_at: String,   // ISO 8601, mirrors created_at; surfaces to MCP for UX
}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Add RememberSymbolNote A2A request/response types + dispatch arm + handler</name>
  <files>experiments/poc-retrieval/core/src/a2a.rs, experiments/poc-retrieval/core/src/server.rs</files>

  <read_first>
    - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md G3 Rust struct sketch (lines 207-235)
    - .planning/phases/codenexus-05-bridge-memory-mvp/05-W0-PLAN.md interfaces block (Store::insert_symbol_note signature)
    - experiments/poc-retrieval/core/src/a2a.rs lines 56-116 (current OperationRequest + OperationResponse layout)
    - experiments/poc-retrieval/core/src/server.rs lines 54-160 (router + task_send + task_get + dispatch)
    - experiments/poc-retrieval/core/src/server.rs dispatch() existing match arms (template for new arm)
  </read_first>

  <behavior>
    - Test 1 (compile): `cargo build -p codenexus-core` exits 0
    - Test 2 (deserialize a2a request): `serde_json::from_str::<OperationRequest>(r#"{"remember_symbol_note":{"symbol_id":42,"note_text":"x","source_session":"s","confidence":0.9}}"#)` succeeds; tags defaults to []; supersedes_note_id defaults to None
    - Test 3 (dispatch with valid symbol_id): given a poc.db with at least one symbol indexed, dispatch RememberSymbolNote{symbol_id=<valid>, note_text="invariant: caller owns retry counter", source_session="s1", confidence=0.92, tags=["must"], supersedes_note_id=None} returns OperationResponse::RememberSymbolNote{note_id, stored_at}; note_id > 0; stored_at parses as ISO 8601
    - Test 4 (dispatch with stale symbol_id): dispatch with symbol_id=99999999 (out of range) returns Err with message containing "symbol_id" + "not found"
    - Test 5 (dispatch with note > 2000 chars): note_text repeated 'x' * 2001 returns Err with message containing "exceeds 2000 char limit"
    - Test 6 (supersede chain): two writes -- first w/ supersedes_note_id=None returning id1; second w/ supersedes_note_id=Some(id1) returning id2; verify via Store::list_notes_for_symbol that id2 is the active leaf
    - Test 7 (symbol_notes row written via SQL): after Test 3, `SELECT note_text, confidence, source_session FROM symbol_notes WHERE id=<note_id>` returns row matching what was sent
  </behavior>

  <action>

**Step A -- extend `core/src/a2a.rs`** OperationRequest enum:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationRequest {
    // ... existing variants ...
    RememberSymbolNote {
        symbol_id: i64,
        note_text: String,
        source_session: String,
        confidence: f32,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        supersedes_note_id: Option<i64>,
    },
}
```

And OperationResponse enum:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationResponse {
    // ... existing variants ...
    RememberSymbolNote {
        note_id: i64,
        stored_at: String,
    },
}
```

Place new variants AFTER existing ones to minimize diff against
historical positions; serde tags handle ordering.

**Step B -- add `handle_remember_symbol_note` in `core/src/server.rs`.** Place after the existing handler functions (locate via grep for `fn handle_` or look at the dispatch match arm structure -- existing handlers may be inlined in dispatch; if so add a new module-level `fn handle_remember_symbol_note` adjacent to dispatch and call it from the match arm).

```rust
fn handle_remember_symbol_note(
    db_path: &str,
    symbol_id: i64,
    note_text: String,
    source_session: String,
    confidence: f32,
    tags: Vec<String>,
    supersedes_note_id: Option<i64>,
    export_dir: Option<&std::path::Path>,
    repo_root: &std::path::Path,
) -> anyhow::Result<OperationResponse> {
    // D-W1-05: soft-enforce note length
    if note_text.chars().count() > 2000 {
        anyhow::bail!(
            "note_text exceeds 2000 char limit (got {} chars)",
            note_text.chars().count()
        );
    }

    let store = codenexus_core::storage::Store::open(db_path)?;

    // D-W1-01: rowid -> fnk resolution
    let (path, name, kind) = match store.symbol_by_id(symbol_id)? {
        Some(t) => t,
        None => anyhow::bail!(
            "symbol_id {} not found in this binary; re-fetch via Query",
            symbol_id
        ),
    };

    let now = chrono::Utc::now().to_rfc3339();
    let note_id = store.insert_symbol_note(
        &path, &name, &kind,
        &note_text,
        &tags,
        confidence,
        &source_session,
        supersedes_note_id,
        &now,
    )?;

    // D-W1-02: JSONL event side-effect (write-only, G1 Mode B)
    let exporter = codenexus_core::jsonl_export::JsonlExporter::for_repo(repo_root, export_dir)?;
    let event = serde_json::json!({
        "event": "remember_symbol_note",
        "ts": now,
        "payload": {
            "resource": {
                "kind": "symbol",
                "path": path,
                "name": name,
                "symbol_kind": kind
            },
            "memory_item": {
                "note_id": note_id,
                "text": note_text,
                "tags": tags,
                "confidence": confidence,
                "source_session": source_session,
                "supersedes_note_id": supersedes_note_id,
            }
        }
    });
    if let Err(e) = exporter.append(&event) {
        // Non-fatal: log + continue. JSONL is V1.1+ replay scaffold,
        // not load-bearing for V1.0 functionality.
        tracing::warn!("jsonl export failed: {}", e);
    }

    Ok(OperationResponse::RememberSymbolNote { note_id, stored_at: now })
}
```

(EXECUTOR: if `tracing` is not yet a dep, use `eprintln!` instead. Verify in Cargo.toml.)

**Step C -- wire dispatch arm.** In `dispatch()`:
```rust
OperationRequest::RememberSymbolNote { symbol_id, note_text, source_session, confidence, tags, supersedes_note_id } => {
    handle_remember_symbol_note(
        &db_path, symbol_id, note_text, source_session, confidence, tags, supersedes_note_id,
        state.export_dir.as_deref(), &state.repo_root,
    )
}
```

(EXECUTOR: this requires `state` to be threaded into `dispatch`. Currently `dispatch` takes `db_path: String`. Plan-time decision D-W1-04 says add `export_dir: Option<PathBuf>` AND `repo_root: PathBuf` to TaskStore; modify dispatch signature OR pass them as additional parameters. Read server.rs line 89-100 to confirm the call site in task_send / task_get and adjust accordingly. If TaskStore does not exist as `state` per D-W1-04, alternative: read from a once-cell global populated at startup. Pick whichever pattern matches existing conventions; document choice in SUMMARY.)

**Step D -- write integration tests** in `core/src/server.rs` (or a new `tests/remember_symbol_note.rs` integration test file). Build a minimal poc.db fixture (use existing test helpers if present, else create one inline indexing 1 file). Run dispatch directly and verify:
- Test 3-7 from `<behavior>` above.

If existing handler tests use a different pattern (e.g., shell out to running server), mirror that. Plan-time: read server.rs end + any tests/ dir to confirm existing pattern.

**Step E -- verify build + run tests:**
```bash
cd D:/projects/codenexus/experiments/poc-retrieval
cargo build --workspace --release 2>&1 | tail -5
cargo test -p codenexus-core --lib -- --test-threads=1 2>&1 | tail -30
```

  </action>

  <acceptance_criteria>
    - `grep -nE 'RememberSymbolNote\s*\{' experiments/poc-retrieval/core/src/a2a.rs` >= 2 hits (one in OperationRequest, one in OperationResponse)
    - `grep -nE 'fn handle_remember_symbol_note' experiments/poc-retrieval/core/src/server.rs` exactly 1 hit
    - `grep -nE 'OperationRequest::RememberSymbolNote' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (dispatch arm)
    - `grep -nE 'JsonlExporter::for_repo' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (G1 Mode B side-effect call)
    - `grep -nF '"remember_symbol_note"' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (event name string in JSONL payload)
    - `grep -nF 'exceeds 2000 char limit' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (D-W1-05 length check)
    - `grep -nF 'not found in this binary' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (D-W1-01 rowid fail message)
    - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -cE '^error'` returns 0 [G-A]
    - `cd experiments/poc-retrieval && cargo test -p codenexus-core --lib remember_symbol_note -- --test-threads=1` exits 0 with all tests passing [G-B]
    - All W0 + 04.5-03 W0 tests still green [G-B regression]
  </acceptance_criteria>

  <verify>
    <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core --lib -- --test-threads=1 2>&1 | tail -30</automated>
  </verify>

  <done>
    a2a.rs has OperationRequest::RememberSymbolNote +
    OperationResponse::RememberSymbolNote variants. server.rs has
    handle_remember_symbol_note function (~80 LOC) + dispatch arm.
    Handler resolves rowid -> fnk, persists to symbol_notes, fires
    JSONL export side-effect, returns note_id. Length limit + stale-id
    error paths behave per D-W1-01 / D-W1-05. Integration tests cover
    happy path + 2 error paths + supersede chain. Build clean (G-A);
    all tests green (G-B).
  </done>
</task>

<task type="auto" tdd="false">
  <name>Task 2: HTTP A2A roundtrip smoke test + JSONL side-effect verification (G-C, G-D)</name>
  <files>experiments/poc-retrieval/core/src/server.rs (test only) OR experiments/poc-retrieval/core/tests/remember_smoke.rs</files>

  <read_first>
    - experiments/poc-retrieval/core/src/server.rs router + task_send + task_get + healthz (lines 54-100)
    - experiments/poc-retrieval/core/tests/ if present (existing integration test pattern)
  </read_first>

  <action>

**Step A -- end-to-end smoke test.** Create a test that:
1. Indexes a fixture (use existing test helpers) to get a valid symbol_id.
2. Spins up the axum router with a TaskStore containing temp db_path + temp export_dir + temp repo_root.
3. POSTs `{"operation": {"remember_symbol_note": {"symbol_id": <id>, "note_text": "test", "source_session": "smoke", "confidence": 0.8}}}` to /tasks/send via tower::ServiceExt or axum::Server in a tokio test.
4. Polls /tasks/{id} until state == Completed.
5. Asserts response body has `note_id` field.
6. Asserts that `<export_dir>/notes.jsonl` exists and contains exactly 1 line, parseable as JSON, with `event == "remember_symbol_note"`.

(EXECUTOR: if existing tests do NOT use the axum router (i.e., all current tests bypass HTTP and call dispatch directly), this task is OPTIONAL and may be replaced with a unit test that calls dispatch + verifies JSONL side-effect on the same temp dir. Document the choice in the SUMMARY. The G-D gate (JSONL side-effect) MUST be verified one way or the other.)

**Step B -- verify:**
```bash
cd D:/projects/codenexus/experiments/poc-retrieval
cargo test -p codenexus-core --test remember_smoke 2>&1 | tail -20
# OR if integrated as unit test:
cargo test -p codenexus-core --lib remember_symbol_note_smoke -- --test-threads=1
```

  </action>

  <acceptance_criteria>
    - Integration test file exists OR unit test added that: (a) writes a note via dispatch/router, (b) verifies symbol_notes row, (c) verifies JSONL line written
    - `cd experiments/poc-retrieval && cargo test -p codenexus-core remember -- --test-threads=1` exits 0 [G-C, G-D]
    - JSONL line written contains `"event":"remember_symbol_note"` AND `"resource":{"kind":"symbol"` (D-W1-02 memU shape)
  </acceptance_criteria>

  <verify>
    <automated>cd experiments/poc-retrieval && cargo test -p codenexus-core remember -- --test-threads=1 2>&1 | tail -20</automated>
  </verify>

  <done>
    End-to-end (or dispatch-level if no HTTP test infra) smoke test
    passes. JSONL side-effect verified per G-D. Path: caller posts
    A2A request -> handler resolves rowid -> writes SQLite row -> writes
    JSONL line -> returns note_id. SUMMARY documents whether the test
    used HTTP or dispatch-level.
  </done>
</task>

</tasks>

<gates>
- **G-A** (build clean): `cargo build --workspace --release` clean. [Tasks 1, 2]
- **G-B** (all tests green): all unit tests + integration tests pass; W0 + 04.5-03 W0 stay green. [Tasks 1, 2]
- **G-C** (A2A roundtrip): RememberSymbolNote dispatchable via OperationRequest; valid call returns note_id; stale-id call returns clear error. [Task 1, Task 2]
- **G-D** (JSONL side-effect): every successful write appends one NDJSON line to <export_dir>/notes.jsonl with memU-shaped payload. [Task 2]
</gates>

<must_haves>
truths:
  - "Agent can POST OperationRequest::RememberSymbolNote with rowid + note_text + source_session + confidence (+ optional tags + supersedes_note_id) and receive note_id"
  - "Server resolves rowid to (path, name, kind) via Store::symbol_by_id; persists row to symbol_notes via Store::insert_symbol_note (W0)"
  - "Server fires JsonlExporter::append with memU-shaped {event, ts, payload: {resource, memory_item}} per G1 Mode B"
  - "Stale rowid (cross-binary) returns clear error containing 'symbol_id ... not found'; no orphan row written"
  - "note_text > 2000 chars rejected with clear error (D-W1-05)"
  - "Supersede chain works: second write w/ supersedes_note_id=Some(id1) marks id1 as ancestor; list_notes(active_leaves) returns only the new row"
  - "JSONL side-effect failure does NOT abort the SQL write (best-effort logging; SQL row remains durable)"
artifacts:
  - path: "experiments/poc-retrieval/core/src/a2a.rs"
    provides: "OperationRequest::RememberSymbolNote + OperationResponse::RememberSymbolNote variants"
    contains: "RememberSymbolNote"
  - path: "experiments/poc-retrieval/core/src/server.rs"
    provides: "handle_remember_symbol_note function + dispatch arm"
    contains: "fn handle_remember_symbol_note"
key_links:
  - from: "core/src/server.rs::handle_remember_symbol_note"
    to: "core/src/storage.rs::insert_symbol_note"
    via: "store.insert_symbol_note(...)"
    pattern: "insert_symbol_note"
  - from: "core/src/server.rs::handle_remember_symbol_note"
    to: "core/src/jsonl_export.rs::JsonlExporter"
    via: "JsonlExporter::for_repo + append"
    pattern: "JsonlExporter"
  - from: "core/src/server.rs::handle_remember_symbol_note"
    to: "core/src/storage.rs::symbol_by_id"
    via: "rowid -> (path, name, kind) resolution"
    pattern: "symbol_by_id"
</must_haves>

<verification>
1. `cargo build --workspace --release` clean (G-A)
2. `cargo test -p codenexus-core --lib -- --test-threads=1` all green (G-B)
3. RememberSymbolNote roundtrip test passes (G-C)
4. After 1 successful write, `wc -l <export_dir>/notes.jsonl` returns 1; line parses as JSON with event=="remember_symbol_note" (G-D)
5. After failed write (stale id), `wc -l <export_dir>/notes.jsonl` unchanged (no orphan event)
</verification>

<open_questions>
- **OQ-W1-01:** Existing dispatch() takes only `db_path: String`. Whether to (a) add TaskStore.state with export_dir + repo_root and modify dispatch signature, or (b) use a once-cell global populated at startup -- pick at execution time based on what minimizes server.rs surface area churn. Lock during plan-checker pass if executor needs guidance; W1 plan accepts either path.
- **OQ-W1-02:** repo_root inference. JsonlExporter::for_repo takes a repo_root. If TaskStore knows db_path but not repo_root, options: (i) infer as parent dir of db_path, (ii) accept --repo-root CLI flag (mirrors --export-dir). Recommend (i) for V1.0 simplicity; revisit if multi-repo support lands in V1.1+.
</open_questions>

<honest_gap_list>
**P1**:
- (none)

**P2**:
- HTTP-level smoke test depends on existing axum test infrastructure. If none exists, dispatch-level test is acceptable for V1.0 and the SUMMARY should note that "true HTTP A2A roundtrip" was tested implicitly via Task 1 unit tests, not via tower::ServiceExt. This is honest documentation, not a defect.
- D-W1-04 (TaskStore.export_dir threading) chooses between two patterns; both work; the choice is made at execution time. Plan-checker may want to force one or the other; current PLAN leaves it to the executor.

**P3**:
- chrono::Utc::now() depends on `chrono` already being in deps (verified via existing a2a.rs `use chrono::{DateTime, Utc};` line 9 -- safe).
- 2000 char limit is char count, not byte count; emoji-heavy notes that fit in 2000 chars but exceed bytes is intentional (matches G6 description prose phrasing "max 2000 chars").
- Existing dispatch() uses `db_path: String` (owned); the new arm needs `state.export_dir.as_deref()` which requires a borrow. Plan-time alternative: clone the PathBuf. Either works; executor picks.
</honest_gap_list>
</content>
