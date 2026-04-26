# POC Retrieval Round 6 -- Pairwise LLM-Judge (R3 RRF-only vs R4 RRF+Rerank)

**Run date:** 2026-04-27
**Status:** **A (RRF-only, no rerank) leads pairwise 36 vs 25 B (rerank) across 3 runs, with 20 ties. Chi-square p=0.084 -- trend toward A but not significant at alpha=0.05. Reranker does not improve retrieval quality as judged by MiniMax-M2.7 pairwise comparison.**

## Motivation

R5 found arm A (binary) and arm B (graded) tied within stochastic noise on Cohen kappa (delta=+0.069 +/- 0.120 over 3 runs). The fundamental problem: pointwise judges score each (query, hit) independently, so the arms never directly compete. Pairwise judging fixes this -- the LLM is shown both top-5 result sets and asked "which is better?". This is:

- **10x cheaper**: 30 calls/round vs 300
- **Higher validity**: pairwise inter-rater agreement with humans is documented at >80% in RAG eval literature (vs ~60-70% for pointwise)
- **Direct signal**: forces the judge to compare configs, eliminating per-query calibration drift that muddies pointwise verdicts

R6 compares:
- **Round A** = R3 results (`results_round3_a06_v2.json`) -- RRF-only, alpha=0.6, no rerank
- **Round B** = R4 results (`results_round4_a06_rr_v2.json`) -- RRF + Jina rerank, pool=50, threshold=0.30

## Implementation

Extended `ragas_spike.py` with `--mode pairwise` flag (default `pointwise`, existing behavior preserved). New `run_pairwise()` loads both round files, builds per-query `set_a_block` / `set_b_block` (top-5 hits formatted as numbered list with snippet truncated to 300 chars), calls `ARM_PAIRWISE_PROMPT` once per query via the existing `call_judge()` / okaoi semaphore infrastructure. `safe_json()` extended with regex fallback to recover `verdict` from prose responses where MiniMax-M2.7 ignores "Output JSON ONLY". Output: per-query verdict+reason in `round_6_results.json`, aggregate counts in `round_6_summary.json`. 30 calls/run, wall ~15-21s @ concurrency=24.

## Headline table (3 runs)

| Run | wall (s) | A wins | B wins | tie | err | n_valid |
|-----|----------|--------|--------|-----|-----|---------|
| Run 1 | 20.9 | 13 | 6 | 7 | 4 | 26 |
| Run 2 | 15.3 | 12 | 8 | 8 | 2 | 28 |
| Run 3 | 15.3 | 11 | 11 | 5 | 3 | 27 |
| **Pooled** | -- | **36** | **25** | **20** | **9** | **81** |
| **Rate (of valid)** | -- | 44% | 31% | 25% | -- | -- |

Errors are okaoi pool returning empty content (transient, not retried by tenacity since no exception raised). Error rate ~11% (9/90 calls). Errors are distributed across queries, not systematic.

## Per-axis breakdown (pooled across 3 runs, valid verdicts only)

| Axis | n_valid | A wins | B wins | tie | A% | B% |
|------|---------|--------|--------|-----|----|----|
| 1 (symbol-exact) | 24 | 8 | 5 | 11 | 33% | 21% |
| 2 (semantic-NL) | 29 | 17 | 9 | 3 | 59% | 31% |
| 3 (call-relation) | 28 | 11 | 11 | 6 | 39% | 39% |

**Key finding**: A dominates axis 2 (semantic-NL queries) 17 vs 9. Axis 3 (call-relation) is a dead heat. Axis 1 (symbol-exact) shows slight A lead but high tie rate (46%), consistent with both configs finding the exact symbol equally well.

## Statistical note

Chi-square test vs uniform null (A=B=tie=27 each, n=81 valid verdicts, df=2):

- Chi2 = 4.963
- p = 0.084

**Not significant at alpha=0.05. Trend-level significance at alpha=0.10.** With N=3 runs and ~87% call success rate, the A lead is real in direction but not proven to significance threshold. A leads in 2/3 individual runs; Run 3 is tied (11:11:5). The axis-2 breakdown (17 vs 9, 59% vs 31%) is the most internally consistent signal.

## Interpretation

**RRF-only (R3, arm A) is not clearly worse than RRF+rerank (R4, arm B) by pairwise LLM judge.** In fact A leads 36:25 pooled, driven by axis-2 semantic queries -- exactly the axis where the Jina reranker was expected to add value.

Three possible explanations:

1. **Reranker hurts axis-2**: Jina reranker (threshold=0.30, pool=50) is over-filtering. R3 RRF retrieves semantically adjacent symbols that are genuinely helpful; reranker scores them below threshold and drops them.
2. **Prompt bias**: The pairwise judge may have position bias (preferring the first set shown). A is always presented as "Set A" (first). Cannot rule out without running A/B position swap.
3. **MiniMax-M2.7 stochasticity**: Run 3 is tied (11:11). With only 3 seeds the variance is high enough that the 36:25 pooled result is p=0.084, not definitive.

**Conservative recommendation**: Do not upgrade Phase 3 Gate verdict from R4 to "reranker proven beneficial". The pairwise signal is ambiguous -- at best neutral, at worst slightly anti-reranker. Investigate reranker threshold (0.30 may be too aggressive) before concluding.

## Error rate note

9/90 calls returned empty content from okaoi (no exception, no retry triggered). The `safe_json` regex recovery rescued 7 additional prose responses into valid verdicts (marked `_recovered=True` in per-query output). Net unrecoverable: 9 queries across 3 runs (~3/run). This is consistent with R5 okaoi behavior and does not affect run-to-run verdict distribution materially.

## Followups

### R6b -- Position swap (anti-bias check)

Re-run with A and B swapped in prompt order. If B-wins count rises significantly, position bias is the explanation. 30 calls, trivial to implement with `--swap` flag.

### R6c -- Reranker threshold sweep

R4 used threshold=0.30. Try 0.15, 0.20, 0.25 with pairwise judge vs R3. If lower threshold recovers axis-2 wins for B, confirms over-filtering hypothesis.

### R7+ (unchanged from R5 backlog)

Chunk size sweep, static-graph augmented axis-3 eval, concept_absent flag, R3 graded re-judge, fine-tuned judge.

## Provenance

- Round A source: `eval/results_round3_a06_v2.json` (RRF-only, alpha=0.6)
- Round B source: `eval/results_round4_a06_rr_v2.json` (RRF + Jina rerank, pool=50, threshold=0.30)
- R6 raw judgments: `eval/round_6_results.json`, `_run2.json`, `_run3.json`
- R6 summaries: `eval/round_6_summary.json`, `_run2.json`, `_run3.json`
- Code: `eval/ragas_spike.py` (extended), `eval/ragas_prompts.py` (ARM_PAIRWISE_PROMPT added)
- LLM: okaoi MiniMax-M2.7 pool, concurrency=24, temp=0.0, max_tokens=500
- Wall: 15-21s/run (vs 71-119s in R5 pointwise -- 5-8x faster)
