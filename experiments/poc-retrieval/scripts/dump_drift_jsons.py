#!/usr/bin/env python3
"""dump_drift_jsons.py -- recover from PS1 silent dump failure.

Reads <probe-root>/<corpus>.db.r{1..5} for both poc and fsc, writes
<corpus>.<run>.{symbols,edges,alias_decls}.json into eval/drift_runs/.

Replaces the broken in-line Python dump in drift_evidence_probe.ps1
that piped via 'py -' (silent fail under PowerShell `Out-Null`).
"""
from __future__ import annotations
import json
import sqlite3
import sys
from pathlib import Path

PROBE_ROOT = Path("D:/projects/codenexus/experiments/poc-retrieval")
EVAL_DIR = PROBE_ROOT / "eval" / "drift_runs"
CORPORA = ["poc", "fsc"]
RUNS = ["r1", "r2", "r3", "r4", "r5"]


def dump(db_path: Path, out_dir: Path, corpus: str, run: str) -> dict:
    if not db_path.exists():
        return {"corpus": corpus, "run": run, "status": "missing", "error": f"{db_path} missing"}

    con = sqlite3.connect(str(db_path))
    con.row_factory = sqlite3.Row
    cur = con.cursor()

    sym_cols_db = {r[1] for r in cur.execute("PRAGMA table_info(symbols)").fetchall()}
    sym_cols = [c for c in ["id", "path", "name", "kind", "start_line", "end_line"] if c in sym_cols_db]
    sym_q = f"SELECT {', '.join(sym_cols)} FROM symbols ORDER BY path, name, start_line"
    sym_rows = [dict(r) for r in cur.execute(sym_q).fetchall()]
    (out_dir / f"{corpus}.{run}.symbols.json").write_text(
        json.dumps(sym_rows, ensure_ascii=False), encoding="utf-8"
    )

    edge_cols_db = {r[1] for r in cur.execute("PRAGMA table_info(edges)").fetchall()}
    edge_cols = [c for c in ["from_id", "to_id", "kind", "confidence"] if c in edge_cols_db]
    edge_q = f"SELECT {', '.join(edge_cols)} FROM edges ORDER BY from_id, to_id, kind"
    edge_rows = [dict(r) for r in cur.execute(edge_q).fetchall()]
    (out_dir / f"{corpus}.{run}.edges.json").write_text(
        json.dumps(edge_rows, ensure_ascii=False), encoding="utf-8"
    )

    alias_count = 0
    try:
        alias_rows = [
            dict(r) for r in cur.execute(
                "SELECT file, alias, target_file, target_member FROM alias_decls "
                "ORDER BY file, alias"
            ).fetchall()
        ]
        (out_dir / f"{corpus}.{run}.alias_decls.json").write_text(
            json.dumps(alias_rows, ensure_ascii=False), encoding="utf-8"
        )
        alias_count = len(alias_rows)
    except sqlite3.OperationalError as e:
        print(f"  alias_decls dump skipped for {corpus}/{run}: {e}", file=sys.stderr)

    con.close()
    return {
        "corpus": corpus,
        "run": run,
        "status": "ok",
        "symbols": len(sym_rows),
        "edges": len(edge_rows),
        "alias_decls": alias_count,
    }


def main() -> int:
    EVAL_DIR.mkdir(parents=True, exist_ok=True)
    results = []
    for c in CORPORA:
        for r in RUNS:
            db_path = PROBE_ROOT / f"{c}.db.{r}"
            res = dump(db_path, EVAL_DIR, c, r)
            results.append(res)
            print(f"[{c}/{r}] {res}")

    # Mock run_log.json (lost from PS1 step) so drift_compare.py "indexer_runtime_total_min"
    # falls back to None; recoverable from individual indexer .log files if needed.
    return 0 if all(r["status"] == "ok" for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
