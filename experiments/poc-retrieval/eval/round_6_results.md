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

## R6b — Position-bias randomization check (executed 2026-04-27)

Same protocol, but per-query A/B prompt slot is randomly flipped (`--randomize-pair-order`, seeds 42/43/44 for 3 runs). LLM raw verdict re-attributed back to real-A/real-B based on the recorded `shown_first` flag. Audit: also count "first-shown wins / second-shown wins" position-only to estimate the bias magnitude.

### R6b headline (3 runs)

| Run | seed | A_first / B_first split | real_A wins | real_B wins | tie | err | 1st-shown wins | 2nd-shown wins | wall (s) |
|-----|------|--------------------------|-------------|-------------|-----|-----|-----------------|------------------|----------|
| 1 | 42 | 14/16 | 13 | 8 | 7 | 2 | 11 | 10 | 21.1 |
| 2 | 43 | 14/16 | 12 | 10 | 4 | 4 | 14 | 8 | 37.5 |
| 3 | 44 | 12/18 | 9 | 8 | 7 | 6 | 9 | 8 | 18.2 |
| **Σ** | — | 40/50 | **34** | **26** | 18 | 12 | 34 | 26 | — |

### R6b vs R6 (real attribution comparison)

| Metric | R6 (no randomize) | R6b (randomized) | Δ |
|--------|-------------------|------------------|----|
| A wins | 36 / 81 (44.4%) | 34 / 78 (43.6%) | -0.8 pp |
| B wins | 25 / 81 (30.9%) | 26 / 78 (33.3%) | +2.4 pp |
| tie | 20 / 81 (24.7%) | 18 / 78 (23.1%) | -1.6 pp |
| err | 9 / 90 (10.0%) | 12 / 90 (13.3%) | +3.3 pp |

R6b's err uptick to 13% reproduces R6's known MiniMax-empty-response stochasticity (~10% baseline, no exception → tenacity skips retry).

Chi² for R6b: 4.92, df=2, p ≈ 0.085 — **virtually identical to R6's 4.96 / p=0.084**.

### Position-only audit

Pooled across 3 runs: 1st-shown wins 34 / 78 (43.6%), 2nd-shown wins 26 / 78 (33.3%) — there IS a position effect (~10pp 1st-shown advantage), consistent with literature reports of 5-15% LLM-judge primacy bias.

But because the A_first / B_first split is roughly balanced (40 / 50 across 90 queries), the position effect distributes evenly across both real-A and real-B trials. **Position bias is real but does not drive R6's A>B finding** — the same A>B pattern holds when position is randomized.

### Verdict

**R6 finding stands: RRF-only (R3) trends ahead of RRF+rerank (R4) on this query set, but not statistically significant at α=0.05.** Position bias was a legitimate concern, R6b confirms it is not the explanation. The +13 margin in R6 (and +8 in R6b) is judge stochasticity + small-sample limitation, not artifact.

### Phase 3 implication

Reranker (R4 Path B with Jina) shows no clear lift over RRF-only (R3 Path A) under both pointwise (R5: κ tied) and pairwise (R6+R6b: trend toward A but p≈0.085) LLM-judge methodologies. ARCHITECTURE.md §9.3 "Path B underperforms Path A" call from R4 is now corroborated by 2 independent eval methods. Phase 3 reranker decision should NOT pick R4-style Jina reranker without first trying alternative candidates (Qwen3-Reranker-4B per §9.5).

## R6c -- Two-orderings consistent wins (executed 2026-04-27)

Per-query, both (A,B) and (B,A) orderings are run. A win is only counted "consistent" if both runs agreed. This is the standard literature mitigation for LLM-judge position bias (Zheng et al. 2023, Wang et al. 2023).

### 3-run headline table

| Run | consistent_A | consistent_B | inconsistent | tie_or_mixed | error | wall (s) |
|-----|-------------|-------------|-------------|-------------|-------|----------|
| Run 1 | 6 | 8 | 2 | 10 | 4 | 27.6 |
| Run 2 | 5 | 3 | 4 | 8 | 10 | 42.9 |
| Run 3 | 7 | 5 | 2 | 9 | 7 | 61.4 |
| **Pooled** | **18** | **16** | **8** | **27** | **21** | -- |

Each run: 30 queries x 2 orderings = 60 judge calls. Error rate is high in Run 2 (10/60 = 17%) due to okaoi empty-content stochasticity; not retried by tenacity (no exception raised).

### Aggregate verdict

- consistent_A=18, consistent_B=16, inconsistent=8, tie_or_mixed=27, error=21
- n_judged (excl. error + tie_or_mixed) = 34 of 90 query slots had a decisive consistent verdict
- **consistent_A margin: 18 vs 16 = +2** (vs R6 raw margin A=36 B=25 = +11, vs R6b A=34 B=26 = +8)
- **Inconsistency rate: 8/69 judged non-error = 11.6%** across all 3 runs

### Comparison to R6 and R6b

| Metric | R6 (single-order) | R6b (randomized) | R6c (consistent-wins) |
|--------|-------------------|------------------|-----------------------|
| A leads | 36 vs 25 (+11) | 34 vs 26 (+8) | 18 vs 16 (+2) |
| A% of decisive | 44% | 44% | 53% (18/34) |
| Method | naive | randomized | two-orderings |

**Key finding**: When controlling for position bias via two-orderings, the R6 A>B margin collapses from +11 to +2. The R6/R6b margin was substantially inflated by position bias and LLM stochasticity. With consistent-wins methodology, A and B are statistically indistinguishable: 18 vs 16 on only 34 decisive pairs out of 90.

### Inconsistency rate vs literature

Observed 11.6% inconsistency rate (8 of 69 non-error pairs flipped verdict across orderings). GPT-4 literature reports ~40% (Zheng et al. 2023). MiniMax-M2.7's lower rate likely reflects: (a) simpler task (code retrieval vs open-ended chat), (b) temp=0.0 forcing greedy decode, (c) high tie_or_mixed rate (27/69 = 39%) absorbing what would otherwise be inconsistent pairs -- when both orderings return "tie", that's classified as tie_or_mixed not inconsistent.

### Verdict

**R6c: R3 (RRF-only) and R4 (RRF+rerank) are indistinguishable under position-bias-immune two-orderings consistent-wins voting.** The previously observed A>B trend (R6: p=0.084, R6b: p=0.085) was not a methodological artifact per se -- randomization in R6b already confirmed that -- but the consistent-wins filter reveals the margin was driven by queries where the judge was internally inconsistent across orderings. Removing those, the remaining decisive pairs split nearly evenly (18:16).

This is the most rigorous R6 variant. The Phase 3 implication from R6b stands: no evidence to prefer R4 reranker over R3 RRF-only.

## Followups

### R6c followup -- Reranker threshold sweep

R4 used threshold=0.30. Try 0.15, 0.20, 0.25 with pairwise judge vs R3. If lower threshold recovers axis-2 wins for B, confirms over-filtering hypothesis.

### R7+ (unchanged from R5 backlog)

Chunk size sweep, static-graph augmented axis-3 eval, concept_absent flag, R3 graded re-judge, fine-tuned judge.

## Provenance

- Round A source: `eval/results_round3_a06_v2.json` (RRF-only, alpha=0.6)
- Round B source: `eval/results_round4_a06_rr_v2.json` (RRF + Jina rerank, pool=50, threshold=0.30)
- R6 raw judgments: `eval/round_6_results.json`, `_run2.json`, `_run3.json`
- R6 summaries: `eval/round_6_summary.json`, `_run2.json`, `_run3.json`
- R6b raw judgments: `eval/round_6b_results.json`, `_run2.json`, `_run3.json` (seeds 42/43/44, includes `shown_first` and `real_verdict` per query)
- R6b summaries: `eval/round_6b_summary.json`, `_run2.json`, `_run3.json`
- R6c raw judgments: `eval/round_6c_results.json` (=run1), `_run2.json`, `_run3.json` (per-query v_ab/v_ba/translate fields)
- R6c summaries: `eval/round_6c_summary.json` (=run1), `_run2.json`, `_run3.json`
- Code: `eval/ragas_spike.py` (extended for `--randomize-pair-order` + `--seed`), `eval/ragas_prompts.py` (ARM_PAIRWISE_PROMPT added), `eval/r6c_two_orderings.py` (new, two-orderings consistent-wins)
- LLM: okaoi MiniMax-M2.7 pool, concurrency=24, temp=0.0, max_tokens=500
- Wall: 15-21s/run (R6/R6b single-order); 28-62s/run (R6c two-orderings, 2x calls)
