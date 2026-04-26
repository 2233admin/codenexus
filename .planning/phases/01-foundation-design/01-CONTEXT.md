# Phase 1: Foundation Design - Context

**Gathered:** 2026-04-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Produce `docs/ARCHITECTURE.md` (clean-room, no GitNexus reference) covering:

1. Rust core / Go server split + responsibilities
2. A2A IPC boundary (schema for `index_repo`, `query`, `get_symbol`, `list_callers`)
3. Data layer trait boundaries (Storage, Resolver, Embedder)
4. Clean-room policy enforcement (24h gap rule + AI-agent stricter variant + violation recovery)

**Code is NOT in scope this phase** — only the design document. Phase 2 (Stack Spike) validates trait impls; Phase 3 (MVP) implements them.

**Carrying forward from PROJECT.md (already locked, not re-asked):**
- Apache 2.0 + NOTICE attribution to braedonsaunders/codeflow
- Rust core (axum) + Go service layer (chi + mark3labs/mcp-go + cobra) split
- IPC = Google A2A v0.2 over localhost HTTP
- Single fat-binary via Go `//go:embed` of Rust binary
- candle as default embedder; ollama-rs / async-openai as pluggable alternatives
- Browser UI (cytoscape.js + HTMX + vanilla JS) served by Go's chi
- Single Cargo crate for MVP (workspace split deferred — see D-R1 for triggers)
- No Python anywhere; no React/Vue/Svelte; no Tauri/Electron
- repo home: `D:/projects/codenexus/` (independent of obsidian-llm-wiki)
- Default Rust core port 9876, with auto-find fallback (D-S4)

**Carrying forward as deferred (out of Phase 1 scope):**
- Storage backend impl (redb vs rusqlite+sqlite-vec) — Phase 2 spike (but trait locks here, see D-R2)
- memU integration mode (self-contained vs shared PG) — Phase 5 Bridge
- GitHub repo visibility (public-from-Phase-1 vs wait-for-MVP) — Phase 1 wrap-up
- Live file-watcher (notify crate) — v2; MVP uses git diff + mtime (D-R4)
- ONNX Runtime backend — Future, not MVP (D-W7)

</domain>

<decisions>
## Implementation Decisions

### A2A schema shape (success criterion #2)

- **D-A1:** **One A2A skill `code-graph`** with `operation` field in message data part. Skill accepts `messages[].parts[].data = {operation: "index_repo"|"query"|"get_symbol"|"list_callers", ...args}`. Reasoning: A2A skills are coarse capabilities, not RPC methods. One health endpoint, simpler agent card, easier to evolve.

- **D-A2:** **Hybrid streaming** — polling default, SSE upgrade via `Accept: text/event-stream`. Default = client polls `GET /tasks/{id}` every Ns. If client requests SSE, upgrade to `/tasks/{id}/stream` with progress events. Two code paths, shared task state. A2A v0.2 spec leaves SSE optional, so polling guarantees any A2A client works.

- **D-A3:** **Composed error parts** — A2A-portable text + machine-readable data. On `state: failed`, `messages[].parts[]` contains BOTH:
  - A `text` part with human-readable error string (A2A-portable)
  - A `data` part with `{code: string, retryable: bool, details: object}` (machine-readable)

- **D-A4:** **Rich query results with meta scores.** Each `query` result item:
  ```
  {symbol_id, kind, name, path, range, parent, snippet,
   bm25_score, vector_score, rrf_score, final_score}
  ```
  Reasoning: 60% precision target requires ablation/debug throughout MVP; meta scores critical for spike-001 eval harness. Cost ~few KB per query is negligible.

### Clean-room policy enforcement (success criterion #4)

- **D-C1:** **Hybrid enforcement** — written rule in ARCHITECTURE.md + pre-commit hook checking `last access` timestamp on local GitNexus repo path. Hook refuses commits to `core/*` if GitNexus was accessed in last 24h (override with logged reason flag). Audit trail at `.codenexus/clean-room.log`.

- **D-C2:** **Stricter rule for AI-driven coding** — AI agents working on CodeNexus core MUST NOT have GitNexus source in their context window at all (no 24h grace). Only ARCHITECTURE.md and design notes. Reasoning: AI "memory" is the conversation; the 24h gap concept doesn't translate. Mechanism: agent prompt template forbids reading paths under any GitNexus repo.

- **D-C3:** **Hard license boundary** — explicit allowlist/denylist in ARCHITECTURE.md §"License Boundaries":
  - CodeFlow MIT = port-allowed (with NOTICE attribution + per-file header naming upstream commit hash)
  - GitNexus PolyForm = study-only, never ported, 24h gap applies
  Unambiguous separation; legal posture review reads one section.

- **D-C4:** **Document + isolate + cooldown on violation** — dated entry to `.codenexus/clean-room.log` describing what was open + which files were edited; affected files enter `tainted` state; refactor in fresh clean-room session 24h+ later, marked in commit message.

### Service supervision (REQ-07)

- **D-S1:** **Eager spawn on `serve` start** — Go boot → spawn Rust → await Rust healthcheck OK → chi/MCP/CLI servers come up. Failed Rust startup → `serve` exits with diagnostic. Matches REQ-07 acceptance verbatim.

- **D-S2:** **HTTP `/healthz` ping every 10s.** Rust core exposes a separate `/healthz` endpoint (NOT under A2A) returning `{ok, version, uptime, indexed_repos}`. Go pings every 10s; 3 consecutive failures → declared dead. Standard pattern, debuggable with `curl`.

- **D-S3:** **Exponential backoff + crash-loop breaker.** Restart with backoff 1s → 2s → 4s → 8s → 16s, cap at 30s. If `>= 5` restarts in any 60s window → log fatal, return 503 on `/tasks/send`, Go process exits non-zero. Reset counter after 5min stable.

- **D-S4:** **Auto-find alternate port + lockfile.** Scan 9876 → 9999 for first free port; bind; write chosen port to `~/.codenexus/port` lockfile. Go reads lockfile to find Rust. On startup, verify the PID named in any stale lockfile is actually our process; else clean and re-bind. Preserves "single fat-binary, zero install" promise even when port 9876 is occupied.

### Wheel inventory / OSS to stitch

- **D-W1:** **Hand-roll A2A over `axum + serde + schemars`** for Rust core. ~300 LOC of A2A v0.2 types + JSON Schema for agent card. Phase 2 spike re-checks for any mature Rust A2A crate that may have emerged (no such crate as of 2026-04).

- **D-W2:** **CodeFlow MIT ports for Phase 4** (NOT MVP):
  - Git overlay (blame/log/diff)
  - Pattern detection (singleton/factory/observer/etc)
  - Security scanners (secrets/SQLi/eval)
  - **Visualization layer is NOT ported** — written fresh. We use cytoscape.js as the lib but write our own interactions/layouts/styling.
  - All ports get per-file header comment naming upstream file + commit hash; NOTICE accumulates Apache 2.0 attribution lines.

- **D-W3:** **Go embeds UI via `//go:embed ui/`, chi serves.** Rust core stays headless (pure A2A agent). Preserves "Rust core callable by any A2A client" — UI is a Go-tier presentation concern.

- **D-W4:** **Logging stack with correlatable trace_id across boundary.**
  - Rust: `tracing` + `tracing-subscriber` (JSON formatter)
  - Go: stdlib `slog` (1.21+, JSON handler)
  - Both emit `trace_id` field; Go originates UUIDv7 (D-B4); propagated through A2A `task_id`. Logs joinable across boundary with `jq`.

- **D-W5:** **Embedder Device Abstraction** (full specification — user-authored, captured verbatim):
  - Device selection resolved once at startup via `probe_device()` (CUDA → Metal → CPU, feature-gated).
  - Resulting `Arc<Device>` stored in `EmbedderConfig` and shared with model weights (`VarBuilder`) and the single embedder worker.
  - All `candle_core` tensor operations are device-transparent; no code path branches on device type after initialization.
  - Batch size derived from device class: `CPU=32, Metal=128, CUDA=256`. User-overridable via `IndexConfig.embedder.batch_size`.
  - Worker uses `tokio::select!` with **50ms flush timeout** to prevent tail latency on sparse input.
  - Worker pool topology: rayon parallel parser (CPU-bound) → bounded mpsc channel (cap 256 symbols) → single Device-aware embedder worker → batched Storage writes (per 1000 symbols + checkpoint).
  - 500k LOC repo budget: ~50k symbols → CPU ~17min @ 50 sym/sec; CUDA significantly faster with batched flush.

- **D-W6:** **CI/CD GPU policy.** Default `cargo build --no-default-features` is CPU-only. CUDA / Metal features behind `cuda` / `metal` cargo features, **not** in `default`. GPU integration tests run on dedicated runners with proper toolkit. Bare CPU machines (most CI workers, contributors without GPU) compile and test cleanly.

- **D-W7:** **ONNX Runtime as Future embedder backend** — documented in ARCHITECTURE.md §"Future: embedder backends" but NOT implemented in MVP. Rationale: `ort` (ONNX Runtime Rust binding) covers AMD / Intel Arc / NPU GPUs that candle-cuda doesn't; arctic-embed has official ONNX export, switching cost is low. Path stays open without committing now.

- **D-W-extras (auxiliary wheels, no separate question):**
  - `ignore` crate (BurntSushi) — gitignore-aware file walking. Standard, no debate.
  - `tantivy` vs `SQLite FTS5` for BM25 — coupled to storage choice (D-R2 trait abstracts). If Phase 2 picks rusqlite+sqlite-vec → FTS5 (free). If redb → tantivy (separate index). Hand-rolled BM25 over redb is **rejected** (high bug surface, low value).
  - `dirs` crate — XDG path resolution for D-B1, D-B2, D-B3.
  - `figment` + TOML — Go-side config layering for D-B2 (or `koanf` if Go-native preferred; figment is Rust — Go uses `koanf` or `viper` — ARCHITECTURE.md picks during write).

### State ownership boundary (success criterion #3)

- **D-B1:** **Graph DB at `<XDG_DATA_HOME>/codenexus/<repo-hash>/db`** — Rust-owned, resolved via `dirs` crate (cross-platform: Linux `~/.local/share/`, Windows `%LOCALAPPDATA%`, macOS `~/Library/Application Support/`). Per-repo isolation by hash of canonical repo path.

- **D-B2:** **User config: XDG global + per-repo override + CLI flag** (git-style precedence). Global = `<XDG_CONFIG_HOME>/codenexus/config.toml`. Per-repo = `<repo-root>/.codenexus/config.toml`. CLI flags override both. Go-owned (Go is the user-facing entry point).

- **D-B3:** **Embedder model cache = HuggingFace default** (`<HF_HOME>/hub/`, default `~/.cache/huggingface/hub/`). Shared with ollama / hf-cli / transformers ecosystem; no duplicate downloads. Override via env var (`HF_HOME`). Bundle-in-binary remains Phase 2 spike question (PROJECT.md already defers).

- **D-B4:** **trace_id origination: Go-side UUIDv7 at request entry**, propagated to Rust via A2A `task_id` field. Both sides log with `trace_id`. Single source of truth; correlation via `jq '.trace_id'` across both log streams.

- **D-B-extras:**
  - Rust binary extraction temp dir: `<XDG_CACHE_HOME>/codenexus/bin/codenexus-core-<version>/`. Persists across runs (faster startup after first); Go-owned (Go is the //go:embed-er).
  - Spawn env vars: Go passes `CODENEXUS_PORT_LOCKFILE`, `CODENEXUS_DATA_DIR`, `RUST_LOG`, `CODENEXUS_DEVICE` to Rust.

### Architecture refinements (from review)

- **D-R1:** **Workspace split trigger conditions.** ARCHITECTURE.md §"When to split crate" lists 4 triggers; ANY ONE triggers a workspace split, NONE → stay single crate:
  1. Clean `cargo build` exceeds 90s on dev machine
  2. Transitive crate dep count exceeds 80 in `core/`
  3. Need separate sub-binaries (e.g., `codenexus-indexer` standalone)
  4. Phase 4 multi-language puts each tree-sitter behind a feature flag (workspace makes feature gating cleaner)

- **D-R2:** **Storage trait locked in Phase 1, impl in Phase 2.** Reasoning: deferring trait shape risks Phase 3 MVP rework after spike concludes. Trait shape (proposed; Phase 2 may amend with command-output evidence):
  ```rust
  pub trait GraphStorage: Send + Sync {
      // Symbol CRUD
      fn put_symbol(&mut self, s: &SymbolNode) -> Result<SymbolId>;
      fn get_symbol(&self, id: SymbolId) -> Result<Option<SymbolNode>>;
      fn list_symbols_by_file(&self, path: &Path) -> Result<Vec<SymbolId>>;  // REQUIRED for incremental indexing
      fn delete_symbol(&mut self, id: SymbolId) -> Result<()>;

      // Edges
      fn put_edge(&mut self, from: SymbolId, to: SymbolId, k: EdgeKind) -> Result<()>;
      fn get_callers(&self, id: SymbolId) -> Result<Vec<SymbolId>>;

      // Embeddings + dual search
      fn put_embedding(&mut self, id: SymbolId, v: &[f32]) -> Result<()>;
      fn search_text(&self, q: &str, k: usize) -> Result<Vec<(SymbolId, f32)>>;  // BM25
      fn search_vec(&self, v: &[f32], k: usize) -> Result<Vec<(SymbolId, f32)>>; // cosine

      // Indexing throughput + incremental anchor
      fn batch_commit(&mut self, ops: Vec<WriteOp>) -> Result<()>;
      fn checkpoint(&self) -> Result<Checkpoint>;
      fn last_indexed_commit(&self) -> Result<Option<GitOid>>;
      fn set_last_indexed_commit(&mut self, oid: GitOid) -> Result<()>;
  }
  ```
  `list_symbols_by_file` is non-negotiable — without it, `last_indexed_commit` anchor is meaningless because incremental updates can't find old symbols to delete.

- **D-R3:** **Non-atomic dual-write acknowledgment + WAL-replay mitigation.** ARCHITECTURE.md explicitly states: if Phase 2 picks redb + tantivy split-store, `batch_commit` is **not transactional across both stores**. Crash window can produce text-index ↔ graph-store inconsistency. Mitigation:
  - Write-ahead log of pending storage writes at `<data_dir>/wal/`
  - Startup: WAL replay verifies consistency between graph store and text index
  - Inconsistency detected → reindex affected files (using `list_symbols_by_file` to find what to redo)
  - If Phase 2 picks rusqlite+sqlite-vec+FTS5 instead → moot (single transactional file); WAL still useful for crash recovery during long index runs.

- **D-R4:** **Incremental indexing strategy: hybrid git diff + mtime walk.**
  - Track `last_indexed_commit` per repo (in storage)
  - On second-and-later index runs:
    - Committed changes: `git diff --name-only <last>..HEAD` → reindex affected files
    - Uncommitted dirty: walk working tree, check `mtime > last_index_time` → reindex
    - Renamed/deleted files: use git diff status R/D → delete stale symbols + edges
  - Live file-watcher (notify crate) deferred to v2 — its cross-platform edge cases (especially Windows ReadDirectoryChangesW) are MVP overreach.

- **D-R5:** **Cross-file symbol resolution: tree-sitter + project-aware import graph + Resolver trait.**
  - MVP TS resolver: parse `import` / `require` per file → build module-resolution map (handles tsconfig paths, node_modules) → resolve calls within import scope
  - Wrapped in `Resolver` trait so Phase 4 multi-language plugs in per-language strategies
  - Tradeoff: ~85% precision on TS; harder cases (dynamic dispatch, eval, runtime-generated calls) unresolved — accepted for MVP
  - LSP-backed approach **rejected** (per-symbol JSON-RPC is performance death on large repos)
  - SCIP pre-indexer (sourcegraph) noted as Future option for languages where tree-sitter resolution is structurally insufficient.

### Claude's Discretion

- Exact JSON envelope shape of A2A messages (operation names, field names) — to be drafted in ARCHITECTURE.md as JSON examples; user sees them then.
- Specific implementation details of `probe_device()` (linker quirks, feature-gate combinations) — Phase 2 spike resolves.
- Per-file CodeFlow attribution comment format (single-line vs SPDX-style block) — picked during Phase 4 port; trivial.
- Pre-commit hook exact command shape — picked during Phase 1 plan-phase or as part of ARCHITECTURE.md examples.

### Folded Todos

- **gsd-sdk reinstall** — done in this session (junction was dangling to stale npx cache; `npm install -g get-shit-done-cc@latest` after junction wipe restored). Future-proofing: workflow gates now system-enforced again.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner) MUST read these before planning or implementing.**

### Project-level (locked decisions, requirements, roadmap)
- `.planning/PROJECT.md` — vision, key decisions table (10 entries), constraints, anti-scope
- `.planning/REQUIREMENTS.md` — REQ-01 through REQ-10 with acceptance criteria
- `.planning/ROADMAP.md` §"Phase 1: Foundation Design" — goal + 4 success criteria + 1 plan
- `.planning/STATE.md` — decisions log, deferred items, blockers

### License + legal
- `LICENSE` — Apache 2.0 full text
- `NOTICE` — codeflow attribution boilerplate (will accumulate per-module entries during Phase 4 ports)

### Historical / clean-room legal trail
- `docs/origin-spec.md` — original Stitch proposal (2026-04-25); kept for clean-room legal trail; section §"Clean-room policy" is the seed for D-C1/C2/C3/C4

### External specifications
- A2A v0.2 spec — `https://google.github.io/A2A/` — defines task envelope, skill model, error states; D-A1/A2/A3 reference this
- spike-001 baseline — `obsidian-llm-wiki/.planning/spikes/001-embed-quality-on-code/` — 7 NL queries, GitNexus 1.6.3 measured at 43% top-5 precision; REQ-10 acceptance gate
- braedonsaunders/codeflow (GitHub) — MIT upstream for Phase 4 ports; clean-room policy treats as port-allowed (D-C3)

### Out-of-tree references (not yet read; planner should fetch when needed)
- candle Rust ML book — `https://huggingface.github.io/candle/` — Device API, VarBuilder, batch processing patterns
- mark3labs/mcp-go README — MCP server patterns; D-A1 affects how the `code-graph` skill exposes via MCP tool
- BurntSushi/ignore crate docs — gitignore semantics; D-W-extras

</canonical_refs>

<code_context>
## Existing Code Insights

**State as of commit `4c47ca6` (initial scaffolding, 2026-04-26):**

### Reusable Assets
- `core/Cargo.toml` + `core/src/main.rs` — Rust crate skeleton (empty main); ARCHITECTURE.md will land alongside as `docs/ARCHITECTURE.md`, no code change yet
- `server/go.mod` + `server/main.go` — Go module skeleton (empty main); ARCHITECTURE.md will name the chi/mcp-go/cobra commitment
- `Makefile` — build entry stub (will be fleshed out Phase 3 to chain `cargo build` → `go build` → embed)
- `LICENSE` (Apache 2.0) + `NOTICE` (codeflow attribution) — legal baseline already in place

### Established Patterns
- None yet — first phase. Phase 1 establishes patterns.

### Integration Points
- `core/` ↔ `server/` boundary — defined by A2A schema (D-A1/A2/A3/A4) in this phase
- `ui/` directory — `//go:embed` target (D-W3); UI files will be added Phase 3 plan 03-05
- `.planning/` — GSD canonical (this phase's output goes here)
- `scripts/linear_sync_decisions.py` — single-shot Linear sync; safe to ignore for ARCHITECTURE.md write
- `docs/` — ARCHITECTURE.md will live next to `origin-spec.md`

### Tooling baseline (this session)
- `gsd-sdk` v0.1.0 (`get-shit-done-cc` 1.38.5) — restored mid-session after dangling-junction diagnosis; workflow gates now operational
- gitleaks pre-commit hook — confirmed working (cleared the initial commit)

</code_context>

<specifics>
## Specific Ideas

- **"缝合" (stitch) is the philosophy, not just a chore.** User explicitly de-selected viz from CodeFlow ports because UI taste matters most and lifting upstream there constrains future iteration. Pattern detection / security / git overlay are *analytical* layers where CodeFlow's value extracts cleanly.

- **Trait boundaries lock in Phase 1, impls lock in Phase 2.** Per user review: "整体设计干净" but Phase 1 must lock the *shape* — Phase 2 spike then becomes a fair test of impls against a fixed contract, not a re-design.

- **Worker pool must be Device-aware from day one.** User-authored D-W5 spec is verbatim — `probe_device()` + `Arc<Device>` + per-class batch size + 50ms flush timeout — this isn't optional GPU optimization, it's the architecture's baseline so CPU/CUDA/Metal paths all work without code branching.

- **gsd-sdk fix is workflow integrity, not convenience.** User framing: "工作流门控坏着，你每次 phase transition 都在靠自律而不是系统保证，这不是工程的做法。五分钟的事。" Fixed mid-session before CONTEXT.md commit per direction.

- **Storage trait `list_symbols_by_file` is structural.** User caught: without it, `last_indexed_commit` anchor is meaningless. Captured in D-R2 trait shape.

- **Non-atomic dual-write must be acknowledged in the doc, not silently inherited.** User caught: redb + tantivy doesn't share a transaction. ARCHITECTURE.md says it explicitly + writes the WAL-replay mitigation so Phase 2 implementor doesn't trip.

- **CI/CD GPU isolation discipline.** Default features must compile on bare CPU; GPU is opt-in. User principle: CI green on commodity workers, opt-in for hardware-required tests.

</specifics>

<deferred>
## Deferred Ideas

### To Phase 2 (Stack Spike)
- Storage backend choice (redb vs rusqlite+sqlite-vec) — bench on 10K embeddings + FTS5 query mix. Trait amendment if either can't satisfy D-R2.
- candle cold-start + throughput measurement — affects Device-aware batch tuning (D-W5)
- A2A roundtrip latency confirmation (REQ-06 acceptance: <5ms p99 localhost)
- mcp-go maintainer-health spot check (mentioned in STATE.md blockers)

### To Phase 3 (MVP)
- Embedder model bundle-in-binary vs HF-cache decision (PROJECT.md defers; affects ~80-120MB binary size)
- WAL replay code path — implemented in Phase 3 even if Phase 2 picks single-file SQL backend (still useful for long index recovery)

### To Phase 4 (Parity)
- Multi-language tree-sitter (TS + Python + Go + Rust)
- Multi-repo registry
- Git overlay (gix) port from CodeFlow MIT
- Pattern detection port from CodeFlow MIT
- Security scanners port from CodeFlow MIT
- Health score computation
- Workspace split (will hit one of D-R1's triggers around here)

### To Phase 5 (Bridge)
- memU integration mode (self-contained vs shared PG vs hybrid)
- Markdown wiki-link graph + Obsidian integration

### To Phase 6 (Reach)
- Plugin system spec (custom embedders, custom tree-sitter languages, custom analyzers)
- A2A agent card publication

### To v2+ (post-MVP without phase assignment)
- Live file-watcher (notify crate) for sub-second incremental updates
- ONNX Runtime backend (`ort` crate) for AMD / Intel Arc / NPU GPUs (D-W7)
- Multi-GPU data-parallel embedder (only if hardware exists; currently YAGNI)
- Tauri native window option (PROJECT.md anti-scope today; could revisit)
- VS Code extension (separate project if ever; MCP integration is enough)

### Out of scope but noted
- The Kami project (`https://github.com/tw93/kami`) — user mentioned mid-discussion. Not folded into Phase 1 scope. Possible roles to evaluate later: (a) doc-generation tool for future ARCHITECTURE / RFC documents, (b) UI inspiration for the cytoscape-fronted graph viewer, (c) wheel for Phase 4 viz layer. Action: review URL after Phase 1 commits, decide whether to add to wheel inventory or backlog.

</deferred>

<unresolved_questions>
## Unresolved Questions

1. **Storage trait may amend post-Phase-2.** Trait shape in D-R2 is the contract going in; if redb or rusqlite+sqlite-vec genuinely can't satisfy `list_symbols_by_file` or `batch_commit` efficiently, Phase 2 spike will surface and amend. ARCHITECTURE.md should mark this trait as "Phase 1 lock, Phase 2 may extend with evidence."

2. **A2A v0.2 → v1.0 transition risk.** STATE.md flags this. ARCHITECTURE.md should commit to a re-evaluation gate: any A2A v1.0 release before Phase 3 MVP triggers a 1-day diff session before locking the schema. Captured in PROJECT.md constraints; needs explicit doc reference.

3. **Pre-commit hook exact form.** D-C1 mandates the hook exists; ARCHITECTURE.md should include either pseudocode or a worked example. Picked during Phase 1 plan-phase or written as part of ARCHITECTURE.md.

4. **Configuration format for `IndexConfig.embedder.batch_size` override (D-W5).** TOML key path? CLI flag form (`--batch-size N`)? Env var (`CODENEXUS_BATCH_SIZE`)? All three? Plan-phase or ARCHITECTURE.md examples decide.

5. **Workspace split criteria — measurement how?** D-R1 names triggers but not who measures or where the threshold check lives. Suggestion: ARCHITECTURE.md adds a "review checkpoint" at Phase 4 entry that runs `cargo build --timings` + `cargo tree | wc -l` and writes results to a phase summary. Confirm during plan-phase.

6. **Kami URL action.** User dropped `https://github.com/tw93/kami` mid-discussion without explicit framing. Capture in deferred above, but ARCHITECTURE.md write itself should NOT consume Kami until user clarifies intent (doc-gen tool vs viz wheel vs unrelated).

</unresolved_questions>

---

*Phase: 01-foundation-design*
*Context gathered: 2026-04-26*
*Workflow: gsd-discuss-phase (interactive, 5 rounds + post-review refinement)*
*Tooling fix this session: gsd-sdk dangling junction → reinstalled get-shit-done-cc@latest*
