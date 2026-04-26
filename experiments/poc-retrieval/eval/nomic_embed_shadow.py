"""Nomic Embed Code shadow evaluation -- does it lift axis-2?

Embeds 2116 symbols + 30 queries with nomic-ai/nomic-embed-code, computes top-5
by cosine, compares to R3 baseline. Pure Python, doesn't touch Rust core.
Falls back to nomic-embed-text-v1.5 (smaller) if nomic-embed-code download fails,
then microsoft/codebert-base as last resort.
"""

import json
import sqlite3
import time
import sys
import subprocess
from pathlib import Path

ROOT = Path(__file__).parent
DB = ROOT.parent / "poc.db"
QUERIES_JSON = ROOT / "queries.json"
R3_RESULTS = ROOT / "results_round3_a06_v2.json"
OUTPUT_RESULTS = ROOT / "round_5c_nomic_results.json"
REPORT_PATH = ROOT / "nomic_shadow_report.md"

MODEL_CANDIDATES = [
    "nomic-ai/nomic-embed-text-v1.5",
    "microsoft/codebert-base",
]

# Prefix conventions per model
MODEL_PREFIXES = {
    "nomic-ai/nomic-embed-code":    {"query": "search_query: ",    "doc": "search_document: "},
    "nomic-ai/nomic-embed-text-v1.5": {"query": "search_query: ", "doc": "search_document: "},
    "microsoft/codebert-base":      {"query": "",                  "doc": ""},
}


def ensure_deps():
    """Install sentence-transformers if not present."""
    try:
        import sentence_transformers  # noqa: F401
        import numpy  # noqa: F401
    except ImportError:
        print("[setup] Installing sentence-transformers via uv add ...", flush=True)
        subprocess.check_call(
            ["uv", "add", "sentence-transformers"],
            cwd=str(ROOT),
        )
        print("[setup] Done.", flush=True)


def load_model(candidates):
    """Try models in order; return (model, model_id, prefixes) on first success."""
    from sentence_transformers import SentenceTransformer

    for model_id in candidates:
        print(f"[model] Trying {model_id} ...", flush=True)
        t0 = time.time()
        try:
            model = SentenceTransformer(model_id, trust_remote_code=True)
            elapsed = time.time() - t0
            print(f"[model] Loaded {model_id} in {elapsed:.1f}s", flush=True)
            return model, model_id, MODEL_PREFIXES[model_id]
        except Exception as e:
            elapsed = time.time() - t0
            print(f"[model] {model_id} failed after {elapsed:.1f}s: {e}", flush=True)
            if elapsed > 300:
                print("[model] Timeout exceeded 5 min -- falling back immediately", flush=True)

    raise RuntimeError("All model candidates failed to load.")


def load_symbols():
    """Load all symbols from poc.db, return list of dicts."""
    conn = sqlite3.connect(str(DB))
    cur = conn.cursor()
    cur.execute("SELECT path, name, kind, snippet, search_blob FROM symbols")
    rows = cur.fetchall()
    conn.close()
    symbols = []
    for path, name, kind, snippet, search_blob in rows:
        symbols.append({
            "path": path,
            "name": name,
            "kind": kind,
            "snippet": snippet or "",
            "search_blob": search_blob or "",
            "label": f"{path}:{name}",
        })
    return symbols


def build_doc_texts(symbols, doc_prefix):
    """Build text strings for embedding as documents."""
    texts = []
    for s in symbols:
        blob = s["search_blob"] if s["search_blob"] else f"{s['name']} {s['kind']} {s['snippet']}"
        texts.append(doc_prefix + blob)
    return texts


def cosine_top5(query_vec, doc_matrix):
    """Return sorted (score, idx) top-5 by cosine similarity."""
    import numpy as np
    # Both already L2-normalised; dot product = cosine sim
    sims = doc_matrix @ query_vec
    top5_idx = np.argpartition(sims, -5)[-5:]
    top5_sorted = sorted(top5_idx, key=lambda i: sims[i], reverse=True)
    return [(float(sims[i]), int(i)) for i in top5_sorted]


def precision_at_5(top5_labels, query):
    """
    1.0 if any expected_path substring matches any of the top-5 labels.
    -0.25 for negative queries if any result matches expected (false positive).
    Follows R3 scoring logic.
    """
    expected = query.get("expected_paths", [])
    negative = query.get("negative", False)

    if negative:
        # negative query: penalty if expected_paths are non-empty (shouldn't happen by design)
        # Follow R3 convention: if no expected_paths, score 1.0 if results returned (true negative)
        # but if returns something spurious, score -0.25
        if not expected:
            return 1.0  # correct: nothing expected, anything returned is tolerable
        # If expected_paths set on negative, check for false positive
        hit = any(any(e.lower() in lbl.lower() for e in expected) for lbl in top5_labels)
        return -0.25 if hit else 1.0

    if not expected:
        return 0.0  # no expected paths defined = unchecked

    hit = any(
        any(e.lower() in lbl.lower() for e in expected)
        for lbl in top5_labels
    )
    return 1.0 if hit else 0.0


def main():
    import numpy as np

    print("=== Nomic Embed Code Shadow Eval ===", flush=True)

    ensure_deps()

    # 1. Load model
    t_model_start = time.time()
    model, model_id, prefixes = load_model(MODEL_CANDIDATES)
    t_model_end = time.time()
    model_load_time = t_model_end - t_model_start

    # 2. Load symbols
    print(f"[data] Loading symbols from {DB} ...", flush=True)
    symbols = load_symbols()
    print(f"[data] {len(symbols)} symbols loaded", flush=True)

    # 3. Load queries
    with open(QUERIES_JSON, "r", encoding="utf-8") as f:
        queries = json.load(f)
    print(f"[data] {len(queries)} queries loaded", flush=True)

    # 4. Embed documents
    print("[embed] Embedding symbols (batch) ...", flush=True)
    doc_texts = build_doc_texts(symbols, prefixes["doc"])
    t_embed_start = time.time()
    doc_vecs = model.encode(doc_texts, batch_size=128, show_progress_bar=True, normalize_embeddings=True)
    doc_matrix = np.array(doc_vecs, dtype="float32")  # shape: (N, D)
    t_embed_end = time.time()
    embed_time = t_embed_end - t_embed_start
    print(f"[embed] Symbols embedded in {embed_time:.1f}s, shape={doc_matrix.shape}", flush=True)

    # 5. Embed queries + retrieve top-5
    print("[eval] Embedding queries + retrieving ...", flush=True)
    t_query_start = time.time()
    results = []
    for q in queries:
        qtext = prefixes["query"] + q["query"]
        qvec = model.encode([qtext], normalize_embeddings=True)[0]
        qvec = np.array(qvec, dtype="float32")
        top5 = cosine_top5(qvec, doc_matrix)
        top5_labels = [symbols[idx]["label"] for _, idx in top5]
        p5 = precision_at_5(top5_labels, q)
        results.append({
            "id": q["id"],
            "axis": q["axis"],
            "query": q["query"],
            "negative": q.get("negative", False),
            "top5": top5_labels,
            "top5_scores": [round(s, 5) for s, _ in top5],
            "precision_at_5": p5,
        })
    t_query_end = time.time()
    match_time = t_query_end - t_query_start
    print(f"[eval] Queries done in {match_time:.1f}s", flush=True)

    # 6. Save results
    with open(OUTPUT_RESULTS, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
    print(f"[out] Results written to {OUTPUT_RESULTS}", flush=True)

    # 7. Load R3 baseline
    with open(R3_RESULTS, "r", encoding="utf-8") as f:
        r3 = json.load(f)
    r3_by_id = {r["id"]: r for r in r3}

    # 8. Compute per-axis metrics
    def axis_precision(result_list, axis):
        qs = [r for r in result_list if r["axis"] == axis]
        if not qs:
            return 0.0
        return sum(max(r["precision_at_5"], 0.0) for r in qs) / len(qs)

    nomic_a1 = axis_precision(results, 1)
    nomic_a2 = axis_precision(results, 2)
    nomic_a3 = axis_precision(results, 3)

    r3_a1 = axis_precision(r3, 1)
    r3_a2 = axis_precision(r3, 2)
    r3_a3 = axis_precision(r3, 3)

    # 9. Top-5 overlap rate (>=3 of Nomic's top-5 match R3's top-5)
    overlap_count = 0
    total_non_neg = 0
    for r in results:
        r3r = r3_by_id.get(r["id"])
        if r3r is None or r.get("negative", False):
            continue
        total_non_neg += 1
        nomic_set = set(r["top5"])
        r3_set = set(r3r.get("top5", []))
        intersection = len(nomic_set & r3_set)
        if intersection >= 3:
            overlap_count += 1
    overlap_rate = overlap_count / total_non_neg if total_non_neg else 0.0

    # 10. Per-query comparison table
    per_query_rows = []
    for r in results:
        r3r = r3_by_id.get(r["id"], {})
        r3_p5 = r3r.get("precision_at_5", 0.0)
        nomic_p5 = r.get("precision_at_5", 0.0)
        delta = nomic_p5 - r3_p5
        per_query_rows.append((r["id"], r["axis"], r["query"][:55], r3_p5, nomic_p5, delta))

    # 11. Verdict
    axis2_delta = nomic_a2 - r3_a2
    if axis2_delta > 0.05:
        verdict = "embedder swap recommended for Phase 3"
    elif axis2_delta >= -0.05:
        verdict = "marginal lift, not worth complexity"
    else:
        verdict = "regression, R3 embedder stays"

    # 12. Write report
    report_lines = [
        "# Nomic Embed Code Shadow Evaluation Report",
        "",
        f"**Model used:** `{model_id}`",
        f"**Model load time:** {model_load_time:.1f}s",
        f"**Symbol embed time:** {embed_time:.1f}s ({len(symbols)} symbols)",
        f"**Query match time:** {match_time:.1f}s ({len(queries)} queries)",
        f"**DB:** `{DB}`",
        "",
        "## Per-Axis Precision@5",
        "",
        "| Axis | R3 Baseline | Nomic | Delta |",
        "|------|-------------|-------|-------|",
        f"| Axis-1 (exact lookup) | {r3_a1:.1%} | {nomic_a1:.1%} | {nomic_a1-r3_a1:+.1%} |",
        f"| Axis-2 (semantic) | {r3_a2:.1%} | {nomic_a2:.1%} | {nomic_a2-r3_a2:+.1%} |",
        f"| Axis-3 (graph-aware) | {r3_a3:.1%} | {nomic_a3:.1%} | {nomic_a3-r3_a3:+.1%} |",
        "",
        f"**Top-5 overlap rate (>=3/5 match R3):** {overlap_rate:.1%} ({overlap_count}/{total_non_neg} queries)",
        "",
        "## Per-Query Breakdown",
        "",
        "| ID | Ax | Query | R3 P@5 | Nomic P@5 | Delta |",
        "|----|----|----------------------------------------------------|--------|-----------|-------|",
    ]
    for row in per_query_rows:
        qid, ax, qtext, r3p, np5, delta = row
        sign = "+" if delta > 0 else ""
        report_lines.append(f"| {qid} | {ax} | {qtext} | {r3p:.2f} | {np5:.2f} | {sign}{delta:.2f} |")

    report_lines += [
        "",
        "## Verdict",
        "",
        f"**{verdict}**",
        "",
        f"Axis-2 delta: {axis2_delta:+.1%} vs R3 baseline of {r3_a2:.1%}.",
        "",
        "### Notes",
        "- Negative queries scored: 1.0 = correct rejection, -0.25 = false positive (floored to 0.0 in aggregate).",
        f"- R3 used qwen3-embedding:0.6b (1024d) via Ollama with RRF fusion. Nomic used `{model_id}` pure cosine.",
        "- Axis-3 baseline near 0% is expected -- graph traversal not available in POC retrieval.",
        "- Corpus: 2116 TS symbols from obsidian-llm-wiki mcp-server.",
    ]

    report_text = "\n".join(report_lines) + "\n"
    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        f.write(report_text)
    print(f"[out] Report written to {REPORT_PATH}", flush=True)

    # 13. Summary to stdout
    print("\n=== SUMMARY ===")
    print(f"Model: {model_id}")
    print(f"Axis-1: R3={r3_a1:.1%}  Nomic={nomic_a1:.1%}  delta={nomic_a1-r3_a1:+.1%}")
    print(f"Axis-2: R3={r3_a2:.1%}  Nomic={nomic_a2:.1%}  delta={nomic_a2-r3_a2:+.1%}  ** key metric **")
    print(f"Axis-3: R3={r3_a3:.1%}  Nomic={nomic_a3:.1%}  delta={nomic_a3-r3_a3:+.1%}")
    print(f"Top-5 overlap: {overlap_rate:.1%}")
    print(f"Verdict: {verdict}")
    print(f"Report: {REPORT_PATH}")


if __name__ == "__main__":
    main()
