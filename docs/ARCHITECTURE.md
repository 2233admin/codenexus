# CodeNexus Architecture

> Phase 1 deliverable, 2026-04-27. Status: non-retrieval sections locked. Retrieval section is a stub pending poc-retrieval Round 3.

---

## 0. Document Scope

**In scope (this document, locked):**

- Service supervision (§2)
- A2A schema shape (§3)
- Clean-room policy and license boundary (§4)
- State ownership boundary between Rust core and Go server (§5)
- Logging stack and trace propagation (§6)
- Embedder device abstraction and worker-pool topology (§7)
- CI/CD GPU compilation policy (§8)
- Future / deferred items (§10)

**Stubbed (this document, §9 only):**

- Storage trait full shape locks the contract (D-R2) but storage backend
  pick (redb vs rusqlite+sqlite-vec) is Phase 2 spike work
- BM25 / vector fusion default tuning, embedder model rationale,
  reranker integration → `experiments/poc-retrieval/` Round 3

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
  {"symbol_id": "sym_caller1", "name": "main", "path": "src/main.ts", "edge_kind": "CALLS"},
  {"symbol_id": "sym_caller2", "name": "init", "path": "src/init.ts", "edge_kind": "CALLS"}
]}
```

**Failure codes:** `SYMBOL_NOT_FOUND` or `RESOLVER_PARTIAL` (latter retryable: false, partial results in `details.partial_callers`).

---

## 4. Clean-Room Policy

### 4.1 Hybrid enforcement — D-C1

Two layers: (1) **written rule** (this section is canon); (2) **pre-commit hook** that refuses commits to `core/**` if the local GitNexus repo was accessed within the last 24 hours. The hook operates on the user's local GitNexus checkout (e.g. `~/code/gitnexus`) by inspecting an "access timestamp" — implementation gets nuanced on Windows.

> **Open implementation detail (Windows NTFS):** NTFS disables last-access tracking by default for performance (`fsutil behavior query DisableLastAccess` typically returns `1` or `2`). On a default Windows install, `stat()` last-access is frozen and the naive hook silently always passes. Three viable mechanisms, to be picked during Phase 1 plan-phase:
>
> 1. **Require user enables NTFS last-access** via `fsutil behavior set DisableLastAccess 0`. Simple but pollutes the user's filesystem behavior globally.
> 2. **Substitute "last fetched" check.** Hook reads `<gitnexus>/.git/FETCH_HEAD` mtime or runs `git -C <gitnexus> reflog --date=iso -1`, compares to `now() - 24h`. Robust on all filesystems; captures the more relevant signal.
> 3. **Manual state stamp.** User runs `clean-room-stamp.sh` at the end of each GitNexus session writing a timestamp to `~/.codenexus/clean-room-state`. Highest discipline cost; lowest false-positive rate.
>
> Recommendation pending plan-phase: option 2 (FETCH_HEAD mtime) as default, option 3 as fallback for sessions that browse code without fetching.

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

## 9. Retrieval — STUB

> **Status**: Pending Round 3 of `experiments/poc-retrieval/`. Storage trait full shape (D-R2 sketch already locked), BM25 + vector fusion default parameters, embedder model rationale, and any reranker integration will land after Round 3 numbers.
>
> **Locked from POC Round 2 empirical evidence (do not relitigate without new data)**:
>
> - **RRF fusion is parameterized, not constant.** `retrieval.fusion_alpha = 0.6` default (vector weight, BM25 weight = 1 − α), tunable via config. Round 2 showed vector path materially outperforming BM25 (42.5% vs collapse on axis 2). Round 3 alpha sweep (`experiments/poc-retrieval/eval/round_3_results.md`) confirmed α=0.6 is the local optimum where both axis 1 (70%) and axis 2 (47.5%) hit local peak; α≥0.7 trades axis 1 for no axis 2 gain. Equal-weight RRF (α=0.5) is rejected as default.
> - **BM25 indexing requires camelCase decomposition.** SQLite FTS5 default `unicode61` tokenizer splits on `_` (snake_case works) but does NOT split camelCase. A `search_blob` column populated with decomposed name + snippet (e.g. `walkSubtree` → `walk subtree`) is part of the indexing contract. BM25 column weights (name 10x, snippet 1x, kind 1x, search_blob 5x) are set per Round 3.
> - **Storage trait `list_symbols_by_file` is mandatory** (D-R2 already locks this — without it, `last_indexed_commit` anchor is meaningless because incremental updates can't find old symbols to delete).
> - **Single-language MVP cost is empirically ≈17%.** Round 1 + 2 query sets show 5/30 queries map to Python files which TS-only POC cannot answer. Phase 4 multi-language ROI argument has its receipt.
>
> **TODO for Round 3 to fill**: storage backend final pick (redb vs rusqlite+sqlite-vec, Phase 2 spike still pending), `search_blob` decomposition exact rules, RRF c parameter default, reranker integration if Path A doesn't clear 60% target.

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
