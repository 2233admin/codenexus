"""Probe data shapes from sentrux mcp on a real project."""
import json
import subprocess

proc = subprocess.Popen(
    ["sentrux", "mcp"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    text=True, bufsize=1, encoding="utf-8",
)


def send(method, params=None):
    req = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}
    proc.stdin.write(json.dumps(req) + "\n")
    proc.stdin.flush()
    return json.loads(proc.stdout.readline())


def call_tool(name, args=None):
    r = send("tools/call", {"name": name, "arguments": args or {}})
    if "error" in r:
        return f"ERROR: {r['error']}"
    content = r.get("result", {}).get("content", [])
    return "\n".join(c.get("text", "") for c in content if c.get("type") == "text")


send("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "probe", "version": "0.1"}})
proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}) + "\n")
proc.stdin.flush()

PROJECT = "D:/projects/obsidian-llm-wiki"
print("[scan]", call_tool("scan", {"path": PROJECT})[:200])

print("\n=== dsm format=matrix ===")
print(call_tool("dsm", {"format": "matrix"})[:1000])

print("\n=== dsm format=json ===")
print(call_tool("dsm", {"format": "json"})[:1000])

print("\n=== dsm format=list ===")
print(call_tool("dsm", {"format": "list"})[:1000])

print("\n=== git_stats(days=90) ===")
print(call_tool("git_stats", {"days": 90})[:1500])

print("\n=== test_gaps(limit=20) ===")
print(call_tool("test_gaps", {"limit": 20})[:1500])

proc.terminate()
proc.wait(timeout=5)
