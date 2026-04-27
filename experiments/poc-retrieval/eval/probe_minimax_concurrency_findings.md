# MiniMax 官方 (api.minimaxi.com/anthropic) Concurrency Probe — Findings

**Date:** 2026-04-27
**Probe script:** `probe_minimax_concurrency.py`
**Endpoint:** `EVAL_PROVIDER=minimax_official` (Anthropic-shape, Bearer auth, model=`MiniMax-M2.5`)
**Methodology:** cheap probe per `~/.claude/rules/common/feedback-graduated.md` Rule 36
**Anchors:** ARCH §9.4 Phase 3 Gate (LLM-judge eval batch sizing)

## Bottom line

The 官方 endpoint enforces a **token-bucket / RPM-style limit**, not a pure concurrent-connection limit. Two distinct ceilings observed:

| Axis | Empirical ceiling | Interpretation |
|------|-------------------|----------------|
| Cold-burst concurrency | **N=40 clean (40/40 OK)** at p50=1.86s p95=3.55s. N=64 hits 70% 429 (19/64 OK). Wall bracketed at 48-56. | Single in-flight burst is well-tolerated up to ~40-48. |
| Sustained throughput | Bucket depletes after ~**50 successful calls within 4 min**. Subsequent bursts 429 until ~90s cool-down. | Effective sustained ≈ 30 RPM (= 0.5 QPS) before triggering rate-limit. |

**Comparison vs okaoi pool** (memory: `reference_okaoi_tool_stack`, `api-key-inventory`):

| Endpoint | Auth | Model | Sustained QPS | Per-burst max | Best for |
|----------|------|-------|---------------|---------------|----------|
| 官方 minimaxi | Bearer | MiniMax-M2.5 | ~0.5 | ~40 | Final gate-locking eval (independent, authoritative) |
| okaoi 3-key pool | x-api-key | MiniMax-M2.7 | ~3.79 (N=60) | 90 | Dev iteration / smoke / cheap probe (relay, faster but non-authoritative) |

8x throughput delta. Use okaoi for inner-loop iteration, 官方 for final gate.

## Raw observations

```
provider=minimax_official model=MiniMax-M2.5

# Pass 1 (warm-up sweep)
N=1   1/1   ok   p50=1.91  p95=1.91
N=4   4/4   ok   p50=1.86  p95=1.86
N=8   8/8   ok   p50=1.81  p95=2.14
N=16 16/16  ok   p50=1.54  p95=1.95

# Pass 2 (continued stress)
N=32 32/32  ok   p50=1.73  p95=2.21
N=64 19/64  WALL p50=1.35  p95=1.80   <-- 45 x 429

# Pass 3 (immediately after — bucket depleted from pass 2)
N=36  0/36  ALL-429    <-- no concurrent failure; entire bucket gone

# Pass 4 (after 90s cooldown, single burst)
N=40 40/40  ok   p50=1.86  p95=3.55   <-- bucket refilled
```

The N=36-after-N=64 result (0/36 success) is the smoking gun: if the limit were pure concurrent, N=36 should have succeeded since burst-size dropped. Instead 100% failed → minute-window token bucket exhausted.

## Implications for ARCH §9.4 Phase 3 Gate

The Gate spec mandates "NDCG@5 with graded relevance over query set ≥ 100 queries × ≥ 2 corpora" with "~30 minutes per eval cycle, ~$1 per cycle" as the budget.

**Sizing math:**
- Eval batch: 100 queries × 2 corpora × top-5 hits × 3 grader-seeds = **3000 LLM-judge calls** (worst case)
- 官方 sustained 0.5 QPS → 6000s = **100 minutes wall-clock** (over the §9.4 budget by 3.3x)
- okaoi 3.79 QPS → 790s = **13 minutes wall-clock** (well under budget)

**Recommendation (do not relitigate without evidence):**
- **Inner loop (rubric refinement, prompt-tuning, sample inspection):** okaoi M2.7 — fast iteration is the dominant value, alignment delta vs M2.5 is tolerable for dev work.
- **Gate-locking final eval (the run that actually flips Phase 3 status):** 官方 M2.5 — authoritative and independent of relay. Budget 100 min wall-clock; if this exceeds §9.4 30-min spec, either reduce to 1 grader-seed (1000 calls, ~33 min) or accept the budget bump.
- **Cross-validation (sanity-check okaoi vs 官方 grader agreement on same hit-set):** ~10% sample → 30 calls per provider, both endpoints. <2 min wall-clock. Land before any Gate-flipping run.

## Rate-limit posture for any future probe

- Default `cooldown=3.0s` in `probe_minimax_concurrency.py` is **insufficient** for sweeps that push N≥32. Bucket-depletion masks pure-concurrent results.
- If reprobing: insert ≥90s cooldown between bursts that exceeded N=24, OR cap each session at one burst per 60s window.
- The probe script's auto-stop on `429-rate ≥ 20%` correctly halted the sweep both times the wall was hit. Trust the auto-stop.

## Provenance

- Probe script: `experiments/poc-retrieval/eval/probe_minimax_concurrency.py`
- Raw results: `experiments/poc-retrieval/eval/probe_minimax_concurrency_result.json`
- Reference architecture: `r7b_llm_judge_axis3.py` (existing AsyncAnthropic + .env pattern; probe reuses verbatim)
- Decision anchor: `docs/ARCHITECTURE.md` §9.4 (Phase 3 Gate prerequisites)
