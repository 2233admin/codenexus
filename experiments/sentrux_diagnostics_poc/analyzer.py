"""sentrux Pro per-file diagnostics POC — TS/JS import-graph analyzer (stdlib only)."""

import json
import math
import os
import re
import sys
import time
from collections import defaultdict, deque

ROOT = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else r"D:/projects/obsidian-llm-wiki")
# OUT_DIR: project-local .cnxq/ by default for daily cnxq use; falls back to spike dir if not writable.
_PROJECT_LOCAL = os.path.join(ROOT, ".cnxq")
try:
    os.makedirs(_PROJECT_LOCAL, exist_ok=True)
    OUT_DIR = _PROJECT_LOCAL
except OSError:
    OUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "out")
# MIRROR: opt-in via CNXQ_MIRROR env (exact file path). Empty/unset = no mirror write.
MIRROR = os.environ.get("CNXQ_MIRROR", "")
# COMPARE_BASELINE: only produce comparison_to_sentrux block when scanning the validated baseline target.
COMPARE_BASELINE = os.path.normcase(os.path.normpath(ROOT)).endswith(os.path.normcase(os.path.normpath("obsidian-llm-wiki")))
SKIP_DIRS = {"node_modules", "dist", "build", ".git", "coverage", ".next", ".turbo", "out"}
EXTS = (".ts", ".tsx", ".js", ".mjs", ".cts", ".mts", ".cjs", ".jsx")
RESOLVE_EXTS = [".ts", ".tsx", ".js", ".mjs", ".cjs", ".cts", ".mts", ".jsx"]

# Import patterns: ES6 import x from '...'; import '...'; export ... from '...'; dynamic import('...'); require('...').
RE_IMPORT = re.compile(
    r"""(?:^|[\s;])(?:import\s+(?:[^'"`;]+?\s+from\s+)?|export\s+(?:\*|\{[^}]*\})\s+from\s+|(?<![\w$])require\s*\(\s*|(?<![\w$])import\s*\(\s*)['"]([^'"]+)['"]""",
    re.MULTILINE,
)


def walk_files(root):
    out = []
    for dp, dns, fns in os.walk(root):
        dns[:] = [d for d in dns if d not in SKIP_DIRS and not d.startswith(".")]
        for fn in fns:
            if fn.endswith(EXTS):
                out.append(os.path.normpath(os.path.join(dp, fn)))
    return out


def strip_comments(src):
    # crude: drop /* ... */ and // ... lines so RE_IMPORT does not match commented-out imports
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.DOTALL)
    src = re.sub(r"(^|[^:])//[^\n]*", lambda m: m.group(1), src)
    return src


def read_imports(path):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            src = f.read()
    except OSError:
        return []
    src = strip_comments(src)
    return RE_IMPORT.findall(src)


def resolve(spec, from_file, file_set):
    # Only resolve relative imports; treat bare specifiers as external/out-of-graph
    if not spec.startswith("."):
        return None
    base = os.path.normpath(os.path.join(os.path.dirname(from_file), spec))
    # exact file
    if base in file_set:
        return base
    # TS/Node ESM convention: imports use .js but source is .ts (or .mjs->.mts, .cjs->.cts).
    # Strip a known JS extension and try TS twins, plus all RESOLVE_EXTS.
    js_to_ts = {".js": [".ts", ".tsx"], ".mjs": [".mts", ".mjs"], ".cjs": [".cts", ".cjs"], ".jsx": [".tsx", ".jsx"]}
    root, ext = os.path.splitext(base)
    if ext in js_to_ts:
        for cand_ext in js_to_ts[ext]:
            cand = root + cand_ext
            if cand in file_set:
                return cand
    # try appending each known extension
    for cand_ext in RESOLVE_EXTS:
        cand = base + cand_ext
        if cand in file_set:
            return cand
    # treat as directory -> index.*
    for cand_ext in RESOLVE_EXTS:
        cand = os.path.normpath(os.path.join(base, "index" + cand_ext))
        if cand in file_set:
            return cand
    return None


def module_of(path, root):
    # module = parent directory of the file (matches sentrux's effective bucketing better
    # than fixed-depth-2; depth-2 collapses 71% of mcp-server/src into one bucket -> Q -> 0).
    rel = os.path.relpath(path, root).replace("\\", "/")
    parts = rel.split("/")
    if len(parts) == 1:
        return "<root>"
    return "/".join(parts[:-1])


def kosaraju_levels(nodes, adj, radj):
    # Standard Kosaraju: 1) iterative DFS to fill stack by finish time on G; 2) DFS on G^T popping stack to get SCCs.
    n = len(nodes)
    idx = {f: i for i, f in enumerate(nodes)}
    visited = [False] * n
    order = []
    for s in range(n):
        if visited[s]:
            continue
        stack = [(s, iter(adj[nodes[s]]))]
        visited[s] = True
        while stack:
            u, it = stack[-1]
            nxt = next(it, None)
            if nxt is None:
                order.append(u)
                stack.pop()
            else:
                v = idx[nxt]
                if not visited[v]:
                    visited[v] = True
                    stack.append((v, iter(adj[nodes[v]])))
    comp = [-1] * n
    c = 0
    for u in reversed(order):
        if comp[u] != -1:
            continue
        stack = [u]
        comp[u] = c
        while stack:
            x = stack.pop()
            for y_name in radj[nodes[x]]:
                y = idx[y_name]
                if comp[y] == -1:
                    comp[y] = c
                    stack.append(y)
        c += 1
    # SCC condensation -> DAG -> longest path = level
    scc_count = c
    cadj = [set() for _ in range(scc_count)]
    cradj_indeg = [0] * scc_count
    for f in nodes:
        u = comp[idx[f]]
        for g in adj[f]:
            v = comp[idx[g]]
            if v != u and v not in cadj[u]:
                cadj[u].add(v)
    for u in range(scc_count):
        for v in cadj[u]:
            cradj_indeg[v] += 1
    # Kahn topo with level = max(level of preds) + 1; leaf SCCs (no incoming from condensation) start at 0.
    # Define level so leaves (no outgoing in original DAG) = 0 — standard "depth from leaves" matches sentrux semantics.
    # We invert: build reverse-condensation, source = SCCs with no outgoing in cadj => level 0.
    rev = [set() for _ in range(scc_count)]
    outdeg = [0] * scc_count
    for u in range(scc_count):
        for v in cadj[u]:
            rev[v].add(u)
            outdeg[u] += 1
    level = [0] * scc_count
    q = deque([u for u in range(scc_count) if outdeg[u] == 0])
    rem_out = outdeg[:]
    # propagate from leaves (outdeg=0) backward via rev
    while q:
        u = q.popleft()
        for p in rev[u]:
            if level[p] < level[u] + 1:
                level[p] = level[u] + 1
            rem_out[p] -= 1
            if rem_out[p] == 0:
                q.append(p)
    # SCC sizes for cycle counting
    scc_size = [0] * scc_count
    for f in nodes:
        scc_size[comp[idx[f]]] += 1
    return comp, level, scc_size, scc_count


def blast_radius(nodes, radj):
    # transitive dependents via reverse BFS
    out = {}
    for src in nodes:
        seen = set()
        q = deque([src])
        while q:
            u = q.popleft()
            for v in radj[u]:
                if v not in seen and v != src:
                    seen.add(v)
                    q.append(v)
        out[src] = len(seen)
    return out


def gini(values):
    vs = sorted(v for v in values if v is not None)
    n = len(vs)
    if n == 0:
        return 0.0
    s = sum(vs)
    if s == 0:
        return 0.0
    cum = 0.0
    for i, v in enumerate(vs, 1):
        cum += i * v
    return (2 * cum) / (n * s) - (n + 1) / n


def newman_modularity(nodes, edges, mod_of):
    m = len(edges)
    if m == 0:
        return 0.0
    sum_kout = defaultdict(int)
    sum_kin = defaultdict(int)
    intra = defaultdict(int)
    for u, v in edges:
        mu, mv = mod_of[u], mod_of[v]
        sum_kout[mu] += 1
        sum_kin[mv] += 1
        if mu == mv:
            intra[mu] += 1
    q = 0.0
    mods = set(mod_of.values())
    for c in mods:
        q += intra[c] / m - (sum_kout[c] * sum_kin[c]) / (m * m)
    return q


def main():
    files = walk_files(ROOT)
    file_set = set(files)
    # adjacency on absolute paths
    adj = {f: [] for f in files}
    radj = {f: [] for f in files}
    edges = []
    for f in files:
        for spec in read_imports(f):
            tgt = resolve(spec, f, file_set)
            if tgt is None or tgt == f:
                continue
            adj[f].append(tgt)
            radj[tgt].append(f)
            edges.append((f, tgt))
    # dedupe edges (multiple imports of same target from same file) for graph metrics
    edges = list({(u, v) for u, v in edges})
    for f in files:
        adj[f] = list(set(adj[f]))
        radj[f] = list(set(radj[f]))

    nodes = files
    comp, scc_level, scc_size, scc_count = kosaraju_levels(nodes, adj, radj)
    idx = {f: i for i, f in enumerate(nodes)}
    file_level = {f: scc_level[comp[idx[f]]] for f in nodes}
    cycles = sum(1 for s in scc_size if s > 1)
    # max_level = depth of deepest node in the longest chain (0-indexed leaves).
    # sentrux reports `depth=4` while our levels span 0..5 (6 levels, 5 transitions).
    # sentrux's `depth` empirically = num_distinct_levels - 2 (intermediate strata only),
    # equivalent here to max_level - 1. Document both raw and sentrux-aligned values.
    max_level = max(scc_level) if scc_level else 0
    max_depth = max(0, max_level - 1)
    longest_chain_edges = max_level

    # per-file metrics
    out_deg = {f: len(adj[f]) for f in nodes}
    in_deg = {f: len(radj[f]) for f in nodes}
    mod_of = {f: module_of(f, ROOT) for f in nodes}
    cross_out = {f: sum(1 for g in adj[f] if mod_of[g] != mod_of[f]) for f in nodes}
    upward = {f: sum(1 for g in adj[f] if file_level[g] > file_level[f]) for f in nodes}

    blast = blast_radius(nodes, radj)

    q = newman_modularity(nodes, edges, mod_of)
    g_out = gini(list(out_deg.values()))
    equality_proxy = 1.0 - g_out

    # culprits
    def rel(p):
        return os.path.relpath(p, ROOT).replace("\\", "/")

    mod_culprits = sorted(
        nodes, key=lambda f: cross_out[f] * math.sqrt(out_deg[f]), reverse=True
    )[:10]
    depth_culprits = sorted(nodes, key=lambda f: (file_level[f], blast[f]), reverse=True)[:10]
    # exclude barrels (basename index.* or named exports.*) from blast ranking — re-export artifact
    def is_barrel(p):
        b = os.path.basename(p).lower()
        return b.startswith("index.") or b.startswith("exports.")

    blast_culprits = sorted(
        [f for f in nodes if not is_barrel(f)], key=lambda f: blast[f], reverse=True
    )[:10]
    out_vals = list(out_deg.values())
    out_vals_sorted = sorted(out_vals)
    median_out = out_vals_sorted[len(out_vals_sorted) // 2] if out_vals_sorted else 0
    eq_culprits = sorted(nodes, key=lambda f: abs(out_deg[f] - median_out), reverse=True)[:10]

    diagnostics = {
        "modularity_culprits": [
            {
                "file": rel(f),
                "score": round(cross_out[f] * math.sqrt(out_deg[f]), 3),
                "cross_out": cross_out[f],
                "out": out_deg[f],
                "module": mod_of[f],
            }
            for f in mod_culprits
        ],
        "depth_culprits": [
            {"file": rel(f), "level": file_level[f], "blast": blast[f], "out": out_deg[f]}
            for f in depth_culprits
        ],
        "blast_culprits": [
            {"file": rel(f), "blast": blast[f], "in": in_deg[f], "out": out_deg[f]}
            for f in blast_culprits
        ],
        "equality_culprits": [
            {"file": rel(f), "out": out_deg[f], "deviation": abs(out_deg[f] - median_out)}
            for f in eq_culprits
        ],
    }

    aggregates = {
        "cycles": cycles,
        "max_depth": max_depth,
        "max_level_raw": max_level,
        "longest_chain_edges": longest_chain_edges,
        "modularity_q": round(q, 4),
        "gini_out_degree": round(g_out, 4),
        "equality_proxy": round(equality_proxy, 4),
        "scc_count": scc_count,
        "median_out": median_out,
        "module_count": len(set(mod_of.values())),
    }

    result = {
        "project": ROOT.replace("\\", "/"),
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "file_count": len(files),
        "edge_count": len(edges),
        "aggregates": aggregates,
        "diagnostics": diagnostics,
    }

    if COMPARE_BASELINE:
        sentrux_baseline = {
            "quality_signal": 0.6872,
            "acyclicity_raw": 0,
            "max_depth": 4,
            "modularity_q": 0.4307,
            "equality_raw": 0.5642,
            "redundancy": 0.150,
        }

        delta_notes = []
        delta_notes.append(
            f"acyclicity match: ours={cycles} sentrux=0 -> "
            + ("MATCH" if cycles == 0 else f"MISMATCH (we found {cycles} cyclic SCCs)")
        )
        delta_notes.append(
            f"max_depth match: ours={max_depth} sentrux=4 -> "
            + ("MATCH" if max_depth == 4 else f"MISMATCH ({max_depth} vs 4)")
        )
        mod_delta = abs(q - 0.4307)
        delta_notes.append(
            f"modularity_q delta: ours={q:.4f} sentrux=0.4307 delta={mod_delta:.4f} -> "
            + ("WITHIN_BAND" if mod_delta <= 0.10 else "OUT_OF_BAND")
            + " (likely due to module-boundary definition differences)"
        )
        delta_notes.append(
            f"equality not directly comparable: sentrux=0.5642 (function CC), ours={equality_proxy:.4f} (1-Gini(out_degree) proxy)"
        )

        result["comparison_to_sentrux"] = {
            "sentrux_quality_signal": sentrux_baseline["quality_signal"],
            "sentrux_acyclicity_raw": sentrux_baseline["acyclicity_raw"],
            "sentrux_max_depth": sentrux_baseline["max_depth"],
            "sentrux_modularity_q": sentrux_baseline["modularity_q"],
            "sentrux_equality_raw": sentrux_baseline["equality_raw"],
            "our_cycles": cycles,
            "our_max_depth": max_depth,
            "our_modularity_q": round(q, 4),
            "our_gini_out_degree": round(g_out, 4),
            "delta_notes": delta_notes,
        }

    os.makedirs(OUT_DIR, exist_ok=True)
    out_path = os.path.join(OUT_DIR, "diagnostics.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2, ensure_ascii=False)

    if MIRROR:
        mirror_dir = os.path.dirname(MIRROR)
        if mirror_dir:
            os.makedirs(mirror_dir, exist_ok=True)
        with open(MIRROR, "w", encoding="utf-8") as f:
            json.dump(result, f, indent=2, ensure_ascii=False)

    sys.stderr.write("\n=== sentrux POC diagnostics ===\n")
    sys.stderr.write(f"project: {ROOT}\n")
    sys.stderr.write(f"files={len(files)}  edges={len(edges)}  scc={scc_count}\n")
    if COMPARE_BASELINE:
        sys.stderr.write(
            f"cycles={cycles} (sentrux=0) [{'match' if cycles == 0 else 'mismatch'}]\n"
        )
        sys.stderr.write(
            f"max_depth={max_depth} (sentrux=4) [{'match' if max_depth == 4 else 'mismatch'}]\n"
        )
        sys.stderr.write(
            f"modularity_q={q:.4f} (sentrux=0.4307) delta={mod_delta:.4f} "
            f"[{'within_band' if mod_delta <= 0.10 else 'out_of_band'}]\n"
        )
        sys.stderr.write(
            f"gini_out={g_out:.4f}  equality_proxy={equality_proxy:.4f} (sentrux=0.5642 fn-CC, not directly comparable)\n"
        )
    else:
        sys.stderr.write(f"cycles={cycles}\n")
        sys.stderr.write(f"max_depth={max_depth}\n")
        sys.stderr.write(f"modularity_q={q:.4f}\n")
        sys.stderr.write(f"gini_out={g_out:.4f}  equality_proxy={equality_proxy:.4f}\n")
    sys.stderr.write(f"wrote {out_path}\n")
    if MIRROR:
        sys.stderr.write(f"wrote {MIRROR}\n")


if __name__ == "__main__":
    main()
