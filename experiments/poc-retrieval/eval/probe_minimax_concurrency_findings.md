# MiniMax 官方 (api.minimaxi.com/anthropic) Concurrency Probe — Findings

**Date:** 2026-04-27
**Probe script:** `probe_minimax_concurrency.py`
**Endpoint:** `EVAL_PROVIDER=minimax_official` (Anthropic-shape, Bearer auth, model=`MiniMax-M2.5`)
**Methodology:** cheap probe per `~/.claude/rules/common/feedback-graduated.md` Rule 36
**Anchors:** ARCH §9.4 Phase 3 Gate (LLM-judge eval batch sizing)

## Bottom line

The 官方 endpoint enforces a **classic token-bucket**: capacity ≈ **80 tokens**, refill rate ≈ **0.5 tokens/sec (= 30 RPM steady-state)**. No per-burst concurrent cap separate from the bucket. The early "wall at N=64" reading was a misdiagnosis caused by prior bursts depleting the bucket — when retested from a 5-min idle (full bucket), N=64 passed cleanly.

| Axis | Empirical ceiling | Interpretation |
|------|-------------------|----------------|
| Cold-burst (single in-flight burst from full bucket) | **N=64 clean** (64/64 OK, p50=1.95s, p95=2.32s). **N=96 = 80/96** (16 × 429). | Bucket capacity ≈ 80 — burst of 80 from cold passes; anything beyond 80 in <2s 429s. |
| Sustained 1 QPS × 30s | **30/30 OK** (5/5 per 5s bucket, no 429) | 1 QPS sustained well below ceiling. |
| Sustained 2 QPS × 30s | **60/60 OK** (10/10 per 5s bucket, no 429) | 2 QPS sustained also clean *for the 30s window* — works because (bucket 80) + (refill 30s × 0.5 = 15) = 95 ≥ 60. |
| Sustained 4 QPS × 30s | 80/120 OK, 19 × 429, 21 × err. **Wall hits at exactly t=20s**, the moment bucket exhausts: 4 QPS × 20s = 80 calls = bucket capacity. | At 4 QPS, consumption (4/s) > refill (0.5/s), bucket drains in 80/3.5 ≈ 23s. Matches observation. |
| True sustained-forever ceiling | ≈ **0.5 QPS = 30 RPM** | Equal to refill rate. Above this, eventually walls regardless of duration. |

**Comparison vs okaoi pool** (memory: `reference_okaoi_tool_stack`, `api-key-inventory`):

| Endpoint | Auth | Model | Sustained QPS (forever) | Cold-burst peak | Best for |
|----------|------|-------|------------------------|-----------------|----------|
| 官方 minimaxi | Bearer | MiniMax-M2.5 | **0.5 QPS (= 30 RPM)** | **80** | Final gate-locking eval (authoritative, independent) |
| okaoi 3-key pool | x-api-key | MiniMax-M2.7 | 3.79 QPS (N=60) | ~90 | Dev iteration / smoke (faster but relay, non-authoritative) |

Sustained delta is ~8x; burst delta is ~1.1x (comparable). The relay's value is for >5-minute sustained workloads; for time-bounded eval cycles, 官方 is competitive thanks to the 80-token burst headroom.

## Raw observations

```
provider=minimax_official model=MiniMax-M2.5

# Pass 1-2 (warm-up + stress, bucket gradually depleting)
N=1   1/1   ok   p50=1.91
N=4   4/4   ok   p50=1.86
N=8   8/8   ok   p50=1.81
N=16 16/16  ok   p50=1.54
N=32 32/32  ok   p50=1.73
N=64 19/64  partial wall (45 x 429) -- bucket already drained from prior 61 calls
N=36  0/36  ALL-429 (immediately after; bucket fully gone)

# Pass 4 (after 90s cooldown)
N=40 40/40  ok   p50=1.86, p95=3.55

# Pass 5 (after 5 min idle = bucket fully replenished)
N=64  64/64 ok   p50=1.95, p95=2.32  <-- earlier "wall at 64" disproven
N=96  80/96 ok   16 x 429              <-- exactly 80 succeed = bucket capacity

# Pass 6 (sustained-rate stream after 120s prewait)
1 QPS x 30s:  30/30  ok   (5/5 every 5s bucket -- no wall)
2 QPS x 30s:  60/60  ok   (10/10 every 5s bucket -- still no wall)
4 QPS x 30s:  80/120 ok, 19 x 429, 21 x err
              wall hits exactly at t=20s when 4 QPS x 20s = 80 calls = bucket capacity
              after that, 4 QPS sustained > 0.5/s refill, bucket stays empty
```

The smoking gun is now the N=64-cold = 64/64 ok (Pass 5) — directly contradicts Pass 2's 19/64 reading. Same N, opposite result, only difference is bucket fill state. This nails the diagnosis as token-bucket, not concurrent-connection limit. The 4 QPS stream test (Pass 6) cements the model: bucket=80 capacity, refill=0.5/s. Wall hits exactly at expected time (t=20s = 80/4).

## Implications for ARCH §9.4 Phase 3 Gate

The Gate spec mandates "NDCG@5 with graded relevance over query set ≥ 100 queries × ≥ 2 corpora" with "~30 minutes per eval cycle, ~$1 per cycle" as the budget.

**Sizing math (corrected after Pass 5-6 data):**

Standard token-bucket formula for total calls in T seconds: `total = capacity + T × refill_rate = 80 + 0.5T`.

| Eval batch size | 官方 wall-clock | okaoi wall-clock | §9.4 budget? |
|-----------------|----------------|------------------|--------------|
| 600 calls (100q × 2corp × top-5 × 0.6 hit-rate × 1 seed) | (600-80)/0.5 = **17.3 min** | 600/3.79 = 2.6 min | ✓ both fit |
| 1500 calls (× 3 seeds) | (1500-80)/0.5 = **47 min** | 1500/3.79 = 6.6 min | 官方 over by 1.6x |
| 3000 calls (worst-case 100q × 2corp × 5hits × 3 seeds) | (3000-80)/0.5 = **97 min** | 13 min | 官方 over by 3.2x |

**Recommendation (do not relitigate without evidence):**
- **Inner loop (rubric refinement, prompt-tuning, sample inspection):** okaoi M2.7 — fast iteration is dominant value; alignment delta vs M2.5 tolerable for dev work.
- **Gate-locking final eval at 1-seed × 600 calls:** 官方 M2.5 fits cleanly in 17 min — authoritative + independent + within §9.4 30-min spec. **Use 官方.**
- **Gate-locking eval at 3-seed × 1500-3000 calls:** Either (a) split into 3 sequential 600-call runs at 官方 (3 × 17 min = 51 min, exceeds §9.4 spec), or (b) run on okaoi after grader cross-validation passes. Open question — the 30-min §9.4 cap is itself a design parameter; with bucket-math evidence, the right move is to revise §9.4 to "60 min" rather than force okaoi.
- **Cross-validation (okaoi vs 官方 grader agreement on same hit-set):** ~10% sample → 30 calls per provider. <2 min wall-clock. Still queued, still prerequisite for okaoi inner-loop trust.

## Rate-limit posture for any future probe

- Default `cooldown=3.0s` in `probe_minimax_concurrency.py` is **insufficient** for sweeps that push N≥32. Bucket-depletion masks pure-concurrent results.
- If reprobing: insert ≥90s cooldown between bursts that exceeded N=24, OR cap each session at one burst per 60s window.
- The probe script's auto-stop on `429-rate ≥ 20%` correctly halted the sweep both times the wall was hit. Trust the auto-stop.

## Provenance

- Probe script: `experiments/poc-retrieval/eval/probe_minimax_concurrency.py`
- Raw results: `experiments/poc-retrieval/eval/probe_minimax_concurrency_result.json`
- Reference architecture: `r7b_llm_judge_axis3.py` (existing AsyncAnthropic + .env pattern; probe reuses verbatim)
- Decision anchor: `docs/ARCHITECTURE.md` §9.4 (Phase 3 Gate prerequisites)
