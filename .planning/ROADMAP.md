# Roadmap: CodeNexus

## Overview

Six phases from clean-room design through ecosystem reach. Phase 1-2 are pre-MVP foundation (design + spike) where we de-risk the riskiest stack choices before writing real product code. Phase 3 ships the MVP that beats GitNexus's 43% precision baseline. Phase 4-6 add parity features (multi-language, git overlay, security/pattern detection), then bridge to obsidian-llm-wiki vault layer, then reach (plugin system, broader integrations).

Origin SPEC uses Phase -1/0/1/2/3/4; GSD uses 1-6. Mapping in PROJECT.md §"Phase numbering note".

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (e.g., 2.1): Urgent insertions (marked with INSERTED)

- [ ] **Phase 1: Foundation Design** — Clean-room ARCHITECTURE.md (no GitNexus reference); A2A endpoint schema draft; Rust/Go IPC boundary
- [ ] **Phase 2: Stack Spike** — 6 sub-spikes (tree-sitter / storage / embedder / A2A IPC / mcp-go / gix); GO/NO-GO per component
- [ ] **Phase 3: MVP** — Working binary, ≥60% precision on spike-001 7 NL queries
- [ ] **Phase 4: Parity** — Multi-language, multi-repo registry, git overlay (gix), pattern detection (CodeFlow port), security scanners, health score
- [ ] **Phase 5: Bridge** — Markdown wiki-link graph (Obsidian-aware); three-way viz: code ↔ vault ↔ memU memory
- [ ] **Phase 6: Reach** — Plugin system; multi-tenant if needed; broader IDE/agent integrations

## Phase Details

### Phase 1: Foundation Design
**Goal**: Produce `docs/ARCHITECTURE.md` (clean-room, no GitNexus reference) covering Rust core / Go server / A2A IPC boundary / data layer / clean-room policy enforcement
**Depends on**: Nothing (first phase)
**Requirements**: REQ-06, REQ-07
**Success Criteria** (what must be TRUE):
  1. `docs/ARCHITECTURE.md` exists with no code-level reference to GitNexus internals
  2. A2A endpoint schema (request/response shapes for `index_repo`, `query`, `get_symbol`, `list_callers`) drafted as JSON examples
  3. Rust core ↔ Go server interface boundary documented (which side owns what state)
  4. Clean-room policy section explicitly states 24h gap rule between studying GitNexus and implementing CodeNexus
**Plans**: 1 plan

Plans:
- [ ] 01-01: Write ARCHITECTURE.md from scratch using own design notes (no GitNexus source open during writing)

### Phase 2: Stack Spike
**Goal**: Validate each component (tree-sitter / storage / candle / A2A IPC / mcp-go / gix) end-to-end on a 50-file TS corpus; produce GO/NO-GO report per component
**Depends on**: Phase 1
**Requirements**: REQ-01, REQ-03, REQ-04, REQ-06, REQ-07
**Success Criteria** (what must be TRUE):
  1. Each spike has a written GO/NO-GO with command output evidence
  2. Storage choice (redb vs rusqlite+sqlite-vec) decided with bench data
  3. A2A endpoint roundtrip < 5ms p99 on localhost demonstrated
  4. mcp-go serves at least one tool that successfully proxies an A2A call to Rust core
  5. candle loads Snowflake/BERT model and produces embedding vectors for 100+ symbols
  6. No spike returns NO-GO without an alternative documented (or scope cut to PROJECT.md Out of Scope)
**Plans**: 6 plans (one per spike)

Plans:
- [ ] 02-01: tree-sitter Rust crate parsing 50-file TS corpus → SymbolNode[]
- [ ] 02-02: Storage shootout — redb vs rusqlite+sqlite-vec on 10K embeddings + FTS5 query mix
- [ ] 02-03: candle embedder loading Snowflake/BERT, batch embed 1000 symbols, measure cold-start + throughput
- [ ] 02-04: axum A2A endpoint (POST /tasks/send + GET /tasks/{id}) + Go A2A client roundtrip; lifecycle (spawn / healthcheck / restart)
- [ ] 02-05: mark3labs/mcp-go serves one tool that wraps an A2A call to Rust core
- [ ] 02-06: gix git overlay reads blame/log/diff on a real repo

### Phase 3: MVP
**Goal**: Ship runnable single fat-binary CodeNexus that beats GitNexus 1.6.3's 43% top-5 precision baseline on spike-001's 7 NL queries
**Depends on**: Phase 2
**Requirements**: REQ-01, REQ-02, REQ-03, REQ-04, REQ-05, REQ-06, REQ-07, REQ-08, REQ-09, REQ-10
**Success Criteria** (what must be TRUE):
  1. `make build` produces single `bin/codenexus` binary (Go embeds Rust binary via `//go:embed`)
  2. `./codenexus index <repo>` parses TS repo, builds CALLS edge graph, embeds all symbols
  3. `./codenexus query "<text>"` returns top-K results via hybrid BM25+vector RRF fusion
  4. `./codenexus serve` starts HTTP server with cytoscape.js graph viewport at localhost
  5. `./codenexus mcp` starts MCP stdio server with at least `query` and `get_symbol` tools
  6. Top-5 precision ≥ 60% on the 7 spike-001 NL queries (vs 43% GitNexus baseline)
  7. Rust core A2A endpoint is reachable from any A2A client (not just our Go server)
**Plans**: 5 plans

Plans:
- [ ] 03-01: Rust core — parser + storage + embedder pipeline, indexing flow
- [ ] 03-02: Rust core — axum A2A server with index/query/get_symbol/list_callers endpoints
- [ ] 03-03: Go server — A2A client + chi HTTP API + cobra CLI subcommands
- [ ] 03-04: Go server — mcp-go MCP stdio handler wrapping A2A calls
- [ ] 03-05: UI — embedded HTML/JS bundle with search box + cytoscape graph view + acceptance benchmark

### Phase 03.6: Candle in-process embedder migration (qwen3-embedding-0.6b GGUF replacing ollama HTTP) -- Phase 3 TRULY CLOSED unblock (INSERTED)

**Goal:** [Urgent work - to be planned]
**Requirements**: TBD
**Depends on:** Phase 3
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd-plan-phase 03.6 to break down)

### Phase 4: Parity
**Goal**: Reach functional parity with GitNexus for the features users actually use; port CodeFlow MIT modules under Apache 2.0 with NOTICE attribution
**Depends on**: Phase 3
**Requirements**: TBD (REQ-11+ to be added during Phase 4 discuss)
**Success Criteria** (what must be TRUE):
  1. Multi-language tree-sitter (TS + Python + Go + Rust at minimum)
  2. Multi-repo registry: one CodeNexus instance can index N repos with separate graphs
  3. Git overlay (blame, log, diff) via gix reachable from query results
  4. Pattern detection (singleton/factory/etc) ported from CodeFlow MIT
  5. Security scanners (secrets/SQLi/eval) ported from CodeFlow MIT
  6. Code health score (cyclomatic complexity / cohesion / coupling) computed per file
  7. NOTICE file lists all CodeFlow attribution lines
**Plans**: TBD (likely 5-7 plans)

### Phase 5: Bridge
**Goal**: Connect CodeNexus to obsidian-llm-wiki vault layer; produce three-way viz (code ↔ vault ↔ memU memory); resolve memU integration question (self-contained vs shared PG)
**Depends on**: Phase 4
**Requirements**: TBD
**Success Criteria** (what must be TRUE):
  1. Markdown wiki-link graph extracted from Obsidian vault and stored in CodeNexus graph
  2. Cross-domain query: "show me code that implements concepts mentioned in this vault note"
  3. memU integration decision: stay self-contained, share PG, or hybrid
  4. Three-way viz: code symbols ↔ vault concepts ↔ memU memory items in one cytoscape view
**Plans**: TBD (likely 2-3 plans)

### Phase 6: Reach
**Goal**: Open CodeNexus to broader integrations — plugin system for custom analyzers/embedders, multi-tenant support if needed, IDE/agent ecosystem documentation
**Depends on**: Phase 5
**Requirements**: TBD
**Success Criteria** (what must be TRUE):
  1. Plugin system spec'd: third parties can add new tree-sitter languages, embedders, or analyzers without forking
  2. At least one external plugin demo (e.g., custom embedder)
  3. A2A agent card published — CodeNexus discoverable in any A2A registry
  4. README + docs/ ready for v1.0 announcement
**Plans**: TBD (likely 2-4 plans)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation Design | 0/1 | Not started | - |
| 2. Stack Spike | 0/6 | Not started | - |
| 3. MVP | 0/5 | Not started | - |
| 4. Parity | 0/TBD | Not started | - |
| 5. Bridge | 0/TBD | Not started | - |
| 6. Reach | 0/TBD | Not started | - |
