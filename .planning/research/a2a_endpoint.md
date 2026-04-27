# A2A Endpoint Design (REQ-06) -- Build-Ready

> Phase 3 MVP design doc for experiments/poc-retrieval/ to grow an axum-based A2A v0.2 HTTP endpoint on top of the existing CLI handlers. Goal: next session can start writing Rust without re-deciding fundamentals.
>
> Author: Claude (research lane), 2026-04-27. Constraint: pure documentation, no Rust code committed yet.

Sources consulted:
- D:/projects/codenexus/.planning/REQUIREMENTS.md REQ-06 / REQ-07
- D:/projects/codenexus/docs/ARCHITECTURE.md sec.2 (supervision), sec.3 (A2A schema), sec.3.5 (envelopes)
- D:/projects/codenexus/experiments/poc-retrieval/{Cargo.toml, src/main.rs, src/storage.rs, src/search.rs}
- A2A v0.2 spec: https://google.github.io/A2A/ -- envelope shapes inherited verbatim from ARCHITECTURE sec.3.5, locked at Phase 1 close
- axum 0.8 release notes: https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0
- axum docs: https://docs.rs/axum/0.8/axum/
- tokio docs: https://docs.rs/tokio/1/tokio/ -- spawn_blocking for sync-DB bridge

---

## 0. TL;DR (10 bullets)

- **One** axum 0.8 router on port 9876 (auto-scan up to 9999, lockfile ~/.codenexus/port). Single A2A skill code-graph, four operations discriminated by data.operation.
- **3 new files** in src/: a2a.rs (envelope types), task_state.rs (Arc<RwLock<HashMap<TaskId, Task>>>), server.rs (router + handlers). main.rs gets one new Cmd::Serve branch.
- **Sync-to-async bridge**: every CLI handler stays sync. The async axum handler wraps the sync call in tokio::task::spawn_blocking. No rewrite of rusqlite / tree-sitter / candle code paths.
- **Long ops vs short ops**: index_repo returns task_id immediately, status transitions submitted -> working -> completed|failed. query, get_symbol, list_callers complete inline (still inside spawn_blocking) and respond with state: completed on the same call -- but client still polls GET /tasks/{id} to retrieve the body, per A2A v0.2.
- **/healthz** is *outside* /tasks/* (per ARCHITECTURE sec.2.2). Returns {ok, version, uptime_sec, indexed_repos}. Used by Go supervisor for the 10s liveness probe.
- **Cargo deps to add**: axum, tokio (full), tower, tower-http, tracing, tracing-subscriber, uuid, chrono. Existing serde, serde_json, anyhow, clap carry over. No version bumps to rusqlite 0.31 / tree-sitter 0.22 / reqwest 0.12 -- additive only.
- **CLI surface**: poc-retrieval serve --port 9876 [--db poc.db]. Standalone-runnable AND spawn-target for Go (REQ-07). Same binary.
- **SSE deferred**: poll-only in Phase 3 MVP. ARCHITECTURE sec.3.2 says SSE is optional in A2A v0.2 -- text/event-stream upgrade lands Phase 3.x once poll path is proven. Skeleton handler returns 501 Not Implemented for GET /tasks/{id}/stream.
- **Estimated wall-clock**: 45-60 min for green smoke (curl POST + GET round trip). Each step <= 10 min. Stretch goal: all 4 operations callable in 90 min.
- **Top risks**: (a) rusqlite Connection is !Send so it cannot cross a spawn_blocking boundary naively -- solve via r2d2-style pool *or* open per-call (POC-acceptable). (b) Long index_repo holds the embedder warm -- Phase 3 ok to spawn fresh per call, optimize Phase 4.

---
## 1. Cargo.toml deps

Append to experiments/poc-retrieval/Cargo.toml [dependencies] section. Versions pinned to MAJOR.MINOR; let Cargo float patch.

~~~toml
# === A2A endpoint deps (REQ-06) ===
axum = "0.8"                    # https://docs.rs/axum/0.8 -- current stable, 0.8.0 announced 2025-01
tokio = { version = "1", features = ["full"] }   # already at 1.52.1 in lockfile; full gives rt-multi-thread + signal + macros
tower = "0.5"                   # axum 0.8 ecosystem default
tower-http = { version = "0.6", features = ["trace", "cors"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
uuid = { version = "1", features = ["v7", "serde"] }   # ARCHITECTURE sec.5.4 D-B4 picks UUIDv7
chrono = { version = "0.4", features = ["serde"] }     # timestamps in Task envelopes
~~~

**Compatibility check** (verified against current Cargo.lock):

| Existing dep      | Pinned version         | A2A deps interaction         |
|-------------------|------------------------|-------------------------------|
| rusqlite 0.31     | bundled, modern_sqlite | sync, !Send -- wrap in spawn_blocking (sec.4) |
| tree-sitter 0.22  | sync                   | wrap in spawn_blocking |
| reqwest 0.12      | blocking + json        | reqwest blocking client lives inside spawn_blocking, no conflict |
| serde 1.0.228     | derive                 | shared with axum Json extractor |
| clap 4            | derive                 | new Cmd::Serve variant |

No version bumps needed for existing crates. axum 0.8 builds on tokio 1.x; the lockfile tokio 1.52.1 is forward-compatible (axum 0.8.x publishes against tokio 1.44+ per release notes).

**Optional later** (not Phase 3 first session):
- r2d2 = "0.8" + r2d2_sqlite = "0.24" -- connection pool. POC opens fresh Connection per request (sec.4) -- fine for MVP.
- axum-extra = "0.10" (with typed-header feature) -- only needed if SSE or richer extractors land.

---

## 2. File structure

Three new files in experiments/poc-retrieval/src/. Module wiring goes in main.rs mod declarations.

~~~
src/
  main.rs            (existing -- add Cmd::Serve branch + mod a2a/server/task_state)
  a2a.rs             (NEW -- envelope types, ~120 LOC)
  task_state.rs      (NEW -- in-memory Task store, ~60 LOC)
  server.rs          (NEW -- axum router + handlers, ~250 LOC)
  parser.rs          (existing, untouched)
  search.rs          (existing, untouched)
  storage.rs         (existing, may add 1 helper for repo_hash)
  graph_build.rs     (existing, untouched)
  graph_ppr.rs       (existing, untouched)
  embedder.rs        (existing, untouched)
  reranker.rs        (existing, untouched)
~~~

### 2.1 a2a.rs -- envelope types

Pure serde structs mirroring ARCHITECTURE sec.3.5 verbatim. No business logic.

~~~text
TaskId          = String          // ULID/UUIDv7 string per ARCHITECTURE sec.5.4
TaskState       = enum { Submitted, Working, InputRequired, Completed, Failed, Canceled }
Role            = enum { User, Agent }
PartType        = enum { Text, Data }     // (#[serde(rename_all = "lowercase")])

MessagePart     = { type: PartType, text: Option<String>, data: Option<serde_json::Value> }
Message         = { role: Role, parts: Vec<MessagePart> }

TaskRequest     = { task_id: TaskId, skill_id: String, messages: Vec<Message> }
TaskResponse    = { task_id: TaskId, state: TaskState, messages: Vec<Message>,
                    created_at: DateTime<Utc>, updated_at: DateTime<Utc> }

OperationRequest = enum {
    IndexRepo { repo_path: String, incremental: bool },
    Query     { repo_hash: String, q: String, k: usize },
    GetSymbol { repo_hash: String, symbol_id: String },
    ListCallers { repo_hash: String, symbol_id: String, depth: u32 },
}

OperationResponse = enum {
    IndexRepo { repo_hash, files_indexed, symbols_indexed, duration_ms, last_indexed_commit },
    Query     { results: Vec<QueryHit> },
    GetSymbol { symbol: SymbolFull },
    ListCallers { callers: Vec<CallerEntry> },
}

ErrorEnvelope   = { code: String, retryable: bool, details: serde_json::Value }
~~~

Use #[serde(tag = "operation", rename_all = "snake_case")] on the OperationRequest enum so JSON {"operation": "query", "q": "...", ...} parses straight into the variant.

### 2.2 task_state.rs -- in-memory store

~~~text
pub struct TaskStore {
    inner: Arc<RwLock<HashMap<TaskId, TaskResponse>>>,
}

impl TaskStore {
    pub fn new() -> Self { ... }
    pub fn insert(&self, task: TaskResponse)
    pub fn get(&self, id: &TaskId) -> Option<TaskResponse>
    pub fn update_state(&self, id: &TaskId, state: TaskState, messages: Vec<Message>)
}
~~~

Lock granularity: tokio RwLock (NOT std::sync::RwLock) so .read() in async handlers does not block the runtime. Phase 4+ may swap for sled/redb for crash recovery; trait-shape stays stable.

### 2.3 server.rs -- axum router + handlers

~~~text
pub fn router(state: AppState) -> Router

routes:
    POST /tasks/send             -> handler_tasks_send
    GET  /tasks/:id              -> handler_tasks_get
    GET  /tasks/:id/stream       -> handler_tasks_stream    (returns 501 in MVP)
    GET  /healthz                -> handler_healthz
    GET  /                       -> handler_agent_card      (A2A discovery)

pub struct AppState {
    pub task_store: TaskStore,
    pub db_path: PathBuf,
    pub started_at: Instant,
}

pub async fn serve(addr: SocketAddr, db_path: PathBuf) -> anyhow::Result<()>
~~~

serve() is the entry point called by Cmd::Serve in main.rs. It builds the router, binds the listener, and runs axum::serve(listener, router).await.

---

## 3. Operation routing

Each handler runs inside tokio::task::spawn_blocking because all four backend functions are sync (rusqlite + tree-sitter). Pseudocode below shows the wiring. **Real implementation must call .await on the JoinHandle and propagate errors via the A2A error envelope (sec.3.3 D-A3).**

### 3.1 index_repo (long-running, async pattern)

Long op -- return task_id immediately, transition state in background.

~~~text
async fn handler_index_repo(state: AppState, req: TaskRequest, args: IndexRepoArgs) -> TaskResponse {
    let task_id = req.task_id.clone();
    let initial = TaskResponse::new(task_id.clone(), TaskState::Submitted);
    state.task_store.insert(initial.clone());

    // Fire-and-forget: spawn_blocking inside spawn so request returns now
    let store_handle = state.task_store.clone();
    let db = state.db_path.clone();
    tokio::spawn(async move {
        store_handle.update_state(&task_id, TaskState::Working, vec![]);
        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<IndexResult> {
            // === wraps existing main.rs Cmd::Index logic verbatim ===
            let store = storage::Store::open(db.to_str().unwrap())?;
            store.clear()?;
            let embedder = embedder::Embedder::new();
            let symbols = parser::parse_repo(&args.repo_path)?;
            for sym in &symbols { /* embed + insert, see main.rs */ }
            Ok(IndexResult { files_indexed, symbols_indexed, ... })
        }).await;

        match result {
            Ok(Ok(r)) => store_handle.update_state(&task_id, TaskState::Completed, vec![success_msg(r)]),
            Ok(Err(e)) | Err(_) => store_handle.update_state(&task_id, TaskState::Failed, vec![error_msg(e)]),
        }
    });

    initial   // return immediately, client polls GET /tasks/{id}
}
~~~

### 3.2 query (short, inline pattern)

Short op -- complete synchronously inside the request, then stash the completed task in the store for the immediately-following GET.

~~~text
async fn handler_query(state: AppState, req: TaskRequest, args: QueryArgs) -> TaskResponse {
    let task_id = req.task_id.clone();
    let db = state.db_path.clone();

    // spawn_blocking so axum runtime is not blocked on rusqlite
    let hits = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<search::Hit>> {
        let store = storage::Store::open(db.to_str().unwrap())?;
        let embedder = embedder::Embedder::new();
        search::search(&store, &embedder, None, &args.q, args.k, 0.6)  // alpha locked per ARCH sec.9.1
    }).await??;

    let task = TaskResponse::completed(task_id, OperationResponse::Query { results: hits.into_iter().map(into_query_hit).collect() });
    state.task_store.insert(task.clone());
    task
}
~~~

### 3.3 get_symbol

~~~text
async fn handler_get_symbol(state, req, args) -> TaskResponse {
    let symbol = tokio::task::spawn_blocking(move || -> Result<SymbolFull> {
        let store = storage::Store::open(&db)?;
        // parse symbol_id (ULID? path:name composite?) -- Phase 3 decides format
        let id_int: i64 = parse_symbol_id(&args.symbol_id)?;
        let (path, name, kind) = store.symbol_by_id(id_int)?.ok_or(SymbolNotFound)?;
        let sym = store.fetch(id_int)?;
        // children/imports = walk edges of kinds [Imports] forward, [Calls] forward at depth 1
        Ok(SymbolFull { ..., children, imports })
    }).await??;
    completed_task(task_id, OperationResponse::GetSymbol { symbol })
}
~~~

store.symbol_by_id and store.fetch already exist (verified in storage.rs). symbol_id format for Phase 3: stringified i64 from symbols.id. Phase 4 can evolve to ULID without breaking the wire.

### 3.4 list_callers

~~~text
async fn handler_list_callers(state, req, args) -> TaskResponse {
    let callers = tokio::task::spawn_blocking(move || -> Result<Vec<CallerEntry>> {
        let store = storage::Store::open(&db)?;
        let id_int: i64 = parse_symbol_id(&args.symbol_id)?;
        // ARCH sec.9.7: list_callers uses [Calls] reverse direction at confidence >= 0.5
        let edges = store.edges_of_kinds(&["Calls"], 0.5)?;
        let callers_ids: Vec<i64> = edges.iter().filter(|(_, to, _)| *to == id_int).map(|(from, _, _)| *from).collect();
        let mut out = Vec::new();
        for cid in callers_ids.into_iter().take(args.depth as usize * 4) { // bfs branching cap sec.9.7
            if let Some((path, name, _kind)) = store.symbol_by_id(cid)? {
                out.push(CallerEntry { symbol_id: cid.to_string(), name, path, edge_kind: "CALLS".into() });
            }
        }
        Ok(out)
    }).await??;
    completed_task(task_id, OperationResponse::ListCallers { callers })
}
~~~

depth > 1 requires real BFS traversal; Phase 3 MVP can return depth-1 only and document the limit (Phase 4 adds full BFS via graph_ppr.rs-style edge-list walk).

### 3.5 Operation dispatch

In handler_tasks_send:

~~~text
async fn handler_tasks_send(State(s): State<AppState>, Json(req): Json<TaskRequest>) -> Json<TaskResponse> {
    let op_args = extract_data_part::<OperationRequest>(&req.messages)?;  // walks parts[] for type=data
    let resp = match op_args {
        OperationRequest::IndexRepo(a)   => handler_index_repo(s, req, a).await,
        OperationRequest::Query(a)       => handler_query(s, req, a).await,
        OperationRequest::GetSymbol(a)   => handler_get_symbol(s, req, a).await,
        OperationRequest::ListCallers(a) => handler_list_callers(s, req, a).await,
    };
    Json(resp)
}
~~~

---

## 4. Async model

### 4.1 The sync/async impedance

rusqlite::Connection is !Send (it embeds raw SQLite pointers). tree-sitter::Parser is also sync. candle is sync. The ONLY way to call them from an async axum handler without poisoning the runtime is tokio::task::spawn_blocking, which moves the closure to tokio blocking thread pool (default 512 threads, plenty for our load).

### 4.2 Connection lifecycle

**Phase 3 MVP**: open a fresh Connection per request inside the spawn_blocking closure. SQLite Connection::open is microsecond-cheap on an existing file. The closure owns the connection for its duration; no !Send violation because the connection never crosses an await.

**Phase 4**: add r2d2 pool keyed by db path. Pool lives in AppState, checkout happens inside spawn_blocking. Defer until profiling shows connection setup is meaningful (it will not -- sqlite-vec/FTS5 dwarfs it).

### 4.3 Embedder warmth

Each query handler does embedder::Embedder::new() -- for the POC ollama HTTP embedder (reqwest::blocking) this is just a struct literal, no model load. For Phase 3 candle embedder, model-load is 2-5s per call. **Mitigation: warm singleton in AppState behind a tokio::sync::OnceCell**, lazy-init on first query. Phase 3 first session can skip and pay the warm-up cost per query (still <= 5s, acceptable for smoke).

### 4.4 Long ops vs short ops summary

| Operation     | Pattern               | Returns immediately? | Polling required? |
|---------------|-----------------------|----------------------|-------------------|
| index_repo    | tokio::spawn + spawn_blocking background | Yes, with Submitted state | Yes -- poll until Completed/Failed |
| query         | inline spawn_blocking.await | No, blocks until done | Optional (response already has body) |
| get_symbol    | inline spawn_blocking.await | No | Optional |
| list_callers  | inline spawn_blocking.await | No | Optional |

A2A v0.2 spec allows both patterns; clients always issue POST /tasks/send then GET /tasks/{id} to retrieve. The short ops complete the body in-band on the POST response; the GET is then idempotent re-fetch.

---

## 5. Service supervision

### 5.1 serve subcommand

~~~text
poc-retrieval serve [OPTIONS]

OPTIONS:
    --port <PORT>     Bind port. Default: $CODENEXUS_PORT or auto-scan 9876..=9999. [default: 9876]
    --db <PATH>       SQLite DB path. Default: $CODENEXUS_DB or poc.db. [default: poc.db]
    --host <HOST>     Bind address. [default: 127.0.0.1]
    --log-format <FMT> json or pretty. [default: json] (matches ARCH sec.6 D-W4)
~~~

Env vars consumed (per ARCHITECTURE sec.5.5 D-B-extras):
- CODENEXUS_PORT -- Go-supervisor passes its chosen port here
- CODENEXUS_PORT_LOCKFILE -- path to ~/.codenexus/port (Phase 3.x: write our pid+port)
- CODENEXUS_DATA_DIR -- overrides --db parent dir
- RUST_LOG -- tracing-subscriber::EnvFilter
- HF_HOME -- passed straight through to candle (Phase 3 candle migration)

### 5.2 Healthcheck

~~~text
GET /healthz   ->   200 OK   { ok: true, version: 0.0.1, uptime_sec: 42, indexed_repos: 1 }
~~~

indexed_repos for MVP = 1 if DB file size > 0 else 0. Refine in Phase 4 when multi-repo registry lands.

**Crash semantics** (per ARCHITECTURE sec.2.2):
- /healthz reports Rust-process liveness ONLY.
- If Rust crashes, Go /healthz poll fails with TCP refused -- Go declares dead after 3 consecutive failures (30s wall-clock).
- Process-level exit codes: 0 clean shutdown (SIGTERM/SIGINT), 2 bind failure, 3 DB open failure, 1 panic.

### 5.3 Standalone vs spawned-by-Go

Same binary, same flags. Two invocation paths:

1. **Standalone (REQ-06 acceptance)**:

~~~bash
./poc-retrieval serve --port 9876 --db poc.db
curl -X POST http://localhost:9876/tasks/send -H content-type:application/json -d @sample.json
~~~

2. **Spawned-by-Go (REQ-07)**: Go exec.Cmd with env CODENEXUS_PORT=9876 CODENEXUS_DATA_DIR=~/.local/share/codenexus/<hash>, then polls http://localhost:9876/healthz until 200 OK or 30s timeout. Identical binary, identical handlers -- no --spawned-by-go flag.

### 5.4 Graceful shutdown

~~~text
tokio::signal::ctrl_c()  OR  SIGTERM (unix)
    -> drop axum listener
    -> wait up to 5s for in-flight handlers
    -> exit(0)
~~~

In-flight index_repo background tasks: best-effort cancel via dropping the tokio::spawn handle. Phase 4 can add task-state persistence so restart resumes; Phase 3 MVP just lets the next index_repo re-run.

---

## 6. Build-ready 30-min plan (Phase 3 first session)

Goal: green smoke `serve + curl /tasks/send` end-to-end in ≤ 60 min wall.

### Step 1 (5 min) — Cargo deps

`experiments/poc-retrieval/Cargo.toml`, append to `[dependencies]`:
~~~toml
axum = { version = "0.7", default-features = false, features = ["http1", "json", "tokio", "matched-path"] }
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "signal", "sync"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "cors"] }
uuid = { version = "1.10", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
~~~

`cargo build --release` → expect ~30s incremental, ~2-3 min cold. If breakage, likely tree-sitter / rusqlite version conflicts — check Cargo.lock diff.

### Step 2 (10 min) — `src/a2a.rs`

Define A2A v0.2 envelope types as serde structs. ARCHITECTURE.md §3.5 has 4 operations × envelope JSON examples — copy field shapes verbatim. Per §3.4 D-A4 query-result shape, the `Symbol` and `Hit` types likely already exist in `parser.rs` / `search.rs` — re-export or adapt.

Key types:
- `Task { id: Uuid, state: TaskState, created_at: DateTime, updated_at: DateTime, ... }`
- `TaskState { Submitted, Working, Completed, Failed }`
- `OperationRequest` enum: `IndexRepo { repo_path }`, `Query { text, top, alpha, rerank }`, `GetSymbol { id }`, `ListCallers { name, top }`
- `OperationResponse` mirroring with result payloads

Acceptance: `cargo check` passes, no logic yet.

### Step 3 (10 min) — `src/task_state.rs`

Wrapping `RwLock<HashMap<Uuid, Task>>` is sufficient for Phase 3 MVP (single-process, no persistence). Methods: `submit(req) -> task_id`, `get(id) -> Option<Task>`, `complete(id, result)`, `fail(id, err_msg)`. ~60 LOC.

### Step 4 (15 min) — `src/server.rs`

Axum router with two routes:
- `POST /tasks/send` — accepts `OperationRequest`, generates task_id, `tokio::spawn` worker (`spawn_blocking` for SQLite ops), returns `Task { state: Submitted }`
- `GET /tasks/{id}` — looks up state, returns `Task` or 404

Worker dispatch (pseudo):
~~~rust
match req {
    Query { text, .. } => {
        let store = Store::open(...)?;
        let hits = task::spawn_blocking(move || search::search(&store, ...)).await??;
        task_state.complete(id, hits.into());
    }
    IndexRepo { .. } => { /* spawn long task, return Submitted immediately */ }
    GetSymbol { id } => { /* spawn_blocking, simple lookup */ }
    ListCallers { name, top } => { /* graph_ppr::personalized_pagerank */ }
}
~~~

Add `/healthz` returning 200 OK.

### Step 5 (5 min) — Wire `serve` subcommand in `main.rs`

~~~rust
#[derive(Subcommand)]
enum Cmd {
    // ... existing
    Serve {
        #[arg(long, default_value_t = 9876)]
        port: u16,
        #[arg(long, default_value = "poc.db")]
        db: String,
    },
}

// In main, becomes:
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve { port, db } => {
            let task_state = Arc::new(task_state::TaskState::new(db));
            let app = server::router(task_state);
            let addr = format!("0.0.0.0:{}", port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            eprintln!("listening on {}", addr);
            axum::serve(listener, app).await?;
        }
        // ... existing sync handlers still work — tokio runtime lets them block fine
    }
    Ok(())
}
~~~

### Step 6 (15 min) — curl smoke test

~~~bash
./target/release/poc-retrieval serve --port 9876 &
sleep 2
curl -X POST http://localhost:9876/tasks/send \
  -H content-type:application/json \
  -d '{"operation":{"query":{"text":"ObsidianAdapter","top":5,"alpha":0.6,"rerank":false}}}'
# Expect: {"id":"<uuid>","state":"Submitted",...}
TASK=<uuid-from-prev>
curl http://localhost:9876/tasks/$TASK
# Expect: {"id":"<uuid>","state":"Completed","result":{"hits":[...]}}
curl http://localhost:9876/healthz
# Expect: 200 OK
kill %1
~~~

If green: REQ-06 acceptance met (modulo full conformance audit later).

### Risks (top 2)

1. **rusqlite is sync** — every DB-touching handler must wrap in `tokio::task::spawn_blocking`. Forgetting this hangs the runtime. Mitigation: a `db_op!()` macro or a single internal helper `async fn run_db<F, T>(f: F) -> T where F: FnOnce(&Store) -> T + Send + 'static, T: Send + 'static`. ~20 LOC, eliminates bugs at source.

2. **tree-sitter parser is `!Send` in some configs** — index_repo worker may need to construct parser inside `spawn_blocking`, not pass it across `.await`. Verify by attempting cross-thread send; if fails, instantiate per-call (negligible cost given index_repo is a long-lived op).

### Phase 4 deferred

- SSE streaming for /tasks/{id} progress events
- Task-state persistence to disk (graceful restart resumes)
- mTLS + auth (currently localhost-only)
- A2A v1.0 migration (when spec stabilizes)

---

## Provenance

Phase 1 deliverable. Build-ready spec for Phase 3 first session.
- A2A v0.2 spec: https://google.github.io/A2A/
- axum 0.7: https://docs.rs/axum/0.7/axum/
- ARCHITECTURE.md §2 (supervision), §3 (A2A schema), §3.5 (operation envelopes)
- POC source: `experiments/poc-retrieval/src/{main.rs,search.rs,storage.rs,graph_build.rs,graph_ppr.rs}`

