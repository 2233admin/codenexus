#!/usr/bin/env python3
"""
linear_sync_decisions.py — Sync 2026-04-26 decision-closure to Linear.

What this script does:
  1. Rename Linear project (id=5c8f1e26-...) "Stitch" -> "CodeNexus"
  2. Close XAR-224..231 (decisions milestone) to Done state, with description appendix
     summarizing which decision was closed
  3. Cancel XAR-238 (rmcp spike) with replacement note pointing to mcp-go
  4. Create new issue "Phase 2 spike: A2A endpoint + Go-Rust IPC over A2A protocol"
     in Phase 0 Spike milestone (replaces the killed rmcp spike risk)

Default mode: --dry-run (prints planned mutations as JSON, sends nothing).
Apply mode: --apply (sends real GraphQL mutations).

Reads $LINEAR_API_KEY from environment.

This script is one-time (single decision-closure event). Kept under scripts/ as
project history rather than ~/AppData/Local/Temp because it documents the
rename event for future audit (e.g., Q4 2026: "when did we go from Stitch to
CodeNexus?" -> grep this file).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

API = "https://api.linear.app/graphql"

PROJECT_ID = "5c8f1e26-c63d-4372-bcd9-4d94d04788a3"
NEW_PROJECT_NAME = "CodeNexus"
DECISION_ISSUE_IDENTIFIERS = [f"XAR-{n}" for n in range(224, 232)]  # 224..231 inclusive
RMCP_SPIKE_IDENTIFIER = "XAR-238"

# State IDs from ~/.claude/projects/C--Users-Administrator/memory/linear_api_access.md
STATE_DONE = "178ede6f-5b43-441f-a604-b3d28720c8b1"
STATE_CANCELED = "a0ecf30a-91e8-41da-8e9b-06e2274e5307"
STATE_BACKLOG = "3054dea2-f130-44e6-9c07-3b79cd5aab8c"

# Phase 0 Spike milestone (where new IPC spike issue lands)
PHASE_0_SPIKE_MILESTONE_ID = "c19e79f8-56d0-4308-8502-bd491ef983eb"

# Per-issue decision results (closure 2026-04-26)
# Each entry: identifier -> (verdict_label, detailed_result_text)
DECISION_RESULTS: dict[str, tuple[str, str]] = {
    "XAR-224": ("DEFERRED to Phase 2 spike",
                "Storage choice (redb vs rusqlite+sqlite-vec) requires bench data on real workload. "
                "Phase 2 (SPEC Phase 0) spike will run shootout on 10K embeddings + FTS5 query mix and pick. "
                "Keeping decision open is correct — premature lock would force a rewrite later."),
    "XAR-225": ("KILLED by architecture pivot",
                "rmcp Rust MCP SDK maturity is no longer a blocker because the architecture pivoted from "
                "pure-Rust to Rust core + Go service layer. MCP server now lives in Go via mark3labs/mcp-go "
                "(mature, well-maintained). See XAR-238 for the related spike cancellation."),
    "XAR-226": ("DECIDED: candle embedded (Snowflake/BERT family)",
                "Trade-off accepted: ~80MB binary size + ~30s cold-start in exchange for zero external "
                "dependency. Users do not need Ollama installed. ollama-rs and async-openai remain "
                "pluggable via EmbedderBackend trait, just not the default."),
    "XAR-227": ("DECIDED: self-contained store (revisit Phase 5)",
                "CodeNexus owns its storage layer entirely for MVP. Phase 5 (Bridge) revisits whether to "
                "share PG with memU for fused recall. Defaulting self-contained avoids cross-project "
                "coupling during MVP race."),
    "XAR-228": ("DECIDED: CodeNexus (locked)",
                "Recovered from working name 'Stitch' to descriptive 'CodeNexus'. Accepted SEO/branding "
                "risk of GitNexus same-root similarity — explicit naming wins over abstract metaphor for "
                "discovery."),
    "XAR-229": ("DECIDED: D:/projects/codenexus/ (new repo)",
                "Clean separation from obsidian-llm-wiki monorepo. Independent Cargo workspace + git "
                "history. Path name aligned to product name (codenexus, not stitch)."),
    "XAR-230": ("DECIDED: option B (axum/Go HTTP-served web + cytoscape.js)",
                "Briefly pivoted to option A (Tauri + SolidJS) then back to B. Single fat-binary "
                "preserved via //go:embed; cross-platform packaging cost of Tauri not justified for MVP. "
                "Tauri stays available as v2 upgrade if user demand surfaces."),
    "XAR-231": ("DEFERRED to post-MVP",
                "GitHub releases binary is sufficient for pre-MVP / alpha distribution. cargo install / "
                "homebrew tap / scoop manifest decisions made after MVP precision target hit."),
}

# License decision (NEW3) is not a separate Linear issue but worth referencing
LICENSE_NOTE = "Plus NEW decision: License MIT -> Apache 2.0 (explicit patent grant + NOTICE)."

# Architecture pivot decision (NEW + NEW2) is referenced from individual issues
PIVOT_REFERENCE = (
    "Cross-cutting NEW decisions: (a) Architecture pivot pure-Rust -> Rust core + Go service layer; "
    "(b) IPC = A2A protocol over localhost HTTP (not stdio JSON-RPC). See PROJECT.md Key Decisions table."
)


def build_appendix(identifier: str) -> str:
    """Build per-issue closure appendix."""
    verdict, detail = DECISION_RESULTS.get(
        identifier, ("CLOSED 2026-04-26", "Decision closed in 2026-04-26 session.")
    )
    return f"""

---
**Closed 2026-04-26 -- {verdict}**

{detail}

{PIVOT_REFERENCE}

{LICENSE_NOTE}

Full decision record: D:/projects/codenexus/.planning/PROJECT.md (Key Decisions table).
"""

RMCP_REPLACEMENT_NOTE = """

---
**Canceled 2026-04-26**

Replaced by `mark3labs/mcp-go` (Go MCP SDK). The architecture pivot from pure-Rust
to Rust core + Go service layer eliminated the need for rmcp — the MCP server now
lives in the Go layer where mcp-go is mature and well-maintained.

See PROJECT.md "Key Decisions" table for the full architecture pivot rationale.
A new spike (Phase 0 / GSD Phase 2) covers Go-Rust IPC over A2A protocol design,
which subsumes the original rmcp spike's question of "can we serve MCP from this
language at all?".
"""

NEW_IPC_SPIKE_TITLE = "Phase 2 (SPEC Phase 0) spike: A2A endpoint + Go-Rust IPC over A2A protocol"
NEW_IPC_SPIKE_DESCRIPTION = """\
**Replaces XAR-238 (rmcp spike)** after 2026-04-26 architecture pivot.

## Goal
Validate that:
1. Rust core can serve a Google A2A v0.2 protocol endpoint via axum (POST /tasks/send + GET /tasks/{id})
2. Go server can act as A2A client via stdlib net/http, with healthcheck + restart on Rust crash
3. Roundtrip latency < 5ms p99 on localhost loopback (excluding actual query work)
4. Concurrent A2A clients (Go local + remote curl) can call Rust core without contention bugs
5. Long-running tasks (e.g. indexing a 50-file repo, ~30s) work via polling without timeout issues

## Sub-tasks
- [ ] Implement minimal A2A endpoint in axum (echo task)
- [ ] Write Go A2A client wrapper (stdlib only, no SDK)
- [ ] Benchmark roundtrip on Win/Linux loopback
- [ ] Test: kill Rust process, verify Go restarts within 5s
- [ ] Test: 2+ concurrent clients (no race conditions)
- [ ] Decide: long-running task uses polling (current) or SSE stream (extension)

## GO/NO-GO criteria
- GO: latency target met + lifecycle robust + spec conformance verified
- NO-GO: fall back to alternate plan (e.g. stdio JSON-RPC if A2A overhead is unacceptable)

## Reference
- A2A spec: https://google.github.io/A2A/
- mcp-go: https://github.com/mark3labs/mcp-go
- axum: https://docs.rs/axum/latest/axum/
"""


def gql(query: str, variables: dict | None = None, api_key: str | None = None) -> dict:
    """Send a GraphQL request. Returns parsed JSON response."""
    if api_key is None:
        api_key = os.environ.get("LINEAR_API_KEY")
    if not api_key:
        raise SystemExit("LINEAR_API_KEY not set in environment")

    body = json.dumps({"query": query, "variables": variables or {}}).encode("utf-8")
    req = urllib.request.Request(
        API,
        data=body,
        headers={
            "Content-Type": "application/json",
            "Authorization": api_key,
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        return {"errors": [{"message": f"HTTP {e.code}: {e.read().decode('utf-8')}"}]}


def fetch_issues(identifiers: list[str], api_key: str) -> list[dict]:
    """Fetch full issue records by identifier."""
    query = """
    query Issues($ids: [String!]!) {
      issues(filter: { number: { in: [] }, identifier: { in: $ids } }, first: 50) {
        nodes {
          id
          identifier
          title
          state { id name }
          description
        }
      }
    }
    """
    # Linear's filter API doesn't accept identifier directly; need to query each.
    # Fall back to per-identifier query.
    issues = []
    for ident in identifiers:
        single_query = """
        query I($team: String!, $number: Float!) {
          issues(filter: { team: { key: { eq: $team } }, number: { eq: $number } }, first: 1) {
            nodes {
              id
              identifier
              title
              state { id name }
              description
            }
          }
        }
        """
        team, num_str = ident.split("-")
        result = gql(single_query, {"team": team, "number": float(num_str)}, api_key)
        if "errors" in result:
            print(f"  ERROR fetching {ident}: {result['errors']}", file=sys.stderr)
            continue
        nodes = result.get("data", {}).get("issues", {}).get("nodes", [])
        if not nodes:
            print(f"  WARN: {ident} not found", file=sys.stderr)
            continue
        issues.append(nodes[0])
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true",
                        help="Send real mutations (default: dry-run, prints only)")
    parser.add_argument("--api-key", default=None,
                        help="Linear API key (default: $LINEAR_API_KEY env var)")
    args = parser.parse_args()

    api_key = args.api_key or os.environ.get("LINEAR_API_KEY")
    if not api_key:
        print("ERROR: LINEAR_API_KEY not set and --api-key not provided", file=sys.stderr)
        return 2

    mode = "APPLY" if args.apply else "DRY-RUN"
    print(f"=== Linear sync — {mode} ===\n")

    # Step 1: Fetch current state of relevant issues
    print(f"Step 1: Fetching {len(DECISION_ISSUE_IDENTIFIERS) + 1} issues...")
    all_idents = DECISION_ISSUE_IDENTIFIERS + [RMCP_SPIKE_IDENTIFIER]
    issues = fetch_issues(all_idents, api_key)
    print(f"  Found {len(issues)} issues:")
    for i in issues:
        print(f"    {i['identifier']}: [{i['state']['name']}] {i['title']}")
    print()

    # Step 2: Plan mutations
    print("Step 2: Planned mutations:\n")

    print(f"  M1. projectUpdate(id={PROJECT_ID[:8]}..., name={NEW_PROJECT_NAME!r})")
    print()

    decision_issues = [i for i in issues if i["identifier"] in DECISION_ISSUE_IDENTIFIERS]
    for issue in decision_issues:
        verdict, _ = DECISION_RESULTS.get(issue["identifier"], ("CLOSED", ""))
        print(f"  M2.{issue['identifier']}. issueUpdate(state=Done) -- {verdict}")
    print()

    rmcp_issue = next((i for i in issues if i["identifier"] == RMCP_SPIKE_IDENTIFIER), None)
    if rmcp_issue:
        print(f"  M3. issueUpdate({RMCP_SPIKE_IDENTIFIER}: state=Canceled, description += RMCP_REPLACEMENT_NOTE)")
    else:
        print(f"  M3. SKIP — {RMCP_SPIKE_IDENTIFIER} not found")
    print()

    print(f"  M4. issueCreate(team=XAR, title={NEW_IPC_SPIKE_TITLE!r}, milestone=Phase 0 Spike)")
    print()

    if not args.apply:
        print("=== DRY-RUN complete. Re-run with --apply to send mutations. ===")
        return 0

    # Step 3: Apply mutations
    print("Step 3: APPLYING mutations...\n")

    # M1: Rename project
    print(f"  M1: Renaming project to {NEW_PROJECT_NAME}...")
    rename = gql(
        "mutation R($id: String!, $input: ProjectUpdateInput!) { "
        "projectUpdate(id: $id, input: $input) { success project { name } } }",
        {"id": PROJECT_ID, "input": {"name": NEW_PROJECT_NAME}},
        api_key,
    )
    if "errors" in rename:
        print(f"    ERROR: {rename['errors']}", file=sys.stderr)
    else:
        new_name = rename.get("data", {}).get("projectUpdate", {}).get("project", {}).get("name")
        print(f"    OK: project name = {new_name!r}")
    print()

    # M2: Close decision issues
    for issue in decision_issues:
        print(f"  M2.{issue['identifier']}: Closing to Done...")
        new_desc = (issue.get("description") or "") + build_appendix(issue["identifier"])
        result = gql(
            "mutation U($id: String!, $input: IssueUpdateInput!) { "
            "issueUpdate(id: $id, input: $input) { success issue { identifier state { name } } } }",
            {"id": issue["id"], "input": {"stateId": STATE_DONE, "description": new_desc}},
            api_key,
        )
        if "errors" in result:
            print(f"    ERROR: {result['errors']}", file=sys.stderr)
        else:
            state = result.get("data", {}).get("issueUpdate", {}).get("issue", {}).get("state", {}).get("name")
            print(f"    OK: state = {state!r}")
    print()

    # M3: Cancel rmcp spike
    if rmcp_issue:
        print(f"  M3: Canceling {RMCP_SPIKE_IDENTIFIER}...")
        new_desc = (rmcp_issue.get("description") or "") + RMCP_REPLACEMENT_NOTE
        result = gql(
            "mutation U($id: String!, $input: IssueUpdateInput!) { "
            "issueUpdate(id: $id, input: $input) { success issue { identifier state { name } } } }",
            {"id": rmcp_issue["id"], "input": {"stateId": STATE_CANCELED, "description": new_desc}},
            api_key,
        )
        if "errors" in result:
            print(f"    ERROR: {result['errors']}", file=sys.stderr)
        else:
            state = result.get("data", {}).get("issueUpdate", {}).get("issue", {}).get("state", {}).get("name")
            print(f"    OK: state = {state!r}")
        print()

    # M4: Create new IPC spike issue
    print("  M4: Creating new IPC spike issue...")
    # Need team ID. Pull from team key=XAR.
    team_q = gql(
        'query { teams(filter: { key: { eq: "XAR" } }, first: 1) { nodes { id } } }',
        api_key=api_key,
    )
    team_id = team_q.get("data", {}).get("teams", {}).get("nodes", [{}])[0].get("id")
    if not team_id:
        print(f"    ERROR: could not resolve XAR team id: {team_q}", file=sys.stderr)
    else:
        result = gql(
            "mutation C($input: IssueCreateInput!) { "
            "issueCreate(input: $input) { success issue { identifier title } } }",
            {"input": {
                "teamId": team_id,
                "projectId": PROJECT_ID,
                "title": NEW_IPC_SPIKE_TITLE,
                "description": NEW_IPC_SPIKE_DESCRIPTION,
                "stateId": STATE_BACKLOG,
                "projectMilestoneId": PHASE_0_SPIKE_MILESTONE_ID,
            }},
            api_key,
        )
        if "errors" in result:
            print(f"    ERROR: {result['errors']}", file=sys.stderr)
        else:
            issue = result.get("data", {}).get("issueCreate", {}).get("issue", {})
            print(f"    OK: created {issue.get('identifier')} - {issue.get('title')}")

    print("\n=== APPLY complete ===")
    return 0


if __name__ == "__main__":
    sys.exit(main())
