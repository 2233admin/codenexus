> **HISTORICAL DOCUMENT — kept for reference only.**
>
> This is the original Stitch proposal (2026-04-25). Decisions closed 2026-04-26 differ on several axes:
>
> - **Naming**: Stitch → **CodeNexus** (locked)
> - **Architecture**: pure Rust → **Rust core (axum A2A endpoint) + Go service layer (chi HTTP, mcp-go MCP, CLI)**
> - **IPC**: subprocess JSON-RPC → **A2A protocol over localhost HTTP** (Rust core is a network-addressable A2A agent, Go is one of N possible clients)
> - **MCP server**: rmcp (Rust, immature) → **mark3labs/mcp-go** (mature, Go) — kills Phase 0 high-risk gate
> - **Embedder default**: ollama-rs → **candle 内嵌** (Snowflake/BERT, no external dep)
> - **UI**: pivoted Tauri-then-back to **option B (axum/chi-served web + cytoscape.js)** — single fat-binary preserved via `//go:embed`
> - **License**: MIT (SPEC default) → **Apache 2.0** — explicit patent grant + trademark protection, same enterprise/agent-mesh adoption profile, better legal posture for solo dev. CodeFlow MIT code will be ported under Apache 2.0 with attribution in NOTICE.
>
> See `.planning/PROJECT.md` and `~/.claude/plans/gsd-abundant-rabbit.md` for current canonical decisions.
>
> **Why kept**: clean-room policy requires not deleting design history. This file documents what we considered and rejected, which is part of the legal trail showing CodeNexus did NOT copy GitNexus code.

---

# Proposal: Stitch — Unified Code+Knowledge Graph Tool (Rust edition)

> Emerged from spike 001 + license analysis + user decision (2026-04-25):
> refactor in Rust, MIT licensed, clean-room reimplementation of GitNexus
> ideas + lift CodeFlow MIT-compatible patterns.

## License

**Stitch = MIT.**

Reason: GitNexus is PolyForm Noncommercial 1.0.0 (source-available, not OSS). To make Stitch usable by anyone for any purpose -- including future commercial paths and broad OSS community adoption -- we cannot copy GitNexus code. We may study its design and reimplement in Rust from clean-room design notes. CodeFlow is MIT-compatible and may be ported.

## Why this exists

Spike 001 proved GitNexus has the right pipeline shape but two structural problems: weak default embedder (43% precision) and architectural debt (Kuzu fork vs LadybugDB npm). CodeFlow has the visualization + git-overlay layer GitNexus lacks but its data layer is shallow.

**Neither is the right home to grow into.** Patching either inherits debt; lifting GitNexus code locks Stitch into PolyForm Noncommercial. Refactor as a new tool in Rust:
- Pure Rust = single binary distribution (matches user preference)
- Different language from upstream = natural clean-room separation, MIT-safe
- tree-sitter, candle, redb, axum ecosystem mature enough for production
- Avoids Bun + LadybugDB napi compat unknowns entirely

## Stack decisions

| Layer | Choice | Why |
|---|---|---|
| Language | **Rust 2024 edition** | User preference; matches memU + ZeroClaw stack |
| Build | **cargo** → single binary | Zero install dependencies for end users |
| Parser | **tree-sitter (Rust crate)** | Official Rust binding, multi-language, mature |
| Graph store | **redb** OR **rusqlite + sqlite-vec** (TBD spike) | redb = pure Rust KV, no FFI. sqlite-vec = SQLite + vector in one. Pick after benchmark |
| Vector store | Same as graph store | Avoid two databases |
| Embedder backends | **Pluggable from day 1** | Don't repeat GitNexus's hardcoded mistake |
| - Local default | **ollama-rs crate** → user's local `qwen3-embedding:0.6b` | Zero external API; user already has it |
| - Native option | **candle** (Hugging Face Rust ML) | If user wants no-Ollama setup, candle runs Snowflake/BERT-family natively |
| - Cloud option | **async-openai** crate for OpenAI-compat APIs | Optional; for users who want hosted |
| HTTP API | **axum** + **tower** | De-facto Rust web stack, async-first, ergonomic |
| MCP server | **rmcp** crate (community, evolving) | Risk: less mature than TS SDK; may need wrap-and-pray. Spike-validate |
| UI strategy | **TBD — three options below** | Need user pick before MVP |
| Git overlay | **gix (gitoxide)** crate | Pure Rust git, fast, no libgit2 FFI |
| Logging | **tracing** + **tracing-subscriber** | Structured, async-aware |
| Config | **figment** + TOML | Layered config, no surprises |

### UI strategy — three options to pick from

| Option | Stack | Pros | Cons |
|---|---|---|---|
| **A. Tauri + SolidJS** | Tauri shell, Solid web UI inside | Native window + small bundle (~20MB) + Rust backend; can use cytoscape.js | Two languages; Tauri config overhead |
| **B. Pure web served by axum** | axum serves HTML/JS, browser does viz with cytoscape.js | One Rust binary serves localhost UI; user opens browser tab; matches GitNexus serve pattern | Browser tab feels less native than window |
| **C. leptos or dioxus (pure Rust UI)** | Rust → WASM, no JS at all | Zero JS, type-safe end-to-end | Cytoscape.js / d3 ecosystem not available; would need to wrap them or build viz in Rust |

**Recommend B for MVP.** Lowest friction, matches GitNexus's existing serve pattern users already know, lets cytoscape.js do graph viz without JS gymnastics. Tauri (A) for v2 if user wants native feel. Skip C unless committed to pure-Rust religion -- viz ecosystem in Rust WASM is too thin today.

## Architecture (3 layers, Rust-native)

```
┌─────────────────────────────────────────────────────────────┐
│  UI (browser, served from axum)                              │
│  - Static HTML/JS bundle in Stitch binary (rust-embed)       │
│  - cytoscape.js for graph viz                                │
│  - Vanilla JS or HTMX (avoid React/Vue framework lock-in)    │
└─────────────────────┬───────────────────────────────────────┘
                      │ HTTP/JSON via axum
┌─────────────────────┴───────────────────────────────────────┐
│  Server (axum + MCP stdio)                                   │
│  - REST endpoints for UI                                     │
│  - MCP tool surface (rmcp) for AI agents                     │
│  - Hybrid query: BM25 (sqlite FTS5) + vector + RRF fusion    │
│  - Pluggable EmbedderBackend trait                           │
└─────────────────────┬───────────────────────────────────────┘
                      │ Direct trait calls
┌─────────────────────┴───────────────────────────────────────┐
│  Core (pure Rust crate, no I/O assumptions)                  │
│  - tree-sitter parser → SymbolNode struct                    │
│  - gitoxide overlay reader (blame, log, diff)                │
│  - Graph builder (Function/Class/Method/File + Process)      │
│  - Embedder trait + impls: ollama / candle / openai-compat   │
│  - Storage trait + impls: redb / sqlite                      │
│  - Pattern detectors (singleton/factory/etc) -- own impl     │
│  - Security scanners (secrets/SQLi/eval) -- own impl         │
└─────────────────────────────────────────────────────────────┘
```

## MVP scope (3 weeks, Rust learning curve included)

- [ ] tree-sitter parse one TS repo → SymbolNode[] in Rust
- [ ] Build CALLS edge graph (skip IMPORTS/EXTENDS for MVP)
- [ ] Storage: pick redb OR rusqlite+sqlite-vec after Phase 0 spike
- [ ] Embed all symbols via ollama-rs against local Ollama
- [ ] Hybrid search: SQLite FTS5 BM25 + vector cosine + RRF fusion
- [ ] axum server with `/api/symbols` `/api/search?q=...` `/api/graph`
- [ ] MCP `query` tool over stdio via rmcp
- [ ] Embedded HTML/JS bundle: cytoscape.js graph view + search box

**Anti-scope MVP cuts**:
- ❌ Multi-language tree-sitter (TS only first)
- ❌ Multi-repo registry (one repo, one DB file)
- ❌ Pattern detection (defer phase 2)
- ❌ Security scanners (defer phase 2)
- ❌ Health score (defer phase 2)
- ❌ Git overlay (defer phase 2)
- ❌ Markdown wiki graph (defer phase 3)
- ❌ Tauri native window (defer to v2)
- ❌ Pure-Rust UI / WASM frontend (probably never)

## Roadmap

| Phase | Duration | Adds |
|---|---|---|
| **-1 — Design notes** | 2-3 days | Write `ARCHITECTURE.md` from clean-room design (no code references to GitNexus). Establishes legal+technical baseline |
| **0 — Stack spike** | 3-4 days | Validate Rust + tree-sitter + redb-or-sqlite + candle/ollama + axum + rmcp end-to-end on a 50-file TS corpus. GO/NO-GO on each component |
| **1 — MVP** | 3 weeks | Above MVP scope. Ship runnable single binary, replace GitNexus on one repo |
| **2 — Parity** | 4-5 weeks | Multi-language, multi-repo registry, git overlay (gix), pattern detection, security scanners, health score |
| **3 — Bridge** | 2 weeks | Markdown wiki-link graph (Obsidian-aware), three-way viz: code ↔ vault ↔ memU memory |
| **4 — Reach** | 4 weeks | Tauri native window option, plugin system, multi-tenant if needed |

## Clean-room policy (legally critical)

**Rule**: when working on Stitch, do NOT have GitNexus source open in another window. The flow is:
1. Read GitNexus to understand a concept (one session)
2. Write down the concept in own words in `ARCHITECTURE.md` (no code references)
3. Wait at least 24h
4. Implement in Rust from your own design notes only

This protects against unconscious copying of structure. Extra paranoia for solo dev.

CodeFlow MIT code may be **studied AND ported directly** -- MIT permits derivation under MIT.

## What this studies (and from where)

| Component | Source | License | Strategy |
|---|---|---|---|
| tree-sitter integration patterns | GitNexus (PolyForm) | NC | Study, design own approach, write Rust from scratch |
| Embedder pipeline shape | GitNexus (PolyForm) | NC | Study, design own trait, write Rust from scratch |
| MCP tool definitions schema | GitNexus (PolyForm) | NC | Study, design own tools, document in Stitch spec |
| Graph viz interaction | CodeFlow (MIT) | MIT | Read JS source, port to embedded HTML/JS bundle |
| Git overlay (blame/heatmap) | CodeFlow (MIT) | MIT | Port logic; replace JS git lib with `gix` crate |
| Pattern detectors | CodeFlow (MIT) | MIT | Port heuristics to Rust |
| Security scanners | CodeFlow (MIT) | MIT | Port regex set + AST checks to Rust |
| Markdown wiki-link graph | CodeFlow (MIT) | MIT | Port for phase 3 |

## Anti-scope (永远不做)

- Python in any layer (runtime, build, plugins)
- C# specific tree-sitter quirks
- Cloud-only features (Zilliz Cloud, OpenAI hard dep)
- Replacing memU / obsidian-llm-wiki -- Stitch only owns code+git domain
- VS Code extension (separate project)
- Electron-based UI (Tauri only if native window needed)
- Implementing semver-aware language version migration (out of scope, leave to LSP)

## 未决问题

1. **Storage choice**: redb (pure KV, simpler) vs rusqlite+sqlite-vec (SQL + vector + FTS5 in one). Phase 0 spike resolves.
2. **rmcp maturity**: Rust MCP SDK landscape less mature than TS. Spike must verify rmcp can serve real tools without bugs blocking MVP.
3. **Embedder default**: ollama-rs (requires Ollama running) vs candle (no external dep but bigger binary). Default depends on user expectation.
4. **memU integration**: Stitch self-contained (its own vector store) or share memU's PG for fused recall? If share: adds Rust pgvector client; defers complexity.
5. **Naming**: "Stitch" is the working name. Alternatives: CodeNexus, Loom, Weaver. Pick before phase 1.
6. **Project home**: `D:/projects/stitch/` (new repo) or sub-package in `obsidian-llm-wiki` monorepo? New repo cleaner; monorepo enables tighter integration with vault layer.
7. **UI option**: A (Tauri+JS) / B (axum-served web, recommended) / C (pure Rust UI). Need pick.
8. **Distribution**: cargo install / GitHub releases binary / homebrew tap? MVP can defer to GitHub releases.

## Next concrete step

Recommend three-step sequence:

- **Step 1 (1 day) -- 答完未决问题**: pick storage, embedder default, UI option, project home. 把未决问题列表逐条转成决策。
- **Step 2 (3 days) -- Phase -1 Design notes**: write `ARCHITECTURE.md` clean-room from concept understanding, no GitNexus code reference. Legal baseline.
- **Step 3 (4 days) -- Phase 0 Stack spike**: minimum viable Rust pipeline end-to-end, GO/NO-GO each component (tree-sitter / storage / embedder / axum / rmcp).

Total before MVP starts: ~8 days investment, but de-risks the 3-week MVP into bounded execution.
