---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: phase_3_active
stopped_at: Phase 3 REQ-09 UI //go:embed ✓ (commits ec3849e + dfdcb95); REQ-10 precision measurement next
last_updated: "2026-04-27T05:30:00.000Z"
last_activity: 2026-04-27 — Completed quick task 260427-i0c: REQ-09 UI bundle (vanilla JS + cytoscape.js, no build step, option B git mv); 12/12 plan invariants verified. Mid-execution handoff again (executor returned partial, orchestrator wrote SUMMARY.md + STATE update). Also: confidence uplift (4af9f4d) + Software 3.0 strategic reframe in PROJECT.md (7f6f44d + d98b16c)
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
**Current focus:** Phase 3 (MVP) — REQ-06 ✓, REQ-07 ✓, REQ-08 ✓, REQ-09 ✓ (all plumbing-only, real binary + browser smoke deferred); REQ-10 precision measurement next

## Current Position

Phase: 3 of 6 (MVP)
Plan: REQ-06 + REQ-07 + REQ-08 + REQ-09 done (4 of 5)
Status: REQ-10 precision measurement next (≥ 60% top-5 on spike-001 7 queries vs GitNexus 1.6.3 baseline 43%); requires running stack end-to-end (real Rust binary + indexed repo + smoke against live /healthz)
Last activity: 2026-04-27 — Phase 3 REQ-09 UI bundle green (commits ec3849e + dfdcb95, 12/12 invariants verified, option B git mv'd ui/ → server/internal/ui/)

Progress: [█████░░░░░] 44% (Phase 1 closed + REQ-06 + REQ-07 + REQ-08 + REQ-09 of 5 in Phase 3)

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

- REQ-10 MVP precision ≥ 60% measurement on spike-001 7-query baseline (after real Rust /healthz round-trip smoke against running stack); GitNexus 1.6.3 baseline 43%
- Real spawn-and-restart smoke (REQ-07 acceptance #1-#3): blocked on `make build-core` producing a working Rust binary. With binary present, smoke (1) `serve --port 8080` starts both Go HTTP + extracted Rust, (2) Rust kill → 5s restart per supervisor backoff, (3) MCP query stdio round-trip
- Real browser load smoke (REQ-09 acceptance #1-#3): same blocker; verify `localhost:8080/` redirects to /ui/, search returns 4-score-column results, list_callers renders cytoscape graph with confidence color bands
- 150 MB total size budget verification (REQ-08 acceptance #2): blocked on real Rust release build. Expected ~110-150 MB Go + cytoscape 374 KB + UI text < 1 MB
- ldflags coreVersion injection: replace hardcoded `"dev"` with `-ldflags "-X .../supervisor.coreVersion=v0.x.y"` (mechanism documented in embed.go line 36 comment)
- (deferred) Phase 2 storage micro-bench: optional Criterion harness on insert/lookup/vector top-5/FTS5; skip per ship-it directive
- (deferred) Nomic Embed Code shadow re-test on AU 5090 host (CPU segfault here)
- (Phase 4+ tactical, see PROJECT.md): Leiden community detection (~30 lines petgraph), confidence-as-Leiden-weight (free), spike → core/ promotion or alias
- (Phase 4+ strategic, see PROJECT.md, Software 3.0 reframe): agent behavioral alignment (target ≤5% graph-tool miss rate vs CodeCompass 58% baseline), cross-session codebase understanding accumulation via memU integration, architectural decision semantic indexing (query_constraints A2A operation)

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
| (no-id) | Edge confidence on list_callers + Makefile/STATE realign + Software 3.0 reframe in PROJECT.md | 2026-04-27 | 4af9f4d + 7f6f44d + d98b16c | (no quick dir — micro-task + doc-only commits) |
| 260427-i0c | REQ-09: UI bundle (vanilla JS + cytoscape.js, no build step, option B git mv) | 2026-04-27 | ec3849e + dfdcb95 | [260427-i0c-req-09-go-embed-ui-bundle-vanilla-js-htm](./quick/260427-i0c-req-09-go-embed-ui-bundle-vanilla-js-htm/) |

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| storage | redb vs rusqlite+sqlite-vec | **In progress** (spike-008 active, see .planning/research/storage_backend.md) | 2026-04-26 |
| distribution | cargo install / GH releases / homebrew | Post-MVP | 2026-04-26 |
| memU integration | self-contained vs shared PG (REQ-04 inherits self-contained default) | Phase 5 (Bridge) | 2026-04-26 |
| GitHub repo visibility | public-from-Phase-1 vs wait-for-MVP | Phase 1 wrap-up | 2026-04-26 |

## Session Continuity

Last session: 2026-04-27 (Phase 3 fourth session — REQ-09 UI done + confidence uplift + Software 3.0 strategic reframe in PROJECT.md; 12/12 plan invariants verified; second mid-execution handoff successfully recovered; 4-of-5 Phase 3 done in single session)
Stopped at: Phase 3 REQ-09 ✓ scaffold (real browser smoke deferred); REQ-10 precision measurement is the last Phase 3 gate
Resume file: progress.txt (root) + this STATE.md
Next-session entry: user says "继续" or "REQ-10" → blocked on `make build-core` producing a working Rust binary first. Order: (a) build Rust release binary from experiments/poc-retrieval, (b) `make build` produces fat-binary bin/codenexus(.exe), (c) run codenexus serve, (d) execute spike-001 7-query eval harness against the running stack, (e) compare top-5 precision vs GitNexus 1.6.3 baseline 43%. If geq 60% → Phase 3 closes. Phase 4+ backlog (tactical + Software 3.0 strategic) is documented in PROJECT.md

## Linear cross-reference

Linear project ID `5c8f1e26-c63d-4372-bcd9-4d94d04788a3` — renamed to **CodeNexus** on 2026-04-26. 44 issues now (XAR-224 → XAR-266 + XAR-270 new IPC spike) across 8 milestones.

**Synced state (2026-04-26 apply)**:

- Decisions milestone XAR-224 → 231: all 8 closed to Done with per-issue decision verdicts in description
- XAR-238 (rmcp spike): Canceled, replacement note points to mcp-go
- XAR-270 (new): Phase 2 (SPEC Phase 0) spike — A2A endpoint + Go-Rust IPC over A2A protocol, in Phase 0 Spike milestone

GSD `.planning/` canonical for phase-internal state; Linear canonical for milestone-level tracking. Sync script kept at `D:/projects/codenexus/scripts/linear_sync_decisions.py` for audit (single-shot, won't re-run).
