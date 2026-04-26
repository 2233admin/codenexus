# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-26)

**Core value:** Top-5 NL search precision ≥ 60% on the spike-001 query set, exposed as an open A2A endpoint that any agent can call.
**Current focus:** Phase 1 (Foundation Design) — pre-execution

## Current Position

Phase: 1 of 6 (Foundation Design)
Plan: — of 1 in current phase
Status: Ready to plan (run `/gsd-discuss-phase 1` then `/gsd-plan-phase 1`)
Last activity: 2026-04-26 — decision-closure session, .planning/ initialized

Progress: [░░░░░░░░░░] 0%

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

### Pending Todos

None yet — Phase 1 will refine.

### Blockers/Concerns

- **A2A v0.2 spec stability** — if Google releases A2A v1.0 with breaking changes during MVP, we eat a refactor. Low-medium risk, mitigated by spec being public + small surface area we use.
- **candle model weight distribution** — Phase 2 spike must decide: bundle in binary (~80-120MB total) vs HuggingFace cache on first run. Affects cold-start UX.
- **mcp-go maintainer health** — Phase 2 spike sub-task to confirm release cadence + responsiveness.

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| storage | redb vs rusqlite+sqlite-vec | Phase 2 spike will decide | 2026-04-26 |
| distribution | cargo install / GH releases / homebrew | Post-MVP | 2026-04-26 |
| memU integration | self-contained vs shared PG (REQ-04 inherits self-contained default) | Phase 5 (Bridge) | 2026-04-26 |
| GitHub repo visibility | public-from-Phase-1 vs wait-for-MVP | Phase 1 wrap-up | 2026-04-26 |

## Session Continuity

Last session: 2026-04-26 — decision-closure + .planning/ init
Stopped at: .planning/ scaffolded, ready to enter Phase 1 (Foundation Design)
Resume file: None (run `/gsd-discuss-phase 1` to start Phase 1)

## Linear cross-reference

Linear project ID `5c8f1e26-c63d-4372-bcd9-4d94d04788a3` — renamed to **CodeNexus** on 2026-04-26. 44 issues now (XAR-224 → XAR-266 + XAR-270 new IPC spike) across 8 milestones.

**Synced state (2026-04-26 apply)**:
- Decisions milestone XAR-224 → 231: all 8 closed to Done with per-issue decision verdicts in description
- XAR-238 (rmcp spike): Canceled, replacement note points to mcp-go
- XAR-270 (new): Phase 2 (SPEC Phase 0) spike — A2A endpoint + Go-Rust IPC over A2A protocol, in Phase 0 Spike milestone

GSD `.planning/` canonical for phase-internal state; Linear canonical for milestone-level tracking. Sync script kept at `D:/projects/codenexus/scripts/linear_sync_decisions.py` for audit (single-shot, won't re-run).
