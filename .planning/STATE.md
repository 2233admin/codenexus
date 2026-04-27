---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: phase_3_active
stopped_at: Phase 3 REQ-08 //go:embed plumbing ✓ (commits f5b6621 + 59b725b); REQ-09 UI //go:embed pending next
last_updated: "2026-04-27T04:55:00.000Z"
last_activity: 2026-04-27 — Completed quick task 260427-hoc: REQ-08 //go:embed Rust core binary plumbing (embed.go + extractRustBinary + Makefile EMBED_DIR + .gitignore); 9 plan invariants verified. Mid-execution handoff (executor returned after Task 1, orchestrator finished Task 2 + fixed clean target bug missed by executor)
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 0
  completed_plans: 0
  percent: 17
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-26)

**Core value:** Top-5 NL search precision ≥ 60% on the spike-001 query set, exposed as an open A2A endpoint that any agent can call.
**Current focus:** Phase 3 (MVP) — REQ-06 ✓, REQ-07 ✓, REQ-08 ✓ (plumbing only, real binary smoke deferred); REQ-09 UI //go:embed next

## Current Position

Phase: 3 of 6 (MVP)
Plan: REQ-06 + REQ-07 + REQ-08 done (3 of 5)
Status: REQ-09 UI //go:embed next (same plumbing pattern as REQ-08 but simpler — http.FS over embed.FS, no extraction); REQ-10 precision measurement after stack runs end-to-end
Last activity: 2026-04-27 — Phase 3 REQ-08 //go:embed plumbing green (commits f5b6621 + 59b725b, 9/9 invariants verified)

Progress: [████░░░░░░] 33% (Phase 1 closed + REQ-06 + REQ-07 + REQ-08 of 5 in Phase 3)

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Foundation Design | 0/1 | — | — |
| 2. Stack Spike | 0/6 | — | — |
| 3. MVP | 0/5 | — | — |

**Recent Trend:**

- Last 5 plans: none yet
- Trend: —

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- 2026-04-26: Architecture pivot — pure Rust → Rust core (axum A2A) + Go service (chi/mcp-go/CLI). IPC = A2A protocol over localhost HTTP.
- 2026-04-26: License — MIT → Apache 2.0.
- 2026-04-26: Naming — Stitch → CodeNexus (locked).
- 2026-04-26: Embedder default — ollama-rs → candle.
- 2026-04-26: UI — pivoted Tauri → axum/Go-served web (option B).
- 2026-04-27: Phase 1 closed. ARCHITECTURE.md §9.1-§9.8 all locked. REQ-02 expanded to 4 edge kinds (Calls + Imports + Implements + Extends, Overrides Phase 3+). LLM-judge graded 0-3 N>=3 seeds primary metric (EVAL Rule 6). Path B Jina reranker NOT adopted (3 LLM-judge methodologies consistent — R5/R6/R6c).
- 2026-04-27: User directive — ship-it pivot. After Phase 2 storage spike, go straight to Phase 3 MVP build (axum A2A endpoint + Go MCP server + fat-binary). No more eval rounds unless explicitly requested.
- 2026-04-27: Phase 2 storage spike SKIPPED — research-conclusive verdict (rusqlite+sqlite-vec) per .planning/research/storage_backend.md; micro-bench recommended but not blocking; ship-it directive prefers building over re-validating.
- 2026-04-27: REQ-06 A2A endpoint ✓ — commit e0727c2; axum 0.7 + tokio 1.40 + 3 new src/ files (a2a, server, task_state); 4 operations (Query/GetSymbol/ListCallers/IndexRepo) working end-to-end; smoke green on existing poc.db (52 files / 2116 symbols / 877+ edges).

  **NOTE for future-you:** REQ-06 is implemented in `experiments/poc-retrieval/` (Phase 3 ship path — see `experiments/poc-retrieval/src/main.rs:71` "Phase 3 MVP entry point" comment). The `core/` directory is a 13-line `println!` placeholder that was never updated post-spike — it is **superseded**, not the build target. Makefile targets (build-core / test-core / fmt / lint / clean) point at `experiments/poc-retrieval/` as of 2026-04-27 (see commit `confidence-uplift`). Phase 4 cleanup: delete `core/` or alias it to `experiments/poc-retrieval/` via cargo workspace, whichever is cleaner.

### Pending Todos

- REQ-09 embedded HTML/JS UI (vanilla + HTMX + cytoscape.js); pattern: `//go:embed all:ui` in server/cmd/serve.go (or new server/internal/ui/embed.go); chi mounts `http.FS(uiFS)` at /ui/*. Simpler than REQ-08 — no extraction needed
- REQ-10 MVP precision ≥ 60% measurement on spike-001 7-query baseline (after REQ-09 lands + real Rust /healthz round-trip smoke)
- Real spawn-and-restart smoke (REQ-07 acceptance #1-#3): blocked on `make build-core` producing a working Rust binary. With binary present, smoke (1) `serve --port 8080` starts both Go HTTP + extracted Rust, (2) Rust kill → 5s restart per supervisor backoff, (3) MCP query stdio round-trip
- 150 MB total size budget verification (REQ-08 acceptance #2): blocked on real Rust release build. Expected ~110-150 MB total
- ldflags coreVersion injection: replace hardcoded `"dev"` with `-ldflags "-X .../supervisor.coreVersion=v0.x.y"` (mechanism documented in embed.go line 36 comment)
- (deferred) Phase 2 storage micro-bench: optional Criterion harness on insert/lookup/vector top-5/FTS5; skip per ship-it directive
- (deferred) Nomic Embed Code shadow re-test on AU 5090 host (CPU segfault here)

### Blockers/Concerns

- **A2A v0.2 spec stability** — if Google releases A2A v1.0 with breaking changes during MVP, we eat a refactor. Low-medium risk, mitigated by spec being public + small surface area we use.
- **candle model weight distribution** — Phase 2 spike must decide: bundle in binary (~80-120MB total) vs HuggingFace cache on first run. Affects cold-start UX.
- **mcp-go maintainer health** — Phase 2 spike sub-task to confirm release cadence + responsiveness.
- **Nomic Embed Code shadow eval blocked** — no-GPU on this host (per spike R5c). Retest needed on AU 5090.

### Quick Tasks Completed

| # | Description | Date | Commits | Directory |
|---|-------------|------|---------|-----------|
| 260427-h71 | REQ-07: Go server scaffold (cobra+chi+mcp-go+supervisor) per ARCH §2/§3.5/§5.5 | 2026-04-27 | 8ff8e11 + 54f23b1 | [260427-h71-req-07-go-server-scaffold-cobra-chi-mcp-](./quick/260427-h71-req-07-go-server-scaffold-cobra-chi-mcp-/) |
| 260427-hoc | REQ-08: //go:embed Rust core binary plumbing (embed.go + extraction + Makefile EMBED_DIR) | 2026-04-27 | f5b6621 + 59b725b | [260427-hoc-req-08-go-embed-rust-core-binary-into-go](./quick/260427-hoc-req-08-go-embed-rust-core-binary-into-go/) |

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| storage | redb vs rusqlite+sqlite-vec | **In progress** (spike-008 active, see .planning/research/storage_backend.md) | 2026-04-26 |
| distribution | cargo install / GH releases / homebrew | Post-MVP | 2026-04-26 |
| memU integration | self-contained vs shared PG (REQ-04 inherits self-contained default) | Phase 5 (Bridge) | 2026-04-26 |
| GitHub repo visibility | public-from-Phase-1 vs wait-for-MVP | Phase 1 wrap-up | 2026-04-26 |

## Session Continuity

Last session: 2026-04-27 (Phase 3 third session — REQ-08 //go:embed plumbing done; 9/9 invariants verified; mid-execution handoff demonstrated executor-returns-partial recovery is viable when orchestrator picks up cleanly)
Stopped at: Phase 3 REQ-08 ✓ plumbing-only (real Rust binary smoke deferred); REQ-09 UI //go:embed next
Resume file: progress.txt (root) + this STATE.md
Next-session entry: user says "继续" or "REQ-09" → //go:embed UI bundle into Go server. Pattern mirrors REQ-08 (`//go:embed all:ui` in same package as chi router) but simpler: chi serves via `http.FS(uiFS)`, no extraction. Splice point: server/cmd/serve.go line where /ui/* placeholder responds 200 "UI not embedded yet"

## Linear cross-reference

Linear project ID `5c8f1e26-c63d-4372-bcd9-4d94d04788a3` — renamed to **CodeNexus** on 2026-04-26. 44 issues now (XAR-224 → XAR-266 + XAR-270 new IPC spike) across 8 milestones.

**Synced state (2026-04-26 apply)**:

- Decisions milestone XAR-224 → 231: all 8 closed to Done with per-issue decision verdicts in description
- XAR-238 (rmcp spike): Canceled, replacement note points to mcp-go
- XAR-270 (new): Phase 2 (SPEC Phase 0) spike — A2A endpoint + Go-Rust IPC over A2A protocol, in Phase 0 Spike milestone

GSD `.planning/` canonical for phase-internal state; Linear canonical for milestone-level tracking. Sync script kept at `D:/projects/codenexus/scripts/linear_sync_decisions.py` for audit (single-shot, won't re-run).
