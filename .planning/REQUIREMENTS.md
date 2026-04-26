# Requirements: CodeNexus

Tracked here in canonical form; cross-referenced from PROJECT.md (active list) and ROADMAP.md (phase mapping).

## Active

### REQ-01: TypeScript parsing pipeline
**Phase**: 2 (spike) → 3 (MVP)
**Description**: tree-sitter Rust crate parses a TS repo into `SymbolNode[]` with kinds: Function, Class, Method, File. Each node carries source range, name, parent reference.
**Acceptance**: Given a 50-file TS corpus, the parser produces ≥ 95% symbol coverage compared to a manual reference set.

### REQ-02: Symbol graph edges (4 kinds)
**Phase**: 3 (MVP)
**Description**: From parsed SymbolNodes, derive 4 edge kinds covering call relations, module structure, and inheritance. Scope refined 2026-04-27 (was CALLS-only):

| EdgeKind | Priority | Source syntax (TS) | Why |
|----------|----------|---------------------|-----|
| `Calls` | ★★★★★ must | `foo()` / `obj.method()` call expressions | Highest axis-3 value (who-calls-X / what-X-calls) |
| `Imports` | ★★★★★ must | `import { X } from "..."` / `require("...")` | Cross-file resolution baseline; without it Calls edges across files lose target accuracy |
| `Implements` | ★★★★☆ must | `class X implements Y` | Architecture queries ("which classes satisfy interface Y") common in TS code |
| `Extends` | ★★★★ must | `class X extends Y` / `interface X extends Y` | Inheritance impact analysis ("subclasses of X"). Many projects retrofit this — adopt up-front |

**Deferred to Phase 3+** (post-MVP observation):
- `Overrides` (★★★☆) — method-override edges. Requires class-hierarchy walking + method-resolution-order. Deferrable: Calls + Extends already covers most "subtype dispatch" queries indirectly. Add after Calls + Extends measured stable on real corpus.

**Resolver strategy (locked, naive 3-step)**:
1. Same-file lookup — exact name match in current file's symbol set
2. Import-file lookup — follow Import edge from current file, exact name match in target file
3. Global unique-name lookup — exact name match across all symbols, only if exactly one global match exists

No alias resolution, no re-export chain follow, no barrel-file expansion. Documented limitation: TS projects using barrel files (`index.ts` re-exports) will under-resolve Imports — accept noise for MVP, fix in Phase 3+ as `import_alias_resolver` enhancement.

**Acceptance**: Hand-verified edges for a 5-file sample match expected edges with ≥ 90% precision **per edge kind**. Imports kind allowed lower precision floor (≥ 80%) due to re-export blind spot.

### REQ-03: candle embedding
**Phase**: 2 (spike) → 3 (MVP)
**Description**: candle (Hugging Face Rust ML) loads a Snowflake/BERT-family embedder and produces 384-or-768-dim vectors for each SymbolNode. No external API dependency.
**Acceptance**: Cold-start ≤ 30s on a typical machine; per-symbol embed throughput ≥ 50/sec on CPU.

### REQ-04: Storage layer
**Phase**: 2 (spike decides) → 3 (MVP)
**Description**: Single embedded database stores graph nodes/edges + embedding vectors + FTS5 inverted index. Choice between redb (pure Rust KV) and rusqlite+sqlite-vec (SQL + vector + FTS5 in one file). Phase 2 bench decides.
**Acceptance**: 10K-symbol repo fits in < 5x source code disk size; query roundtrip < 100ms p95 (excluding embedder time).

### REQ-05: Hybrid search
**Phase**: 3 (MVP)
**Description**: Search blends BM25 (FTS5) + vector cosine similarity via Reciprocal Rank Fusion (RRF). Default top-K = 10.
**Acceptance**: Top-5 precision ≥ 60% on the 7 spike-001 NL queries; falls back gracefully if either signal is missing.

### REQ-06: Rust core as A2A agent
**Phase**: 1 (design) → 2 (spike) → 3 (MVP)
**Description**: Rust core compiles to a binary that runs an axum HTTP server on a local port (default 9876, configurable via `~/.codenexus/port` lockfile). Server implements Google A2A v0.2 protocol: `POST /tasks/send` accepts task envelope, `GET /tasks/{id}` polls task status. Optional SSE stream for long tasks.
**Acceptance**: `curl -X POST http://localhost:9876/tasks/send -d @sample.json` succeeds end-to-end; conformant to A2A v0.2 spec at https://google.github.io/A2A/.

### REQ-07: Go server as A2A client + service frontend
**Phase**: 1 (design) → 3 (MVP)
**Description**: Go server (chi HTTP + mark3labs/mcp-go MCP + cobra CLI) acts as A2A client to Rust core. Spawns Rust core binary on `serve` start, manages lifecycle (healthcheck, restart on crash). Translates incoming HTTP/MCP requests into A2A task calls.
**Acceptance**: `./codenexus serve --port 8080` starts both processes; killing the Rust core triggers automatic restart within 5s; MCP `query` tool round-trips correctly.

### REQ-08: Single fat-binary distribution
**Phase**: 3 (MVP)
**Description**: `make build` produces ONE binary (`bin/codenexus`) where Go binary embeds the Rust binary via `//go:embed`. On first `serve`, the embedded Rust binary is extracted to a temp dir and spawned.
**Acceptance**: `bin/codenexus` runs on a machine with no Rust/Go toolchain installed; ≤ 150 MB total size.

### REQ-09: Embedded HTML/JS UI
**Phase**: 3 (MVP)
**Description**: Static HTML/JS bundle (vanilla JS + HTMX + cytoscape.js) served by Go's chi router from a `//go:embed` of the `ui/` directory. Provides search box and graph viewport.
**Acceptance**: `./codenexus serve` exposes browser UI at `http://localhost:8080`; UI talks to Go HTTP API which proxies to Rust core; no build step required for UI.

### REQ-10: MVP precision acceptance
**Phase**: 3 (MVP)
**Description**: On the 7 NL queries from `obsidian-llm-wiki/.planning/spikes/001-embed-quality-on-code/`, CodeNexus must achieve top-5 precision ≥ 60%. GitNexus 1.6.3 baseline is 43%.
**Acceptance**: Run the spike-001 evaluation harness against CodeNexus; emit per-query precision and overall mean ≥ 0.60.

## Out of Scope

See `PROJECT.md §Out of Scope` — same content lives in PROJECT.md as the canonical anti-scope.

## Future (post-MVP, will become REQ-11+)

- Multi-language tree-sitter (Phase 4)
- Multi-repo registry (Phase 4)
- Git overlay via gix (Phase 4)
- Pattern detection ported from CodeFlow (Phase 4)
- Security scanners ported from CodeFlow (Phase 4)
- Health score computation (Phase 4)
- Markdown wiki-link graph + Obsidian integration (Phase 5)
- Three-way viz code/vault/memU (Phase 5)
- Plugin system (Phase 6)
- A2A agent card publication (Phase 6)
