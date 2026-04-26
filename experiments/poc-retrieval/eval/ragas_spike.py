"""R5 LLM-judge spike — A/B 0-1 binary vs 0-3 graded.

Reads R4 retrieval output, looks up snippets from poc.db, calls a Minimax
model via Anthropic Messages protocol — twice per (query, top-5 hit) pair,
once per arm, concurrently.

Providers (set EVAL_PROVIDER):
  minimax_official  — single key via api.minimaxi.com/anthropic   (default)
  okaoi             — 3-key pool via www.okaoi.com/v1, round-robin (90 parallel ceiling)

Usage:
  uv run python ragas_spike.py --round 4 --arm both
  uv run python ragas_spike.py --round 4 --arm both --limit 2  (smoke)
"""

import argparse
import asyncio
import itertools
import json
import os
import sqlite3
import sys
import time
from pathlib import Path
from typing import Any

import anthropic
from anthropic import AsyncAnthropic
from dotenv import load_dotenv
from tenacity import (
    retry,
    retry_if_exception_type,
    stop_after_attempt,
    wait_exponential,
)

from ragas_prompts import (
    ARM_A_BINARY_PROMPT,
    ARM_B_GRADED_PROMPT,
    JUDGE_SYSTEM,
)

ROOT = Path(__file__).parent
DB = ROOT.parent / "poc.db"
QUERIES = ROOT / "queries.json"

load_dotenv(ROOT / ".env")


def build_clients() -> tuple[list[AsyncAnthropic], str]:
    """Return (clients, model). Round-robin across clients for okaoi pool."""
    provider = os.environ.get("EVAL_PROVIDER", "minimax_official").lower()
    if provider == "okaoi":
        base = os.environ["OKAOI_BASE_URL"]
        keys = [os.environ[f"OKAOI_KEY_{i}"] for i in (1, 2, 3)]
        model = os.environ.get("OKAOI_MODEL", "MiniMax-M2.7")
        clients = [
            AsyncAnthropic(base_url=base, auth_token=k, timeout=60.0) for k in keys
        ]
        return clients, model
    base = os.environ["ANTHROPIC_BASE_URL"]
    token = os.environ["ANTHROPIC_AUTH_TOKEN"]
    model = os.environ.get("ANTHROPIC_MODEL", "MiniMax-M2.5")
    return [AsyncAnthropic(base_url=base, auth_token=token, timeout=60.0)], model


CLIENTS, MODEL = build_clients()
_CLIENT_CYCLE = itertools.cycle(CLIENTS)
CONCURRENCY = int(os.environ.get("EVAL_CONCURRENCY", "16"))
RETRY_MAX = int(os.environ.get("EVAL_RETRY_MAX", "5"))


def parse_hit(hit: str) -> tuple[str, str]:
    i = hit.rfind(":")
    return hit[:i], hit[i + 1 :]


def load_snippet(cur, hit: str) -> dict[str, Any] | None:
    path, name = parse_hit(hit)
    for p in (path, path.replace("\\", "/")):
        cur.execute(
            "SELECT path, name, kind, snippet FROM symbols WHERE path=? AND name=? LIMIT 1",
            (p, name),
        )
        r = cur.fetchone()
        if r:
            return {"path": r[0], "name": r[1], "kind": r[2], "snippet": r[3]}
    return None


def safe_json(raw: str) -> dict[str, Any]:
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        s = raw.find("{")
        e = raw.rfind("}")
        if s >= 0 and e > s:
            try:
                return json.loads(raw[s : e + 1])
            except json.JSONDecodeError:
                pass
    return {"_parse_error": True, "raw": raw[:300]}


@retry(
    stop=stop_after_attempt(RETRY_MAX),
    wait=wait_exponential(multiplier=1, min=1, max=30),
    retry=retry_if_exception_type(
        (
            anthropic.APIError,
            anthropic.APITimeoutError,
            anthropic.APIConnectionError,
            asyncio.TimeoutError,
        )
    ),
    reraise=True,
)
async def call_judge(prompt: str) -> dict[str, Any]:
    client = next(_CLIENT_CYCLE)
    resp = await client.messages.create(
        model=MODEL,
        max_tokens=500,
        system=JUDGE_SYSTEM,
        messages=[{"role": "user", "content": prompt}],
        temperature=0.0,
    )
    text = ""
    for block in resp.content:
        if getattr(block, "type", None) == "text":
            text = block.text
            break
    return safe_json(text or "")


async def judge_pair(sem: asyncio.Semaphore, query: str, snip: dict, arms: str):
    async with sem:
        snippet_truncated = snip["snippet"][:2000]
        coros = []
        if arms in ("A", "both"):
            coros.append(
                call_judge(
                    ARM_A_BINARY_PROMPT.format(
                        query=query,
                        path=snip["path"],
                        kind=snip["kind"],
                        name=snip["name"],
                        snippet=snippet_truncated,
                    )
                )
            )
        if arms in ("B", "both"):
            coros.append(
                call_judge(
                    ARM_B_GRADED_PROMPT.format(
                        query=query,
                        path=snip["path"],
                        kind=snip["kind"],
                        name=snip["name"],
                        snippet=snippet_truncated,
                    )
                )
            )
        return await asyncio.gather(*coros, return_exceptions=True)


def cohen_kappa(a: list[int], b: list[int]) -> float:
    if len(a) != len(b) or len(a) == 0:
        return float("nan")
    n = len(a)
    po = sum(1 for x, y in zip(a, b) if x == y) / n
    p_a1 = sum(a) / n
    p_b1 = sum(b) / n
    pe = p_a1 * p_b1 + (1 - p_a1) * (1 - p_b1)
    if pe >= 1.0:
        return 1.0 if po == 1.0 else float("nan")
    return (po - pe) / (1 - pe)


def spearman(x: list[float], y: list[float]) -> float:
    if len(x) != len(y) or len(x) < 2:
        return float("nan")

    def rank(v):
        sorted_v = sorted(enumerate(v), key=lambda t: t[1])
        ranks = [0.0] * len(v)
        i = 0
        while i < len(sorted_v):
            j = i
            while j + 1 < len(sorted_v) and sorted_v[j + 1][1] == sorted_v[i][1]:
                j += 1
            avg_rank = (i + j) / 2 + 1
            for k in range(i, j + 1):
                ranks[sorted_v[k][0]] = avg_rank
            i = j + 1
        return ranks

    rx = rank(x)
    ry = rank(y)
    n = len(x)
    mx = sum(rx) / n
    my = sum(ry) / n
    num = sum((rx[i] - mx) * (ry[i] - my) for i in range(n))
    dx = sum((r - mx) ** 2 for r in rx) ** 0.5
    dy = sum((r - my) ** 2 for r in ry) ** 0.5
    if dx == 0 or dy == 0:
        return float("nan")
    return num / (dx * dy)


async def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--round", type=int, default=4)
    parser.add_argument("--arm", choices=["A", "B", "both"], default="both")
    parser.add_argument("--limit", type=int, default=0, help="0=all queries")
    parser.add_argument("--out", default="round_5_results.json")
    parser.add_argument("--summary", default="round_5_summary.json")
    args = parser.parse_args()

    with open(QUERIES, "r", encoding="utf-8") as f:
        queries_by_id = {q["id"]: q for q in json.load(f)}

    src = ROOT / f"results_round{args.round}_a06_rr_v2.json"
    if not src.exists():
        src = ROOT / f"results_round{args.round}.json"
    print(f"Source: {src.name}", file=sys.stderr)
    with open(src, "r", encoding="utf-8") as f:
        round_data = json.load(f)
    if args.limit > 0:
        round_data = round_data[: args.limit]

    conn = sqlite3.connect(str(DB))
    cur = conn.cursor()

    tasks_meta = []
    for entry in round_data:
        qid = entry["id"]
        q = queries_by_id[qid]
        for idx, hit_str in enumerate(entry["top5"]):
            snip = load_snippet(cur, hit_str)
            tasks_meta.append(
                {
                    "qid": qid,
                    "axis": entry["axis"],
                    "query": q["query"],
                    "expected_paths": q.get("expected_paths", []),
                    "negative": entry.get("negative", False),
                    "hit_idx": idx,
                    "hit_str": hit_str,
                    "snip": snip,
                    "hand_p_at_5": entry.get("precision_at_5"),
                }
            )
    conn.close()

    valid = [t for t in tasks_meta if t["snip"] is not None]
    provider = os.environ.get("EVAL_PROVIDER", "minimax_official")
    print(
        f"Tasks: {len(tasks_meta)} (valid: {len(valid)}, "
        f"missing snippet: {len(tasks_meta) - len(valid)}) | "
        f"concurrency={CONCURRENCY} | provider={provider} | model={MODEL}",
        file=sys.stderr,
    )

    sem = asyncio.Semaphore(CONCURRENCY)
    t0 = time.time()
    coros = [judge_pair(sem, t["query"], t["snip"], args.arm) for t in valid]
    results = await asyncio.gather(*coros, return_exceptions=True)
    wall = time.time() - t0

    for t, r in zip(valid, results):
        if isinstance(r, Exception):
            t["arm_a"] = {"_error": str(r)[:200]}
            t["arm_b"] = {"_error": str(r)[:200]}
            continue
        if args.arm == "both":
            a, b = (r[0], r[1]) if len(r) >= 2 else (r[0], None)
            t["arm_a"] = a if not isinstance(a, Exception) else {"_error": str(a)[:200]}
            t["arm_b"] = b if not isinstance(b, Exception) else {"_error": str(b)[:200]}
        elif args.arm == "A":
            t["arm_a"] = r[0] if not isinstance(r[0], Exception) else {"_error": str(r[0])[:200]}
        elif args.arm == "B":
            t["arm_b"] = r[0] if not isinstance(r[0], Exception) else {"_error": str(r[0])[:200]}

    (ROOT / args.out).write_text(
        json.dumps(tasks_meta, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    per_query: dict[str, dict] = {}
    for t in tasks_meta:
        d = per_query.setdefault(
            t["qid"],
            {
                "axis": t["axis"],
                "query": t["query"],
                "negative": t["negative"],
                "hand_p_at_5": t["hand_p_at_5"],
                "arm_a_v": [],
                "arm_b_g": [],
                "missing": 0,
                "errors": 0,
            },
        )
        if t["snip"] is None:
            d["missing"] += 1
            continue
        a = t.get("arm_a", {})
        b = t.get("arm_b", {})
        if isinstance(a, dict) and "verdict" in a:
            v = a["verdict"]
            if isinstance(v, (int, float)):
                d["arm_a_v"].append(int(v))
        elif isinstance(a, dict) and ("_error" in a or "_parse_error" in a):
            d["errors"] += 1
        if isinstance(b, dict) and "grade" in b:
            g = b["grade"]
            if isinstance(g, (int, float)):
                d["arm_b_g"].append(int(g))

    rollup = []
    for qid, d in sorted(per_query.items()):
        a = d["arm_a_v"]
        b = d["arm_b_g"]
        rollup.append(
            {
                "qid": qid,
                "axis": d["axis"],
                "query": d["query"],
                "negative": d["negative"],
                "hand_p_at_5": d["hand_p_at_5"],
                "arm_a_p_at_5": (sum(a) / len(a)) if a else None,
                "arm_b_mean_grade": (sum(b) / len(b)) if b else None,
                "arm_b_max_grade": max(b) if b else None,
                "arm_b_p_at_5_at_t2": (sum(1 for g in b if g >= 2) / len(b)) if b else None,
                "n_judged": len(a) or len(b),
                "missing": d["missing"],
                "errors": d["errors"],
            }
        )

    a_bin: list[int] = []
    h_bin: list[int] = []
    for r in rollup:
        if r["arm_a_p_at_5"] is None or r["hand_p_at_5"] is None:
            continue
        a_bin.append(1 if r["arm_a_p_at_5"] >= 0.5 else 0)
        h_bin.append(1 if r["hand_p_at_5"] >= 0.5 else 0)
    kappa_a = cohen_kappa(a_bin, h_bin)
    match_a = sum(1 for x, y in zip(a_bin, h_bin) if x == y)

    b_grade: list[float] = []
    h_for_b: list[float] = []
    for r in rollup:
        if r["arm_b_mean_grade"] is None or r["hand_p_at_5"] is None:
            continue
        b_grade.append(r["arm_b_mean_grade"])
        h_for_b.append(r["hand_p_at_5"])
    spearman_b = spearman(b_grade, h_for_b)

    b_bin_at_t2: list[int] = []
    h_bin_at_t2: list[int] = []
    for r in rollup:
        if r["arm_b_p_at_5_at_t2"] is None or r["hand_p_at_5"] is None:
            continue
        b_bin_at_t2.append(1 if r["arm_b_p_at_5_at_t2"] >= 0.5 else 0)
        h_bin_at_t2.append(1 if r["hand_p_at_5"] >= 0.5 else 0)
    kappa_b_at_t2 = cohen_kappa(b_bin_at_t2, h_bin_at_t2)

    summary = {
        "n_queries": len(rollup),
        "n_judge_calls": sum(1 for t in tasks_meta if t["snip"] is not None)
        * (2 if args.arm == "both" else 1),
        "wall_clock_seconds": round(wall, 2),
        "concurrency": CONCURRENCY,
        "provider": provider,
        "model": MODEL,
        "arm_a_vs_hand_cohen_kappa": kappa_a,
        "arm_a_vs_hand_match": f"{match_a}/{len(a_bin)}",
        "arm_b_mean_vs_hand_spearman": spearman_b,
        "arm_b_at_t2_vs_hand_cohen_kappa": kappa_b_at_t2,
        "rollup": rollup,
    }
    (ROOT / args.summary).write_text(
        json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    print(
        f"arm_A κ={kappa_a:.3f} ({match_a}/{len(a_bin)}) | "
        f"arm_B Spearman={spearman_b:.3f} | arm_B@t2 κ={kappa_b_at_t2:.3f} | "
        f"wall={wall:.1f}s | n_queries={len(rollup)} | provider={provider} | model={MODEL}"
    )


if __name__ == "__main__":
    asyncio.run(main())
