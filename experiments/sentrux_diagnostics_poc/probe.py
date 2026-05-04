"""Probe sentrux mcp tool surface and example outputs."""
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


send("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "probe", "version": "0.1"}})
proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}) + "\n")
proc.stdin.flush()

tools = send("tools/list")
print("=== TOOLS ===")
for t in tools.get("result", {}).get("tools", []):
    print(f"\n{t['name']}")
    print(f"  desc: {t.get('description', '')[:120]}")
    schema = t.get("inputSchema", {})
    for prop, spec in (schema.get("properties") or {}).items():
        enum = spec.get("enum", "")
        print(f"    - {prop}: {spec.get('type', '?')} {enum}")

proc.terminate()
proc.wait(timeout=5)
