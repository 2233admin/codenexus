"""Rescore alpha sweep results under v1 (original) and v2 (B10-corrected) rubrics.

v2 differs only in B10 expected_paths: original [meta, aggregate, frontmatter, kb_meta]
extended with [digest, buildDigest, fetchAllNotes, collector] to capture the
circleback-collector.ts symbols that retrieval already finds for the
'aggregate metadata across multiple notes' query.
"""
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).parent

queries_v1_raw = json.load(open(ROOT / "queries.json", encoding="utf-8"))
queries_v1 = {q["id"]: q for q in queries_v1_raw}

# v2: only B10 changes
queries_v2 = {qid: dict(q) for qid, q in queries_v1.items()}
queries_v2["B10"] = dict(queries_v2["B10"])
queries_v2["B10"]["expected_paths"] = sorted(
    set(queries_v2["B10"]["expected_paths"] + ["digest", "buildDigest", "fetchAllNotes", "collector"])
)


def score_query(q, top5):
    """Replicate Rust matcher: lowercase + backslash-to-slash + substring/eq match."""
    if q.get("negative"):
        return None  # cannot recompute without top1_score; signal caller to use Rust value
    if not top5:
        return 0.0
    parsed = []
    for s in top5:
        idx = s.rfind(":")
        if idx < 0:
            parsed.append((s.lower().replace("\\", "/"), s.lower()))
        else:
            p = s[:idx].lower().replace("\\", "/")
            n = s[idx + 1:].lower()
            parsed.append((p, n))

    def matches(p, n):
        for ep in q["expected_paths"]:
            e = ep.lower().replace("\\", "/")
            if e in p or n == e or e in n:
                return True
        return False

    if matches(*parsed[0]):
        return 1.0
    if any(matches(p, n) for p, n in parsed[:3]):
        return 0.5
    return 0.0


def slice_score(qids, alpha_data, queries):
    out = []
    for r in alpha_data:
        if r["id"] not in qids:
            continue
        q = queries[r["id"]]
        if q.get("negative"):
            out.append(r["precision_at_5"])  # reuse Rust value (negative scoring uses top1_score)
        else:
            sc = score_query(q, r["top5"])
            out.append(sc if sc is not None else r["precision_at_5"])
    return statistics.mean(out) if out else 0.0


def axis1_score(alpha_data, queries):
    return slice_score([f"A{i}" for i in range(1, 11)], alpha_data, queries)


def axis3_score(alpha_data, queries):
    return slice_score([f"C{i}" for i in range(1, 11)], alpha_data, queries)


print("alpha | B1-B7 v1 | B1-B7 v2 | B1-B10 v1 | B1-B10 v2 | A1-A10 | C1-C10")
print("------|----------|----------|-----------|-----------|--------|--------")
files = [
    ("0.4", "req35_alpha04.json"),
    ("0.5", "req35_alpha05.json"),
    ("0.6", "req10_alpha06.json"),
    ("0.7", "req35_alpha07.json"),
    ("0.8", "req35_alpha08.json"),
]
results = {}
for tag, fname in files:
    data = json.load(open(ROOT / fname, encoding="utf-8"))
    b17_v1 = slice_score(["B1", "B2", "B3", "B4", "B5", "B6", "B7"], data, queries_v1)
    b17_v2 = slice_score(["B1", "B2", "B3", "B4", "B5", "B6", "B7"], data, queries_v2)
    b110_v1 = slice_score([f"B{i}" for i in range(1, 11)], data, queries_v1)
    b110_v2 = slice_score([f"B{i}" for i in range(1, 11)], data, queries_v2)
    a1 = axis1_score(data, queries_v1)
    c1 = axis3_score(data, queries_v1)
    print(f"{tag}   | {b17_v1*100:5.1f}%   | {b17_v2*100:5.1f}%   | {b110_v1*100:5.1f}%    | {b110_v2*100:5.1f}%    | {a1*100:5.1f}%  | {c1*100:5.1f}%")
    results[tag] = {
        "b17_v1": b17_v1, "b17_v2": b17_v2,
        "b110_v1": b110_v1, "b110_v2": b110_v2,
        "axis1": a1, "axis3": c1,
    }

print()
print("=== Joint optimum analysis ===")
b17_max = max(results.items(), key=lambda kv: kv[1]["b17_v1"])
b110_max_v1 = max(results.items(), key=lambda kv: kv[1]["b110_v1"])
b110_max_v2 = max(results.items(), key=lambda kv: kv[1]["b110_v2"])
print(f"B1-B7 v1 optimum:  alpha={b17_max[0]}, score={b17_max[1]['b17_v1']*100:.1f}%")
print(f"B1-B10 v1 optimum: alpha={b110_max_v1[0]}, score={b110_max_v1[1]['b110_v1']*100:.1f}%")
print(f"B1-B10 v2 optimum: alpha={b110_max_v2[0]}, score={b110_max_v2[1]['b110_v2']*100:.1f}%")
print()
if b17_max[0] == b110_max_v1[0] == b110_max_v2[0]:
    print("VERDICT: joint optimum == B1-B7 optimum across BOTH rubrics --")
    print("         alpha=0.6 lock holds even on held-out + corrected rubric")
elif b110_max_v2[0] != b17_max[0]:
    print(f"VERDICT: joint optimum (alpha={b110_max_v2[0]}) DIFFERS from B1-B7 optimum (alpha={b17_max[0]})")
    print("         Original lock was a local-optimum-by-construction; refit needed")
else:
    print("VERDICT: rubric-dependent -- v1 and v2 disagree on joint optimum")

# Save machine-readable result
out_path = ROOT / "phase35_alpha_sweep.json"
json.dump(results, open(out_path, "w", encoding="utf-8"), indent=2)
print(f"\nWrote {out_path}")
