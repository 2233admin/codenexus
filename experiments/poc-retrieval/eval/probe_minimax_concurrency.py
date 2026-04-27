"""Cheap probe for MiniMax 官方 endpoint concurrency ceiling.

Per ~/.claude/rules/common/feedback-graduated.md Rule 36 (cheap-probe-evidence-generation):
this is a wrong-but-cheap micro-slice that converts the subjective "I don't know how
hard I can hammer minimaxi" into a structured number (max-stable-N + p95 latency at
ceiling). The number gates ARCH §9.4 Phase 3 Gate LLM-judge batch sizing — we cannot
plan a 100-query x 2-corpora x graded-relevance eval without knowing the throughput
wall.

Reuses .env + EVAL_PROVIDER pattern from r7b_llm_judge_axis3.py. No new deps.

Usage:
    cd experiments/poc-retrieval/eval
    uv run python probe_minimax_concurrency.py [--max-N 96] [--reps 1]

Output:
    stdout: per-N table (ok/429/err counts, p50/p95 latency)
    file:   probe_minimax_concurrency_result.json (raw per-call results, for post-hoc)
"""
from __future__ import annotations

import argparse
import asyncio
import itertools
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

import anthropic
from anthropic import AsyncAnthropic
from dotenv import load_dotenv

ROOT = Path(__file__).parent
load_dotenv(ROOT / ".env")

PROBE_PROMPT = "Reply with ONLY the single word: pong"
PROBE_MAX_TOKENS = 16


def build_clients() -> tuple[list[AsyncAnthropic], str]:
    provider = os.environ.get("EVAL_PROVIDER", "minimax_official").lower()
    if provider == "okaoi":
        base = os.environ["OKAOI_BASE_URL"]
        keys = [os.environ[f"OKAOI_KEY_{i}"] for i in (1, 2, 3)]
        model = os.environ.get("OKAOI_MODEL", "MiniMax-M2.7")
        return [AsyncAnthropic(base_url=base, auth_token=k, timeout=60.0) for k in keys], model
    base = os.environ["ANTHROPIC_BASE_URL"]
    token = os.environ["ANTHROPIC_AUTH_TOKEN"]
    model = os.environ.get("ANTHROPIC_MODEL", "MiniMax-M2.5")
    return [AsyncAnthropic(base_url=base, auth_token=token, timeout=60.0)], model


CLIENTS, MODEL = build_clients()
_CLIENT_CYCLE = itertools.cycle(CLIENTS)


async def one_call(idx: int) -> dict[str, Any]:
    client = next(_CLIENT_CYCLE)
    t0 = time.perf_counter()
    try:
        resp = await client.messages.create(
            model=MODEL,
            max_tokens=PROBE_MAX_TOKENS,
            messages=[{"role": "user", "content": PROBE_PROMPT}],
            temperature=0.0,
        )
        dt = time.perf_counter() - t0
        text = ""
        for block in resp.content or []:
            if getattr(block, "type", None) == "text":
                text = block.text
                break
        return {
            "idx": idx,
            "status": 200,
            "dt": dt,
            "text": (text or "").strip()[:32],
            "input_tokens": getattr(resp.usage, "input_tokens", None),
            "output_tokens": getattr(resp.usage, "output_tokens", None),
        }
    except anthropic.RateLimitError as e:
        dt = time.perf_counter() - t0
        return {"idx": idx, "status": 429, "dt": dt, "error": f"RateLimitError: {str(e)[:160]}"}
    except anthropic.APIStatusError as e:
        dt = time.perf_counter() - t0
        return {"idx": idx, "status": getattr(e, "status_code", -1), "dt": dt, "error": f"APIStatusError {getattr(e,'status_code','?')}: {str(e)[:160]}"}
    except (anthropic.APITimeoutError, asyncio.TimeoutError) as e:
        dt = time.perf_counter() - t0
        return {"idx": idx, "status": -2, "dt": dt, "error": f"Timeout: {str(e)[:160]}"}
    except Exception as e:
        dt = time.perf_counter() - t0
        return {"idx": idx, "status": -1, "dt": dt, "error": f"{type(e).__name__}: {str(e)[:160]}"}


async def burst(n_concurrent: int) -> list[dict[str, Any]]:
    coros = [one_call(i) for i in range(n_concurrent)]
    return await asyncio.gather(*coros)


def summarize(N: int, results: list[dict[str, Any]]) -> dict[str, Any]:
    by_status: dict[int, int] = {}
    for r in results:
        by_status[r["status"]] = by_status.get(r["status"], 0) + 1
    ok_dts = sorted([r["dt"] for r in results if r["status"] == 200])
    p50 = ok_dts[len(ok_dts) // 2] if ok_dts else None
    p95 = ok_dts[max(0, int(len(ok_dts) * 0.95) - 1)] if ok_dts else None
    first_err = next((r for r in results if r["status"] != 200), None)
    return {
        "N": N,
        "by_status": by_status,
        "ok_count": by_status.get(200, 0),
        "ok_rate": by_status.get(200, 0) / N if N else 0.0,
        "p50_s": round(p50, 2) if p50 is not None else None,
        "p95_s": round(p95, 2) if p95 is not None else None,
        "first_error_status": first_err["status"] if first_err else None,
        "first_error_msg": (first_err.get("error") or "")[:120] if first_err else None,
    }


def fmt_status_counts(by_status: dict[int, int]) -> str:
    parts = []
    for code in sorted(by_status):
        parts.append(f"{code}={by_status[code]}")
    return ",".join(parts)


async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sweep", default="1,4,8,16,32,48,64,96",
                    help="comma-separated concurrency levels")
    ap.add_argument("--reps", type=int, default=1,
                    help="repetitions per N (smooth out noise)")
    ap.add_argument("--cooldown", type=float, default=3.0,
                    help="seconds to sleep between bursts (let token-bucket refill)")
    ap.add_argument("--stop-on-wall", action="store_true", default=True,
                    help="stop sweep when 429 ratio >= 0.2 (default on)")
    args = ap.parse_args()

    sweep = [int(x) for x in args.sweep.split(",") if x.strip()]
    provider = os.environ.get("EVAL_PROVIDER", "minimax_official")
    print(f"# provider={provider} model={MODEL} reps={args.reps} cooldown={args.cooldown}s",
          file=sys.stderr)
    print(f"{'N':>4} | {'rep':>3} | {'ok':>4} | {'429':>4} | {'other':>5} | {'p50':>6} | {'p95':>6} | first_err")
    print("-" * 88)

    all_results: list[dict[str, Any]] = []
    for N in sweep:
        wall_hit = False
        for rep in range(args.reps):
            t_burst = time.perf_counter()
            results = await burst(N)
            burst_wall = time.perf_counter() - t_burst
            s = summarize(N, results)
            other = sum(v for k, v in s["by_status"].items() if k not in (200, 429))
            err_str = "-" if not s["first_error_status"] else f"{s['first_error_status']} {s['first_error_msg'][:50] if s['first_error_msg'] else ''}"
            print(f"{N:>4} | {rep:>3} | {s['ok_count']:>4} | {s['by_status'].get(429,0):>4} | {other:>5} | {str(s['p50_s']):>6} | {str(s['p95_s']):>6} | {err_str}")
            all_results.append({
                "N": N,
                "rep": rep,
                "burst_wall_s": round(burst_wall, 2),
                "summary": s,
                "raw": results,
            })
            if s["by_status"].get(429, 0) / N >= 0.2:
                wall_hit = True
            if rep + 1 < args.reps:
                await asyncio.sleep(args.cooldown)
        if wall_hit and args.stop_on_wall:
            print(f"\n*** 429 wall hit at N={N} (>=20% rate); stopping sweep ***", file=sys.stderr)
            break
        await asyncio.sleep(args.cooldown)

    out_path = ROOT / "probe_minimax_concurrency_result.json"
    out_path.write_text(json.dumps({
        "provider": provider,
        "model": MODEL,
        "sweep_arg": args.sweep,
        "reps": args.reps,
        "results": all_results,
    }, indent=2, ensure_ascii=False))
    print(f"\nFull raw: {out_path}", file=sys.stderr)


if __name__ == "__main__":
    asyncio.run(main())
