# poc-retrieval

CodeNexus Phase 1 sidecar spike. Validates whether a minimal retrieval stack clears the 60% precision gate **before** Phase 1 ARCHITECTURE.md locks any retrieval-domain decision.

## What this is NOT

- Not Phase 1 deliverable. ARCHITECTURE.md retrieval sections wait on this output.
- Not the MVP impl. No graph/CALLS edges, no UI, no MCP, no A2A. Pure retrieval.
- Not optimized. ~300 LOC across 5 files, brute-force cosine in Rust.

## Stack

- tree-sitter + tree-sitter-typescript — symbol extraction (Function/Class/Method/Interface/arrow-fn)
- ollama HTTP `/api/embeddings` + `qwen3-embedding:0.6b` (595M params, ~27x bigger than Snowflake-arctic-embed-xs 22M)
- rusqlite + FTS5 — BM25 keyword
- Rust-side cosine (no sqlite-vec extension)
- RRF fusion (c=60, top-50 each side)

## Build

```bash
cd D:/projects/codenexus/experiments/poc-retrieval
cargo build --release
```

## Run

```bash
# 1. index a corpus (ollama must be running, qwen3-embedding:0.6b pulled)
./target/release/poc-retrieval index --repo D:/projects/obsidian-llm-wiki --db poc.db

# 2. interactive query
./target/release/poc-retrieval query "filesystem fallback when obsidian not running"

# 3. eval against query set
./target/release/poc-retrieval eval --queries eval/queries.json --db poc.db
```

## Evaluation contract

`eval/queries.json` schema:
```json
{ "id": "axis1_q01", "axis": 1, "query": "...", "expected_paths": ["..."], "negative": false }
```

- `axis 1` — symbol-exact (BM25 should dominate)
- `axis 2` — semantic NL (embedding should dominate; spike-001 baseline 43% lives here)
- `axis 3` — call relations (POC will score low; this is the data point that justifies REQ-02 CALLS edge graph)

`precision@5` = (count of hits whose `path` contains or `name` equals an `expected_paths` entry) / 5.
For `negative: true`, score = 1.0 if no hits OR top hit rrf_score < 0.01, else 0.0.

## Decision gate

Per-axis precision is what feeds back to Phase 1 ARCHITECTURE.md retrieval sections.
Axis 1 < 80% — BM25 path is broken, redesign needed.
Axis 2 < 50% — embedder choice doesn't move the needle, need reranker or bigger model.
Axis 2 ≥ 65% — qwen3-embedding:0.6b alone clears the spike-001 gap; candle@Phase-3 is justified.
Axis 3 ≥ 40% — surprising; CALLS edge urgency drops.
Axis 3 < 20% — confirms REQ-02 priority.

## Anti-scope

- No incremental indexing (`index` is full reindex every run)
- No git overlay
- No MCP / A2A / Go layer
- No HTTP server
- No multi-language

These all live in PROJECT.md REQ-01..10 and are deliberately out of POC scope.
