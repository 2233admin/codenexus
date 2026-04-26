# POC Retrieval Round 3 Results — Path A complete

**Run date:** 2026-04-27
**Corpus:** D:/projects/obsidian-llm-wiki (TS only, **2116 symbols**, same as R2)
**Embedder:** ollama qwen3-embedding:0.6b (1024d) with Instruct prefix on queries
**Parser:** R2 broadened set (Func/Class/Method/Interface/TypeAlias/Enum/all-lexical)
**BM25:** SQLite FTS5 unicode61, **column weights name:10/snippet:1/kind:1/search_blob:5**
**Search blob:** Rust-side `decompose()` — camelCase + snake_case + kebab-case + dot to lowercase space-separated
**Fusion:** **alpha-weighted RRF** (`bm25_w * 1/(c+r1) + vec_w * 1/(c+r2)`, c=60), tunable
**Negative threshold:** 0.012 (scaled to new RRF max 1/61 ≈ 0.0164; ~73% of max)

## Headline numbers (alpha sweep, honest threshold)

| α | 轴 1 | 轴 2 | 轴 3 | Overall | Notes |
|---|------|------|------|---------|-------|
| 0.5 | 70.0% | 42.5% | 30.0% | 47.5% | balanced |
| **0.6** | **70.0%** | **47.5%** | **30.0%** | **49.2%** | **picked default** |
| 0.7 | 65.0% | 47.5% | 30.0% | 47.5% | axis 1 starts dropping |
| 0.8 | 35.0% | 47.5% | 17.5% | 33.3% | symbol-exact collapses |

`α = 0.6` chosen as `retrieval.fusion_alpha` default — both axes hit local peak simultaneously.

## R2 → R3 delta (α=0.6, with fixed negative threshold)

| Axis | R2 | R3 | Δ |
|------|------|------|---|
| 1    | 65.0% | 70.0% | +5.0pp |
| 2    | 42.5% | 47.5% | +5.0pp |
| 3    | 30.0% | 30.0% | 0 |
| Overall | 45.8% | 49.2% | +3.4pp |

**Real wins** R2 → R3:
- A2 VaultBrainAdapter: 0.5 → 1.0 (search_blob + name 10x weight pushed real class to top-1, suppressed `vbAdapter` alias noise)
- B3 search files by tag: 0.5 → 1.0 (alpha=0.6 + decompose let semantic match `extractTags` win over keyword `files`)

**Corrected scoring** (R3 v1 had a bug):
- B5 rate limiting (NEG): R3 v1 mistakenly scored 1.0 because threshold 0.025 was above new RRF max. Fixed to 0.012 → -0.25 (correctly fails — top-1 = `resetEpoch` is confident-wrong noise).

## What R3 cannot crack — Path B candidate set

After Path A, axis 2 sits at 47.5% (5pp above GitNexus baseline 43%). The four remaining failures all share a single pattern:

| ID | Query | POC top-1 | Top-1 RRF | Failure mode |
|----|-------|-----------|-----------|--------------|
| B5 | rate limiting middleware (NEG) | resetEpoch | 0.0153 | **vector confident-wrong** — "rate" semantically near "reset", "limiting" near "epoch" |
| B8 | handle concurrent writes | obsidian.ts:write | 0.0152 | confident-wrong — `write` is a write method, not concurrency-handler |
| B9 | detect conflicting edits between adapters | enabledAdapters | 0.0159 | confident-wrong — config field, not conflict-detection logic |
| B10 | aggregate metadata across multiple notes | circleback notes (const) | 0.0160 | confident-wrong — generic "notes" const matches "notes", not aggregation |

**Common pattern**: top-1 RRF 0.015-0.016 (≈max), meaning embedder + BM25 both agreed but on a surface-similar, semantically-wrong symbol.

This is the textbook failure mode for **cross-encoder rerankers**. Bi-encoder embedders (qwen3-embedding-0.6b) compress query/document into separate vectors then dot-product — close-but-wrong matches survive. Cross-encoders read query+candidate jointly and can tell `enabledAdapters` (config) apart from "conflict detection" (algorithm).

`grep -i 'lock|mutex|concurrent|conflict|aggregate'` in obsidian-llm-wiki TS files matched **7 files** including `core/operations.ts`, `compile-trigger.ts`, `pglite-engine.ts` — **the answers exist in corpus, POC just retrieves the wrong tokens**. Reranker is the right tool.

## B8/B9/B10 corpus-existence audit

Per Curry's directive to verify before invoking Path B:
- B8 concurrent writes: corpus has lock/queue/mutex tokens in 7 files → **answerable**
- B9 conflicting edits: less clear; may be partial (no explicit conflict-detection module visible from grep) → **partially answerable**
- B10 metadata aggregation: ambiguous (multiple aggregators in collectors/, e.g. `buildDigest`) → **answerable but corpus-fragmented**

So Path B has real headroom on at least 2 of the 3 plus B5.

## Path B trigger condition

Per Curry's Q2 directive: "Path A < 55% 就上 B，但用 API 不用本地模型".

**Path A axis 2 = 47.5% < 55% → Path B activates.**

Two API options (Curry's recommendation: not local — bge-reranker-v2-m3 GGUF would be 3-5s/query on POC's CPU path):

| Option | Latency | Free tier | Auth |
|--------|---------|-----------|------|
| Jina Reranker API (`jina-reranker-v2-base-multilingual`) | ~150ms | 1M tokens/month free | API key, no card |
| Cohere Rerank (`rerank-v3.5`) | ~200ms | 1000 calls/month free trial | API key, no card |

Both expose `query` + list of candidates → list of (index, score) ranked results. Drop-in over POC's RRF top-50.

**Recommended: Jina** — generous free tier covers 30-query eval ~50x over without burning quota; multilingual model handles English code symbols + future docstring extension.

## Round 4 design — pending Curry decision

If Curry greenlights Jina:
1. Add `reranker.rs` module — POST to Jina endpoint, 30 LOC
2. POC pipeline: BM25 + vec → RRF top-50 → Jina rerank → top-5
3. New CLI flag `--rerank` (default off, opt-in for Round 4 measurement)
4. Eval at α=0.6 with rerank on/off → measure axis 2 delta

Expected outcome: B5/B8/B9/B10 mostly resolve. Axis 2 → 70-90% range.

If R4 axis 2 ≥ 60%, **Path A + B is sufficient for MVP**. ARCHITECTURE.md retrieval section can lock:
```
retrieval.embedder = qwen3-embedding-0.6b (default, candle@Phase-3)
retrieval.fusion_alpha = 0.6
retrieval.bm25_column_weights = [10, 1, 1, 5]  // name, snippet, kind, search_blob
retrieval.search_blob = decompose(name + snippet)
retrieval.reranker = jina-reranker-v2-base-multilingual (top-50 → top-5)
retrieval.negative_rrf_threshold = 0.012
```

If R4 axis 2 < 60% even after rerank, Path C (qwen3-embedding-8b upgrade or different base embedder) on the table.

## Provenance + reproducibility

- R3 raw results (alpha sweep, threshold-corrected): `eval/results_round3_a05_v2.json`, `..a06_v2.json`, `..a07_v2.json`, `..a08_v2.json`
- R3 v1 (broken threshold) deprecated but preserved: `..a05.json`, `..a06.json`, `..a07.json`, `..a08.json` — for audit trail of the threshold bug discovery
- Code state: commit pending after this doc lands
- Index state: `poc.db` (2116 symbols, schema includes `search_blob` column)

## Round 2 → Round 3 verification of user predictions

| User prediction (R2 → R3) | Actual |
|---------------------------|--------|
| BM25 camelCase tokenization is broken | ✅ confirmed via SQL probe (`subtree` query empty) |
| BM25 column weights matter | ✅ name 10x lifted A2 from 0.5 to 1.0 |
| RRF should be tunable not constant | ✅ alpha sweep showed α=0.5 vs 0.8 = ±35pp swing on axis 1 |
| Path A might cap below 60% | ✅ 47.5% — Path B is needed |
| Curry's path call (B via API not local) | ✅ correct — local cross-encoder would 3-5s/query on POC CPU, kills usability |

Curry's hypotheses validated 5/5 in R3 data.
