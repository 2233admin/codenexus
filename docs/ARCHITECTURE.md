# CodeNexus Architecture

> Phase 1 deliverable, 2026-04-27. Status: **all sections locked**, including §9 retrieval (R3 plateau validated, R4 stalled, Path B deferred to Phase 3 with LLM-judge prerequisite).

---

## 0. Document Scope

**In scope (this document, all locked):**

- Service supervision and `/healthz` failure semantics under crash-loop (§2)
- A2A schema shape and four operation envelopes (§3)
- Clean-room policy and license boundary; NTFS atime mechanism decided (§4)
- State ownership boundary; `koanf` picked for Go config (§5)
- Logging stack and trace propagation (§6)
- Embedder device abstraction and worker-pool topology (§7)
- CI/CD GPU compilation policy (§8)
- **Retrieval architecture (§9): R3 Path A configuration locked; Path B reranker code retained, gated on Phase 3 LLM-judge prerequisite**
- Future / deferred items, including Phase 2 storage backend spike and Phase 4 multi-language ROI receipt (§10)

**Phase-deferred (acknowledged, not Phase 1 scope):**

- Storage backend pick (redb vs rusqlite+sqlite-vec) — Phase 2 spike, trait shape already locked (D-R2)
- LLM-as-judge eval pipeline — Phase 3 prerequisite before any reranker / embedder change can be measured cleanly (see §9.4)
- Multi-language tree-sitter — Phase 4, ROI quantified at ≈17% query coverage gap (§9.2)
- memU integration mode — Phase 5

Everything else not listed above is out of scope for Phase 1.

---

## 1. System Overview

```
+----------------------------------------------------------------+
| Embedded UI  (vanilla JS + HTMX + cytoscape.js)                |
| Bundled into Go binary via //go:embed ui/                      |
+----------------------------+-----------------------------------+
                             | HTTP (chi router)
+----------------------------+-----------------------------------+
| Go service layer  (server/)                                    |
|  chi HTTP router  +  mark3labs/mcp-go (MCP stdio)              |
|  cobra CLI (index/query/serve/mcp)  +  A2A client + supervisor |
+----------------------------+-----------------------------------+
                             | A2A v0.2 over localhost HTTP
                             | POST /tasks/send + GET /tasks/{id}
                             | (optional GET /tasks/{id}/stream SSE)
                             | + GET /healthz (out-of-band)
+----------------------------+-----------------------------------+
| Rust core  (core/)  --  A2A-native agent                       |
|  axum  +  tree-sitter parser  +  candle embedder               |
|  storage (redb OR rusqlite+sqlite-vec, Phase 2)  +  gix        |
+----------------------------------------------------------------+
```

Build commands and end-user invocation live in `README.md`. The Rust core is a network-addressable A2A agent; the Go server is one A2A client among potentially many. Any A2A-compliant client (remote agent, script, other model) can call the same endpoint — no private RPC path.

---

## 2. Service Supervision (REQ-07)

### 2.1 Spawn timing — D-S1

`codenexus serve` boot sequence: Go starts → extracts embedded Rust binary to `<XDG_CACHE_HOME>/codenexus/bin/codenexus-core-<version>/` → spawns Rust with the env vars in §5.5 → awaits `/healthz` 200 OK (30s timeout) → on success, chi/MCP/CLI come up; on failure, `serve` exits with diagnostic and nothing is served. Lazy spawn modes are not supported in MVP.

### 2.2 Healthcheck — D-S2

Rust core exposes `GET /healthz` **outside** the A2A skill surface, returning `{ok, version, uptime_sec, indexed_repos}`. Polled by Go every 10s; 3 consecutive failures → core declared dead. `curl http://localhost:9876/healthz` is the canonical debug command. `/healthz` is NOT under `/tasks/*`, so an unhealthy A2A skill does not cascade to liveness probes.

**Failure semantics under D-S3 crash-loop breaker.** `/healthz` reports **Rust-process liveness only**. It is not a Go-supervisor status endpoint. When Rust is alive, `/healthz` returns `{ok: true, ...}`. When Rust is dead and Go has given up (5 restarts in 60s tripped, see §2.3), `/healthz` is **unreachable** (TCP connection refused, no port bound) — not a 503 from Rust. Go-supervisor state is reflected through Go's own surfaces: `POST /tasks/send` returns `503 Service Unavailable` after the breaker trips, and Go exits non-zero so process supervisors (systemd / Windows Service Manager / parent shell) observe the failure. External callers needing combined status should poll Go's API endpoints, not Rust's `/healthz`.

### 2.3 Restart strategy — D-S3

- Backoff: `1s → 2s → 4s → 8s → 16s`, cap `30s`.
- Crash-loop breaker: `>= 5` restarts in any 60s window → log fatal, `503` on `POST /tasks/send`, Go exits non-zero.
- Reset: 5 minutes of stable uptime clears the counter.

### 2.4 Port collision — D-S4

```
for port in 9876..=9999:
    if can_bind(port):
        write_lockfile("~/.codenexus/port", {pid, port})
        spawn rust with CODENEXUS_PORT=port; break
else:
    fatal("all ports 9876..9999 in use")
```

Lockfile schema: `{"pid": 12345, "port": 9876, "started_at_unix": 1745700000}`. Stale-PID handling: on startup, if the lockfile exists Go verifies the PID is alive and is a `codenexus-core` process; otherwise the lockfile is unlinked and the scan re-runs. Preserves the "single fat-binary, zero install" promise even when 9876 is occupied.

---

## 3. A2A Schema (REQ-06)

Compliant with Google A2A v0.2 (`https://google.github.io/A2A/`).

### 3.1 Skill granularity — D-A1

**One** A2A skill: `code-graph`. The `operation` field inside the message data part discriminates among the four operations.

```json
{
  "skills": [
    {
      "id": "code-graph",
      "name": "Code Graph",
      "description": "Index, query, and navigate code as a symbol graph.",
      "operations": ["index_repo", "query", "get_symbol", "list_callers"]
    }
  ]
}
```

Reasoning: A2A skills are coarse capabilities, not RPC methods. One agent card, one health surface, one set of permissions.

### 3.2 Streaming — D-A2

Hybrid: polling default, SSE upgrade.

| Client preference | Server behavior |
|---|---|
| Default (no `Accept` header for SSE) | `POST /tasks/send` returns `task_id`; client polls `GET /tasks/{id}` every N seconds. |
| `Accept: text/event-stream` | Server upgrades to `GET /tasks/{id}/stream`, emits progress events as SSE. |

Both code paths share task state. A2A v0.2 leaves SSE optional; polling guarantees any A2A v0.2 client interoperates.

### 3.3 Error envelope — D-A3

On `state: failed`, `messages[].parts[]` contains BOTH a `text` part (A2A-portable, human-readable) AND a `data` part (machine-readable):

```json
{
  "state": "failed",
  "messages": [{
    "parts": [
      {"type": "text", "text": "Repository path /foo/bar does not exist."},
      {"type": "data", "data": {
        "code": "REPO_NOT_FOUND",
        "retryable": false,
        "details": {"path": "/foo/bar"}
      }}
    ]
  }]
}
```

### 3.4 Query result shape — D-A4

`query` returns rich items with all four meta scores. The 60% precision target requires ablation/debug throughout MVP, so meta scores are not optional.

```json
{
  "symbol_id": "...",
  "kind": "Function",
  "name": "walkSubtree",
  "path": "core/src/walk.ts",
  "range": {"start_line": 42, "end_line": 88},
  "parent": "WalkerService",
  "snippet": "function walkSubtree(node: Node, ...) { ... }",
  "bm25_score": 4.81,
  "vector_score": 0.732,
  "rrf_score": 0.0312,
  "final_score": 0.0312
}
```

`final_score` is whatever the configured fusion strategy outputs (see §9). `rrf_score` is the raw RRF value before any post-fusion reranking.

### 3.5 Operation envelopes (JSON examples)

All four operations share the same A2A wrapper. The full request envelope is shown once for `index_repo`; subsequent operations show only the `data` part (the discriminator and operation-specific args).

#### 3.5.1 `index_repo` — full envelope shown

**Request:**

```json
POST /tasks/send
{
  "task_id": "01HZ8K9N7Q2RVB3XPM4F5G6H7J",
  "skill_id": "code-graph",
  "messages": [{
    "role": "user",
    "parts": [{"type": "data", "data": {
      "operation": "index_repo",
      "repo_path": "/abs/path/to/repo",
      "incremental": true
    }}]
  }]
}
```

**Success response (data part of agent message after polling):**

```json
{"operation": "index_repo", "repo_hash": "ab12cd34",
 "files_indexed": 412, "symbols_indexed": 7894,
 "duration_ms": 102345, "last_indexed_commit": "5fa3e2b1c..."}
```

**Failure response (full envelope, illustrating D-A3 composed error):**

```json
{
  "task_id": "01HZ8K9N7Q2RVB3XPM4F5G6H7J",
  "state": "failed",
  "messages": [{
    "role": "agent",
    "parts": [
      {"type": "text", "text": "Tree-sitter parse failed for src/foo.ts at line 42."},
      {"type": "data", "data": {
        "code": "PARSE_ERROR", "retryable": false,
        "details": {"file": "src/foo.ts", "line": 42}
      }}
    ]
  }]
}
```

#### 3.5.2 `query`

**Request data part:**

```json
{"operation": "query", "repo_hash": "ab12cd34",
 "q": "where is rate limiting middleware", "k": 5}
```

**Success data part:**

```json
{"operation": "query", "results": [
  {"symbol_id": "...", "kind": "Function", "name": "rateLimit",
   "path": "src/middleware/rate.ts",
   "range": {"start_line": 12, "end_line": 40},
   "parent": null, "snippet": "...",
   "bm25_score": 6.22, "vector_score": 0.81,
   "rrf_score": 0.0421, "final_score": 0.0421}
]}
```

**Failure codes:** `REPO_NOT_INDEXED`, `INVALID_QUERY`, `INTERNAL`. Envelope shape per §3.5.1 failure example.

#### 3.5.3 `get_symbol`

**Request data part:**

```json
{"operation": "get_symbol", "repo_hash": "ab12cd34", "symbol_id": "sym_abc123"}
```

**Success data part:**

```json
{"operation": "get_symbol", "symbol": {
  "symbol_id": "sym_abc123", "kind": "Class", "name": "WalkerService",
  "path": "src/walker.ts", "range": {"start_line": 1, "end_line": 200},
  "parent": null, "snippet": "...",
  "children": ["sym_def456", "sym_ghi789"], "imports": ["sym_xyz000"]
}}
```

**Failure code:** `SYMBOL_NOT_FOUND`, `retryable: false`.

#### 3.5.4 `list_callers`

**Request data part:**

```json
{"operation": "list_callers", "repo_hash": "ab12cd34",
 "symbol_id": "sym_abc123", "depth": 1}
```

**Success data part:**

```json
{"operation": "list_callers", "callers": [
  {"symbol_id": "sym_caller1", "name": "main", "path": "src/main.ts", "edge_kind": "CALLS", "confidence": 1.0},
  {"symbol_id": "sym_caller2", "name": "init", "path": "src/init.ts", "edge_kind": "CALLS", "confidence": 0.9}
]}
```

`confidence` is the **highest** edge confidence observed on any Calls edge from this caller to the queried target (see §9.7 for resolver step → confidence mapping; default filter `confidence_min = 0.5`). When multiple Calls edges exist between the same pair (different resolver passes), we surface the maximum so consumers see the strongest evidence — never silently overwrite. Phase 4 Leiden community detection reuses this directly as edge weight, so the field is forward-compatible at zero added cost.

**Failure codes:** `SYMBOL_NOT_FOUND` or `RESOLVER_PARTIAL` (latter retryable: false, partial results in `details.partial_callers`).

---

## 4. Clean-Room Policy

### 4.1 Hybrid enforcement — D-C1

Two layers: (1) **written rule** (this section is canon); (2) **pre-commit hook** that refuses commits to `core/**` if the local GitNexus repo was accessed within the last 24 hours. The hook operates on the user's local GitNexus checkout (e.g. `~/code/gitnexus`) by inspecting an "access timestamp" — implementation gets nuanced on Windows.

**Implementation mechanism (Windows NTFS).** NTFS disables last-access tracking by default for performance (`fsutil behavior query DisableLastAccess` typically returns `1` or `2`). On a default Windows install, `stat()` last-access is frozen, so a naive `atime`-based hook silently always passes. **Decided (Phase 1 close, 2026-04-27): option 2 — FETCH_HEAD mtime as primary signal, option 3 as fallback.**

The pre-commit hook reads `<gitnexus_local_checkout>/.git/FETCH_HEAD` mtime (set by every `git fetch` / `git pull`); if the mtime is within `now() - 24h`, the hook refuses commits to `core/**`. This is robust across NTFS / APFS / ext4 (no `DisableLastAccess` quirk) and captures the operationally-relevant signal: "did you recently sync GitNexus?" rather than the weaker "did you read its files?". Browsing a stale checkout without fetching does not trip the hook — for those sessions, the stricter manual state-stamp fallback applies: `clean-room-stamp.sh` writes a timestamp to `~/.codenexus/clean-room-state` at the end of each GitNexus reading session, and the hook checks `max(FETCH_HEAD_mtime, clean-room-state_mtime)`.

Rejected: option 1 (forcing user to enable NTFS last-access via `fsutil behavior set DisableLastAccess 0`) — pollutes the user's filesystem performance globally for one tool's convenience.

Override with `--clean-room-override <reason>`; reason is appended to `.codenexus/clean-room.log`.

### 4.2 Stricter rule for AI-driven coding — D-C2

AI agents working on CodeNexus core MUST NOT have GitNexus source in their context window at all. **No 24-hour grace period applies to AI sessions** — AI "memory" is the conversation context, not the calendar; the 24h gap concept does not translate. Mechanism: agent prompt templates for CodeNexus work include an explicit forbid clause naming GitNexus repo paths and refusing tool calls that read files under them. Only ARCHITECTURE.md and design notes (this document, `origin-spec.md`, `.planning/**`) are allowed reference material.

### 4.3 License boundary — D-C3

Hard allowlist/denylist. Legal review reads this table only.

| Project | License | CodeNexus posture | Mechanics |
|---|---|---|---|
| `braedonsaunders/codeflow` | MIT | **Port-allowed** | Per-file header comment naming upstream file + commit hash. Each port appends an entry to `NOTICE`. Apache 2.0 covers the merged result (MIT → Apache 2.0 is a permitted upgrade with attribution). |
| `gitnexus` | PolyForm Noncommercial 1.0.0 | **Study-only, never ported** | 24h gap rule (D-C1) + AI stricter rule (D-C2). Concepts may be re-derived; no code, no comments, no doc strings, no symbol names lifted. |

PolyForm Noncommercial forbids sublicensing; copying any code propagates non-OSS terms into CodeNexus and would void Apache 2.0. The boundary is bright-line.

### 4.4 Violation procedure — D-C4

If GitNexus source was open while editing `core/**`:

1. Append a dated entry to `.codenexus/clean-room.log` describing what was open + which files were edited.
2. Mark affected files as `tainted` in the same log entry.
3. Refactor in a fresh clean-room session **at least 24h later**; commit message must include `clean-room-refactor: <log-entry-id>`.
4. The pre-commit hook checks `clean-room.log` and refuses commits to tainted files until the refactor commit lands.

---

## 5. State Ownership

### 5.1 Graph DB path — D-B1

| Aspect | Value |
|---|---|
| Location | `<XDG_DATA_HOME>/codenexus/<repo-hash>/db` |
| Linux default | `~/.local/share/codenexus/<repo-hash>/db` |
| Windows default | `%LOCALAPPDATA%\codenexus\<repo-hash>\db` |
| macOS default | `~/Library/Application Support/codenexus/<repo-hash>/db` |
| Owner | Rust core |
| Crate | `dirs` for cross-platform XDG resolution |
| Per-repo isolation | `<repo-hash> = sha256(canonical_repo_path)[..16]` |

### 5.2 User config — D-B2

Git-style precedence. Owner: Go (Go is the user-facing entry).

| Layer | Path | Precedence |
|---|---|---|
| Global | `<XDG_CONFIG_HOME>/codenexus/config.toml` | lowest |
| Per-repo | `<repo-root>/.codenexus/config.toml` | middle |
| CLI flags | `--port 9876 --batch-size 64` | highest |

Config library: `koanf` (Go-native, mature). `figment` is the Rust analogue but Rust core does not currently expose user-tunable config beyond what the Go server passes via env vars (D-B-extras).

### 5.3 Embedder model cache — D-B3

`<HF_HOME>/hub/`, default `~/.cache/huggingface/hub/`. Shared with the broader HuggingFace ecosystem (`huggingface_hub`, `transformers`, `ollama`, `hf-cli`) so model weights are not duplicated. Override via `HF_HOME` env var.

Bundle-in-binary remains a Phase 2 spike question (PROJECT.md). If selected, binary size grows ~80–120MB.

### 5.4 trace_id propagation — D-B4

- **Origination:** Go-side, at request entry, as UUIDv7.
- **Propagation:** Go places the UUIDv7 in the A2A `task_id` field of `POST /tasks/send`.
- **Logging:** Both Go and Rust emit `trace_id` in every log line (see §6).
- **Correlation:** `jq '.trace_id == "<uuid>"' logs.json` joins both streams.

UUIDv7 chosen over UUIDv4 because the timestamp prefix makes log ordering and time-bucketed aggregation cheap.

### 5.5 Rust spawn env vars (D-B-extras)

| Variable | Purpose |
|---|---|
| `CODENEXUS_PORT` | Port chosen by Go's auto-find (D-S4) |
| `CODENEXUS_PORT_LOCKFILE` | Path to `~/.codenexus/port` |
| `CODENEXUS_DATA_DIR` | Resolved `<XDG_DATA_HOME>/codenexus/` |
| `CODENEXUS_DEVICE` | `auto` / `cpu` / `cuda` / `metal` → `probe_device()` |
| `RUST_LOG` | `tracing-subscriber` filter (e.g. `info,codenexus_core=trace`) |
| `HF_HOME` | Inherited if user-set, else default |

Rust binary extraction dir: `<XDG_CACHE_HOME>/codenexus/bin/codenexus-core-<version>/`. Persists across runs; owned by Go (Go is the `//go:embed`-er).

---

## 6. Logging

D-W4. Structured JSON on both sides; correlation via `trace_id`.

| Side | Library | Format | Required fields |
|---|---|---|---|
| Rust core | `tracing` + `tracing-subscriber` | JSON via `fmt().json()`, `RUST_LOG` filter | `timestamp`, `level`, `target`, `trace_id`, `message` |
| Go server | stdlib `log/slog` (1.21+) | `slog.NewJSONHandler(os.Stdout, ...)` | `time`, `level`, `source`, `trace_id`, `msg` |

`trace_id` originates in Go (D-B4) and is placed in the A2A `task_id` of the request. Rust extracts `task_id` and threads it through `tracing` spans for the lifetime of the task. Join both streams via:

```bash
jq -s 'flatten | sort_by(.timestamp // .time) | .[] | select(.trace_id == "01HZ...")' \
  go-server.log rust-core.log
```

---

## 7. Embedder Device Abstraction

D-W5. User-authored specification, captured near-verbatim.

### 7.1 Device selection

Resolved **once at startup** via `probe_device()`:

```
fn probe_device() -> Arc<Device>:
    if cuda_available() && feature("cuda")  -> Device::cuda(0)
    elif metal_available() && feature("metal") -> Device::new_metal()
    else                                      -> Device::Cpu
```

The resulting `Arc<Device>` is stored in `EmbedderConfig` and shared with both:

- `VarBuilder` (model weight loading)
- The single embedder worker

All `candle_core` tensor operations are device-transparent. **No code path branches on device type after initialization** — the architecture's invariant.

### 7.2 Batch sizes by device class

| Device class | Default batch size |
|---|---|
| CPU | 32 |
| Metal | 128 |
| CUDA | 256 |

User-overridable via `IndexConfig.embedder.batch_size` (per-repo `.codenexus/config.toml` or CLI flag `--batch-size N`).

### 7.3 Worker pool topology

```
+----------------------+
| rayon parallel       |   CPU-bound, scales to N cores
| parser (tree-sitter) |
+----------+-----------+
           |
           v  bounded mpsc channel, capacity 256 symbols
+----------+-----------+
| Single Device-aware  |   tokio::select! { batch_full | flush_50ms }
| embedder worker      |
+----------+-----------+
           |
           v  per-1000-symbols batched
+----------+-----------+
| Storage write +      |   batch_commit + checkpoint
| checkpoint           |
+----------------------+
```

Tail-latency control: `tokio::select!` with a **50 ms flush timeout** ensures sparse input does not stall in a half-full batch.

### 7.4 Capacity envelope

500k LOC repo → ~50k symbols. CPU baseline: ~50 sym/sec → ~17 min total. CUDA significantly faster with batched flush; Metal and CUDA exact numbers measured in Phase 2 spike against the chosen embedder model.

---

## 8. CI/CD GPU Policy

D-W6. Default-CPU, GPU opt-in. `cargo build --no-default-features` is CPU-only and compiles + tests cleanly on bare CI workers. CUDA support behind `--features cuda`, Metal behind `--features metal`; neither is in `default`. GPU integration tests live in `core/tests/gpu/` and run only on dedicated runners with the appropriate toolkit (CUDA 12.x / Xcode + Metal SDK). `make ci-cpu` and `make ci-gpu` are the two CI entry points; commodity contributors and CI workers run only `ci-cpu`.

---

## 9. Retrieval Architecture

### 9.1 Validated Configuration (R3 Plateau)

This is the empirically-validated POC configuration as of 2026-04-27. Phase 3 MVP inherits these values verbatim unless a measured regression argues otherwise.

| Parameter | Value | Source |
|-----------|-------|--------|
| `retrieval.fusion_alpha` | `0.6` (vector weight; BM25 weight = `1 − α`) | R3 alpha sweep, `eval/round_3_results.md` |
| `retrieval.fusion_c` | `60` (RRF rank smoothing constant) | RRF literature default |
| `retrieval.bm25_weights` | `[name:10, snippet:1, kind:1, search_blob:5]` | R3 column-weight tuning |
| `retrieval.tokenizer` | SQLite FTS5 `unicode61` + Rust-side `decompose()` for camelCase | R2 BM25 SQL probe |
| `retrieval.search_blob` | `decompose(name) + " " + decompose(snippet)` per symbol | R3 |
| `retrieval.embedder.model` | ollama `qwen3-embedding:0.6b` (1024d) for POC; candle-loaded for Phase 3 per D-W5 | R2 |
| `retrieval.embedder.query_prefix` | `"Instruct: Given a natural language code search query, retrieve the most relevant code symbol from a TypeScript codebase\nQuery: "` | R2 — qwen3-embedding is instruction-tuned |
| `retrieval.embedder.passage_prefix` | empty | R2 — passage side raw |
| `retrieval.negative_rrf_threshold` | `0.012` (≈73% of alpha-weighted RRF max `1/61`) | R3 |
| `retrieval.candidate_pool` | top-50 from each of BM25 and vector before fusion | implementation default |

**Measured precision** (n=30 over obsidian-llm-wiki, 2116 symbols): Axis-1 70% / Axis-2 47.5% / Axis-3 ≈0% (ablation-confirmed retrieval-without-graph behavior).

Implementation: `experiments/poc-retrieval/src/{search.rs,storage.rs,embedder.rs,parser.rs}` (~400 LOC total).

### 9.2 Design Contracts (locked, do not relitigate)

- **RRF is parameterized, not constant.** Equal-weight RRF (α=0.5) is rejected as default — POC data shows vector path materially stronger on semantic queries; equal weighting dilutes the better signal. See `eval/round_3_results.md` alpha sweep.
- **BM25 requires camelCase decomposition.** FTS5 unicode61 splits on `_` but NOT camelCase. `walkSubtree` indexes as one token; query "subtree" cannot find it. The `search_blob` column populated with `decompose(name + snippet)` is **mandatory for any FTS5-based BM25 path**. Reference implementation in `search.rs:decompose()` with unit tests.
- **Storage trait `list_symbols_by_file` is mandatory** (D-R2 lock). Without it, `last_indexed_commit` anchor is meaningless — incremental updates cannot find old symbols to delete. Trait shape stays as PROJECT.md D-R2.
- **Single-language MVP cost is ≈17% empirically.** Round 1+2 query sets show 5/30 queries map to Python files (concept_graph.py, kb_meta.py) that TS-only POC cannot answer. Phase 4 multi-language ROI has its receipt. Do not pretend the POC plateau is the multi-lang ceiling.

### 9.3 Known Limits

- **Axis-2 gap to 60% target: 12.5pp** — not addressable by BM25 / embedding tuning alone within Path A. Round 4 demonstrated this empirically: Path A peaked at 47.5% across all alpha values 0.5-0.8. Further lift requires either (a) cross-encoder reranking with a different eval methodology, or (b) better embedder (e.g. jina-embeddings-v5-text-small with task adapter), or (c) corpus-side improvements (more meaningful snippet extraction, better symbol granularity).
- **Axis-3 ≈ 0% is structural, not a bug.** Call-graph queries ("who calls X", "what does X return", "X implements which interface") require CALLS edges and graph traversal — not retrieval. REQ-02 CALLS edge graph is the **necessary** complement, validated by Round 3's flat 30% (most of which was substring-matching noise, not real call-graph success).
- **Path B (cross-encoder reranker) showed eval infrastructure insufficient to measure lift.** Round 4 with Jina rerank-v2 produced moderate-quality reranking on smoke tests (B2 PROTECTED_DIRS at 0.43, A2/A3/A4 stable) but headline numbers regressed because: (a) reranker prefers verbose surface matches over canonical implementations, (b) "fuzzy negative" queries with conceptual neighbors get correctly-moderate scores that eval misclassifies, (c) `expected_paths` ground truth was authored against R3's specific picks and doesn't generalize. See `eval/round_4_results.md` and `eval/EVAL_DESIGN_NOTES.md`.

### 9.4 Phase 3 Gate

**Cross-encoder reranker MUST NOT be introduced in Phase 3 MVP until LLM-as-judge eval pipeline exists.** Hand-annotated `expected_paths` is not sustainable past 30 queries × 1 truth-per-query.

**Phase 3 prerequisite**: NDCG@5 with graded relevance (0–3) over a query set ≥ 100 queries × ≥ 2 corpora. Implementation options:
- LLM-judge: send `(query, candidate)` pairs to a strong LLM (Claude/GPT-4) for 0-3 grading, cache results. ~30 minutes per eval cycle, ~$1 per cycle.
- Human spot-check: sample 10% of LLM-judge labels for inter-rater agreement audit.

Until this exists, retrieval changes that involve reranking, embedder swaps, or corpus reorganization **cannot be measured cleanly** — and the temptation to overfit to the 30-query dev set is high. See `eval/EVAL_DESIGN_NOTES.md` "Known Eval Limitations" for the full argument.

**LLM-judge throughput sizing (2026-04-27 probe).** Two judge endpoints are wired: 官方 `api.minimaxi.com/anthropic` (Bearer, default model `MiniMax-M2.7-highspeed` per mmx-cli; endpoint also serves M2.7 / M2 / M2-highspeed / M2.5) and the okaoi 3-key pool (Anthropic-shape relay, MiniMax-M2.7). Token-bucket parameters measured empirically in `eval/probe_minimax_concurrency_findings.md` — 官方 = capacity 80, refill 0.5/s (= 30 RPM steady, but 80-call burst headroom from cold). Total calls in T seconds = `80 + 0.5T`. Bucket is **account-bound, not model-bound** — multi-account round-robin via `~/claude-minimax-official.ps1` is the documented capacity-scaling path. Sizing for 600-call eval cycle: 官方 = 17 min wall-clock, okaoi = 2.6 min — **both fit the §9.4 30-min budget for 1-seed × 600-call runs**. Recommended posture: okaoi for inner-loop iteration; 官方 (M2.7-highspeed) for gate-flipping final eval at 1 seed; multi-seed runs require either §9.4 budget revision to 60 min, multi-account 官方 round-robin, or okaoi-vs-官方 grader cross-validation (30-call sample) to license the relay. Cross-val is queued, not yet run.

### 9.5 Reranker code stays opt-in

`experiments/poc-retrieval/src/reranker.rs` and the `--rerank` CLI flag remain in the POC for future re-activation. Reranker uses `JINA_API_KEY` from environment (never hardcoded, per security feedback rule #35). When LLM-judge eval is in place (Phase 3+), Path B can be re-evaluated with confidence.

**Phase 3 reranker candidates** (in priority order, do not pick blindly — measure all under Rule 6 LLM-judge eval):

1. **Qwen3-Reranker-4B** *(primary candidate)* — instruction-tuned cross-encoder, Apache 2.0 license (commercial-OK). Accepts custom task instruction via `prompts={"custom": "..."}` parameter (sentence-transformers `CrossEncoder` API), e.g. *"This is code symbol retrieval. Prioritize function definitions over comment/docstring matches."* Strong fit for code-retrieval queries, where R4 jina-reranker-v2 verbose-bias (Cause A) suggests instruction-tuned would behave better. **Test first.**
   - **Lineup**: Qwen3-Reranker-0.6B / 4B / 8B all available (`Qwen/Qwen3-Reranker-{0.6B,4B,8B}` on HF)
   - **Footprint**: Full FP16 ~8GB, Q4 quantized ~2-3GB (49 quantization variants on HF as of 2026-04)
   - **Inference compat**: transformers ≥ 4.51.0 ✓, vLLM ≥ 0.8.5 ✓, **no native Candle (Rust) binding** — Phase 3 deployment options: (a) Python sidecar service over HTTP, (b) ONNX Runtime via `ort` Rust crate (per §10.2 D-W7), (c) wait for community Candle port. Option (b) preferred for fat-binary distribution (REQ-08).
   - **Output shape**: cross-encoder logit difference, sigmoid → 0-1 probability per (query, doc) pair
2. **jina-reranker-v2-base-multilingual** — R4 baseline, retained as control. Free tier quota viable for eval batches; verbose-bias documented (R4 Cause A).
3. **bge-reranker-v2-m3** — fallback if both above lose.

Selection lands in Phase 3 as a measured pick, not pre-decided here. R4 reranker.rs scaffolding is generic over endpoint — only the request shape and model header change per candidate.

### 9.7 Symbol graph traversal (REQ-02 implementation spec)

REQ-02 is locked at requirement level with **4 edge kinds** (`.planning/REQUIREMENTS.md` REQ-02, scope expanded 2026-04-27): `Calls`, `Imports`, `Implements`, `Extends`. `Overrides` is deferred to Phase 3+. Traversal parameters copied from GitNexus' production tuning (proven on multi-million-LOC repos), generalized to per-edge-kind:

| Parameter | Value | Applies to | Source / rationale |
|-----------|-------|-----------|---------------------|
| `graph.confidence_min` | `0.5` | all kinds | GitNexus default — filters dynamic-dispatch / unresolved-import edges with low static-analysis confidence |
| `graph.bfs_depth_limit` | `10` | all kinds | GitNexus default — covers typical query horizon ("who calls X 5 hops away"), prevents pathological full-graph walks |
| `graph.bfs_branching_limit` | `4` per node | all kinds | GitNexus default — caps fan-out at popular call-sites (`register()`, `log()`) and at root classes; first 4 per node is empirically high-signal slice |
| `graph.terminal_kinds` | `[file_root, exported_api, test_function]` | all kinds | stops at semantically-meaningful boundaries |
| `graph.cycle_detection` | visited-set per traversal | all kinds | prevents recursive-call / cyclic-inheritance infinite loops |
| `graph.allowed_edge_kinds` | per-query subset | query-specific | e.g. `list_callers` uses `[Calls]`, `subclasses_of` uses `[Extends]`, `impact_of` uses `[Calls, Extends, Implements]` |

Algorithm: BFS from entry symbol → follow edges where `confidence ≥ 0.5` AND `kind ∈ allowed_edge_kinds` → at each node take first 4 outgoing edges per allowed kind → halt at `bfs_depth_limit` or terminal kind.

**Per-query default `allowed_edge_kinds`**:
- `list_callers(X)` → `[Calls]` reverse direction
- `what_X_calls` / `transitive_callees(X)` → `[Calls]` forward
- `subclasses_of(X)` / `subinterfaces_of(X)` → `[Extends]` reverse
- `implementations_of(I)` → `[Implements]` reverse
- `impact_of(X)` → `[Calls, Extends, Implements]` reverse (find everything that depends on X)
- `cross_file_resolution(symbol)` → `[Imports]` (path-finding from current file)

**Resolver (naive 3-step, locked)** — used during edge construction at index time:
1. **Same-file lookup** — exact name match within the current file's symbol set
2. **Import-file lookup** — follow Import edge from current file, exact name match in target file
3. **Global unique-name lookup** — exact name match across all symbols, only if exactly one global match exists

No alias resolution, no re-export chain follow, no barrel-file (`index.ts`) expansion. **Known limitation**: TS projects with heavy barrel-file re-exports (e.g. `obsidian-llm-wiki` corpus) will under-resolve Imports edges → propagate to under-resolved cross-file Calls. Accept noise for MVP; Phase 3+ followup is `import_alias_resolver` enhancement (estimate +200 LOC, decoupled from MVP).

**Why these specific numbers**: GitNexus tuned them across multi-million-LOC repos and surfaced them in their docs. Adopting their default avoids re-tuning from scratch and lets us focus Phase 2/3 measurement on CodeNexus-specific deviations. Re-tune only when an axis-3 query empirically fails on a CodeNexus corpus that GitNexus' params can't handle.

### 9.8 Embedder version lock (D-W8)

The embedder model is **pinned in config**. Changing the model requires a **full re-index** — partial migration is forbidden, no incremental embedding swap.

| Field | Behavior |
|-------|----------|
| `retrieval.embedder.model` | Pinned in `config.toml`; loaded at startup |
| `retrieval.embedder.version_hash` | SHA-256 of `(model_id, dim, prefix_strings)` stored in index metadata at build time |
| Drift detection | At query time, compare config's `version_hash` against index's stored hash. Mismatch → refuse to serve, log explicit error, require full reindex |
| Re-index trigger | Any change to model_id, dimensionality, or instruction prefix → full corpus reindex (~10 min for 2000 symbols, scales linearly) |

**Rationale**: embedding spaces are not interpolable across models — vectors from `qwen3-embedding:0.6b` (1024d) and `jina-embeddings-v5-text-small` (768d) cannot be mixed in fusion, even after dimensionality projection. Phase 3 jina-v5 swap (per §9.3 Known Limits option (b)) WILL require full reindex. The version-hash gate makes this explicit instead of silently producing nonsense scores.

Reference: this is a documented failure mode in production RAG systems (`embedding version drift` posts on r/MachineLearning, summer 2025). Catching it at startup beats debugging mysterious recall regressions later.

**Active hash.** As of Phase 03.6 close, current locked config:

| Date | model_id | dim | prefix | version_hash (12-char SHA-256 prefix) | commit | notes |
|------|----------|-----|--------|---------------------------------------|--------|-------|
| 2026-04-28 | `Qwen/Qwen3-Embedding-0.6B` (safetensors via fastembed-rs/candle) | 1024 | `QUERY_INSTRUCT` (incl. trailing space, byte-preserved from prior poc.db) | `f2b47aa16b17` | `67320ec` | Replaces ollama HTTP qwen3-embedding:0.6b. Prefix retains trailing space "Query: " — empirical config that produced poc.db's 67.9% B1-B7 baseline; preserved byte-identically in Plan 1 to avoid invalidating the baseline. `compute_version_hash` bin (Plan 1 Task 2.5) is the single source of truth — recompiles pick up any prefix change automatically. |
| 2026-04-28 | `Qwen/Qwen3-Embedding-0.6B` (safetensors via fastembed-rs/candle) | 1024 | `QUERY_INSTRUCT` (incl. trailing space, byte-preserved from prior poc.db) | `f2b47aa16b17` | (Phase 4 first slice) | HF revision pinned to `97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3` (the SHA whose model produced 03.6's 67.9% B1-B7 baseline; recovered from local cache `refs/main` 2026-04-26 14:48 timestamp). Pin enforced via `embedder.rs:QWEN3_REVISION` const + `hf-hub::Repo::with_revision` snapshot fetch + `Qwen3TextEmbedding::new` direct construction (bypassing `from_hf` which re-fetches `config.json` from default `main`). Same model bytes, same dim, same prefix as the previous row — audit-relevant, not embedding-version-hash-changing; this row records the supply-chain control upgrade. |

### 9.9 Embedder resilience layer ownership (D-W9, Phase 3.5b)

**Locked design contract** — applies to any future refactor of the embedder retry / abort / counter primitives. Captures the rationale behind the Phase 3.5b split so Phase 4 unification doesn't relitigate it.

**Layer ownership table:**

| Concern | Layer | Reason |
|---------|-------|--------|
| Single-call HTTP/RPC retry on transient failure | **Embedder** (`embed()` wraps `embed_once()`) | All callers benefit identically; transient = ollama queue overflow, GPU pressure, network blip. Stateless — embedder doesn't need to know which caller is calling. |
| Error class discrimination (`Transient` / `Permanent` / `Timeout`) | **Embedder** (Phase 4 P2 task; Phase 3.5b uses anyhow blanket) | The classification IS the embedder's job — only it knows whether an error is retry-eligible. Caller policy varies, but classification is canonical. |
| Per-error retry policy decision | **Caller** | Query path returns 503 on `Timeout` immediately (UX); Index path swallows `Timeout` into `consecutive_fails`; A2A Server path returns 503 on `consecutive_fails` threshold (different from CLI Index which exits process). Same primitive, three policies. |
| Consecutive-failure counter | **Caller's loop body**, never embedder | "N consecutive failures" is loop semantics, not call semantics. Embedder is stateless — each `embed()` is independent of every other. Pushing the counter into embedder would (a) violate single-responsibility (embedder gets aware of caller's iteration pattern), (b) break Query path (one-shot, has no "consecutive" notion), (c) couple unrelated callers (Index N=5, BatchEval N=20, Server N=3 would all share embedder state, requiring per-caller config indirection). |
| Abort action (exit / 503 / partial-write) | **Caller** | Each loop has different cleanup needs: CLI Index exits process with anyhow::bail, A2A Server returns HTTP 503 + structured error body, future BatchEval writes `partial_results.json` for resumability. Cannot be unified at embedder layer. |

**Phase 3.5b explicit non-decisions** (do NOT treat as locked):
- Blanket 5-attempt retry on every error type → wrong-but-cheap; Phase 4 splits by error class.
- CLI flag `--max-consecutive-fail` → migrate to `config.toml` `[embedder.resilience]` table once unified.
- Single counter in `main.rs:155` → duplicate-not-extract when Phase 4 lands the A2A Server fix; the duplication is correct because the abort actions diverge.

**Anti-pattern (block at review):** any PR that moves `consecutive_fails` into `Embedder` struct state, or makes `embed()` aware of "this is the Nth call in a sequence." The right shape stays: stateless embedder + caller-owned loop counters + classified errors flowing through.

Provenance: Phase 3.5b micro-slice (`260427-e7r`), Curry review 2026-04-27.

### 9.10 Embedder Runtime: Candle Migration (Phase 03.6 — LANDED)

**Decision trigger.** Phase 3.5b retry probe (commit `8f4da66`) confirmed ollama burst-failure mode is unrecoverable at the call layer: per-call 60s reqwest send-timeout × 5 attempts = 5 min/symbol, total ~20 min wall-clock per fail-cluster with zero recovery. Throttle and HTTP keep-alive workarounds explicitly considered and rejected as symptom-treatment; candle in-process inference is the architecturally locked direction since §9.1 D-W5.

**Locked path (Phase 03.6).** Replace `embedder.rs` `embed_once()` HTTP call with in-process safetensors inference via `fastembed::Qwen3TextEmbedding` (which wraps `candle-transformers` under the hood; direct `candle-transformers::models::qwen3::Model` held in reserve as fallback but not needed — fastembed exposed the full surface required for our use case). Stack: `candle-core` 0.10 / `candle-nn` 0.10 / `candle-transformers` 0.10 + `hf-hub` 0.5 + `tokenizers` 0.22 + `fastembed` 5.13 (qwen3 feature, Apache-2.0), with `Qwen/Qwen3-Embedding-0.6B` from HF Hub. Last-token pool + L2 normalize → 1024-dim vectors. F32 weights for the equivalence milestone; F16 deferred to Phase 4+ as a measured optimization. Retry wrapper from §9.9 stays as defensive-only — network is gone, so only OOM / model-load-failure / dtype-shape transients remain.

**Negative rationale (do not relitigate).** The previously-locked GGUF cheap path (`llama.cpp/convert_hf_to_gguf.py` → `candle-transformers` `quantized::llama` loader) was inverted by Phase 03.6 research:

- `candle-transformers/src/models/quantized_qwen3.rs::ModelWeights::forward()` returns vocab logits via `lm_head` projection, NOT hidden states required for last-token pooling. Source: candle source via WebFetch, Phase 03.6 RESEARCH.md §"Summary" finding #1.
- The GGUF-tokenizer is upstream-flagged broken with ~10× slower inference vs ONNX (HF `Qwen/Qwen3-Embedding-0.6B-GGUF` discussion #8).

The expensive-fallback framing in PROJECT.md ("would require hand-writing forward pass + BPE tokenizer") was wrong: `tokenizers` crate + `candle-transformers::models::qwen3::Model` together solve both, and `fastembed-rs` 5.13 packages exactly that path behind a single `Qwen3TextEmbedding` constructor. Net cost equivalent to the GGUF path if it had worked; we adopted fastembed's wrapper rather than writing our own glue.

**Anchor discipline.** Phase 4 PLAN's first sub-task locks this section number. All candle-related decisions — weight format pick (safetensors via fastembed locked), tokenizer source, version-hash refresh policy, regression-test methodology, optional ONNX fallback per §10.2 — hang here. Do not split into §9.11 / §9.12 until this anchor exceeds ~5 locked sub-decisions (Kolmogorov: collapse early, expand only when redundancy is observed).

Provenance: Phase 3.5b SUMMARY § "Decision triggered" (commit `8f4da66`), Phase 03.6 RESEARCH.md (2026-04-27), Phase 03.6 SUMMARY (2026-04-28), Curry review 2026-04-28.

### 9.6 Pending storage backend pick (Phase 2 Spike)

The storage trait shape is locked (D-R2). The implementation choice between `redb` (pure KV) and `rusqlite + sqlite-vec` (SQL + vector + FTS5 in one file) is a Phase 2 Spike output, not a Phase 1 decision. POC currently uses rusqlite + FTS5 + Rust-side cosine (no sqlite-vec extension); Phase 2 bench will determine the production pick. The trait abstraction allows either to land without re-architecting downstream code.

---

## 10. Future / Deferred

### 10.1 Workspace split triggers — D-R1

CodeNexus core is a single Cargo crate for MVP. ANY ONE of the following triggers a workspace split; if NONE → stay single crate.

| # | Trigger | Measurement |
|---|---|---|
| 1 | Clean `cargo build` exceeds 90s on dev machine | `cargo build --timings` |
| 2 | Transitive crate dep count exceeds 80 in `core/` | `cargo tree \| wc -l` |
| 3 | Need separate sub-binaries (e.g. `codenexus-indexer` standalone) | requirements-driven |
| 4 | Phase 4 multi-language puts each tree-sitter behind a feature flag | feature-gating cleanliness |

Review checkpoint: at Phase 4 entry, run measurements 1 and 2 and write results to that phase's summary. Owner: phase planner.

**Trigger #4 fired (2026-04-29, Phase 04.5-01).** Sentrux adaptation lift requires three sub-crates (`core` / `metrics` / `parser`) over ~8000 LoC of liftable MIT-licensed material. Inline-match multi-language pattern from 04-09-SPEC was superseded by sentrux's data-driven `lang_extractors` framework (configs over match arms), and that framework requires its own sub-crate boundary to avoid cross-pollinating with `core`. Workspace promotion landed in slice 04.5-01: `experiments/poc-retrieval/` is now a `[workspace]` root with `core` as the only member; `metrics` (04.5-02) and `parser` (04.5-03) join in subsequent slices. `package.name` rewrote `poc-retrieval` → `codenexus-core` to match the existing `[[bin]]` name and avoid the reserved `core` crate name (which shadows `std::core`). No semantic change: same source tree, same dependencies, same `embedder.version_hash = f2b47aa16b17` per §9.8. G-D gate evidence (B1-B7 mean = 67.86% from `eval/req10_alpha06_candle.json`, in [65.9%, 69.9%]) plus 22/22 tests plus fsc.db end-to-end smoke confirm semantic equivalence under the new layout.

### 10.2 ONNX Runtime backend — D-W7

`ort` (ONNX Runtime Rust binding) covers AMD GPUs, Intel Arc, and NPUs that `candle-cuda` does not. Snowflake-arctic-embed has official ONNX export; switching cost is low. Path stays open as a Future embedder backend; NOT implemented in MVP.

### 10.3 Phase 4 (Parity)

- Multi-language tree-sitter (Python, Go, Rust, Java).
- CodeFlow MIT ports: git overlay (blame/log/diff), pattern detection, security scanners. Visualization NOT ported — written fresh (cytoscape.js as lib, our own interactions/layout/styling).
- **Repomix ingestion path** — accept `yamadashy/repomix` packed bundles (XML/MD/JSON) as alternative input for repos we don't clone locally. MIT, port-eligible under Apache 2.0 + NOTICE if code is lifted.
- **Repomix output mode** — emit "neighborhood pack around symbol X" as a query response format for LLM consumers (extends D-A4). Pattern only, not code (TS upstream vs our Rust).

### 10.4 Phase 5 (Bridge) and Phase 6 (Reach)

- Phase 5: memU integration mode (self-contained vs shared PG vs hybrid); Markdown wiki-link graph + Obsidian integration via `obsidian-llm-wiki`.
- Phase 6: Plugin system spec (custom embedders, custom tree-sitter languages, custom analyzers); A2A agent card publication for remote-mesh discovery.

### 10.5 v2 / post-MVP, no phase assignment

- Live file-watcher (`notify` crate); MVP uses git diff + mtime walk (D-R4) — cross-platform edge cases (Windows `ReadDirectoryChangesW`) are MVP overreach.
- Multi-GPU data-parallel embedder (YAGNI today).
- Tauri native window (currently anti-scope; revisit if browser UI proves insufficient).
- VS Code extension (separate project; MCP integration is enough).

### 10.6 A2A v0.2 → v1.0 transition gate

A2A v1.0 release before Phase 3 MVP ship → trigger a 1-day diff session before locking the schema. PROJECT.md constraint references this; this section commits explicitly.

---

## 11. References

- A2A v0.2 spec — `https://google.github.io/A2A/`
- spike-001 baseline — `obsidian-llm-wiki/.planning/spikes/001-embed-quality-on-code/`
- braedonsaunders/codeflow — MIT upstream for Phase 4 ports (`https://github.com/braedonsaunders/codeflow`)
- POC eval rounds — `experiments/poc-retrieval/eval/baseline_v1_results.md`, `experiments/poc-retrieval/eval/round_2_results.md`
- candle Rust ML book — `https://huggingface.github.io/candle/`
- mark3labs/mcp-go README — `https://github.com/mark3labs/mcp-go`

---

*End of ARCHITECTURE.md (Phase 1). Non-retrieval sections are locked. §9 reopens after poc-retrieval Round 3.*
