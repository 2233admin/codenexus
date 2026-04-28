# CodeNexus

> **Status: pre-MVP, alpha. Phase -1 (Design) starting.**

Code + knowledge graph tool. Apache 2.0, single fat-binary. Designed as A2A-native: the Rust core is a network-addressable agent, not just a private library.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Embedded UI (vanilla JS + HTMX + cytoscape.js)              │
│  Served by Go via //go:embed                                 │
└─────────────────────────┬────────────────────────────────────┘
                          │ HTTP
┌─────────────────────────┴────────────────────────────────────┐
│  Go service layer (server/)                                  │
│  - chi HTTP router (UI + REST API)                           │
│  - mark3labs/mcp-go (MCP stdio for LLM/IDE integration)      │
│  - cobra CLI (index/query/serve/mcp subcommands)             │
│  - A2A client → talks to core over localhost HTTP            │
└─────────────────────────┬────────────────────────────────────┘
                          │ A2A protocol (POST /tasks/send + GET /tasks/{id})
                          │ over localhost:9876
┌─────────────────────────┴────────────────────────────────────┐
│  Rust core (core/) — A2A-native agent                        │
│  - axum HTTP server implementing A2A spec                    │
│  - tree-sitter parsing pipeline                              │
│  - candle embedder (Snowflake/BERT, no external deps)        │
│  - storage: redb OR rusqlite + sqlite-vec (Phase 0 spike)    │
│  - gix git overlay (blame, log, diff)                        │
└──────────────────────────────────────────────────────────────┘
```

## Why A2A as IPC

Rust core exposes the same A2A endpoint to its local Go sibling and to any remote agent — no private RPC path. This makes CodeNexus an open node in any agent mesh from day one. Trades ~0.1ms localhost HTTP framing overhead for ecosystem compatibility.

See `~/.claude/plans/gsd-abundant-rabbit.md` for the full architecture rationale (唯物主义 / 前瞻 / 启发性 论证).

## Layout

| Path | Purpose |
|---|---|
| `core/` | Rust crate. Builds `codenexus-core` binary (axum A2A server). |
| `server/` | Go module. Builds `codenexus` binary (embeds core, serves UI/MCP/CLI). |
| `ui/` | Static web frontend. Embedded by Go via `//go:embed`. |
| `docs/origin-spec.md` | Original Stitch proposal (historical, decisions superseded). |
| `.planning/` | GSD planning artifacts (PROJECT.md, ROADMAP.md, per-phase dirs). |
| `Makefile` | Build entry (`make build` / `make test` / `make clean`). |

## Build

```bash
make build         # builds core (Rust) then server (Go), produces bin/codenexus
./bin/codenexus serve --port 8080
# in another shell:
./bin/codenexus query "where is rate limiting?"
```

> First-run downloads ~1.2 GB of model weights from `huggingface.co`. If
> you are offline or behind Clash, see
> [docs/embedder-offline-bootstrap.md](docs/embedder-offline-bootstrap.md)
> for recovery (manual download / HF_HOME pre-seeding / HF_HUB_OFFLINE / mirror).

## License

Apache 2.0. See `LICENSE` (canonical text from apache.org) and `NOTICE` (attribution).

Why Apache 2.0 over MIT: explicit patent grant + trademark protection; same enterprise/agent-mesh adoption profile as MIT but with real legal teeth in patent-troll scenarios.

Why not GPL: would conflict with the A2A "open agent in any mesh" strategy by spooking enterprise / commercial agent adoption (legal teams routinely ban GPL-licensed dependencies).

## Acceptance bar (MVP)

Top-5 precision ≥ 60% on the 7 NL queries from `obsidian-llm-wiki/.planning/spikes/001-embed-quality-on-code/` (vs GitNexus 1.6.3 baseline of 43%).
