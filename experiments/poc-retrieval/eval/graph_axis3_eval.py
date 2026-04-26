"""Spike-007 — Run axis-3 queries (C1-C10) through QueryGraph + measure
precision_at_5 vs hand-annotated expected_paths. Compares to retrieval
R3 baseline ≈0% (per spike-006 stretch + R3 round_3_results.md).

Per query: subprocess.run(query-graph --json), parse top-5, apply same
matcher as Rust Eval (substring match on path or symbol name vs
expected_paths), compute precision per query + axis aggregate.

Output: round_7_graph_axis3.json + axis3_graph_report.md
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent
QUERIES = ROOT / "queries.json"
BIN = ROOT.parent / "target" / "release" / "poc-retrieval.exe"
DB = ROOT.parent / "poc.db"

# All-kinds traversal — axis-3 spans Calls (who-calls/X-calls) + Implements
# (X implements Y) + Extends (X extends Y). Imports rarely useful for axis-3
# but cheap to include.
KINDS = "Calls,Implements,Extends,Imports"


def matches(hit_path: str, hit_name: str, expected_paths: list[str]) -> bool:
    """Same matcher as Rust Eval — case-insensitive, slash-normalized."""
    p = hit_path.lower().replace("\\", "/")
    n = hit_name.lower()
    for ep in expected_paths:
        e = ep.lower().replace("\\", "/")
        if p.find(e) >= 0 or n == e or n.find(e) >= 0:
            return True
    return False


def run_query_graph(query_text: str, top: int = 5) -> dict:
    res = subprocess.run(
        [
            str(BIN),
            "query-graph",
            query_text,
            "--db",
            str(DB),
            "--kinds",
            KINDS,
            "--top",
            str(top),
            "--json",
        ],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if res.returncode != 0:
        return {"_error": res.stderr[:300]}
    try:
        return json.loads(res.stdout)
    except json.JSONDecodeError as e:
        return {"_parse_error": str(e), "_stdout": res.stdout[:300]}


def main():
    with open(QUERIES, "r", encoding="utf-8") as f:
        all_queries = json.load(f)
    axis3 = [q for q in all_queries if q["axis"] == 3]
    print(f"Loaded {len(axis3)} axis-3 queries (C1-C10)")
    print(f"Binary: {BIN}")
    print(f"DB: {DB}")
    print(f"Kinds: {KINDS}")
    print()

    results = []
    total_precision = 0.0
    n_unresolved_subject = 0
    for q in axis3:
        out = run_query_graph(q["query"])
        if "_error" in out or "_parse_error" in out:
            print(f"{q['id']:5} ERR  | {q['query'][:50]}")
            print(f"      err: {out.get('_error') or out.get('_parse_error')}")
            results.append(
                {
                    "id": q["id"],
                    "query": q["query"],
                    "expected_paths": q["expected_paths"],
                    "negative": q.get("negative", False),
                    "subject": None,
                    "top5": [],
                    "precision_at_5": 0.0,
                    "error": out.get("_error") or out.get("_parse_error"),
                }
            )
            continue

        subject = out.get("subject")
        entry_ids = out.get("entry_ids", [])
        hits = out.get("results", [])
        top5_paths = [f"{h['path']}:{h['name']}" for h in hits]

        # Negative case: subject not found OR explicit negative=true
        is_neg = q.get("negative", False)
        if not entry_ids:
            n_unresolved_subject += 1
            # If query is negative AND subject is unresolved, that's correct identification
            precision = 1.0 if is_neg else 0.0
            note = "subject unresolved (NEG correct)" if is_neg else "subject unresolved (axis-3 fail)"
        elif is_neg:
            # Negative with hits → wrong, score 0
            precision = 1.0 if not hits else 0.0
            note = "negative with hits=wrong" if hits else "negative correctly empty"
        else:
            # Positive: precision_at_5 = 1.0 if any of top-5 matches expected_paths
            top1_match = hits and matches(hits[0]["path"], hits[0]["name"], q["expected_paths"])
            top5_any = any(matches(h["path"], h["name"], q["expected_paths"]) for h in hits)
            if top1_match:
                precision = 1.0
            elif top5_any:
                precision = 0.5
            else:
                precision = 0.0
            note = ""

        total_precision += precision
        marker = "✓" if precision >= 1.0 else ("~" if precision >= 0.5 else "✗")
        print(f"{q['id']:5} {marker} {precision:.2f} | subj={subject} | {q['query'][:50]}")
        for i, h in enumerate(hits[:3], 1):
            tag = "+" if matches(h["path"], h["name"], q["expected_paths"]) else "-"
            print(f"        #{i} {tag} {h['name']} ({h['path'][:50]}) score={h['score']:.4f}")
        if note:
            print(f"        note: {note}")

        results.append(
            {
                "id": q["id"],
                "query": q["query"],
                "expected_paths": q["expected_paths"],
                "negative": is_neg,
                "subject": subject,
                "entry_ids": entry_ids,
                "top5": top5_paths,
                "top5_full": hits,
                "precision_at_5": precision,
                "note": note,
            }
        )

    n = len(results)
    avg_precision = total_precision / n if n > 0 else 0.0
    print()
    print(f"=== axis-3 graph-traversal precision_at_5 summary ===")
    print(f"queries: {n}")
    print(f"avg precision_at_5: {avg_precision:.3f} ({avg_precision*100:.1f}%)")
    print(f"unresolved subjects: {n_unresolved_subject}")
    print(f"R3 retrieval baseline (axis-3): ~0% (per round_3_results.md)")
    print(f"Lift: {avg_precision*100:.1f}pp absolute")

    summary = {
        "n_queries": n,
        "avg_precision_at_5": avg_precision,
        "unresolved_subjects": n_unresolved_subject,
        "r3_retrieval_baseline_axis3": 0.0,
        "kinds": KINDS,
        "results": results,
    }
    out_path = ROOT / "round_7_graph_axis3.json"
    out_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\nWrote {out_path}")


if __name__ == "__main__":
    main()
