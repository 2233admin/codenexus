"""
Sentrux McpDiagnostics POC — file-level "嫌犯名单" generator.

Replicates the sentrux Pro `McpDiagnostics` feature using only OSS MCP outputs:
- scan(path): builds in-memory index
- health(): aggregate 5-dim signal + raw counts
- dsm(format="text"): per-file levels + edges classified above/below diagonal

Output: ranked list of files most contributing to each weak dimension.

Usage:
    python poc.py <path-to-project>

Requires: sentrux 0.5.7+ on PATH (or override via SENTRUX_BIN env var).
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path


SENTRUX_BIN = os.environ.get("SENTRUX_BIN", "sentrux")


class SentruxMcpClient:
    """Minimal JSON-RPC stdio client for sentrux mcp."""

    def __init__(self, sentrux_bin: str = SENTRUX_BIN):
        self.proc = subprocess.Popen(
            [sentrux_bin, "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            encoding="utf-8",
        )
        self.req_id = 0
        self._initialize()

    def _send(self, method: str, params: dict | None = None) -> dict:
        self.req_id += 1
        req = {
            "jsonrpc": "2.0",
            "id": self.req_id,
            "method": method,
            "params": params or {},
        }
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            stderr = self.proc.stderr.read()
            raise RuntimeError(f"sentrux mcp died. stderr:\n{stderr}")
        return json.loads(line)

    def _initialize(self) -> None:
        resp = self._send(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "sentrux-diagnostics-poc", "version": "0.1"},
            },
        )
        if "error" in resp:
            raise RuntimeError(f"initialize failed: {resp['error']}")
        # mcp protocol: send initialized notification (no response expected)
        notif = {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}
        self.proc.stdin.write(json.dumps(notif) + "\n")
        self.proc.stdin.flush()

    def call_tool(self, name: str, args: dict | None = None) -> dict:
        resp = self._send("tools/call", {"name": name, "arguments": args or {}})
        if "error" in resp:
            raise RuntimeError(f"tool {name} error: {resp['error']}")
        return resp.get("result", {})

    def close(self) -> None:
        try:
            self.proc.stdin.close()
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()


def extract_text(result: dict) -> str:
    """sentrux mcp returns content as [{type:'text', text:'...'}]."""
    content = result.get("content", [])
    if not content:
        return ""
    return "\n".join(item.get("text", "") for item in content if item.get("type") == "text")


def parse_dsm_text(dsm_text: str) -> dict:
    """
    Parse sentrux dsm format=text. Expected sections:
      - "## Files (level | path)" listing each file with its level
      - Edge classifications somewhere (above/below/same)

    sentrux 0.5.7 dsm text format may vary; we extract whatever we can find.
    Returns: {file: {"level": int, "out_edges": [(to, level_diff)]}}
    """
    files: dict[str, dict] = {}
    lines = dsm_text.splitlines()

    # Pass 1: find file level lines like "  3 | path/to/file.ts"
    in_file_section = False
    for line in lines:
        s = line.strip()
        if s.lower().startswith("## files") or s.lower().startswith("# files"):
            in_file_section = True
            continue
        if in_file_section and s.startswith("##"):
            in_file_section = False
            continue
        if in_file_section and "|" in s:
            parts = [p.strip() for p in s.split("|", 1)]
            if len(parts) == 2 and parts[0].lstrip("-").isdigit():
                lvl = int(parts[0])
                path = parts[1]
                files[path] = {"level": lvl, "out_edges": []}

    # Pass 2: find edge lines like "from -> to (level_diff: +1)" or "above_diagonal: from -> to"
    # sentrux's actual format may differ; this is a best-effort parser.
    for line in lines:
        s = line.strip()
        if "->" not in s:
            continue
        parts = s.split("->")
        if len(parts) != 2:
            continue
        from_part = parts[0].strip().split()[-1] if parts[0].strip() else ""
        to_part = parts[1].strip().split()[0] if parts[1].strip() else ""
        if from_part in files and to_part in files:
            from_lvl = files[from_part]["level"]
            to_lvl = files[to_part]["level"]
            files[from_part]["out_edges"].append((to_part, to_lvl - from_lvl))

    return files


def extract_health_metrics(health_text: str) -> dict:
    """Extract numeric metrics from health() text output. Best-effort regex."""
    import re

    metrics: dict = {}
    patterns = {
        "quality_signal": r"quality[_ ]signal[:\s]+([0-9.]+)",
        "modularity": r"modularity[:\s]+([-0-9.]+)",
        "acyclicity": r"acyclicity[:\s]+([0-9.]+)",
        "depth": r"depth[:\s]+([0-9.]+|\d+)",
        "equality": r"equality[:\s]+([0-9.]+)",
        "redundancy": r"redundancy[:\s]+([0-9.]+)",
        "max_depth": r"max[_ ]depth[:\s]+(\d+)",
        "circular_dep_count": r"(?:cycles|circular[_ ]dep)[:\s]+(\d+)",
        "god_file_count": r"god[_ ]files[:\s]+(\d+)",
    }
    for key, pat in patterns.items():
        m = re.search(pat, health_text, re.IGNORECASE)
        if m:
            try:
                metrics[key] = float(m.group(1))
            except ValueError:
                pass
    return metrics


def rank_modularity_culprits(files: dict) -> list[tuple[str, float, dict]]:
    """
    Rank files by their contribution to MODULARITY weakness.
    Score = cross_module_out_degree / total_out_degree, with floor for low-degree files.
    Higher = more cross-module spaghetti.
    """
    def module_of(path: str) -> str:
        # Heuristic: first 2 path segments = module
        parts = path.replace("\\", "/").split("/")
        return "/".join(parts[:2]) if len(parts) >= 2 else parts[0]

    ranked = []
    for path, info in files.items():
        out = info.get("out_edges", [])
        if not out:
            continue
        from_mod = module_of(path)
        cross = sum(1 for (to, _) in out if module_of(to) != from_mod)
        total = len(out)
        score = cross / total if total else 0.0
        # Penalize files with both high cross-ratio and high out-degree
        weighted = score * (total ** 0.5)
        ranked.append((path, weighted, {"cross": cross, "total": total, "ratio": round(score, 3)}))

    ranked.sort(key=lambda x: x[1], reverse=True)
    return ranked[:20]


def rank_depth_culprits(files: dict) -> list[tuple[str, int, dict]]:
    """Rank files at the deepest levels — they sit on top of long chains."""
    if not files:
        return []
    max_lvl = max(info["level"] for info in files.values())
    ranked = [
        (path, info["level"], {"level": info["level"], "max": max_lvl})
        for path, info in files.items()
        if info["level"] >= max_lvl - 1
    ]
    ranked.sort(key=lambda x: x[1], reverse=True)
    return ranked[:20]


def rank_acyclicity_culprits(files: dict) -> list[tuple[str, int, dict]]:
    """Rank files participating in upward edges (above-diagonal)."""
    upward = defaultdict(int)
    for path, info in files.items():
        for to, diff in info.get("out_edges", []):
            if diff > 0:
                upward[path] += 1
                upward[to] += 1
    ranked = [(p, c, {"upward_touches": c}) for p, c in upward.items()]
    ranked.sort(key=lambda x: x[1], reverse=True)
    return ranked[:20]


def main() -> int:
    if len(sys.argv) < 2:
        print("Usage: poc.py <project-path>", file=sys.stderr)
        return 2
    project_path = str(Path(sys.argv[1]).resolve())
    out_dir = Path(__file__).parent / "out"
    out_dir.mkdir(exist_ok=True)

    print(f"[poc] Connecting to sentrux mcp...", file=sys.stderr)
    t0 = time.monotonic()
    client = SentruxMcpClient()
    try:
        print(f"[poc] scan({project_path})...", file=sys.stderr)
        scan_result = client.call_tool("scan", {"path": project_path})
        scan_text = extract_text(scan_result)
        (out_dir / "scan.txt").write_text(scan_text, encoding="utf-8")
        print(f"[poc] scan done in {time.monotonic()-t0:.1f}s, output {len(scan_text)} chars", file=sys.stderr)

        print(f"[poc] health()...", file=sys.stderr)
        health_result = client.call_tool("health", {})
        health_text = extract_text(health_result)
        (out_dir / "health.txt").write_text(health_text, encoding="utf-8")
        metrics = extract_health_metrics(health_text)

        print(f"[poc] dsm(format=text)...", file=sys.stderr)
        dsm_result = client.call_tool("dsm", {"format": "text"})
        dsm_text = extract_text(dsm_result)
        (out_dir / "dsm.txt").write_text(dsm_text, encoding="utf-8")

    finally:
        client.close()

    print(f"[poc] Parsing dsm and ranking culprits...", file=sys.stderr)
    files = parse_dsm_text(dsm_text)

    report: dict = {
        "project": project_path,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "health_metrics": metrics,
        "file_count_parsed": len(files),
        "diagnostics": {
            "modularity_culprits": [
                {"file": p, "score": round(s, 3), **info}
                for (p, s, info) in rank_modularity_culprits(files)
            ],
            "depth_culprits": [
                {"file": p, "level": s, **info}
                for (p, s, info) in rank_depth_culprits(files)
            ],
            "acyclicity_culprits": [
                {"file": p, "upward_touches": s, **info}
                for (p, s, info) in rank_acyclicity_culprits(files)
            ],
        },
    }

    out_path = out_dir / "diagnostics.json"
    out_path.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"[poc] Written: {out_path}", file=sys.stderr)
    print(json.dumps({k: v for k, v in report.items() if k != "diagnostics"}, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
