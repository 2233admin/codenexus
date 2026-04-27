# Storage Backend: redb vs rusqlite + sqlite-vec

> Phase 2 spike research, 2026-04-27. Resolves ARCHITECTURE.md section 9.6 (pending)
> and unblocks Phase 3 MVP. Decision-ready: recommendation taken at the end.
> A 60-90 minute confirmation spike is suggested but not blocking -- research alone
> is conclusive on the primary axes.

## TL;DR

**Pick `rusqlite + sqlite-vec` for Phase 3 MVP.** Keep the storage trait (D-R2 lock)
as the seam so a future swap remains a 1-week migration, not a rewrite.

Headline reasons:

1. **Single-file co-location of three required indexes.** CodeNexus needs BM25 (FTS5),
   vector cosine (sqlite-vec), and graph edges (relational joins) at query time.
   Only SQLite delivers all three in one ACID file. redb gives KV; everything else
   (FTS, vectors, graph joins) would be re-implemented in Rust on top of it.
2. **POC sunk cost is on the SQLite side and the POC plateau validates the path.**
   experiments/poc-retrieval/src/storage.rs is ~280 LOC of working SQL, including
   the search_blob decompose trick for FTS5 BM25, edge joins, and the confidence-
   bucketed dump. R3 plateau (Axis-1 70%, Axis-2 47.5%) was achieved on this stack.
3. **The 10K-symbol budget is far below sqlite-vec pain point.** At 100K x 384-d with
   brute-force scan, sqlite-vec returns in <75ms on an M1; CodeNexus target is
   10K x 1024-d. p95 < 100ms (REQ-04) is hit without ANN indexing. redb has no
   native vector primitive.

## 1. Tradeoff matrix

| Dimension | redb | rusqlite + sqlite-vec | Weight | Winner |
|---|---|---|---|---|
| Read latency, point lookup | ~1-5 us (zero-copy B-tree, MVCC) [1] | ~10-50 us (parsed SQL + B-tree) | low | redb |
| Read latency, range / scan | LMDB-class, sequential B-tree walk | FTS5 MATCH ~1-10ms over 10K rows; SQL scan ~200K rows/sec | med | tie at our scale |
| Write latency, single insert | ~920 us median in cberner bench [1] | ~50-200 us with WAL + prepared statement | low | rusqlite |
| Write latency, batch 10K inserts | Bulk load ~2-3s per cberner bench [1] | Bulk insert in transaction ~2-5s for 10K [3] | low | tie |
| Disk footprint, 10K symbols | KV blob-only ~40-50MB raw bytes (no FTS, no vec metadata) -- but CodeNexus must also ship a separate FTS impl (tantivy) and a separate vector store, exploding total | rusqlite+vec0+FTS5 in one file. POC poc.db is 10MB at Phase 1; ~55-80MB extrapolated for 10K symbols | high | rusqlite |
| ACID guarantees | Full ACID, copy-on-write B-trees, configurable durability per txn [2] | Full ACID, WAL mode, well-known semantics | high | tie |
| Concurrency model | MVCC, multi-reader + single-writer; zero-copy AccessGuard [2] | WAL: many concurrent readers + single writer | med | tie (same model) |
| Vector similarity (cosine top-k) | **No native primitive.** Requires Rust HNSW (hnsw_rs, instant-distance) as separate store | sqlite-vec vec0 virtual table, brute-force or 8-bit quantized; ~75ms for 100K x 384d, ~200ms at 1M x 1024d [4][5] | high | rusqlite |
| Full-text search (BM25) | **No native primitive.** Tantivy is the obvious pick (~2x Lucene perf [6]) but is a separate index store | FTS5 bm25() with R3-validated weights [name:10, snippet:1, kind:1, search_blob:5], unicode61 + Rust-side decompose() for camelCase. Already in production POC | high | rusqlite |
| Graph traversal (PPR over edges) | Manual KV scans + adjacency-list encoding; no JOIN | SQL JOIN over edges with kind + confidence_min filter; current POC edges_of_kinds already does this | high | rusqlite |
| Crate maturity | redb 4.4k stars, 4.1.0 (2026-04), 618k monthly downloads, ACID-stable since 1.0 (Jun 2023) [1][7] | rusqlite ~2.7k stars, 0.31.x stable; SQLite format is forwards/backwards stable forever; sqlite-vec 0.1.9 still pre-1.0 [8] | high | rusqlite for SQLite core, mixed for sqlite-vec |
| Binary size impact (REQ-08, <=150MB) | Pure Rust, ~150KB compiled | rusqlite bundled + sqlite-vec adds ~1-2MB of C code. Trivially under envelope | high | redb (margin), irrelevant at our budget |
| Cross-compile (Win/Linux/Mac) | Pure Rust, trivially cross-compiles | rusqlite bundled compiles SQLite C amalgamation in-tree, solved [9] | high | redb (slight), but rusqlite is solved |
| Migration cost off later | Off redb -> write reader for B-tree blobs, re-derive FTS+vectors. ~1-2 weeks | Off SQLite -> standard INSERT INTO target SELECT ... FROM source; or jsonl dump. ~1 week | med | rusqlite |
| Operability / debugging | redb CLI or custom Rust tooling only | sqlite3 CLI opens it interactively; EXPLAIN QUERY PLAN free | med | rusqlite |
| Backup / replication | File-copy works (CoW); no native incremental | .backup API, WAL checkpointing, Litestream ecosystem | low | rusqlite |
| AI-coding-agent training data | redb 4.1 itself was hardened by AI agents [1]; smaller surface | SQLite has order-of-magnitude more SQL/FTS/vec examples in any LLM corpus | low | rusqlite |

Citations:
- [1] webpronews.com, "Rust Redb Hits 4.1: AI Agents Squash Bugs, Deliver 1.5x Write Speedups in Embedded KV Store" (2026-04). https://www.webpronews.com/rusts-redb-hits-4-1-ai-agents-squash-bugs-deliver-1-5x-write-speedups-in-embedded-kv-store/
- [2] redb crate docs, https://docs.rs/redb/latest/redb/ (4.x). Zero-copy AccessGuard, MVCC.
- [3] rusqlite, https://github.com/rusqlite/rusqlite. WAL + prepared-statement insert latency.
- [4] Alex Garcia, "Introducing sqlite-vec v0.1.0" (2024-08). https://alexgarcia.xyz/blog/2024/sqlite-vec-stable-release/index.html -- 100K x 384d full-scan 67.84ms on M1 Pro; int8 17.44ms; preload 3.97ms.
- [5] github.com/asg017/sqlite-vec issue #186 "Performance tuning for vec search". 1M x 3072d ~= 8.5s; 1M x 192d ~= 192ms. https://github.com/asg017/sqlite-vec/issues/186
- [6] Tantivy, https://github.com/quickwit-oss/tantivy. Lucene-class BM25, ~2x Lucene perf, MIT.
- [7] redb crates.io, https://crates.io/crates/redb. 4.4k stars / 1.8k dependents / 618k monthly downloads as of Apr 2026.
- [8] sqlite-vec crate, https://crates.io/crates/sqlite-vec -- 0.1.9, pre-1.0; loadable-extension via sqlite3_auto_extension. Rust usage: https://alexgarcia.xyz/sqlite-vec/rust.html
- [9] rusqlite README, https://github.com/rusqlite/rusqlite -- bundled feature compiles SQLite C amalgamation in-tree; pregenerated bindings since 0.10.1.

## 2. CodeNexus-specific scoring

### Workload re-stated

- 10K symbols x 1024-d FP32 vectors = 40 MB raw embeddings
- FTS5 inverted index ~ 5 MB at this scale
- Graph: ~30K edges (Calls + Imports + Implements + Extends), ~10 MB with confidence + indexes
- Total index ~ 55-80 MB (well under REQ-04 5x source-disk envelope)
- Query mix: ~80% hybrid (BM25 + cosine + RRF + edge boost), ~20% pure graph (PPR over edges)
- Index update: ~50% incremental (file change -> re-embed touched), ~50% full rebuild
- Single-process, single-writer, no shared-DB requirement

### `rusqlite + sqlite-vec` -- fit score 9/10

Hits:
- BM25, vectors, edges all in one ACID file. One .db to ship, back up, sync.
- search.rs is already one prepared statement composing FTS5 + manual cosine. Wiring vec0 in is `CREATE VIRTUAL TABLE symbols_vec USING vec0(embedding float[1024])` + INSERT mirror keyed by symbol_id + replace Rust cosine with vec0 MATCH. Estimated 200 LOC, 2-day migration.
- FTS5 + camelCase search_blob decompose is already R3-validated; throwing it out mid-Phase-2 is anti-pattern.
- Edge joins for graph traversal use SQL JOINs that POC already ships (edges_of_kinds, dump_edges_join). PPR matrix construction pulls (from_id, to_id, confidence) tuples directly.
- Operationally, `sqlite3 poc.db` lets a maintainer (or AI agent) inspect/query/diff with zero project-specific tooling.

Caveats:
- sqlite-vec 0.1.x pre-1.0 churn risk. Mitigation: pin exact version + smoke test; surface < 10 SQL functions.
- No ANN. Brute-force cosine. At 10K x 1024d that is ~40 MB float-mul per query. Even unoptimized Rust does this in 10-30 ms; with int8 quantization 3-5 ms. Well under REQ-04 100ms p95.

### `redb` -- fit score 4/10

Hits:
- Smaller binary, faster point lookups, cleaner Rust-only build.
- 4.x mature, AI-hardened, proven at scale.

Misses:
- **Zero native FTS.** Adding tantivy = second store, second on-disk format, second crash-consistency surface, 2x backup logic. R3 BM25 config (column weights, search_blob decompose, FTS5 unicode61) does NOT port -- tantivy tokenizer and weighting are different. Invalidates ~half the R3 retrieval lock.
- **Zero native vector.** Roll our own brute-force float scan or add a third index (hnsw_rs).
- **Zero JOIN.** Graph traversal becomes manual adjacency-list KV walking with per-edge filter loop.
- D-R2 trait abstraction was supposed to make backend swap cheap. When responsibilities expand to KV + FTS + vector + graph, the abstraction stops paying.

### Decision posture

The redb pitch is "pure Rust, no C, fastest KV". A real win in isolation but the wrong unit of analysis. CodeNexus needs an index database, not a KV store. SQLite (FTS5 + vec0) gives the index database in one file with no extra moving parts. redb + tantivy + hnsw_rs gives the same capability across three stores with three failure modes and one shared transaction boundary that has to be invented.

## 3. Spike plan (optional confirmation, 60-90 min)

Research alone is conclusive on primary axes. A short spike is recommended to put numbers on close calls (insert throughput, vector top-k under quantization). Skip only if Phase 3 schedule pressure is extreme.

### Scope

`experiments/storage-bench/` -- a single Rust binary, two backends, four micro-benchmarks. Use Criterion.

### Backend A: rusqlite + sqlite-vec

Schema additions to current POC:
- `CREATE TABLE symbols(...)` -- as today
- `CREATE VIRTUAL TABLE symbols_fts USING fts5(...)` -- as today
- `CREATE VIRTUAL TABLE symbols_vec USING vec0(symbol_id INTEGER PRIMARY KEY, embedding FLOAT[1024])`

### Backend B: redb baseline (KV-only, no FTS/vector)

Three tables:
- `symbols: u64 -> Symbol{kind, name, path, ...}`
- `embeddings: u64 -> [u8; 4096]`  (1024 floats)
- `edges: (u64, u64, u8) -> f32`  (from, to, kind, confidence)

Vector top-k = brute-force scan of embeddings; FTS intentionally not implemented.

### Measurements (4 ops, 5 trials each, report median)

1. Insert 10K symbols (single transaction). Wall-clock seconds.
2. Point lookup, 1K random symbol_id queries. Median microseconds.
3. Vector top-5 over 10K, FP32. Median ms.
4. FTS5 top-5 over 10K, fixed query "function parse". Backend A only.

### Acceptance / weighted decision

Backend A wins if:
- Insert 10K within 2x of B (expected: A within 1.5x)
- Point lookup within 5x of B (expected: A 5-10x slower in absolute us, well under 1ms)
- Vector top-5 < 50ms median (expected: 10-30ms FP32, 3-5ms int8)
- FTS5 top-5 < 30ms (expected: 1-10ms)

If A passes all four, lock the pick. If A fails insert by >2x AND point lookup by >10x AND Phase 3 is genuinely write-heavy, escalate.

### Output

`experiments/storage-bench/results.md` with the 4 numbers, machine spec, one-paragraph verdict. Commit alongside lock decision in ARCHITECTURE.md section 9.6.

## 4. Recommendation

### Pick: rusqlite + sqlite-vec

Rationale, in priority order:

1. CodeNexus is an index database. SQLite is the only candidate that delivers BM25 + vector + relational graph in one ACID file at our scale, with R3-validated config intact.
2. POC sunk cost is real and well-validated. storage.rs (~280 LOC) is the de facto Phase 2 storage layer; sqlite-vec adds ~200 LOC for vec0 plus a SQL fragment in search.rs. redb means re-implementing FTS5 (tantivy port), vector search (hnsw_rs or hand-rolled), and graph joins (manual KV scans). 1-2 weeks of throwaway work on the wrong critical path.
3. At 10K-symbol scale, sqlite-vec brute-force scan is comfortably under REQ-04 100ms p95 budget, even before int8 quantization. ANN is not on critical path until ~100K symbols, well past MVP.
4. Operability: `sqlite3 poc.db` opens the production database interactively. redb has no equivalent. Matters for AI-driven debugging.
5. Migration optionality is preserved by D-R2 trait lock. If sqlite-vec is ever outgrown (1M-scale, multi-process), the swap target is more likely qdrant or lancedb than redb anyway. The trait absorbs either swap.

### Why redb is rejected

- **Wrong abstraction level.** KV is a primitive; CodeNexus needs an index database. Adding tantivy + hnsw_rs on top of redb to recover parity is a worse architecture than just using SQLite -- three files, three failure modes vs one.
- **Throws out R3 BM25 lock.** FTS5 column-weighted bm25 + search_blob decompose was the R3 plateau lever. Tantivy weighting model is different; porting cost is unacknowledged in any "redb is faster" framing. ARCHITECTURE.md 9.2 calls search_blob "mandatory for any FTS5-based BM25 path"; redb forces a re-tune.
- **Vector primitive missing.** Rust HNSW crates exist but each is a separate store, separate snapshot, separate version drift surface. sqlite-vec ships in the same .db file -- atomic transactions span KV + FTS + vec.
- **No JOIN.** Section 9.7 graph traversal explicitly does `JOIN edges WHERE kind IN (...) AND confidence >= ...`. One SQL line in rusqlite. In redb it is a manual adjacency-list walk with per-edge filter loop. Every new query shape pays this tax.
- **Ecosystem fit.** sqlite3 CLI, EXPLAIN QUERY PLAN, online community, AI training data all overwhelmingly favor SQLite for this workload class.

### What we accept by picking rusqlite + sqlite-vec

- sqlite-vec 0.1.x pre-1.0 churn risk. Mitigation: pin exact crate version, smoke test on upgrade, surface is small.
- No ANN at 1M-scale. Acceptable: not Phase 3 critical path.
- ~2MB binary size cost vs pure-Rust redb. Negligible vs REQ-08 150MB envelope.
- Slight write-perf disadvantage on bulk insert vs redb 4.1 optimized cache. Not a bottleneck at our index-rebuild cadence.

### Migration plan from POC

POC already uses rusqlite. Migration to "rusqlite + sqlite-vec" is incremental:

1. Add `sqlite-vec = "0.1"` to Cargo.toml; register `sqlite3_auto_extension(sqlite3_vec_init)` in `Store::open`.
2. Add `CREATE VIRTUAL TABLE symbols_vec USING vec0(...)` to schema.
3. Mirror `INSERT INTO symbols_vec` in `Store::insert` after existing `INSERT INTO symbols`.
4. Replace `Store::all_embeddings` + Rust-side cosine in search.rs with `SELECT symbol_id, distance FROM symbols_vec WHERE embedding MATCH ? ORDER BY distance LIMIT 50`.
5. Re-run the 30-query eval set; expect parity (vector path is mathematically the same brute-force cosine, just running in C instead of Rust).
6. Update ARCHITECTURE.md section 9.6 with locked pick + pointer to storage-bench/results.md if spike was run.

Total ETA: 2 days plus eval re-run.

## Unresolved Questions

1. **sqlite-vec int8 quantization timing.** R3 used FP32 cosine. Moving to int8 quantized vectors gets ~3x speedup [4] but introduces a small recall hit. Recommendation: ship FP32 for Phase 3 MVP (already under budget), revisit quantization in Phase 3+ once LLM-judge eval is in place (per section 9.4) so any recall delta is measurable.
2. **Spike: ship or skip?** Default position: ship the 60-90 min spike to get numbers for the lock commit. Skip only if Phase 3 schedule pressure makes it infeasible.
3. **Backup story for sqlite-vec virtual tables.** SQLite .backup API works on the whole file including virtual tables, but verify on a smoke fixture before treating it as solved.
4. **Future migration target if sqlite-vec is outgrown.** Likely candidates: qdrant (single-binary, gRPC) or lancedb (Rust-native, columnar). Not Phase 3 concern, but note in trait abstraction docstring so the next maintainer knows the swap targets are not redb.
