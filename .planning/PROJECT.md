# CodeNexus

## What This Is

Code + knowledge graph tool. A Rust core (parser/embedder/storage/git overlay) that exposes itself as a network-addressable A2A agent, fronted by a Go service layer (HTTP + MCP + CLI) that serves a browser-based viz UI. Built for solo devs and small teams who want code search and graph navigation that beats grep without inviting commercial vendor lock-in. Apache 2.0 licensed, single fat-binary distribution.

## Core Value

**Top-5 NL search precision ≥ 60% on the spike-001 query set, exposed as an open A2A endpoint that any agent can call.** Everything else (UI polish, multi-language support, plugin system, etc.) is secondary to this two-part claim being true.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

(None yet — pre-MVP.)

### Active

<!-- Current scope. Building toward these. -->

- [ ] **REQ-01** — tree-sitter pipeline parses TypeScript repo into SymbolNode[] (Functions, Classes, Methods, Interfaces, Type Aliases, Enums, Top-level Constants/Lexical Declarations, Arrow-fn Variables, Files). _Refined 2026-04-27 from poc-retrieval Round 1: original wording omitted Interfaces/TypeAliases/Enums/Constants and made A4/A6/A8 architecturally unanswerable._
- [ ] **REQ-02** — Symbol graph: 4 edge kinds (Calls + Imports + Implements + Extends). Overrides deferred to Phase 3+. Resolver = naive 3-step (same-file → import-file → global-unique). _Scope expanded 2026-04-27 from CALLS-only after upstream review; details in REQUIREMENTS.md REQ-02_
- [ ] **REQ-03** — candle embedder produces vectors for all symbols (Snowflake/BERT-family, no external API)
- [ ] **REQ-04** — Storage layer: redb OR rusqlite+sqlite-vec (decided in Phase 2 spike)
- [ ] **REQ-05** — Hybrid search: SQLite FTS5 BM25 + vector cosine + RRF fusion
- [ ] **REQ-06** — Rust core exposes A2A protocol endpoint (POST /tasks/send + GET /tasks/{id}) over localhost HTTP via axum
- [ ] **REQ-07** — Go server is A2A client to Rust core; serves chi HTTP API + mark3labs/mcp-go MCP stdio + cobra CLI
- [ ] **REQ-08** — Single fat-binary: Go binary embeds Rust binary via `//go:embed`, spawns it on `serve`
- [ ] **REQ-09** — Embedded HTML/JS UI (vanilla JS + HTMX + cytoscape.js) served at localhost; search box + graph view
- [ ] **REQ-10** — MVP acceptance: top-5 precision ≥ 60% on spike-001's 7 NL queries (vs GitNexus 1.6.3 baseline of 43%)

### Out of Scope

<!-- Explicit boundaries. Includes reasoning to prevent re-adding. -->

- **Python in any layer** — anti-scope per origin SPEC; ecosystem fragmentation, deployment overhead. Even build/plugin layers stay Rust+Go.
- **Multi-language tree-sitter (MVP)** — TS only first; multi-lang lands Phase 4 (Parity).
- **Pattern detection / security scanners (MVP)** — defer to Phase 4; CodeFlow MIT will be ported under Apache 2.0 with NOTICE attribution.
- **Markdown wiki-link graph (MVP)** — defer to Phase 5 (Bridge); needs the obsidian-llm-wiki integration.
- **Tauri native window** — considered, rejected. axum/Go-served browser UI is sufficient and avoids cross-platform packaging cost.
- **Pure-Rust UI / WASM frontend (leptos/dioxus)** — viz ecosystem in Rust WASM too thin; cytoscape.js + vanilla JS is pragmatic.
- **rmcp (Rust MCP SDK)** — replaced by mark3labs/mcp-go; rmcp maturity was a Phase 2 high-risk gate, killed by going Go for the service layer.
- **Replacing memU / obsidian-llm-wiki** — CodeNexus owns code+git domain; vault layer stays in obsidian-llm-wiki; integration via Phase 5 Bridge.
- **VS Code extension** — separate project if ever; CodeNexus exposes MCP for IDE integration, that's enough.
- **GPL/AGPL license** — would conflict with A2A "open agent in any mesh" strategy; enterprise legal teams routinely ban GPL deps.
- **Cargo workspace / sub-crates inside core/ (MVP)** — single binary, single crate; restructure to workspace only if Phase 4 demands it.
- **Embedding GitNexus PolyForm code** — clean-room policy; designs studied, code never copied or referenced. CodeFlow MIT may be ported (Apache 2.0 upgrade, with attribution).

## Differentiation vs Prior Art

<!-- "What we have that GitNexus 1.6.3 / CodeFlow don't." Real moat, not marketing. -->
<!-- Each item must point to in-repo evidence — no aspirational claims. -->

- **Graded LLM-judge eval pipeline** — GitNexus has zero eval infrastructure; every config change is a coin flip. CodeNexus has spike-001 7-query baseline + R3/R4/R5/R6/R6c LLM-judge methodology rounds documented in `experiments/poc-retrieval/eval/` (commit `8bf6a4a` axis-3 graph 23.3% > hand 15% > retrieval 0% with N≥3 seeds, EVAL Rule 6). When we change retrieval, we know within minutes if it helped. Compounds across the project lifetime — every other moat below was discoverable because of this.
- **Parameterized RRF fusion** — GitNexus hardcodes its BM25+vector blend. CodeNexus reads `config/recipe.yaml` for `bm25_weight` / `vector_weight` / `rrf_k`, exposed as `OperationRequest::Query` args (see `experiments/poc-retrieval/src/search.rs`). Tunable per-query for ablation; Phase 4 can ship per-repo recipes.
- **Incremental indexing readiness** — GitNexus docs explicitly mark "incremental indexing is on the roadmap" (i.e. not built). CodeNexus already has `Store::list_symbols_by_file` (`experiments/poc-retrieval/src/storage.rs`) — the primitive needed to diff files since last index and update only deltas. Phase 4 wires it into a watcher; the data structure is ready today.
- **Edge-confidence on caller results** — Calls edges carry `confidence: f64` (resolver step 1=1.0 direct AST, step 2=0.95 import-resolved, step 3=0.9 same-file fallback) and `list_callers` surfaces fold-take-max per (caller, target) pair (commit `4af9f4d`, ARCH §3.5.4). Agents can distinguish "definitely calls" from "might call" — neither GitNexus nor CodeFlow exposes this.

## Phase 4+ Backlog (committed but not scheduled)

<!-- Things we will build, design space already locked. Differs from "Out of Scope" (never) and "Active" (now). -->

- **Leiden community detection** — `petgraph` Rust binding (~30 lines) in graph builder. Inputs: existing edge list. Outputs: community labels for graph clustering. Reuses `confidence: f64` directly as edge weight — zero added cost since the field already exists from REQ-06 spike. Useful for "show me the call graph clusters" queries and downstream PPR teleport-set construction.
- **Confidence-as-Leiden-weight** — already plumbed (see Differentiation #4). Phase 4 Leiden flips a switch, doesn't add a column.
- **Spike → core/ promotion or alias** — `core/` is currently a 13-line `println!` placeholder superseded by `experiments/poc-retrieval/` since REQ-06. Cleanup options: (a) cargo workspace with `core` aliasing `poc-retrieval`, (b) `git mv experiments/poc-retrieval core` and delete the placeholder, (c) leave as-is with STATE.md note (current state). Decision deferred; not blocking MVP.

## Context

CodeNexus emerged from spike 001 (`obsidian-llm-wiki/.planning/spikes/001-embed-quality-on-code/`) which measured GitNexus 1.6.3's hybrid search at 43% top-5 precision over 7 NL queries — well below usable threshold. Q5 (negative test, "rate limiting middleware" with no such concept in corpus) returned 6 LIMIT-named constants, confirming pure keyword fallback with no semantic discrimination. Snowflake-arctic-embed-xs (22M params) is the bottleneck embedder.

GitNexus is licensed PolyForm Noncommercial 1.0.0 — non-OSS, no sublicense, no commercial use. Patching upstream propagates these terms into anything we build; copying source is a license violation. CodeFlow (github.com/braedonsaunders/codeflow) covers the visualization + git overlay layer GitNexus lacks but is itself shallow on data layer; CodeFlow is MIT, freely portable.

Decision (2026-04-25, refined 2026-04-26): refactor as new tool. Rust core for parser/embedder/storage (clean-room, no GitNexus reference) + Go service layer (HTTP/MCP/CLI). A2A protocol as IPC + service interface (Rust core is a network-addressable agent from day one, not just a private library). Apache 2.0 license for explicit patent grant + ecosystem compatibility.

## Constraints

- **Tech stack**: Rust 2024 (core), Go 1.23+ (server), vanilla JS + HTMX + cytoscape.js (UI). No Python in any layer. No React/Vue/Svelte. No Tauri/Electron. No express/koa/fastify in TS land (we don't have a TS layer).
- **Distribution**: Single fat-binary via Go `//go:embed` of Rust binary. End user runs `./codenexus serve` — zero install dependencies.
- **License**: Apache 2.0. Explicit patent grant + trademark protection + NOTICE attribution. Locked decision (2026-04-26).
- **Performance baseline (MVP acceptance)**: top-5 precision ≥ 60% on spike-001 7 queries; A2A localhost roundtrip < 5ms p99 (excluding actual query work).
- **Embedder default**: candle (Snowflake/BERT family), zero external dependency. ollama-rs and async-openai are pluggable alternatives, not defaults.
- **Storage budget**: < 5x source code size for the graph DB on a typical TS repo (vs GitNexus's ~10x).
- **Clean-room separation**: GitNexus source must NOT be open while implementing CodeNexus core. CodeFlow may be referenced and ported (MIT → Apache 2.0 attribution in NOTICE).
- **A2A spec compliance**: Rust core endpoint follows Google A2A v0.2 spec (POST /tasks/send + GET /tasks/{id} polling, optional SSE stream). Spec stability is a constraint (re-evaluate if A2A v1.0 breaks compat).
- **Repo layout**: core/ (Rust crate), server/ (Go module), ui/ (static assets), docs/ (origin-spec.md + future), .planning/ (GSD), Makefile (build entry).

## Key Decisions

<!-- Decisions that constrain future work. Add throughout project lifecycle. -->

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| **Pure Rust → Rust core + Go service layer** (2026-04-26) | Go MCP SDK (mark3labs/mcp-go) is mature; rmcp Rust SDK was the Phase 2 high-risk gate. Splitting kills that risk. | — Pending (validated by Phase 2 spike) |
| **stdio JSON-RPC IPC → A2A protocol over localhost HTTP** (2026-04-26) | A2A makes Rust core a network-addressable agent; any A2A client (远程 agent / Python script / 其他模型) can call it directly. Single interface, no private/public split. ~0.1ms localhost framing overhead is negligible. | — Pending (validated by Phase 2 spike) |
| **License: MIT → Apache 2.0** (2026-04-26) | Explicit patent grant + trademark protection + NOTICE clause. Same enterprise/agent-mesh adoption profile as MIT but with real legal teeth. GPL/AGPL would block A2A "open agent" strategy. | ✓ Good |
| **Embedder default: ollama-rs → candle** (2026-04-26) | Zero external dependency; users don't need Ollama installed. Costs ~50-80MB binary size + cold-start latency, both acceptable. | — Pending (validated by Phase 2 spike) |
| **UI: pivoted Tauri → axum/chi-served web** (2026-04-26) | Browser UI keeps single fat-binary distribution simple; Tauri's cross-platform packaging cost not justified for MVP. | ✓ Good |
| **Naming: Stitch (working) → CodeNexus (locked)** (2026-04-26) | "Stitch" was a placeholder from the user's "缝合" word; CodeNexus is more descriptive. Accepted SEO/branding risk of GitNexus same-root similarity. | ✓ Good |
| **Project home: D:/projects/codenexus/ (new repo)** (2026-04-26) | Clean separation, independent Cargo workspace + git history. Avoids monorepo coupling with obsidian-llm-wiki. | ✓ Good |
| **memU integration: self-contained store** (2026-04-26) | Phase 5 (Bridge) may revisit fused recall via shared PG; for now, simpler to own the storage layer entirely. | — Pending (revisit Phase 5) |
| **Storage backend: deferred to Phase 2 spike** | redb (pure KV) vs rusqlite+sqlite-vec (SQL+vector+FTS5 in one). Bench-driven choice. | — Pending |
| **Clean-room separation from GitNexus** | PolyForm Noncommercial 1.0.0 forbids sublicense; copying any code propagates non-OSS terms. Solo dev clean-room: never have GitNexus source open while implementing CodeNexus. | ✓ Locked |

## Phase numbering note

Origin SPEC uses Phase -1 / 0 / 1 / 2 / 3 / 4 (where -1 and 0 are pre-MVP). GSD convention uses integer phases starting at 1. Mapping in this project:

| SPEC | GSD | Name |
|---|---|---|
| -1 | 1 | Foundation Design |
| 0 | 2 | Stack Spike |
| 1 | 3 | MVP |
| 2 | 4 | Parity |
| 3 | 5 | Bridge |
| 4 | 6 | Reach |

GSD numbering is canonical inside `.planning/`; SPEC numbering remains in `docs/origin-spec.md` for historical reference.

---
*Last updated: 2026-04-26 after decision-closure session*
