# Phase 1: Foundation Design - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions captured in `01-CONTEXT.md` — this log preserves alternatives considered and the user-driven refinements.

**Date:** 2026-04-26
**Phase:** 01-foundation-design
**Areas discussed:** A2A schema shape, Clean-room enforcement, Service supervision, Wheel inventory, State ownership boundary
**Workflow:** /gsd-discuss-phase 1 (interactive, 5 AskUserQuestion rounds + 1 post-review correction round)

---

## Round 0: Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| A2A schema shape (Recommended) | Schema for index_repo / query / get_symbol / list_callers; skill granularity; pagination/streaming; error model. | ✓ |
| Rust ↔ Go state ownership boundary | Port lockfile; user config; storage path; embedder cache; UI temp; env vars. | (deferred — handled in Round 5 post-prompt) |
| Clean-room enforcement mechanism | 24h gap rule operationalization; AI-agent scope; CodeFlow vs GitNexus boundary; violation recovery. | ✓ |
| Service supervision model | Spawn timing; healthcheck; restart strategy; port collision behavior. | ✓ |

**User-added (via "Other"):** "以及 我们应该缝合那些 有哪些轮子能拿来用" → Wheel inventory area.

---

## Round 1: A2A Schema Shape

### Q1: Skill granularity
| Option | Description | Selected |
|--------|-------------|----------|
| One skill, operation field (Recommended) | Single `code-graph` skill with `operation` field in data part. | ✓ |
| Four separate skills | `code-graph.index_repo` etc. registered independently. | |
| Two skills (admin + read) | `code-graph.admin` + `code-graph.read`. | |

### Q2: Long-running index_repo
| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid: polling default + SSE upgrade (Recommended) | Default polls; `Accept: text/event-stream` upgrades. | ✓ |
| Polling only (spec-minimal) | Just POST + GET; no SSE. | |
| SSE-required for long ops | index_repo always streams. | |

### Q3: Error envelope
| Option | Description | Selected |
|--------|-------------|----------|
| A2A-with-discriminator (Recommended) | Compose `text` part + `data` part with `{code, retryable, details}`. | ✓ |
| Spec-minimal text only | Just `state: failed` + text. | |
| Custom extension type | Define `code-graph/error` part type. | |

### Q4: Query response shape
| Option | Description | Selected |
|--------|-------------|----------|
| Rich + meta scores (Recommended) | symbol_id, kind, name, path, range, parent, snippet, all 4 scores. | ✓ |
| Lean (id + score only) | Client must call get_symbol for details. | |
| Rich without meta | Full SymbolNode but only final score. | |

**Notes:** All 4 recommendations accepted clean.

---

## Round 2: Clean-Room Enforcement

### Q1: Enforcement style
| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid: written rule + light tooling (Recommended) | Rule + pre-commit hook checking GitNexus access timestamps; audit log. | ✓ |
| Honor system + journal | Daily date-stamped journal entry. | |
| Tooling-only (strict gate) | Pre-commit + Gitea server hook hard-blocks. | |

### Q2: AI-agent scope
| Option | Description | Selected |
|--------|-------------|----------|
| Stricter for AI: never read GitNexus source (Recommended) | AI agents have no GitNexus source in context window at all. | ✓ |
| Same rule for AI and human (24h gap) | AI sessions check audit log. | |
| AI exempt | Treat AI as stateless. | |

### Q3: License boundary statement
| Option | Description | Selected |
|--------|-------------|----------|
| Hard boundary: explicit allowlist/denylist (Recommended) | §"License Boundaries" with CodeFlow MIT = port-allowed, GitNexus PolyForm = study-only. | ✓ |
| Soft boundary | Treat both as study-only. | |
| Inline annotations only | Per-file headers only. | |

### Q4: Violation recovery
| Option | Description | Selected |
|--------|-------------|----------|
| Document + isolate + cooldown (Recommended) | Log entry + tainted state + refactor 24h+ later. | ✓ |
| Hard reset: any violation invalidates the doc | Full rewrite from scratch. | |
| No process | No defined recovery. | |

**Notes:** All 4 recommendations accepted clean.

---

## Round 3: Service Supervision

### Q1: Spawn timing
| Option | Description | Selected |
|--------|-------------|----------|
| Eager on `serve` start (Recommended) | Spawn Rust → await healthcheck → start chi/MCP/CLI. | ✓ |
| Lazy on first A2A request | Spawn on demand. | |
| Eager + `--lazy` flag | Default eager; flag for embedded use. | |

### Q2: Healthcheck protocol
| Option | Description | Selected |
|--------|-------------|----------|
| HTTP `/healthz` ping every 10s (Recommended) | Separate /healthz endpoint, 3 fails → dead. | ✓ |
| A2A meta-task ping | Send `{operation: "ping"}` task. | |
| PID-only (kill -0) | OS-level liveness. | |

### Q3: Restart strategy
| Option | Description | Selected |
|--------|-------------|----------|
| Exponential backoff + crash-loop breaker (Recommended) | 1→2→4→8→16s, cap 30s; ≥5 in 60s → fatal. | ✓ |
| Pure exponential (no cap on attempts) | Retry forever. | |
| Fixed N retries then surface error | 3 retries with 1/3/9s waits. | |

### Q4: Port collision
| Option | Description | Selected |
|--------|-------------|----------|
| Auto-find alternate + lockfile (Recommended) | Scan 9876→9999, write `~/.codenexus/port`. | ✓ |
| Fail fast with clear message | Print override instructions. | |
| Kill-if-ours, else fail | Inspect existing process signature. | |

**Notes:** All 4 recommendations accepted clean.

---

## Round 4: Wheel Inventory

### Q1: Rust A2A library
| Option | Description | Selected |
|--------|-------------|----------|
| Hand-roll over axum+serde+schemars (Recommended) | ~300 LOC; no mature crate exists 2026-04. | ✓ |
| Use community crate IF exists, else hand-roll | Phase 2 spike survey. | |
| Hand-roll behind own `a2a` trait | Wrap behind abstraction. | |

### Q2: CodeFlow modules to port (multiSelect)
| Option | Description | Selected |
|--------|-------------|----------|
| Visualization | Port viz/layout/interaction layer. | (deselected) |
| Git overlay (blame/log/diff) | Port the git-aware UI overlays. | ✓ |
| Pattern detection (singleton/factory/etc) | Port heuristic detectors. | ✓ |
| Security scanners (secrets/SQLi/eval) | Port regex+AST scanners. | ✓ |

**User decision:** Visualization NOT ported — written fresh. Sharper boundary; UI taste matters most where lifting upstream constrains future iteration.

### Q3: UI delivery
| Option | Description | Selected |
|--------|-------------|----------|
| Go embeds via //go:embed, chi serves (Recommended) | Rust stays headless A2A agent. | ✓ |
| Rust embeds via rust-embed; Go proxies | UI baked into Rust binary. | |
| Both embed nothing | Ship binary + ui/ alongside. | |

### Q4: Logging stack
| Option | Description | Selected |
|--------|-------------|----------|
| Rust tracing + Go stdlib slog, JSON, shared trace_id (Recommended) | trace_id propagated via A2A task_id. | ✓ |
| Rust tracing + Go zerolog | Faster JSON; third-party Go dep. | |
| Rust log + Go stdlib log | Plain text, no structure. | |

**Notes:** 3 recommendations accepted; user thoughtfully de-selected viz from CodeFlow ports.

---

## Round 5: State Ownership Boundary (post-prompt addition)

User chose "Quick 5th round of questions on the boundary now" over baking defaults as Claude's Discretion.

### Q1: Graph DB path
| Option | Description | Selected |
|--------|-------------|----------|
| XDG data dir, per-repo-hash (Recommended) | `<XDG_DATA_HOME>/codenexus/<hash>/db`, Rust-owned via `dirs` crate. | ✓ |
| Per-repo local: `<repo-root>/.codenexus/db` | DB inside indexed repo. | |
| User-config-controlled, default to XDG | Override via flag. | |

### Q2: User config
| Option | Description | Selected |
|--------|-------------|----------|
| XDG + per-repo override, git-style precedence (Recommended) | Global + per-repo + CLI flags. | ✓ |
| Global only (XDG) | Single config file. | |
| Per-repo only | No global defaults. | |

### Q3: Embedder model cache
| Option | Description | Selected |
|--------|-------------|----------|
| HuggingFace default cache (Recommended) | `<HF_HOME>/hub/`, shared with HF ecosystem. | ✓ |
| Codenexus-isolated cache | `<XDG_CACHE_HOME>/codenexus/models/`. | |
| User-config-controlled, default HF | Override via config. | |

### Q4: trace_id origin + propagation
| Option | Description | Selected |
|--------|-------------|----------|
| Go originates UUIDv7, propagates via A2A task_id (Recommended) | Single source of truth at request entry. | ✓ |
| Rust originates, returns in response | Rust controls log identity. | |
| W3C tracecontext (span chain) | Industry-standard distributed tracing. | |

**Notes:** All 4 recommendations accepted clean.

---

## Round 6: Post-Review Refinement (user-initiated)

User pasted a substantial review identifying 7 gaps/reservations. Each addressed:

### Reservations (refinements to existing decisions)

| Item | User Concern | Resolution |
|------|--------------|------------|
| Single Cargo crate | Will hurt later as deps grow; ARCHITECTURE.md should specify when to split | Added D-R1: 4 trigger conditions; ANY one triggers split |
| Storage trait deferred | Phase 1 must lock trait boundary even if Phase 2 picks impl | Added D-R2: full GraphStorage trait shape, with `list_symbols_by_file` per user's later catch |
| gsd-sdk broken | Workflow integrity issue, not inconvenience; fix priority elevated | Fixed mid-session: dangling junction → reinstall `get-shit-done-cc@latest` |

### Gaps (new decisions added)

| Item | User Concern | Decision Captured |
|------|--------------|-------------------|
| Incremental indexing | First run is full; what about re-runs? | D-R4: hybrid git diff + mtime walk; live-watcher deferred to v2 |
| Cross-file resolution | Hard part of get_symbol/list_callers; tree-sitter or LSP? | D-R5: tree-sitter + project-aware import graph + Resolver trait; LSP rejected (perf killer) |
| Index scale | 500k LOC: how long, how much memory, batching? | Captured in D-W5 budget; rayon parser + bounded mpsc + single Device-aware embedder; ~17min/50k symbols on CPU |
| Missing wheels | `ignore` crate for file walking; `tantivy` or FTS5 for BM25 | D-W-extras: ignore crate (yes, no debate); tantivy/FTS5 coupling to Storage trait abstraction |

### User-authored content captured verbatim

User provided full Embedder Device Abstraction specification mid-discussion. Captured in D-W5 verbatim:
- `probe_device()` at startup (CUDA → Metal → CPU, feature-gated)
- `Arc<Device>` shared with VarBuilder + worker
- Per-class batch sizes (CPU=32, Metal=128, CUDA=256)
- `tokio::select!` + 50ms flush timeout for sparse-input tail latency

User also authored:
- D-W6 (CI/CD GPU policy: default --no-default-features, GPU on dedicated runners)
- D-W7 (ONNX Runtime as Future backend for AMD/Intel Arc/NPU)
- D-R3 (non-atomic dual-write acknowledgment + WAL replay mitigation)
- D-Storage-extra (`list_symbols_by_file` is non-negotiable for incremental updates)

### Storage trait amendment from review

Added to D-R2 trait:
```rust
fn list_symbols_by_file(&self, path: &Path) -> Result<Vec<SymbolId>>;
```
Without this, `last_indexed_commit` anchor is meaningless because incremental updates can't find old symbols to delete.

### Non-atomic dual-write (D-R3)

User caught: tantivy + redb don't share a transaction. ARCHITECTURE.md will explicitly state this and document WAL-replay mitigation:
- WAL of pending writes at `<data_dir>/wal/`
- Startup consistency check between graph store and text index
- Inconsistency → reindex affected files via `list_symbols_by_file`

---

## Claude's Discretion

Captured in CONTEXT.md `<decisions>` section:
- Exact JSON envelope shape of A2A messages
- `probe_device()` implementation details (linker, feature gates)
- CodeFlow attribution comment format (single-line vs SPDX block)
- Pre-commit hook exact command shape

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section across 5 buckets (Phase 2/3/4/5/6 + v2-and-beyond + out-of-scope-but-noted).

Notable out-of-scope: `https://github.com/tw93/kami` URL dropped mid-discussion; user did not specify intent. Recorded as deferred for later evaluation (doc-gen tool? viz wheel? unrelated?).

---

## Discussion meta

- Total AskUserQuestion calls: 6 (Round 0 + Rounds 1-5 + Round 5 boundary follow-up)
- Total decisions captured: **24** (4 A2A + 4 clean-room + 4 supervision + 7 wheels + 4 boundary + 5 refinements; D-W-extras and D-B-extras are auxiliary clusters not separately numbered)
- User acceptance rate of recommendations: **18/20 explicit options (90%)** + 1 thoughtful de-selection (viz from CodeFlow) + 1 multi-step refinement round
- Mid-session tooling fix: gsd-sdk junction repair (5min); workflow gates restored before CONTEXT.md commit

---

*End of discussion log*
