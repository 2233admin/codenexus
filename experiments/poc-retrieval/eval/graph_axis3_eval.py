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


def run_query_graph(query_text: str, top: int = 5, subject: str | None = None) -> dict:
    cmd = [
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
    ]
    # Explicit subject bypasses extract_subject heuristic in Rust
    if subject:
        cmd += ["--subject", subject]
    res = subprocess.run(
        cmd,
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
    n_excluded_unindexed = 0
    for q in axis3:
        # Skip queries whose subject is not in the TS-only indexed corpus
        if q.get("subject_unindexed"):
            n_excluded_unindexed += 1
            print(f"{q['id']:5} SKIP (subject_unindexed=true: {q.get('subject','?')}) | {q['query'][:50]}")
            results.append({
                "id": q["id"],
                "query": q["query"],
                "expected_paths": q["expected_paths"],
                "negative": q.get("negative", False),
                "subject": q.get("subject"),
                "top5": [],
                "precision_at_5": None,  # excluded from aggregate
                "note": f"subject_unindexed=true — excluded from precision aggregate",
            })
            continue
        # Use explicit subject field if present, else fall back to extract_subject heuristic in Rust
        explicit_subject = q.get("subject")
        out = run_query_graph(q["query"], subject=explicit_subject)
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

    n_total = len(results)
    scored = [r for r in results if r["precision_at_5"] is not None]
    n_scored = len(scored)
    avg_precision = total_precision / n_scored if n_scored > 0 else 0.0
    avg_precision_all10 = total_precision / n_total if n_total > 0 else 0.0
    print()
    print(f"=== axis-3 graph-traversal precision_at_5 summary ===")
    print(f"queries total: {n_total}")
    print(f"excluded (subject_unindexed): {n_excluded_unindexed}")
    print(f"scored: {n_scored}")
    print(f"avg precision_at_5 (scored {n_scored}): {avg_precision:.3f} ({avg_precision*100:.1f}%)")
    print(f"avg precision_at_5 (all 10, excluded=0): {avg_precision_all10:.3f} ({avg_precision_all10*100:.1f}%)")
    print(f"unresolved subjects (heuristic fail): {n_unresolved_subject}")
    print(f"R3 retrieval baseline (axis-3): ~0% (per round_3_results.md)")
    print(f"spike-007 baseline (all 10): 15.0%")
    print(f"Lift vs spike-007 (scored {n_scored}): {(avg_precision - 0.15)*100:+.1f}pp")

    summary = {
        "n_queries_total": n_total,
        "n_excluded_unindexed": n_excluded_unindexed,
        "n_scored": n_scored,
        "avg_precision_at_5_scored": avg_precision,
        "avg_precision_at_5_all10": avg_precision_all10,
        "unresolved_subjects_heuristic": n_unresolved_subject,
        "r3_retrieval_baseline_axis3": 0.0,
        "spike007_baseline_all10": 0.15,
        "kinds": KINDS,
        "results": results,
    }
    out_path = ROOT / "round_7c_explicit_subject.json"
    out_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\nWrote {out_path}")


if __name__ == "__main__":
    main()
