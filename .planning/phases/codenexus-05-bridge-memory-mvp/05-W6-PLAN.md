---
phase: 5
slice: 05-W6
plan_id: 05-W6
title: "W6: Eval harness skeleton -- 30-task curated set + B2/B3/B3-min runner"
wave: 6
depends_on: [05-W3, 05-W4, 05-W5]
status: PLAN-AUTHORED (awaits plan-checker iter)
files_modified:
 - .planning/EVAL-INSTANCES.md
 - experiments/poc-retrieval/eval/affordance_harness.py
 - experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl
 - experiments/poc-retrieval/eval/affordance_runner.ps1
locked_decisions_honored:
 - G6  # B2 (control) / B3 (treatment) / B3-min (ablation) eval design per discuss-mcp section 6
 - UQ-B4  # spurious-call penalty: penalize both under-call and over-call
gates:
 - G-A  # harness skeleton exists; 30-task JSONL ships; runner orchestrates 3 modes
 - G-B  # smoke run: 1 task x 3 modes completes end-to-end (results dump produced)
 - G-C  # EVAL-INSTANCES.md authored with judge model + agent model locks per BETA-V1-SPEC section 8 line 215
 - G-D  # eval result schema documented; downstream plan-checker / Curry can interpret without authoring
---

> **!! PROVISIONAL !!** This plan was authored 2026-05-03 in parallel with
> CCG round 2 challenge. Codex surfaced 4 critical issues (CI-1 G2 LOC,
> CI-2 G3 SQL FK, CI-3 G4 handler, CI-4 G5 FTS5) plus 3 missed constraints
> that affect this slice. **Do NOT execute this plan as-is.** See
> `.planning/phases/codenexus-05-bridge-memory-mvp/05-CCG-ROUND-2-FINDINGS.md`
> for required amendments before plan-checker iter and execution.


<objective>
Land the W6 eval harness skeleton per BETA-V1-SPEC section 8 line 215 + G6
section 6. The MUST 6 cost gate (~25% improvement / >=20pp / <=2x cost
of B2) cannot be evaluated without the harness existing. W6 ships:

1. 30-task curated task set spanning 3 repos (per BETA-V1-SPEC MUST 6).
  Tasks have ground truth: `expected_tools` (subset of 9 MCP tools),
  `constraint_anchor` (file/symbol where a real ADR or note applies;
  NULL if N/A), `edit_target` (symbol_id ending up edited; NULL if
  read-only).
2. B2 / B3 / B3-min runner: spins up an agent (claude-code or
  equivalent) against each task in each mode, captures tool-invocation
  trace + final output, computes 5 metrics per G6 section 6.
3. EVAL-INSTANCES.md: locks judge model + agent model + run config per
  BETA-V1-SPEC section 8 acceptance gate.

W6 does NOT run the actual eval against V1.0 binary -- that is the
post-W6 ratification step. W6 ships the SKELETON that makes the eval
runnable. Per discuss SUMMARY: "W6 may live in EVAL-INSTANCES.md per
BETA-V1-SPEC section 8 line 215 -- decide at plan-time". DECIDED at plan-time:
EVAL-INSTANCES.md owns the locks (judge + agent model, pass criteria);
experiments/poc-retrieval/eval/ owns the executable harness.

Out of scope: actual eval results (post-V1.0 ship); 100+ task
expansion (V1.1+); multi-agent eval (V1.1+); auto-mining tasks from
session traces (V1.1+); A/B description prose variants (V1.1+).

Output:
- `.planning/EVAL-INSTANCES.md` (NEW or extend if exists): judge model
 + agent model lock + run procedure + pass criteria.
- `experiments/poc-retrieval/eval/affordance_harness.py` (NEW): Python
 harness orchestrating 3 run modes; consumes 30-tasks.jsonl; produces
 results.json with the 5 G6 section 6 metrics.
- `experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl`
 (NEW): 30 tasks with ground truth labels.
- `experiments/poc-retrieval/eval/affordance_runner.ps1` (NEW):
 Windows-friendly runner that drives the Python harness end-to-end
 (start MCP server, run agent, capture trace, score, dump results).
</objective>

<plan_time_decisions>
- **D-W6-01 (judge model lock):** Anthropic Claude Opus 4.7 (1M
 context) per drift_evidence_probe.md probe runner convention +
 PROJECT.md primary eval model. Judge runs ground-truth comparison
 against agent output. SAME judge across all 3 modes (B2 / B3 /
 B3-min) to control for judge variance.
- **D-W6-02 (agent model lock):** Anthropic Claude Sonnet 4.5 OR
 Claude Opus 4.7 -- pick the cheaper one for V1.0 30-task x 3-mode x
 N=3 seed = 270 runs cost-budget. RECOMMEND Sonnet 4.5 as agent +
 Opus 4.7 as judge (asymmetric). Per UQ-B4 the eval should reflect a
 REAL Claude session; Sonnet 4.5 is the realistic agent model.
- **D-W6-03 (3 repos):** Use existing fsc + poc-retrieval (already
 drift-probed) + ONE additional unrelated repo to stress
 cross-codebase generalization. Candidates per BETA-V1-SPEC: any
 open-source mid-size Rust / Python / TypeScript repo. RECOMMEND:
 cline (TypeScript MCP client) OR ripgrep (Rust). Lock in
 EVAL-INSTANCES.md.
- **D-W6-04 (task authoring):** 30 tasks distributed roughly
 10 per repo. Each task type:
 - 10 "edit-with-constraint" tasks (constraint_anchor != NULL,
  expected_tools includes query_constraints OR get_edit_context)
 - 10 "edit-without-constraint" tasks (constraint_anchor == NULL,
  expected_tools should NOT include query_constraints to avoid
  over-call penalty)
 - 5 "exploration" tasks (read-only, expected_tools = {query,
  get_symbol})
 - 5 "post-edit annotation" tasks (after editing, agent should call
  remember_symbol_note; expected_tools includes
  remember_symbol_note)
- **D-W6-05 (3 run modes):**
 - B2 (control): MCP server registers ONLY existing 4 tools
  (index_repo, query, get_symbol, list_callers). Agent has no
  access to constraint / note / composite tools. Implementation:
  Go MCP server flag `--tools=base` excludes 5 new tools.
 - B3 (treatment): MCP server registers all 9 tools with G6 W5
  descriptions.
 - B3-min (ablation): MCP server registers all 9 tools but with
  minimal descriptions ("Returns notes for a symbol." style). This
  requires a build flag or runtime mode in mcpsrv/server.go --
  LOCKED in W6 plan-time: add `--description-mode=full|minimal` to
  the Go MCP serve command.
- **D-W6-06 (metrics)** per G6 section 6:
 1. Final task success (judge LLM scores agent's edit/answer 0/1)
 2. Tool-invocation precision + recall per tool vs ground truth
 3. Spurious-call rate (tool called when ground truth says no)
 4. Constraint-surfaced rate (% of constraint_anchor != NULL tasks
   where query_constraints invoked AND retrieved expected clause)
 5. Cost ratio (total tokens + wall-clock vs B2 baseline)
- **D-W6-07 (pass gate)** per G6 section 6:
 - B3 beats B2 by >= 25% on metric 1
 - B3 beats B2 by >= 20pp on metric 4
 - B3 cost <= 2x B2 cost (metric 5)
 - B3-min DOES NOT beat B2 by the same margin (ablation expectation
  -- if B3-min wins, description quality didn't matter)
- **D-W6-08 (W6 PLAN-only ships skeleton, NOT first eval run):** Per
 objective, W6 lands the harness + tasks + instances doc. The
 ACTUAL first eval run (270 trials, ~$X cost) is a post-V1.0
 ratification step gated on Curry decision. W6 SUMMARY documents
 what the runner does + how to invoke + expected runtime + cost.
</plan_time_decisions>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-mcp.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W3-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W4-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W5-PLAN.md
@.planning/BETA-V1-SPEC.md
@PROJECT.md
@server/internal/mcpsrv/server.go

<interfaces>
<!-- Target 30-tasks.jsonl line shape -->
```jsonl
{"task_id": "fsc-001", "repo": "full-self-coding", "task_type": "edit-with-constraint", "prompt": "Refactor the consecutive_fails counter in src/embedder.rs to use a shared atomic instead of a per-task field.", "expected_tools": ["query", "get_edit_context", "remember_symbol_note"], "constraint_anchor": {"path": "docs/ARCHITECTURE.md", "line": 412}, "edit_target": "embedder.rs::EmbedClient", "judge_criteria": "Did the agent surface the 'counter MUST stay in caller's loop' constraint BEFORE editing?"}
```

<!-- Target affordance_harness.py surface -->
```python
# experiments/poc-retrieval/eval/affordance_harness.py
def run_task(task: dict, mode: str, agent_model: str, judge_model: str) -> dict:
  """Run one task in one mode. Returns trace + scored result."""
  ...

def main(args):
  """Iterate 30 tasks x 3 modes x N seeds; dump results.json"""
  ...
```

<!-- Target EVAL-INSTANCES.md outline -->
```markdown
# EVAL-INSTANCES (Phase 5 W6 lock)

## Locks
- Judge model: claude-opus-4-7 (1M context)
- Agent model: claude-sonnet-4-5 (per cost budget)
- 3 repos: full-self-coding, poc-retrieval, <third>
- 3 run modes: B2 / B3 / B3-min
- N seeds: 3 per (task, mode) cell

## Pass criteria (per G6 section 6)
- B3 vs B2: >= 25% on success rate, >= 20pp on constraint-surfaced rate
- B3 cost <= 2x B2 cost
- B3-min ablation: SHOULD NOT beat B2 by same margin

## Run procedure
[step-by-step]

## Schema
[results.json schema]
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="false">
 <name>Task 1: 30-tasks.jsonl + affordance_harness.py + Go MCP --tools / --description-mode flags</name>
 <files>experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl, experiments/poc-retrieval/eval/affordance_harness.py, experiments/poc-retrieval/eval/affordance_runner.ps1, server/internal/mcpsrv/server.go, server/cmd/mcp.go</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-mcp.md section 6 (eval design)
  - server/internal/mcpsrv/server.go (W3 + W4 + W5 final state -- locate s.AddTool calls; add filtering by --tools flag)
  - server/cmd/mcp.go (cobra command for `mcp` subcommand -- add --tools + --description-mode flags)
  - existing experiments/poc-retrieval/eval/ scripts for invocation conventions
 </read_first>

 <action>

**Step A -- Go MCP serve flags.** In `server/cmd/mcp.go` add two cobra
flags:
- `--tools` string (default "all"; values: "base" | "all"). When
 "base": Go MCP server skips the 5 new tools (only registers
 index_repo, query, get_symbol, list_callers).
- `--description-mode` string (default "full"; values: "full" |
 "minimal"). When "minimal": replaces the 5 G6-grade descriptions
 with 1-line stubs ("Returns notes." / etc.) for the B3-min ablation.

In `server/internal/mcpsrv/server.go` thread these via the constructor
(`RunStdio(client, toolsMode, descriptionMode string) error` or
similar). The 5 new s.AddTool calls become conditional:
```go
if toolsMode == "all" {
  desc := queryConstraintsDesc
  if descriptionMode == "minimal" {
    desc = "Returns ranked constraints for a file, symbol, or topic."
  }
  s.AddTool(mcp.NewTool("query_constraints", mcp.WithDescription(desc), ...), ...)
}
// Same for the other 4 new tools.
```

**Step B -- 30-tasks.jsonl authoring.** Create
`experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl` with
30 lines, distributed per D-W6-04:
- Tasks fsc-001 to fsc-010 (full-self-coding repo)
- Tasks poc-001 to poc-010 (poc-retrieval repo)
- Tasks third-001 to third-010 (third repo, locked in EVAL-INSTANCES.md)

Each task line per the schema in `<interfaces>`. For Phase 5 W6
SKELETON purposes: AUTHOR REAL TASKS for fsc + poc (executor pulls
from existing PLAN files / commits / known constraints in each repo);
for third repo, ship 10 PLACEHOLDER tasks with `task_type: "TODO_third_repo"` and a comment noting that real authoring depends on the
third-repo lock. EVAL-INSTANCES.md flags this as P2 follow-up.

Authoring sources:
- fsc constraints: docs/ARCHITECTURE.md known MUST/MUST-NOT clauses
 (use extract_adrs --dry_run --scope=docs/ARCHITECTURE.md to enumerate)
- poc constraints: similar; CONTEXT.md flagged ambiguities provide
 natural constraint anchors
- Real ADRs from extract_adrs output -> task design

**Step C -- affordance_harness.py.** Python script that:
1. Reads 30-tasks.jsonl.
2. For each task x mode x seed:
  a. Spawns Go MCP server with appropriate --tools / --description-mode.
  b. Invokes claude-code (or equivalent) agent with the task prompt + MCP server stdio attached.
  c. Captures the agent's tool-invocation trace + final output.
  d. Sends agent output + ground truth to judge model for scoring.
  e. Records into a results dict.
3. Computes 5 metrics from G6 section 6 across all runs.
4. Dumps `results.json` with per-task + per-mode + per-metric breakdowns.

Pseudocode:
```python
def run_task(task, mode, agent_model, judge_model):
  server_proc = spawn_mcp(mode) # passes --tools and --description-mode
  try:
    trace = invoke_agent(agent_model, task["prompt"], mcp_server=server_proc)
    score = judge(judge_model, task, trace)
    return {"task": task["task_id"], "mode": mode, "trace": trace, "score": score}
  finally:
    server_proc.terminate()

def main():
  tasks = [json.loads(l) for l in open("affordance_tasks/30-tasks.jsonl")]
  results = []
  for task in tasks:
    for mode in ["B2", "B3", "B3-min"]:
      for seed in range(3):
        r = run_task(task, mode, AGENT_MODEL, JUDGE_MODEL)
        results.append({**r, "seed": seed})
  metrics = compute_metrics(results)
  json.dump({"results": results, "metrics": metrics}, open("results.json", "w"))
```

For W6 SKELETON: `invoke_agent` and `judge` are STUBBED to return
canned shapes -- W6 ships the SCAFFOLD; first real run is post-V1.0
per D-W6-08. SUMMARY documents that stubs return synthetic data so
the harness pipeline runs end-to-end without burning eval budget.

**Step D -- affordance_runner.ps1.** Windows wrapper:
```powershell
# Build Go server first
cd D:/projects/codenexus/server
go build -o codenexus-mcp.exe ./cmd

# Run harness
cd D:/projects/codenexus/experiments/poc-retrieval/eval
python affordance_harness.py --tasks affordance_tasks/30-tasks.jsonl --modes B2,B3,B3-min --seeds 3 --agent claude-sonnet-4-5 --judge claude-opus-4-7
```

**Step E -- smoke test:** invoke runner with stub agent + stub judge,
verify pipeline produces results.json:
```bash
cd D:/projects/codenexus/experiments/poc-retrieval/eval
python affordance_harness.py --tasks affordance_tasks/30-tasks.jsonl --modes B2,B3,B3-min --seeds 1 --stub
test -f results.json
```

 </action>

 <acceptance_criteria>
  - `test -f experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl` exits 0
  - `wc -l experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl` returns 30
  - All 30 lines parse as JSON: `python -c "import json; [json.loads(l) for l in open('experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl')]"` exits 0
  - `test -f experiments/poc-retrieval/eval/affordance_harness.py` exits 0
  - `test -f experiments/poc-retrieval/eval/affordance_runner.ps1` exits 0
  - `grep -nF '--tools' server/cmd/mcp.go` >= 1 hit
  - `grep -nF '--description-mode' server/cmd/mcp.go` >= 1 hit
  - `grep -nE 'toolsMode == "all"' server/internal/mcpsrv/server.go` >= 1 hit (conditional registration)
  - `cd server && go build ./... && go vet ./...` exits 0 (G-A Go)
  - `python experiments/poc-retrieval/eval/affordance_harness.py --stub --tasks experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl --modes B2,B3 --seeds 1` exits 0 AND produces results.json
  - 30-tasks.jsonl distribution audit (executor): 10 edit-with-constraint + 10 edit-without-constraint + 5 exploration + 5 post-edit annotation = 30 (D-W6-04 distribution)
 </acceptance_criteria>

 <verify>
  <automated>cd server && go build ./... && go vet ./... && wc -l ../experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl && python ../experiments/poc-retrieval/eval/affordance_harness.py --stub --modes B2,B3 --seeds 1</automated>
 </verify>

 <done>
  Go MCP server gains --tools + --description-mode flags; conditional
  tool registration honored. 30-tasks.jsonl ships 30 valid task
  lines (fsc + poc real; third repo placeholder). affordance_harness.py
  + affordance_runner.ps1 ship; stub mode runs end-to-end producing
  results.json. G-A (Go build clean), G-B (smoke run completes).
 </done>
</task>

<task type="auto" tdd="false">
 <name>Task 2: EVAL-INSTANCES.md authoring + results.json schema doc</name>
 <files>.planning/EVAL-INSTANCES.md</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-mcp.md section 6 (eval design + pass gate)
  - .planning/BETA-V1-SPEC.md section 8 line 215 (EVAL-INSTANCES.md authority)
  - PROJECT.md lines 102-110 (Software 3.0 reframe + eval-as-load-bearing)
  - if .planning/EVAL-CONTRACT.md exists, read it for cross-reference (BETA-V1-SPEC section 8 line 226 references EVAL-CONTRACT v1.0)
 </read_first>

 <action>

**Step A -- author `.planning/EVAL-INSTANCES.md`** (~150-250 lines).
Outline:

```markdown
---
phase: 5
artifact: EVAL-INSTANCES
status: LOCKED (Phase 5 W6 ratifies; first run post-V1.0)
authority:
 - BETA-V1-SPEC.md section 8 line 215 (this document is the deliverable)
 - 05-discuss-mcp.md section 6 (eval design + pass gate)
 - PROJECT.md lines 102-110 (Software 3.0 reframe)
parent_artifacts:
 - .planning/phases/codenexus-05-bridge-memory-mvp/05-W6-PLAN.md
 - experiments/poc-retrieval/eval/affordance_harness.py
 - experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl
authored: 2026-05-XX (Phase 5 W6)
---

# EVAL-INSTANCES (Phase 5 V1.0 Affordance Eval)

This document locks the eval configuration that runs the B2 / B3 /
B3-min comparison gating Phase 5 V1.0 ship. The harness lives at
`experiments/poc-retrieval/eval/affordance_harness.py`.

## Locks (per Phase 5 W6)

| Item | Lock | Rationale |
|------|------|-----------|
| Judge model | claude-opus-4-7 (1M context) | Cost-efficient + matches PROJECT.md primary eval model |
| Agent model | claude-sonnet-4-5 | Realistic agent profile; cost budget for 270-run sweep |
| Repos (3) | full-self-coding, poc-retrieval, <THIRD locked at run-time> | drift-probed + diverse |
| Tasks | 30 (10 per repo) | per BETA-V1-SPEC MUST 6 |
| Seeds per (task, mode) | 3 | variance estimate |
| Modes | B2 / B3 / B3-min | per G6 section 6 |
| Total runs | 30 x 3 x 3 = 270 | budget-bounded |

## Pass criteria (per G6 section 6)

- **PASS** if all of:
 - B3 success rate (metric 1) >= B2 + 25%
 - B3 constraint-surfaced rate (metric 4) >= B2 + 20pp
 - B3 cost (metric 5) <= 2x B2 cost
 - B3-min DOES NOT beat B2 by the above margins (ablation: confirms
  description quality is the lever, not tool existence)

If B3-min PASSES the same margins as B3, the result is INVALID:
description quality didn't matter, which means either (a) the eval
is not measuring what we think OR (b) the tools are good enough that
even minimal descriptions don't matter (unlikely per CodeCompass
58% skip baseline).

## Modes

### B2 (control)
Go MCP server runs with `--tools=base`. Only the 4 existing tools
(index_repo, query, get_symbol, list_callers) are registered. No
constraint / note / composite tools.

### B3 (treatment)
Go MCP server runs with `--tools=all --description-mode=full`. All 9
tools registered (4 existing + 5 new) with the G6 W5 production-grade
descriptions.

### B3-min (ablation)
Go MCP server runs with `--tools=all --description-mode=minimal`. All
9 tools registered but the 5 new tools have 1-line stub descriptions
("Returns notes for a symbol." style). Tests whether description
quality is the lever (per G6 section 6 ablation expectation).

## Task structure

[Reference 30-tasks.jsonl schema; show 1-2 example tasks; explain
each ground-truth field and its scoring contribution.]

## Run procedure

1. Build Go MCP server: `cd server && go build -o codenexus-mcp.exe ./cmd`
2. Build Rust core: `cd experiments/poc-retrieval && cargo build --workspace --release`
3. Index 3 repos: `target/release/codenexus-core index --repo <repo> --db <repo>.db` for each
4. Verify ADRs extracted: `sqlite3 <repo>.db "SELECT COUNT(*) FROM adrs"` > 0
5. Run harness: `python experiments/poc-retrieval/eval/affordance_harness.py --tasks affordance_tasks/30-tasks.jsonl --modes B2,B3,B3-min --seeds 3 --agent claude-sonnet-4-5 --judge claude-opus-4-7`
6. Inspect results.json
7. Apply pass criteria from above
8. If PASS: ratify Phase 5 V1.0 (commit `docs(beta-v1): ratify Phase 5 V1.0 per W6 eval pass`); if FAIL: triage failure modes per G6 section 6 metrics 2/3 to identify which tools / descriptions need iteration

## Results schema

`results.json`:
```json
{
 "metadata": {"timestamp": "...", "agent_model": "...", "judge_model": "...", "git_sha": "..."},
 "results": [
  {"task_id": "...", "mode": "B2|B3|B3-min", "seed": 0..N, "trace": [...], "scores": {...}}
 ],
 "metrics": {
  "B2":   {"success_rate": 0.X, "constraint_surfaced_rate": 0.Y, "cost_tokens": N, ...},
  "B3":   {...},
  "B3-min": {...}
 },
 "pass": true|false,
 "pass_evidence": {...}
}
```

## V1.0 vs V1.1+ scope split

V1.0 (this doc): 30 tasks x 3 repos x 3 modes x 3 seeds, claude-sonnet-4-5
agent + claude-opus-4-7 judge.

V1.1+ (deferred):
- 100+ tasks (auto-mined from real session traces)
- multi-agent eval (Sonnet + Opus + GPT-5)
- A/B description prose variants (telemetry-driven)
- Per-agent-model description tuning

## Honest gap list

- Third repo not yet locked at PLAN-time; W6 task placeholders
 noted; lock at first-run preparation.
- 270 runs at sonnet pricing ~ $X (executor estimates at run-time);
 budget-confirm with Curry before kick-off.
- Judge model variance not bounded; if first run shows high variance,
 consider N=5 seeds OR different judge.
```

(EXECUTOR: keep ASCII-safe per rule 17. Use `--` not em-dash.)

 </action>

 <acceptance_criteria>
  - `test -f .planning/EVAL-INSTANCES.md` exits 0
  - `wc -l .planning/EVAL-INSTANCES.md` returns >= 100 AND <= 300
  - `grep -F 'B2' .planning/EVAL-INSTANCES.md` >= 1 hit
  - `grep -F 'B3' .planning/EVAL-INSTANCES.md` >= 1 hit
  - `grep -F 'B3-min' .planning/EVAL-INSTANCES.md` >= 1 hit
  - `grep -F 'claude-opus-4-7' .planning/EVAL-INSTANCES.md` >= 1 hit (judge model lock)
  - `grep -F 'claude-sonnet-4-5' .planning/EVAL-INSTANCES.md` >= 1 hit (agent model lock)
  - `grep -F '25%' .planning/EVAL-INSTANCES.md` >= 1 hit (pass criterion)
  - `grep -F '20pp' .planning/EVAL-INSTANCES.md` >= 1 hit (pass criterion)
  - No Unicode arrows / em-dashes (ASCII-safe per rule 17)
 </acceptance_criteria>

 <verify>
  <automated>test -f .planning/EVAL-INSTANCES.md && wc -l .planning/EVAL-INSTANCES.md && grep -cF 'claude-opus-4-7' .planning/EVAL-INSTANCES.md && grep -cF 'B3-min' .planning/EVAL-INSTANCES.md</automated>
 </verify>

 <done>
  .planning/EVAL-INSTANCES.md authored (~150-250 lines, ASCII-safe).
  Contains: locks (judge + agent + repos + tasks + seeds + modes),
  pass criteria from G6 section 6, B2/B3/B3-min mode definitions, run
  procedure, results.json schema, V1.0/V1.1+ scope split, honest
  gap list. G-C (instances doc ships) + G-D (results schema
  documented) verified.
 </done>
</task>

</tasks>

<gates>
- **G-A** (harness exists + Go build clean): all 4 new files ship; Go MCP server builds with new flags. [Task 1]
- **G-B** (smoke run completes): stub harness invocation produces results.json end-to-end without error. [Task 1]
- **G-C** (EVAL-INSTANCES.md ships): authoritative doc with locks + pass criteria + run procedure. [Task 2]
- **G-D** (results schema documented): schema in EVAL-INSTANCES.md is interpretable by Curry / plan-checker without re-authoring. [Task 2]
</gates>

<must_haves>
truths:
 - "30-tasks.jsonl ships with 30 valid task lines distributed per D-W6-04 (10 edit-with-constraint, 10 edit-without-constraint, 5 exploration, 5 post-edit annotation)"
 - "Go MCP server gains --tools (base|all) + --description-mode (full|minimal) flags supporting the 3 run modes"
 - "affordance_harness.py orchestrates 30 tasks x 3 modes x 3 seeds, computes 5 metrics from G6 section 6, dumps results.json"
 - "Stub-mode smoke run completes end-to-end producing results.json (W6 ships SKELETON; first real run is post-V1.0 per D-W6-08)"
 - ".planning/EVAL-INSTANCES.md locks judge model (claude-opus-4-7), agent model (claude-sonnet-4-5), 3 repos, pass criteria (>= 25% success, >= 20pp constraint-surfaced, <= 2x cost), B3-min ablation expectation"
 - "BETA-V1-SPEC section 8 line 215 deliverable satisfied (EVAL-INSTANCES.md exists with judge + agent locks)"
artifacts:
 - path: ".planning/EVAL-INSTANCES.md"
  provides: "Eval lock document per BETA-V1-SPEC section 8 line 215"
  contains: "claude-opus-4-7"
 - path: "experiments/poc-retrieval/eval/affordance_harness.py"
  provides: "Python harness orchestrating 30 tasks x 3 modes x N seeds"
  contains: "def run_task"
 - path: "experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl"
  provides: "30-task curated set with ground truth labels"
  contains: "expected_tools"
 - path: "experiments/poc-retrieval/eval/affordance_runner.ps1"
  provides: "Windows-friendly invocation wrapper"
  contains: "affordance_harness.py"
 - path: "server/cmd/mcp.go"
  provides: "--tools + --description-mode cobra flags"
  contains: "--tools"
 - path: "server/internal/mcpsrv/server.go"
  provides: "conditional tool registration based on toolsMode + descriptionMode"
  contains: "toolsMode"
key_links:
 - from: "experiments/poc-retrieval/eval/affordance_harness.py"
  to: "experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl"
  via: "task loader"
  pattern: "30-tasks\\.jsonl"
 - from: "experiments/poc-retrieval/eval/affordance_harness.py"
  to: "server/cmd/mcp.go (via subprocess)"
  via: "spawn_mcp passes --tools + --description-mode"
  pattern: "--tools|--description-mode"
 - from: ".planning/EVAL-INSTANCES.md"
  to: "experiments/poc-retrieval/eval/affordance_harness.py + 30-tasks.jsonl"
  via: "run procedure references the executable harness"
  pattern: "affordance_harness|30-tasks"
</must_haves>

<verification>
1. `cd server && go build ./...` clean (G-A Go)
2. `wc -l experiments/poc-retrieval/eval/affordance_tasks/30-tasks.jsonl` returns 30 (G-A)
3. `python experiments/poc-retrieval/eval/affordance_harness.py --stub` produces results.json (G-B)
4. `test -f .planning/EVAL-INSTANCES.md && wc -l` returns 100-300 (G-C)
5. results.json schema documented in EVAL-INSTANCES.md (G-D verified by grep)
</verification>

<open_questions>
- **OQ-W6-01:** Third repo lock -- pull from BETA-V1-SPEC if specified, else Curry decision at first-run preparation. W6 ships placeholder tasks pending lock.
- **OQ-W6-02:** Cost estimate for first real eval run (270 trials). Executor estimates at run-time per Anthropic API current pricing; budget-confirm with Curry before kick-off (per D-W6-08 the first real run is NOT in W6 scope).
- **OQ-W6-03:** Agent harness (claude-code vs raw API) -- if agent runs via claude-code subprocess, harness MUST handle stdio multiplexing (claude-code stdout vs MCP server stdin/stdout). If raw Anthropic API + manual MCP simulation, harness implements MCP protocol stub itself. Plan-checker confirms feasibility either way.
</open_questions>

<honest_gap_list>
**P1**:
- Third repo placeholder tasks (D-W6-04) -- 10 of 30 tasks are stubs until third repo locks. Mitigation: EVAL-INSTANCES.md flags as P2 follow-up; harness can run with 20 tasks in stub mode for skeleton verification.
- W6 ships SKELETON only per D-W6-08; first real eval run (270 trials, ~$X cost) is post-V1.0 ratification step. The B3 vs B2 PASS/FAIL determination that gates BETA-V1-SPEC MUST 6 is NOT part of W6 -- this PLAN's scope is "make the eval RUNNABLE", not "run the eval". This is intentional but plan-checker should confirm with Curry that this scope split is acceptable.

**P2**:
- 30 tasks for fsc + poc are AUTHORED at execution time by the executor. Quality of task ground-truth labels (expected_tools, constraint_anchor) directly drives metric reliability. Plan-checker may want to spot-check 3-5 tasks for quality.
- Stub agent + stub judge in smoke mode return canned shapes; the harness pipeline is exercised but no real eval signal. Acceptable per D-W6-08; SUMMARY documents.
- Description-mode ablation requires editing server.go to support BOTH full and minimal descriptions in one binary. This is a small refactor of W5's const-based approach into a runtime branch. If W5 hard-coded the consts, W6 Step A undoes that and re-introduces the if/else.

**P3**:
- 270-run cost is variable based on agent + judge model pricing changes. Estimate at run-time.
- Judge variance not characterized; first real run may need N=5 seeds if metric stddev too high. Defer.
- W6 does NOT actually verify B3-min < B3 (the ablation claim) -- that requires the first real run. The PLAN's job is to make the experiment possible, not pre-confirm the result.
</honest_gap_list>
</content>
