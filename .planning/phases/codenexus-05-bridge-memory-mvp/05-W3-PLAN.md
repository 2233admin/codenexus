---
phase: 5
slice: 05-W3
plan_id: 05-W3
title: "W3: A2A composite -- get_edit_context handler + 5-tool MCP wrap stubs"
wave: 3
depends_on: [05-W0, 05-W1, 05-W2]
status: PLAN-AUTHORED (awaits plan-checker iter)
files_modified:
  - experiments/poc-retrieval/core/src/a2a.rs
  - experiments/poc-retrieval/core/src/server.rs
  - server/internal/mcpsrv/server.go
  - server/internal/proxy/a2a.go
locked_decisions_honored:
  - G4   # composite handler over G2/G3/get_symbol/list_callers; symbol-only target in V1.0; single-blob; caller_depth 1..3
  - UQ-A5   # file-scope get_edit_context deferred to V1.1+ (V1.0 = symbol-only)
  - UQ-A3   # 5 MCP tools surfaced (extract_adrs is a stub here; W4 implements)
gates:
  - G-A   # build clean (Rust + Go)
  - G-B   # regression-green; all W0-W2 + 04.5-03 tests pass
  - G-C   # get_edit_context returns single composite blob with all 6 fields per G4 EditContextBrief
  - G-D   # 5 MCP tools registered with first-cut descriptions; calls dispatch through A2A proxy
  - G-E   # File-scope target returns clear "deferred to V1.1" error per UQ-A5
---

> **!! PROVISIONAL !!** This plan was authored 2026-05-03 in parallel with
> CCG round 2 challenge. Codex surfaced 4 critical issues (CI-1 G2 LOC,
> CI-2 G3 SQL FK, CI-3 G4 handler, CI-4 G5 FTS5) plus 3 missed constraints
> that affect this slice. **Do NOT execute this plan as-is.** See
> `.planning/phases/codenexus-05-bridge-memory-mvp/05-CCG-ROUND-2-FINDINGS.md`
> for required amendments before plan-checker iter and execution.


<objective>
Land W3: composite read op + MCP wrap stubs.

**Composite handler:** `get_edit_context(target: Symbol{symbol_id})`
returns one JSON blob containing: (1) symbol body, (2) callers
(depth 1, max 3), (3) constraints (notes + ADRs from W2), (4) notes
(active leaves only from W2), (5) edges_in, (6) edges_out. Per G4
05-discuss-api.md lines 270-301.

**MCP wrap stubs:** Register 5 NEW tools in `server/internal/mcpsrv/
server.go` (currently registers 4: index_repo, query, get_symbol,
list_callers per server.go lines 33-65). New tools per UQ-A3:
- query_constraints (W2)
- remember_symbol_note (W1)
- list_notes (W2)
- get_edit_context (W3 -- this plan)
- extract_adrs (W4 -- STUB returns "not yet implemented" until W4 lands)

W3 ships FIRST-CUT MCP descriptions (1-2 sentences each); W5 polishes
them to G6 production-grade prose (~200-300 words each per CodeCompass
quality bar).

V1.0 cut per UQ-A5: get_edit_context with `target: File{path}` returns
clear "file-scope deferred to V1.1" error. The EditContextTarget enum
ships both variants so MCP description can document the V1.1 path; the
File arm just returns Err.

Out of scope: ADR extraction (W4 implements extract_adrs handler);
production MCP descriptions (W5); eval harness (W6).

Output:
- `core/src/a2a.rs`: GetEditContext request/response variants +
  EditContextTarget enum + EditContextBrief struct + EdgeView struct.
- `core/src/server.rs::handle_get_edit_context`: composite handler
  ~80 LOC sequentially calling: list_notes, query_constraints
  (scope=Symbol), get_symbol-equivalent, list_callers, edges-in/out.
- `server/internal/mcpsrv/server.go`: 5 new s.AddTool registrations
  + 5 makeXxxHandler wrappers calling proxy.Client.
- `server/internal/proxy/a2a.go`: proxy methods for the 5 new ops
  (mirrors existing pattern for the 4 current ops).
</objective>

<plan_time_decisions>
- **D-W3-01 (composite serial vs parallel):** Serial sequential calls
  per G4 line 264-267. No futures::join_all for V1.0. Optimization is
  V1.1+. Justification: backend latency dominated by SQLite + embedder
  load; serial overhead trivial; simpler code = fewer bugs.
- **D-W3-02 (handler internal refactor):** Per G2 cross-coupling line
  362-369: refactor handle_query_constraints + handle_list_notes to
  expose `query_constraints_internal()` + `list_notes_internal()` as
  module-private functions returning Vec<ConstraintHit> /
  Vec<NoteView>. Both A2A handlers + composite handler call these.
  Avoids in-server HTTP recursion. Pure refactor; W2's external
  surface unchanged.
- **D-W3-03 (edges_in / edges_out wiring):** Use existing
  Store::edges_of_kinds (storage.rs line 67) -- kinds = ["Calls",
  "Implements", "Extends"] per CONTEXT.md EdgeKind variants. Filter
  by from_id == symbol_id (out) or to_id == symbol_id (in). When
  04.5-03 W3 has not landed yet (current state per drift probe:
  edges = 0), edges_in/out return [] gracefully -- per G7 line 134
  "edge-aware ops degrade gracefully if edges=0".
- **D-W3-04 (caller_depth implementation):** depth=1 returns direct
  callers; depth=2 returns callers + their callers; depth=3 same. Use
  existing list_callers backend (PPR or simple lookup). Per OQ #3 in
  05-discuss-api.md lines 401-404: callers shown WITHOUT their own
  edges (lighter payload). caller_depth=0 = skip callers entirely.
- **D-W3-05 (256KB payload cap):** Per G4 line 313-322. W3 unit test
  measures actual payload sizes on poc.db fixtures. If a real symbol
  exceeds 256KB, doc the failure and add caller_depth=0 mode + "use
  caller_depth=0 if payload too large" hint to MCP description (W5
  polishes). V1.0 ships no automatic pagination.
- **D-W3-06 (extract_adrs MCP stub semantics):** Stub returns
  `mcp.NewToolResultError("extract_adrs not yet implemented; ships in
  W4 of Phase 5")`. This is INTENTIONAL: the tool is REGISTERED so
  MCP clients see it in their tool list and the agent's mental model
  includes it; calling it before W4 lands gives a clear error not a
  silent miss. UQ-A3 = 5 ops public.
</plan_time_decisions>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W0-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W1-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W2-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md
@CONTEXT.md
@experiments/poc-retrieval/core/src/server.rs
@server/internal/mcpsrv/server.go
@server/internal/proxy/a2a.go

<interfaces>
<!-- Target a2a.rs additions -->

```rust
// New OperationRequest variant:
GetEditContext {
    target: EditContextTarget,
    #[serde(default = "default_caller_depth")] caller_depth: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditContextTarget {
    Symbol { symbol_id: i64 },
    File { path: String },         // V1.0: returns "deferred to V1.1" error
}

fn default_caller_depth() -> usize { 1 }

// New OperationResponse variant:
GetEditContext { brief: EditContextBrief }

#[derive(Serialize, Deserialize)]
pub struct EditContextBrief {
    pub symbol: SymbolView,                // existing a2a.rs SymbolView
    pub callers: Vec<CallerView>,          // existing a2a.rs CallerView
    pub constraints: Vec<ConstraintHit>,   // from W2 a2a.rs
    pub notes: Vec<NoteView>,              // from W2 a2a.rs
    pub edges_in: Vec<EdgeView>,
    pub edges_out: Vec<EdgeView>,
}

#[derive(Serialize, Deserialize)]
pub struct EdgeView {
    pub other: SymbolView,
    pub kind: String,                      // CONTEXT.md EdgeKind: "Calls"|"Implements"|"Extends"
    pub confidence: f64,
}
```

<!-- Existing MCP server.go pattern (verified 2026-05-03 via grep) -->

```go
// server/internal/mcpsrv/server.go currently has:
s.AddTool(
    mcp.NewTool("index_repo", ...),
    makeIndexHandler(client),
)
s.AddTool(mcp.NewTool("query", ...), makeQueryHandler(client))
s.AddTool(mcp.NewTool("get_symbol", ...), makeGetSymbolHandler(client))
s.AddTool(mcp.NewTool("list_callers", ...), makeListCallersHandler(client))

// makeQueryHandler pattern (line 109+):
func makeQueryHandler(client *proxy.Client) server.ToolHandlerFunc {
    return func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
        // parse req.Params; call client.Query(...); marshalToolResult(out)
    }
}
```

<!-- Existing proxy.Client pattern (read at execution time from server/internal/proxy/a2a.go) -->
<!-- Each existing op (Index, Query, GetSymbol, ListCallers) has a Client method -->
<!-- W3 adds: QueryConstraints, RememberSymbolNote, ListNotes, GetEditContext, ExtractAdrs (stub) -->
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: get_edit_context A2A variant + composite handler (Rust)</name>
  <files>experiments/poc-retrieval/core/src/a2a.rs, experiments/poc-retrieval/core/src/server.rs</files>

  <read_first>
    - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-api.md G4 (lines 249-340; full G4 section)
    - experiments/poc-retrieval/core/src/server.rs handle_query_constraints + handle_list_notes (W2)
    - experiments/poc-retrieval/core/src/storage.rs edges_of_kinds (line 67) + symbol_by_id (line 255)
    - CONTEXT.md lines 24-28 (EdgeKind variants: Calls, Implements, Extends; NOT Imports per A3.3 lift)
  </read_first>

  <behavior>
    - Test 1 (compile + a2a deserialize): `serde_json::from_str::<OperationRequest>(r#"{"get_edit_context":{"target":{"symbol":{"symbol_id":42}},"caller_depth":1}}"#)` succeeds; defaults caller_depth=1 if omitted
    - Test 2 (file scope returns error per UQ-A5): dispatch GetEditContext{target: File{...}} returns Err containing "file-scope" + "V1.1"
    - Test 3 (symbol scope happy path): given fixture with 1 symbol + 1 note + 1 caller (and 0 ADRs because W4 not yet shipped + 0 edges because 04.5-03 W3 not shipped), composite returns brief with: symbol populated, callers (1 row), notes (1 row), constraints (1 row from note), edges_in=[], edges_out=[]
    - Test 4 (caller_depth=0 omits callers): same fixture with caller_depth=0 returns brief with callers=[]; symbol/notes/constraints still populated
    - Test 5 (caller_depth=2 expands): fixture with A->B->C call chain; get_edit_context(C, depth=2) returns callers including B AND A
    - Test 6 (caller_depth > 3 capped): caller_depth=10 silently capped to 3; documented in MCP description (W5)
    - Test 7 (stale symbol_id): symbol_id=99999999 returns Err containing "not found"
    - Test 8 (edges populated when 04.5-03 W3 lands): mock test inserting edges directly; edges_in / edges_out return correctly. (Marked #[ignore] if 04.5-03 W3 not yet shipped; manual run command in SUMMARY.)
  </behavior>

  <action>

**Step A -- D-W3-02 refactor.** In server.rs, extract `handle_query_constraints` body into `query_constraints_internal(store: &Store, target: &ConstraintTarget, top: usize, alpha: f32, embedder: &Embedder) -> Result<(Vec<ConstraintHit>, bool /* adr_extracted */)>`. Similarly extract `list_notes_internal(store: &Store, symbol_id: i64, include_history: bool) -> Result<Vec<NoteView>>`. The A2A handlers handle_query_constraints / handle_list_notes become thin wrappers calling these.

Verification: W2 tests still pass after refactor.

**Step B -- add a2a.rs variants** per `<interfaces>` block. EditContextTarget + EditContextBrief + EdgeView are new structs / enums. Severity / ConstraintHit / NoteView / SymbolView / CallerView all already exist (W0-W2 + existing).

**Step C -- handle_get_edit_context** in server.rs:
```rust
fn handle_get_edit_context(
    db_path: &str,
    target: EditContextTarget,
    caller_depth: usize,
    embedder: &Embedder,
) -> anyhow::Result<OperationResponse> {
    // UQ-A5: file scope deferred to V1.1+
    let symbol_id = match target {
        EditContextTarget::Symbol { symbol_id } => symbol_id,
        EditContextTarget::File { .. } => {
            anyhow::bail!("file-scope get_edit_context deferred to V1.1; use Symbol target in V1.0");
        }
    };

    let depth = caller_depth.min(3);  // D-W3-04 cap

    let store = codenexus_core::storage::Store::open(db_path)?;
    let (path, name, kind) = store.symbol_by_id(symbol_id)?
        .ok_or_else(|| anyhow::anyhow!("symbol_id {} not found", symbol_id))?;

    let symbol = SymbolView { id: symbol_id, path: path.clone(), name: name.clone(), kind: kind.clone() };

    // Callers (depth-aware)
    let callers = if depth == 0 {
        Vec::new()
    } else {
        // Use existing list_callers logic (PPR or direct lookup; refactor if currently inlined)
        callers_at_depth(&store, &name, depth)?
    };

    // Notes (active leaves only via internal helper)
    let notes = list_notes_internal(&store, symbol_id, false)?;

    // Constraints (scope=symbol via internal helper; ADRs auto-included if W4 has run)
    let target_for_constraints = ConstraintTarget::Symbol { id: symbol_id, include_callers: false };
    let (constraints, _adr_extracted) = query_constraints_internal(
        &store, &target_for_constraints, /* top */ 50, /* alpha */ 0.6, embedder
    )?;

    // Edges in/out via existing storage API
    let edges_out = edges_for(&store, symbol_id, /* outgoing */ true)?;
    let edges_in  = edges_for(&store, symbol_id, /* outgoing */ false)?;

    Ok(OperationResponse::GetEditContext {
        brief: EditContextBrief {
            symbol, callers, constraints, notes, edges_in, edges_out,
        }
    })
}
```

`callers_at_depth(&store, &name, depth)` and `edges_for(&store, symbol_id, outgoing)` are new helpers. Implementations:
- `callers_at_depth`: BFS from name, calling existing `Store::callers_of` (or whatever the W2 list_callers logic uses) up to `depth` hops. Dedupe by symbol_id.
- `edges_for`: query `Store::edges_of_kinds(["Calls","Implements","Extends"], 0.0)` filtered by from_id==id (out) or to_id==id (in); for each edge, fetch the OTHER symbol via Store::fetch + populate EdgeView { other: SymbolView{...}, kind, confidence }.

**Step D -- dispatch arm** in server.rs:
```rust
OperationRequest::GetEditContext { target, caller_depth } => {
    handle_get_edit_context(&db_path, target, caller_depth, &state.embedder)
}
```

**Step E -- unit tests** Tests 2-7 from `<behavior>`. Test 8 marked `#[ignore]` if no edges fixture; document manual cargo command.

  </action>

  <acceptance_criteria>
    - `grep -nE 'OperationRequest::GetEditContext' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (dispatch arm)
    - `grep -nE 'fn handle_get_edit_context' experiments/poc-retrieval/core/src/server.rs` exactly 1 hit
    - `grep -nE 'fn query_constraints_internal' experiments/poc-retrieval/core/src/server.rs` exactly 1 hit (D-W3-02 refactor)
    - `grep -nE 'fn list_notes_internal' experiments/poc-retrieval/core/src/server.rs` exactly 1 hit
    - `grep -nE 'pub enum EditContextTarget' experiments/poc-retrieval/core/src/a2a.rs` exactly 1 hit
    - `grep -nE 'pub struct EditContextBrief' experiments/poc-retrieval/core/src/a2a.rs` exactly 1 hit
    - `grep -nE 'pub struct EdgeView' experiments/poc-retrieval/core/src/a2a.rs` exactly 1 hit
    - `grep -nF 'file-scope get_edit_context deferred to V1.1' experiments/poc-retrieval/core/src/server.rs` >= 1 hit (UQ-A5 error message)
    - `cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | grep -cE '^error'` returns 0 (G-A)
    - `cd experiments/poc-retrieval && cargo test -p codenexus-core get_edit_context -- --test-threads=1` >= 6 tests pass (G-C)
    - All W0-W2 + 04.5-03 tests still green (G-B)
  </acceptance_criteria>

  <verify>
    <automated>cd experiments/poc-retrieval && cargo build --workspace --release 2>&1 | tail -5 && cargo test -p codenexus-core --lib -- --test-threads=1 2>&1 | tail -30</automated>
  </verify>

  <done>
    a2a.rs has GetEditContext request/response + EditContextTarget +
    EditContextBrief + EdgeView. server.rs has handle_get_edit_context
    + query_constraints_internal + list_notes_internal (D-W3-02
    refactor) + dispatch arm. UQ-A5 file-scope returns clear error.
    caller_depth 1..3 enforced. Edges degrade gracefully when
    edges=0. Build clean (G-A); 6+ tests pass (G-C); regression-green
    (G-B).
  </done>
</task>

<task type="auto" tdd="false">
  <name>Task 2: 5 MCP tool stubs in Go server + proxy.a2a.go methods</name>
  <files>server/internal/mcpsrv/server.go, server/internal/proxy/a2a.go</files>

  <read_first>
    - server/internal/mcpsrv/server.go (full file -- read existing 4-tool registration pattern + handler factories)
    - server/internal/proxy/a2a.go (full file -- read existing Client method pattern; mirror it for 5 new methods)
    - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-mcp.md (G6 -- first-cut descriptions can be 1-line summaries; W5 polishes to ~200-300 word prose)
  </read_first>

  <action>

**Step A -- proxy.a2a.go new methods.** Add 5 methods on `proxy.Client`:
```go
func (c *Client) QueryConstraints(scope string, target string, top int, alpha float32) (json.RawMessage, error)
func (c *Client) RememberSymbolNote(symbolID int64, noteText, sourceSession string, confidence float32, tags []string, supersedes *int64) (json.RawMessage, error)
func (c *Client) ListNotes(symbolID int64, includeHistory bool) (json.RawMessage, error)
func (c *Client) GetEditContext(targetType string, targetValue interface{}, callerDepth int) (json.RawMessage, error)
func (c *Client) ExtractAdrs(scope []string) (json.RawMessage, error)  // stub method; backend returns error until W4
```

Each follows the existing pattern: build OperationRequest JSON, POST to /tasks/send, poll /tasks/{id}, return result.

**Step B -- mcpsrv/server.go register 5 new tools.** Add 5 `s.AddTool(mcp.NewTool("<name>", mcp.WithDescription("<first-cut description>"), mcp.WithString("<param>", ...)), make<Name>Handler(client))` calls. First-cut descriptions (W5 will replace with G6 production-grade prose):

- `query_constraints`: "Returns ranked constraint clauses (MUST/MUST-NOT/SHOULD) extracted from project ADRs and per-symbol notes for a file, symbol, or NL topic. Use BEFORE editing code when prior decisions might apply."
- `remember_symbol_note`: "Persist a per-Symbol annotation (gotcha, invariant, design constraint) for future agents to discover via query_constraints / get_edit_context. Append-only; supersede via supersedes parameter."
- `list_notes`: "Returns all per-Symbol notes for a given symbol_id. By default returns active leaves only; pass include_history=true for full supersede chain."
- `get_edit_context`: "Composite pre-edit brief for a Symbol: definition + callers + applicable constraints + prior notes + edges in/out. Call IMMEDIATELY BEFORE editing; replaces sequence of get_symbol + list_callers + query_constraints."
- `extract_adrs`: "Extract ADR-style constraints (MUST / MUST-NOT / SHOULD prose) from markdown docs into the constraints store. Auto-coupled to index_repo; standalone op for re-extraction without re-indexing. **Phase 5 W4 implements; W3 ships stub.**"

(EXECUTOR: read mcp-go API to confirm exact pattern for parameter declaration. Existing 4 tools use `mcp.WithString` etc. -- mirror.)

**Step C -- 5 makeXxxHandler functions** in server.go. Each parses MCP CallToolRequest, calls the corresponding client method, marshalls result. The extract_adrs handler returns `mcp.NewToolResultError("extract_adrs not yet implemented; ships in W4 of Phase 5 (see ROADMAP.md)")`. The other 4 dispatch normally and surface backend errors.

**Step D -- verify Go build:**
```bash
cd D:/projects/codenexus/server
go build ./...
go vet ./...
```

**Step E -- smoke test (manual or add Go integration test).** With Rust core running on localhost:8080, MCP stdio server should:
1. List 9 tools (4 existing + 5 new) when MCP client queries tool list.
2. Calling `remember_symbol_note` with valid args returns note_id.
3. Calling `extract_adrs` returns the "not yet implemented" error.

If a Go test infrastructure for the MCP server exists, add tests; otherwise document manual smoke command in SUMMARY.

  </action>

  <acceptance_criteria>
    - `grep -nE 's.AddTool\(\s*mcp.NewTool\("(query_constraints|remember_symbol_note|list_notes|get_edit_context|extract_adrs)"' server/internal/mcpsrv/server.go` exactly 5 hits
    - `grep -nE 'func make(QueryConstraints|RememberSymbolNote|ListNotes|GetEditContext|ExtractAdrs)Handler' server/internal/mcpsrv/server.go` exactly 5 hits
    - `grep -nE 'func \(c \*Client\) (QueryConstraints|RememberSymbolNote|ListNotes|GetEditContext|ExtractAdrs)' server/internal/proxy/a2a.go` exactly 5 hits
    - `grep -nF 'extract_adrs not yet implemented' server/internal/mcpsrv/server.go` >= 1 hit (W4 stub message)
    - `cd server && go build ./...` exits 0 (G-A Go)
    - `cd server && go vet ./...` exits 0 (G-A vet)
    - `cd experiments/poc-retrieval && cargo build --workspace --release` exits 0 (G-A Rust unaffected)
  </acceptance_criteria>

  <verify>
    <automated>cd server && go build ./... && go vet ./... && grep -cE 's.AddTool' internal/mcpsrv/server.go</automated>
  </verify>

  <done>
    Go MCP server registers 5 new tools (query_constraints,
    remember_symbol_note, list_notes, get_edit_context, extract_adrs)
    with first-cut 1-line descriptions. proxy.Client has 5 new
    methods mirroring existing pattern. extract_adrs returns clear
    "W4 not yet implemented" error. Go build + vet clean (G-A).
    G-D: 5 tools registered, all dispatch through proxy except
    extract_adrs which returns W4 error.
  </done>
</task>

</tasks>

<gates>
- **G-A** (build clean): Rust + Go both build clean. [Tasks 1, 2]
- **G-B** (regression-green): all W0-W2 + 04.5-03 tests pass after Task-1 internal-fn refactor. [Task 1]
- **G-C** (composite returns 6 fields): get_edit_context returns EditContextBrief with symbol + callers + constraints + notes + edges_in + edges_out. [Task 1]
- **G-D** (5 MCP tools registered): MCP server lists 9 total (4 existing + 5 new); 4 of 5 new dispatch through A2A; extract_adrs returns "not yet implemented" stub. [Task 2]
- **G-E** (file-scope deferred): GetEditContext{target: File{...}} returns Err with V1.1 deferral message per UQ-A5. [Task 1]
</gates>

<must_haves>
truths:
  - "Agent can call get_edit_context(symbol_id, caller_depth) and receive single composite brief with 6 fields per G4"
  - "File-scope target returns clear 'deferred to V1.1' error per UQ-A5; no panic; symbol-scope works"
  - "caller_depth defaults to 1, max 3; caller_depth=0 omits callers"
  - "Edges degrade gracefully when 04.5-03 W3 has not landed (edges_in=[] / edges_out=[])"
  - "MCP server registers 5 new tools (query_constraints, remember_symbol_note, list_notes, get_edit_context, extract_adrs) with first-cut descriptions (W5 polishes to G6 quality)"
  - "extract_adrs returns clear 'W4 not yet implemented' stub error -- tool is REGISTERED so agent's tool list is complete; calling pre-W4 surfaces the error not a silent miss"
  - "query_constraints_internal + list_notes_internal refactor (D-W3-02) preserves W2 external API; W2 tests stay green"
artifacts:
  - path: "experiments/poc-retrieval/core/src/a2a.rs"
    provides: "GetEditContext request/response + EditContextTarget + EditContextBrief + EdgeView"
    contains: "pub struct EditContextBrief"
  - path: "experiments/poc-retrieval/core/src/server.rs"
    provides: "handle_get_edit_context (~80 LOC composite) + internal helpers (query_constraints_internal, list_notes_internal, callers_at_depth, edges_for)"
    contains: "fn handle_get_edit_context"
  - path: "server/internal/mcpsrv/server.go"
    provides: "5 new s.AddTool registrations + 5 handler factories"
    contains: "get_edit_context"
  - path: "server/internal/proxy/a2a.go"
    provides: "5 new Client methods (QueryConstraints, RememberSymbolNote, ListNotes, GetEditContext, ExtractAdrs)"
    contains: "func (c *Client) GetEditContext"
key_links:
  - from: "core/src/server.rs::handle_get_edit_context"
    to: "core/src/server.rs::query_constraints_internal + list_notes_internal"
    via: "composite aggregation via internal helpers (no in-server HTTP recursion)"
    pattern: "query_constraints_internal|list_notes_internal"
  - from: "core/src/server.rs::handle_get_edit_context"
    to: "core/src/storage.rs::edges_of_kinds"
    via: "edges_in / edges_out population"
    pattern: "edges_of_kinds"
  - from: "server/internal/mcpsrv/server.go"
    to: "server/internal/proxy/a2a.go::Client"
    via: "5 new tool handlers dispatch through proxy"
    pattern: "client\\.(QueryConstraints|RememberSymbolNote|ListNotes|GetEditContext|ExtractAdrs)"
</must_haves>

<verification>
1. `cargo build --workspace --release` clean; `cd server && go build ./...` clean (G-A)
2. All Rust + Go tests pass; W0-W2 unchanged behavior (G-B)
3. get_edit_context (Symbol target) returns 6-field brief on a fixture (G-C)
4. `grep -cE 's.AddTool' server/internal/mcpsrv/server.go` returns 9 (4 existing + 5 new) (G-D)
5. get_edit_context (File target) returns Err with "V1.1" in message (G-E)
</verification>

<open_questions>
- **OQ-W3-01:** D-W3-02 refactor of W2 handlers into _internal helpers may have surface-area implications if W2's tests were structured around the public handler. Plan-checker re-validates W2 tests still pass after refactor.
- **OQ-W3-02:** callers_at_depth implementation depends on the existing list_callers backend (PPR vs direct lookup). If it's currently inlined in the ListCallers dispatch arm, this task INDIRECTLY refactors it into a callable. Plan-time can't know without reading server.rs end-to-end; executor decides.
- **OQ-W3-03:** Go MCP test infrastructure availability. If `cd server && go test ./internal/mcpsrv/...` runs nothing useful, smoke testing is manual. Document in SUMMARY.
</open_questions>

<honest_gap_list>
**P1**:
- (none)

**P2**:
- D-W3-02 refactor touches W2 code. If executed during plan-checker iter 1 and W2 tests fail, the fix loops back to W2's handler structure. Mitigation: refactor is mechanical (extract function); tests should pass unchanged.
- Edge population depends on 04.5-03 W3. Per drift probe SUMMARY 2026-05-03, current edges = 0 across both corpora. So get_edit_context's edges_in/out fields will return [] in production until 04.5-03 W3 ships. This is INTENTIONAL graceful degrade per G7 line 134; document in W3 SUMMARY + W5 MCP description.
- Test 8 (edges populated) is `#[ignore]` if no edges fixture is available. Plan-checker may want a synthetic edge inserted directly via SQL to enable Test 8 always-on. Acceptable either way; SUMMARY documents.

**P3**:
- 256KB payload cap (D-W3-05) is a measurement target, not enforcement. W3 unit test should at minimum measure payload size on a representative symbol (e.g., a heavily-called utility) and report. If exceeds, W3 SUMMARY flags for V1.1.
- Severity ranking weights inherited from W2 via constraint hits; no new tuning here.
- mcp-go library version may have evolved; Step B's `mcp.NewTool` + `mcp.WithDescription` + `mcp.WithString` API may have changed. Executor confirms by reading existing server.go pattern.
</honest_gap_list>
</content>
