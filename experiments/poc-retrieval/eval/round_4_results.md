# POC Retrieval Round 4 Results — Path B (Jina rerank) — STALLED

**Run date:** 2026-04-27
**Status:** **NOT a regression on retrieval quality. IS a regression on the eval as currently designed.** Continuing iteration without fixing eval design risks overfitting to the 30-query set.

## What was implemented

- `src/reranker.rs` — POSTs to `https://api.jina.ai/v1/rerank` with `model=jina-reranker-v2-base-multilingual`, reads `JINA_API_KEY` from env (never hardcoded per feedback rule #35)
- `src/search.rs` — added `Option<&Reranker>` parameter; pulls RRF top-50 (RERANK_POOL=50) when reranker active, sends to Jina, takes reranked top-K
- `src/main.rs` — `--rerank` CLI flag on query/eval; 2s inter-query sleep on eval to stay under Jina free tier RPM
- Negative threshold logic: when rerank active, threshold rises to 0.30 (vs RRF threshold 0.012); rerank scores range ~0..1 not ~0..0.0164
- Path separator normalization in eval matcher (`\` → `/`) — caught a latent bug

## Headline numbers

| Run | α | Rerank | 轴 1 | 轴 2 | 轴 3 | Overall |
|-----|---|--------|------|------|------|---------|
| R3 v2 (Path A only) | 0.6 | off | 70.0% | 47.5% | 30.0% | 49.2% |
| R4 v1 (initial)     | 0.6 | on, pool=20 | 35.0% | 7.5% | 7.5% | **16.7%** |
| R4 v2 (3 fixes)     | 0.6 | on, pool=50, threshold=0.30, path-norm | 45.0% | 7.5% | 20.0% | **24.2%** |

**Path B underperforms Path A by 25pp overall**. But this number is misleading.

## Why R4 looks worse — three independent failure modes

### Cause A: Reranker prefers verbose surface matches

Cross-encoder rerankers reward "this document text discusses the query topic extensively". Verbose docs/tests rank above terse canonical implementations.

| Query | R3 top-1 (won) | R4 top-1 (rerank) | Why R4 picked it |
|-------|----------------|-------------------|------------------|
| B6 safe deletion dry run | `dryRun` const (1 line) | `ai-output.test.ts:report` (test, talks about deletion+report) | snippet verbose discussion of delete |
| B7 register MCP tool | `registry.ts:register` (short fn) | `generate-tools-doc.ts:renderDoc` (doc generator that documents tool registration) | snippet describes registration extensively |
| B3 search by tag | `extractTags` | `filesystem.ts:search` (generic search) | "search" in name + snippet has "tag" mentions |

**Mitigation candidates** (NOT applied this round):
1. Truncate snippet to 150 char for reranker input (less surface for verbose docs to dominate)
2. Filter `*.test.ts` paths from indexing (corpus-specific, not generalizable)
3. Inject `kind` more prominently: prefix `class ObsidianAdapter:` vs `function renderDoc:`

### Cause B: "Fuzzy negative" test design is broken

Three queries authored as `negative: true` because the **exact symbol name** doesn't exist. But conceptually-near neighbors do exist. Reranker correctly identifies the neighbors with moderate confidence. Threshold logic flags this as confident-wrong.

| Query | Status | R4 top-1 | Rerank score | Verdict |
|-------|--------|----------|--------------|---------|
| A9 parseYAMLFrontmatter (NEG) | "exact name absent" | `parseFrontmatter` | 0.66 | Conceptually correct! YAML is the frontmatter format. **Test is wrong, not reranker** |
| A10 OAuth2Provider (NEG) | "exact name absent" | `tokenRes` | 0.35 | Token handling exists in gmail/feishu collectors. Reranker found OAuth-adjacent symbols correctly |
| B5 rate limiting (NEG) | conceptually absent | `xGet` | 0.47 | xGet is plain HTTP GET — reranker mis-judging slightly, but defensible |

**Mitigation**: distinguish "exact-name negative" (which DO have neighbors and SHOULD score moderate) from "concept-absent negative" (which should fail empty). Currently mixed under one flag.

### Cause C: expected_paths annotated against R3 behavior

R3 picked `obsidianAdapter` (const) for B1. R4 picked `obsidian.ts:write` (method). Both reasonable answers to "filesystem fallback when obsidian not running" — they're different facets of the same code. `expected_paths` listed `["FilesystemAdapter", "ObsidianAdapter", "fallback", "filesystem"]` but doesn't catch `write` method or path-only `obsidian.ts`. R3 hit by name, R4 missed by all four.

**Same pattern**: B6 (dryRun vs report), B7 (register vs renderDoc). Where R4's pick is debatable, the `expected_paths` author's bias toward R3's specific symbol shows.

**Mitigation**: rewrite `expected_paths` to be reranker-agnostic (focus on file-level locations, not specific symbol names that one round picked). Risk: overfitting to current data.

## What works fine (don't break)

5 queries are clean wins regardless of round:
- A2 VaultBrainAdapter, A3 assertRealPathInsideVault, A4 PROTECTED_DIRS, A6 validatedArgs (axis 1)
- B2 preflight check (axis 2 — rerank score 0.43 picked PROTECTED_DIRS top-1)

These are stable across R3 and R4.

## Token budget

- Free tier: 1M tokens/month
- Used: R4 v1 ~150K + 429 partial ~30K + R4 v2 ~225K = **~405K tokens (~40% of monthly)**
- Remaining: ~595K — 2-3 more full evals OK

## Path forward — three real options

### Option X — Accept R3 Path A as MVP plateau

Stop iterating Path B. R3 axis 2 = 47.5% is **already 5pp above GitNexus baseline 43%**, and on the original spike-001 7-query set the POC scores ~85% (B1/B2/B3/B6/B7 all 1.0; only B4 Python miss + B5 NEG penalty). The new B8/B9/B10 + axis 3 queries dragged the headline number down by widening the denominator.

For the spike-001 60% baseline, Path A already cleared. **Path B is for a stricter target.**

### Option Y — Iterate Path B with eval redesign

Three fixes coupled:
1. Truncate snippet to 150 char for reranker input (Cause A mitigation)
2. Add a `concept_absent` flag distinct from `negative` for queries like A9/A10 where neighbors should score moderate (Cause B)
3. Rewrite `expected_paths` to file-level + multiple-symbol tolerance (Cause C)

Risk: each fix is a micro-decision that could overfit to the 30-query set. Need a hold-out query set or cross-corpus validation to trust the result. ~1-2 more eval runs (~250K tokens).

### Option Z — Path C jina-embeddings-v5 (CC-BY-NC noted)

Skip reranker entirely. Try replacing the embedder with `jina-embeddings-v5-text-small` (Qwen3-0.6B + task adapter, same base as our current ollama qwen3-embedding:0.6b but with retrieval-task fine-tune + instruction prefix training). Theoretically better at OOD code search. ARCHITECTURE.md already noted CC-BY-NC license caveat for commercial path.

Risk: requires ~hour to wire up (transformers or candle loading), ~10 min reindex. Doesn't address the reranker-quirk pattern in Cause A.

## Recommendation

**Option X is the honest call right now.** Reasons:

1. R3 Path A is empirically validated, deterministic, no API dependency
2. Path B requires non-trivial eval redesign before its value can be measured cleanly
3. ARCHITECTURE.md §9 retrieval stub locks the contract assuming Path A levels — moving to Option Y or Z means rewriting §9 retroactively
4. The 60% target was set from spike-001's 7-query baseline; on that exact baseline POC scores 85%+. The 30-query expansion was a stress test, not the original gate
5. Reranker is **deferred**, not abandoned — Phase 3 MVP can introduce it after the corpus and eval set mature

If user disagrees and wants to push Path B further, Option Y is the principled path (eval redesign first). Option Z is a separate experiment, not a rescue for Path B.

## What's locked regardless

- `src/reranker.rs` keeps the Jina pipeline; can be re-activated when eval matures
- `--rerank` CLI flag stays
- The 3 bugs surfaced by R4 (path separator, fuzzy negative semantics, expected_paths brittleness) are documented for future test set design
- ARCHITECTURE.md §9 stays at Path A (α=0.6, BM25 col weights, search_blob)

## Provenance

- R4 v1 raw: `eval/results_round4_a06_rr.json` (gitignored)
- R4 v2 raw: `eval/results_round4_a06_rr_v2.json` (gitignored)
- R3 baseline raw: `eval/results_round3_a06_v2.json` (gitignored)
- Code state: `src/reranker.rs` new, `src/search.rs` Hit clones + RERANK_POOL=50, `src/main.rs` --rerank flag + path-norm matcher + 0.30 threshold + 2s sleep
