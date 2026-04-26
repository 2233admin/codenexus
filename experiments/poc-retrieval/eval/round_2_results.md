# POC Retrieval Round 2 Results

**Run date:** 2026-04-27
**Corpus:** D:/projects/obsidian-llm-wiki (TS only, **2116 symbols** — 4.5x R1's 474)
**Embedder:** ollama qwen3-embedding:0.6b (1024d) — **with Instruct prefix on queries** (passages raw)
**Parser:** Functions / Classes / Methods / Interfaces / TypeAliases / Enums / **all lexical_declaration top-level + nested** (R1 had only arrow-fn lexical)
**BM25:** SQLite FTS5 default unicode61, equal column weights (unchanged from R1)
**Eval matcher:** **case-insensitive** (R1 was case-sensitive); subjects removed from C5/C6/C8/C10 expected_paths

## Headline numbers vs R1

| Axis | R1 apparent | R1 real | R2 |  Δ vs R1 real | vs target |
|------|-------------|---------|-----|---------------|-----------|
| 1 — symbol exact   | 50.0% | 30%    | **65.0%** | +35 | ⚠️ ~70% target |
| 2 — semantic NL    | 2.5%  | 2.5%   | **42.5%** | +40 | ⚠️ 60% target — at GitNexus baseline 43%, NOT ahead |
| 3 — call relation  | 35.0% | ≈0%    | **30.0%** | inflation reduced, still some leak | data point still partial |
| Overall            | 29.2% | ≈10%   | **45.8%** | +35.8 | — |

## Per-query delta highlights

### Real wins (parser + Instruct fix)

| ID | R1 | R2 | What changed |
|----|------|------|--------------|
| A4 PROTECTED_DIRS    | 0    | 1.0 | parser now captures `const PROTECTED_DIRS = ...` |
| A6 validatedArgs     | 0    | 1.0 | parser drops arrow_function constraint |
| B1 fs fallback       | 0.5  | 1.0 | Instruct prefix → top-1 = obsidianAdapter |
| B2 preflight protected dirs | 0 | 1.0 | top-1 = PROTECTED_DIRS (semantic + snake_case BM25) |
| B3 search by tag     | 0    | 0.5 | partial — top-1 still `files` not `searchByTag` |
| B6 dry-run deletion  | 0    | 1.0 | top-1 = `dryRun` const |
| B7 register MCP tool | 0    | 1.0 | top-1 = `register` (was at top-5 in R1) |
| C4 error handler     | 0    | 1.0 | top-1 = `ValidationError` (matches "error") |
| C7 reads Node tags   | 0    | 1.0 | top-1 = `tags` (still partial inflation, see below) |

### "Regressions" (mostly bogus-removal)

| ID | R1 | R2 | Verdict |
|----|------|------|---------|
| A2 VaultBrainAdapter | 1.0  | 0.5 | **REAL regression** — parser now captures `const vbAdapter = ...` aliases, pushed class to top-3 |
| C5 concept_graph callees | 0.5 | 0 | bogus-fix — R1 0.5 was substring "compile" coincidence |
| C8 after MemUAdapter sync | 1.0 | 0 | bogus-fix — R1 1.0 was subject self-match |
| C10 GitNexusAdapter callees | 1.0 | 0 | bogus-fix — same |

So real axis 3 dropped from R1's inflated 35% to honest ≈30%. Predicted ablation (retrieval-only ≈ 0% on call-graph queries) **substantively confirmed** — the residual 30% is BM25 finding generic tokens like "tags" / "error" / "validate" that happen to live in axis 3 expected_paths.

### Stuck zeros (architectural / corpus)

- A5 / A7 / B4 / C5 / C6 — Python file targets (`concept_graph.py`, `kb_meta.py`); POC is TS-only.
- B8 / B9 / B10 — semantic concepts (concurrent writes, conflict detection, metadata aggregation) the obsidian-llm-wiki codebase doesn't actually implement.
- B5 (NEG) — top-1 RRF still > 0.025 → -0.25 confident-wrong.
- C1 / C2 / C3 — POC retrieves the queried subject (assertRealPathInsideVault, ObsidianAdapter, FilesystemAdapter), not the callers/callees. **REQ-02 CALLS edge graph is necessary.**

## BM25 tokenization probe (Round 2 review checkpoint #1)

User's pre-Round-2 hypothesis: BM25 tokenization missing camelCase decomposition is contributing wrong signal.

**Verified**, with refinement:

| Probe | Returns | Interpretation |
|-------|---------|----------------|
| `walkSubtree` | walkSubtree only | exact-token match, single token |
| `walk` | walkMd, walk(s) — **NOT walkSubtree** | confirms walkSubtree is one indivisible token |
| `subtree` | **empty** | confirms camelCase NEVER splits |
| `protected` | PROTECTED_DIRS, walk, subDirs... | snake_case DOES split (FTS5 unicode61 treats `_` as separator) |
| `PROTECTED_DIRS` | PROTECTED_DIRS (multi-row) | snake_case query also splits |

**Refined diagnosis**: SQLite FTS5 default `unicode61` tokenizer:
- ✅ Splits on `_` (snake_case → tokens) — A4 PROTECTED_DIRS works because of this
- ✅ Splits on whitespace
- ❌ Does NOT split camelCase

**Implication**: `assertRealPathInsideVault` indexes as one token. Queries like "real path" / "path inside vault" cannot find it via BM25. Vector embedding (now with Instruct prefix) DOES find it because it sees full identifier and decodes its semantic content. But BM25 contributes wrong signal — pulling other symbols whose snippets happen to mention "real" or "path" as standalone tokens.

This is the second ceiling axis 2 hits beyond the Instruct prefix.

## What axis 2 = 42.5% really means

42.5% **ties GitNexus's reported 43%**. But:
- GitNexus uses Snowflake-arctic-embed-xs at 22M params
- POC uses qwen3-embedding:0.6b at 595M params (27x larger)
- Tying with a 27x larger model means **the embedder is held back by something else** — almost certainly BM25 contributing wrong signal in RRF fusion.

The 60% target is reachable but **not by embedder alone** — needs BM25 cleanup or post-fusion reranking.

## Round 3 proposal

Two paths to break 42.5% → 60%+ on axis 2.

### Path A — BM25 cleanup (cheap, ~30 LOC)

1. **camelCase decomposer**: Rust-side `decompose(s) -> String` turns `walkSubtree` → `walk subtree`, `assertRealPathInsideVault` → `assert real path inside vault`.
2. **Add `search_blob` column** to symbols table = `decompose(name) + " " + decompose(snippet)`. Index in FTS5 alongside name/snippet/kind.
3. **BM25 column weights**: `bm25(symbols_fts, 10, 1, 1, 5)` — name 10x, snippet 1x, kind 1x, search_blob 5x.

Expected: axis 1 → 75-80% (name weight dominates), axis 2 → 50-55% (BM25 stops poisoning RRF on camelCase queries).

### Path B — Cross-encoder reranker (industry SOTA, +1 dep)

1. After RRF top-50, pass `(query, symbol)` pairs through reranker model (e.g. ollama `bge-reranker-v2-m3` or via API).
2. Take top-5 by reranker score.

Expected: axis 2 → 60-70% (reranker is the strongest single lever for retrieval precision per literature).

### Recommended order

A first (cheap, fixes a real bug), then B if A doesn't clear 60%. A also helps axis 1 which still has parser noise (A2 regression).

### Path C — Bigger embedder (expensive, slow)

qwen3-embedding-8b (4096d) is what memU uses ("Phase 0a 升维中" per MEMORY.md). Could provide further axis-2 lift but indexing time goes from 1m43s to maybe 8-15min on ollama HTTP. Held until A and B exhaust their gains.

## Negative threshold revisit (Round 2 review checkpoint #5)

R1 negatives' top-1 RRF: A9=0.0164, A10=0.0164, B5=0.0301, C9=0.0164.
R2 negatives' top-1 RRF: A9=0.025x, A10=0.025x, B5=0.025x, C9=0.025x (RRF cap given larger candidate pool).

A9, A10, C9 all lucky-pass at threshold 0.025 — **threshold is right at the edge, fragile**. Round 3 should push this to 0.020 or use a contrastive metric (top-1 minus top-2 ratio) instead of absolute RRF.

## Provenance

- Round 1 baseline: `eval/baseline_v1_results.md`
- Round 1 raw results: `eval/results.json` (.gitignored)
- Round 2 raw results: `eval/results_round2.json` (.gitignored)
- Code at this point: poc-retrieval scaffold + 3 fixes (Instruct prefix / parser broadening / case-insensitive matcher)
- REQ-01 in PROJECT.md updated 2026-04-27 to include Interfaces / TypeAliases / Enums / Lexical Constants
