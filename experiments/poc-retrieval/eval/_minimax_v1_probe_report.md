# Minimax /v1/chat/completions endpoint probe

**Date:** 2026-04-27
**Goal:** Determine whether MiniMax `/v1/chat/completions` OpenAI-compat endpoint has independent quota from `/anthropic` Anthropic-compat (which Token-Plan-rate-limited at 13% 429 in R5 spike).

## Method

Same `sk-cp-...` Token Plan key used for the R5 `/anthropic` baseline, sent via Bearer auth to `https://api.minimaxi.com/v1`. Burst of 30 dummy JSON-output requests at concurrency=24 per model variant. Three models tested: `MiniMax-M2.7`, `MiniMax-M2.7-highspeed`, `MiniMax-M2.5`. Script: `_minimax_v1_probe.py` (gitignored `_*` pattern). Key read from `.env`, never printed or committed.

## Results

| Model | OK / 30 | wall (s) | p50 latency (s) | p95 latency (s) | throughput (req/s) | error sample |
|-------|---------|----------|-----------------|-----------------|---------------------|--------------|
| MiniMax-M2.7 | 30 / 30 | 4.43 | 2.13 | 2.60 | 6.78 | none |
| MiniMax-M2.7-highspeed | 30 / 30 | 3.74 | 1.82 | 2.17 | 8.02 | none |
| MiniMax-M2.5 | 20 / 30 | 3.45 | 1.54 | 2.67 | 5.80 | `429 Token Plan 主要面...` (same message as /anthropic) |

## Comparison vs /anthropic baseline (R5 spike anchor)

The R5 `/anthropic` run used MiniMax-M2.5 at concurrency=16 with 300 calls and hit 13% 429s. This probe used concurrency=24 with only 30 calls — a 10x smaller batch so a direct rate comparison is asymmetric. Key finding: at `/v1`, M2.5 still gets the identical "Token Plan" 429 at 33% error rate (10/30) even at 30 calls, while M2.7 and M2.7-highspeed delivered 0 errors at the same concurrency. This strongly suggests M2.5 has a tighter per-model quota under the Token Plan, not an endpoint-level difference.

## Verdict

- **Quota:** Shared — `/v1/chat/completions` uses the same Token Plan quota pool as `/anthropic`. M2.5 429s carry identical "Token Plan 主要面" message on both endpoints.
- **Recommendation:** Do NOT switch to `/v1` as a quota fix for M2.5. Continue okaoi 3-key pool for R6. However, **try M2.7-highspeed on `/v1` as a replacement model** — 0 errors, 8.0 req/s throughput (vs M2.5's 5.8 when not rate-limited), p95=2.17s.
- **M2.7-highspeed signal:** Faster than M2.7 — 8.02 vs 6.78 req/s, p50 1.82s vs 2.13s, p95 2.17s vs 2.60s. Meaningful ~18% throughput gain with no errors.
- **Followup:** (1) Benchmark M2.7-highspeed output quality against M2.5 on 10 eval queries before adopting for judge batch. (2) Test M2.7-highspeed at 300-call R6 scale via okaoi pool to confirm 0-error rate holds. (3) Check if Token Plan quota is per-model or aggregate — M2.7 family's clean run may mean it has a separate bucket.
