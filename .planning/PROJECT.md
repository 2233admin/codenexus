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

### Tactical (incremental wins on existing surface)

- ~~**Leiden community detection** — `petgraph` Rust binding (~30 lines) in graph builder.~~ **[PROMOTED to Phase 04.1 Graph Clustering and Evolution Layer, 2026-04-28]** — original tactical scope (static Leiden over existing edge list, reusing `confidence: f64` as edge weight) survives intact as the foundation slice of Phase 04.1; the dedicated phase additionally bundles Static Infomap call-flow refinement, DF-Leiden incremental layer over Phase 4's file-watcher harness, CoDÆN-NeGMA evaluation harness, and A2A `query_clusters` / `query_evolution` ops. See `.planning/phases/codenexus-04.1-graph-clustering-and-evolution-layer/` for canonical scope.
- ~~**Confidence-as-Leiden-weight** — already plumbed (see Differentiation #4). Phase 4 Leiden flips a switch, doesn't add a column.~~ **[PROMOTED to Phase 04.1, 2026-04-28]** — same plumbing observation, applies to Phase 04.1's Leiden + DF-Leiden slices.
- **Spike → core/ promotion or alias** — `core/` is currently a 13-line `println!` placeholder superseded by `experiments/poc-retrieval/` since REQ-06. Cleanup options: (a) cargo workspace with `core` aliasing `poc-retrieval`, (b) `git mv experiments/poc-retrieval core` and delete the placeholder, (c) leave as-is with STATE.md note (current state). Decision deferred; not blocking MVP.
- ~~**Embedder runtime: ollama → candle in-process (P0, blocks full FSC index)**~~ — **[CLOSED via Phase 03.6 commit `67320ec`]**. Shipped 2026-04-28. Pivoted from GGUF cheap path (proved infeasible: `quantized_qwen3::forward()` returns logits via `lm_head` not hidden states — see ARCH §9.10 negative rationale block) to safetensors via `fastembed::Qwen3TextEmbedding` (which wraps `candle-transformers::models::qwen3::Model`; direct candle held in reserve as fallback, not needed). Cosine equivalence on 30-query set: mean=0.9994, p10=0.9993; REQ-10 B1-B7 post-migration: 67.9% (literal 60% gate PASS, byte-identical to ollama baseline); F1-F10 generous-denominator: 72% (gate ≥50% PASS); fsc.db FULL re-index 2307 symbols clean (Phase 3.5b's 132/2307 burst-hang resolved). Phase 03.6 SUMMARY: `.planning/phases/03.6-candle-in-process-embedder-migration-qwen3-embedding-0-6b-gg/03.6-SUMMARY.md`.
- ~~**Cold-start / offline UX (P1, Phase 4 first-step)**~~ — **[CLOSED via Phase 4 first slice 2026-04-28 — code-complete; full E2E runtime validation DEFERRED, see hf-hub Windows entry below]** — open-source first-impression cluster, elevated from P2/P3 in 03.6-SUMMARY honest-gap-list per 2026-04-28 user directive: "第一次运行时如果 HF 下载失败或者网络不通,用户看到的是什么? 这不是 P3, 这是第一印象问题。" Three coupled sub-tasks, addressed together in the Phase 4 PLAN's first slice (BEFORE multi-language / multi-repo / git overlay user-power features):

  1. **HF Hub revision pinning.** `fastembed::Qwen3TextEmbedding::try_new` currently uses default revision (no commit-pin). Silent re-uploads on HuggingFace invalidate our cache and re-download without notice — supply-chain drift risk. Fix: expose `revision: "<sha>"` parameter in `Qwen3TextInitOptions` if the crate supports it (verify by `cargo doc` first); else vendor the safetensors blob directly into the build. When pinning lands, append a new ARCH §9.8 history row with the locked revision SHA (treat revision pin as version-hash-affecting per §9.8 protocol — same discipline as model_id / dim / prefix changes).

  2. **First-run download UX.** Currently silent until HF cache hit. New users on a clean machine wait 30-60s for ~1.2GB download with no progress indication and unclear failure mode if Clash/proxy is down. Fix: explicit startup print on first-run model load (`[embedder] first-run download starting (~1.2GB to ~/.cache/huggingface/) — this can take 30-60s on broadband; check internet to huggingface.co if it stalls`); explicit failure-mode print on network error pointing to recovery docs (sub-task 3). Use `tracing::info!` not `eprintln!` (project already wires tracing elsewhere).

  3. **Offline / Clash-down recovery docs.** Write `docs/embedder-offline-bootstrap.md` covering: manual safetensors download path (HF Hub URL + sha256 + target cache location), `HF_HOME` / `HF_HUB_CACHE` env-var pre-seeding, `HF_HUB_OFFLINE=1` mode usage, and a Clash-China-down recovery walkthrough (CN users are the first real-world failure mode). README.md "Quick start" must link to this doc — first-run failure path needs to be one click away from the install instructions, not buried.

  **Why P1 not P3.** Open-source tools live or die on first-run UX. A user who hits an opaque HF download failure on their first run and can't find recovery docs walks away — they don't open an issue, they just close the tab; the signal is invisible. P0 (candle migration) shipped engineering correctness; P1 here ships the *first-impression contract* without which adoption stalls. Phase 4 PLAN addresses this BEFORE multi-language tree-sitter / multi-repo registry / git overlay / CodeFlow port — those are user-power features for users we already have, this is user-onboarding gating for users we don't yet have.

  **Heuristic boundary** (general principle, applies beyond this project): internal tools / single-author projects → first-run UX can stay P3 (fix when you hit it next time). Open-source / external-distribution → first-run UX is P1. CodeNexus is the latter (Apache 2.0, GitHub `2233admin/codenexus`, intended for adoption per PROJECT.md Core Value).

- **Production-grade embedding resilience (P2)** — **[PARTIALLY CLOSED via Phase 4 first slice 2026-04-28 — R4 (server.rs:198 A2A IndexRepo counter + envelope override) + R5 (search.rs:31 embed_query 2-attempt 250ms budget) + fault injection scaffolding (CODENEXUS_EMBED_FAIL=always|once|after_N) all landed in Wave 2 commits `d09d3a9`/`4c7694d`/`fafda6e`. Bound check `1..=MAX_RAISED_THRESHOLD (=100)` named const. EmbedError enum + classified retry policies REMAIN P2 deferred (Q5=B locked, Phase 4 first slice scope: mechanical patch only).]** Phase 3.5b micro-slice landed `--max-consecutive-fail` + retry-with-backoff on the **CLI Index path only** (`main.rs:156`). Two more call sites have the same `embedder.embed()` pattern and currently inherit retry but lack the fail counter / structured abort: `server.rs:198` (A2A endpoint Index handler — same silent-partial-state risk as fixed CLI Index) and `search.rs:31` (Query path — single failure should surface as clean user-visible error, not a silent retry storm). Phase 4 task: unify all three under a shared resilience primitive (retry policy + counter + structured `EmbedderError` enum), expose threshold knobs in `config.toml` rather than CLI flag, and add metrics emission (consecutive_fails gauge, total_embed_calls counter) for the eventual Prometheus / OpenTelemetry hookup. Not blocking on Phase 3 acceptance but blocks any "production-ready" claim.

  **Required error taxonomy (locked design hint, not yet implemented):**
  ```rust
  pub enum EmbedError {
      Transient(String),   // queue overflow, GPU pressure, ollama 5xx — retry-eligible at embedder layer
      Permanent(String),   // bad input, model not loaded, schema mismatch — bubble to caller, do NOT retry
      Timeout,             // caller decides retry policy; Query path should return 503 immediately,
                           // Index path can swallow into consecutive_fails counter
  }
  ```
  Embedder layer retries ONLY `Transient`. `Permanent` and `Timeout` pass through untouched, letting Query / Index / Server callers apply different policies on the same primitive. Phase 3.5b's blanket 5-attempt retry on every error type is intentionally wrong-but-cheap: it eats UX latency on Query path failures (~7.5s sleep chain) but unblocks Index path immediately. Phase 4 splits the policy by error class.

  **Counter location rationale (do not relocate during Phase 4 refactor):** the consecutive-fails counter belongs in the **caller's loop body** (`main.rs Index`), not in the embedder. Embedder is stateless — each `embed()` call is independent. "N consecutive failures" is loop semantics, owned by the loop. Pushing the counter down into the embedder violates single-responsibility (embedder would need to know it's being called from a loop) and breaks reuse (Query path is one-shot, has no "consecutive" notion). When Phase 4 unifies the three call sites, the counter pattern duplicates per loop; that duplication is correct because the loops have different abort semantics (Index: bail and exit, A2A Server: bail and respond 503, future BatchEval: bail and write partial-results-file). Resist the temptation to refactor `consecutive_fails` into shared embedder state.

- **hf-hub 0.5 Windows fresh-download bug (P2, discovered 2026-04-28 during Phase 4 first slice E2E)** — `hf-hub::api::sync::download_with_progress` (and `repo.get(filename)`) consistently aborts at exactly **49% / 567 MB** of a 1136 MB file with `Error: I/O error 磁盘空间不足。 (os error 112)` on Windows + git-bash + NTFS. NOT a real disk-full: D: drive showed 216 GB free before AND after the failed run; trap cleanup left no orphan tempfiles. NOT a network issue: direct `curl` of the same URL completes cleanly in 21s at 53 MB/s. Always at the same 567 MB byte offset across 4 reproducer runs — deterministic, not transient. Exact root cause not yet isolated; suspect (a) `std::env::temp_dir()` misroute in subprocess, OR (b) Windows-specific `set_len()`/sparse-file handling at chunk boundary, OR (c) chunk handler state-machine reset at ~50%. **Affects:** Phase 4 first slice E2E full-cycle harness execution (6 of 9 acceptance gates DEFERRED — see `.planning/phases/codenexus-04-parity/04-03-SUMMARY.md`). **Doesn't affect:** Phase 03.6 closure (uses already-cached model), Phase 4 code-level acceptance (R1-R5 grep contracts all PASS), CodeNexus production usage when model is already cached locally. **Required follow-ups:** (1) file upstream issue at `github.com/huggingface/hf-hub` with reproducer evidence; (2) run full E2E harness on Linux/macOS to validate platform-specificity; (3) add `robocopy`-based pre-seed path to harness for HF_HUB_OFFLINE-only validation when fresh-download is not viable. **Workaround for users hitting this on Windows clean-install:** download `model.safetensors` (1.19 GB) manually via browser/curl, place at `~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3/` per `docs/embedder-offline-bootstrap.md` "Manual safetensors download" section.

### Strategic (Software 3.0 framing — reframe of project's long-term value)

CodeNexus is not "a better code search tool." It is **the LLM's external long-term memory and structured perception layer for code** in Karpathy's Software 3.0 era (natural language as programming language, LLM as the new computer, agents as the new developers). LLMs have language understanding, reasoning, and generation. They lack persistent, precise structural perception of a specific codebase. CodeNexus closes that gap.

Three concrete bets, all things Sourcegraph is building and GitNexus / CodeFlow are not — we can be lighter and faster than Sourcegraph because we accept Software 3.0 from day one (no IDE plugin, no enterprise SSO, no human-team UX baggage):

- **Agent behavioral alignment** — Make Claude Code (and other agents) actually invoke `list_callers` / `query` / `get_symbol` at the right moments instead of guessing. CodeCompass (arxiv) measured agents skipping graph tools 58% of the time when the right answer required them. Target: drive that to ≤5%. Mechanism: MCP tool descriptions that score on retrieval-as-affordance, not just "tool exists." Phase 4 deliverable: A/B harness measuring tool-invocation rate on a curated task set, not just precision.
- **Cross-session codebase understanding accumulation** — Every Claude Code session learns things about the codebase ("this function is dangerous, always check callers before editing it", "this module owns the locking discipline, breaking its invariants causes deadlocks under load"). Today that knowledge dies at session end. CodeNexus + memU integration should persist these per-symbol annotations and surface them on next access. This is the real value of memU coupling — not "remember user preferences" but "remember codebase intelligence accumulated over agent-hours." Phase 5 (Bridge) territory; spec lives in `obsidian-llm-wiki` integration plan.
- **Architectural decision semantic indexing** — `ARCHITECTURE.md §9.4` contains the line "MUST NOT introduce reranker without LLM-judge." If an agent edits retrieval code, it should encounter that constraint *automatically* via search — not require humans to remember it exists. Index ADR-style decisions (MUST/MUST NOT/SHOULD/should-not statements) as first-class graph nodes; expose via a new A2A operation `query_constraints` that returns relevant decisions for a given file/function/topic. Phase 4 deliverable: the indexer + the operation; Phase 5 wires it into the IDE/MCP affordance layer above.

These three are interlocked: (1) makes the agent willing to ask, (3) makes the answer authoritative, (2) makes the answer compound over time. Each one alone is a feature; together they form the moat.

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
