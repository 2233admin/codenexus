"""R6c -- Pairwise LLM-judge with two-orderings consistent-wins voting.

Standard literature mitigation for position bias: per query, run prompt twice,
once with set_A in slot-A and once with set_B in slot-A. Only count "consistent"
wins where both orderings agreed.

Output:
  consistent_A_wins  : both orderings voted A -> real A clearly better
  consistent_B_wins  : both orderings voted B -> real B clearly better
  inconsistent       : orderings disagreed (one said A, other said B) -- undecidable
  tie_or_mixed       : at least one was tie, neither double-A nor double-B
"""

import argparse
import asyncio
import json
import os
import sqlite3
import sys
import time
from pathlib import Path

# Reuse existing scaffolding
sys.path.insert(0, str(Path(__file__).parent))
from ragas_spike import (
    judge_pairwise, load_snippet, build_clients, CONCURRENCY,
    DB, QUERIES, ROOT
)


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--round-a", default="results_round3_a06_v2.json")
    parser.add_argument("--round-b", default="results_round4_a06_rr_v2.json")
    parser.add_argument("--out", default="round_6c_results.json")
    parser.add_argument("--summary", default="round_6c_summary.json")
    parser.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()

    # Load queries + R3 + R4 results
    with open(QUERIES, "r", encoding="utf-8") as f:
        queries_by_id = {q["id"]: q for q in json.load(f)}
    with open(ROOT / args.round_a, "r", encoding="utf-8") as f:
        data_a = {e["id"]: e for e in json.load(f)}
    with open(ROOT / args.round_b, "r", encoding="utf-8") as f:
        data_b = {e["id"]: e for e in json.load(f)}

    # Build per-query dual-task (A-first + B-first)
    conn = sqlite3.connect(str(DB))
    cur = conn.cursor()
    qids = sorted(set(data_a.keys()) & set(data_b.keys()))
    if args.limit > 0:
        qids = qids[: args.limit]

    tasks = []
    for qid in qids:
        q = queries_by_id.get(qid, {})
        snips_a = [load_snippet(cur, h) for h in data_a[qid].get("top5", [])]
        snips_b = [load_snippet(cur, h) for h in data_b[qid].get("top5", [])]
        tasks.append({
            "qid": qid, "axis": data_a[qid].get("axis"),
            "query": q.get("query", ""),
            "snips_a": snips_a, "snips_b": snips_b,
        })
    conn.close()

    provider = os.environ.get("EVAL_PROVIDER", "minimax_official")
    print(
        f"R6c: {len(tasks)} queries x 2 orderings = {2*len(tasks)} judge calls | "
        f"concurrency={CONCURRENCY} | provider={provider}",
        file=sys.stderr,
    )

    sem = asyncio.Semaphore(CONCURRENCY)
    t0 = time.time()
    # Each query -> 2 calls: (A,B) order + (B,A) order
    coros = []
    for t in tasks:
        coros.append(judge_pairwise(sem, t["query"], t["snips_a"], t["snips_b"]))  # A first
        coros.append(judge_pairwise(sem, t["query"], t["snips_b"], t["snips_a"]))  # B first
    results = await asyncio.gather(*coros, return_exceptions=True)
    wall = time.time() - t0

    # Pair up results and aggregate
    counts = {"consistent_A": 0, "consistent_B": 0, "inconsistent": 0, "tie_or_mixed": 0, "error": 0}
    for i, t in enumerate(tasks):
        r_ab = results[2 * i]
        r_ba = results[2 * i + 1]
        v_ab = r_ab.get("verdict") if isinstance(r_ab, dict) else None
        v_ba = r_ba.get("verdict") if isinstance(r_ba, dict) else None
        # In r_ba, slot-A held real-B, slot-B held real-A. Translate verdicts back to real identities:
        #   v_ba=="A" means the LLM preferred slot-A, which was real-B -> real_B won
        #   v_ba=="B" means the LLM preferred slot-B, which was real-A -> real_A won
        #   v_ba=="tie" stays tie
        translate_ab = {"A": "real_A", "B": "real_B", "tie": "tie"}.get(v_ab) if v_ab else None
        translate_ba = {"A": "real_B", "B": "real_A", "tie": "tie"}.get(v_ba) if v_ba else None

        if translate_ab == "real_A" and translate_ba == "real_A":
            counts["consistent_A"] += 1
        elif translate_ab == "real_B" and translate_ba == "real_B":
            counts["consistent_B"] += 1
        elif (translate_ab == "real_A" and translate_ba == "real_B") or \
             (translate_ab == "real_B" and translate_ba == "real_A"):
            counts["inconsistent"] += 1
        elif translate_ab is None or translate_ba is None:
            counts["error"] += 1
        else:
            counts["tie_or_mixed"] += 1

        t["v_ab"] = v_ab
        t["v_ba"] = v_ba
        t["translate_ab"] = translate_ab
        t["translate_ba"] = translate_ba

    # Strip snips_a/snips_b from output (large, not needed for audit)
    out_tasks = [
        {k: v for k, v in t.items() if k not in ("snips_a", "snips_b")}
        for t in tasks
    ]

    (ROOT / args.out).write_text(json.dumps(out_tasks, ensure_ascii=False, indent=2), encoding="utf-8")
    n_judged = counts["consistent_A"] + counts["consistent_B"] + counts["inconsistent"] + counts["tie_or_mixed"]
    inconsistency_rate = counts["inconsistent"] / n_judged if n_judged > 0 else float("nan")
    summary = {
        "n_queries": len(tasks),
        "n_judge_calls": 2 * len(tasks),
        "wall_clock_seconds": round(wall, 2),
        "verdicts": counts,
        "inconsistency_rate": round(inconsistency_rate, 3),
        "round_a": args.round_a,
        "round_b": args.round_b,
    }
    (ROOT / args.summary).write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(
        f"R6c consistent_A={counts['consistent_A']} consistent_B={counts['consistent_B']} "
        f"inconsistent={counts['inconsistent']} tie_or_mixed={counts['tie_or_mixed']} "
        f"err={counts['error']} | inconsistency_rate={inconsistency_rate:.1%} | "
        f"wall={wall:.1f}s | calls={2*len(tasks)}"
    )


if __name__ == "__main__":
    asyncio.run(main())
