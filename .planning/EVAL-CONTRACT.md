---
frozen_at: 2026-05-02
status: FROZEN
governs: agent-outcome eval (memory-assisted edit MVP)
does_not_govern: retrieval-layer regression eval (spike-001 B1-B7, FSC F1-F10 -- continue as separate regression guards)
authorship_session: 2026-05-02 next-session opening per audit Decision 2 LOCKED gamma-split
audit_source: .planning/audits/2026-05-02-codex-strategic-review.md (Decision 2)
preregistration_basis: R1 attack #2 ("preregistered scoring rubric isolates motivation") and R2 prescription ("Frozen eval v1 created BEFORE any tuning lands")
---

# CodeNexus EVAL-CONTRACT (Agent-Outcome Eval)

This contract preregisters the eval that will judge whether CodeNexus
changes agent edit behavior. It is frozen Week 0 (before any code in
the memory-assisted MVP path is written). Specific task instances and
specific repo selections ride Week 4-6 per the Codex 6-week cadence.
Locked fields below are not negotiable post-freeze without an explicit
changelog entry plus written rationale (see `## Changelog discipline`).

The contract exists because retrieval-precision numbers (REQ-10 B1-B7
mean=67.9%) are not evidence that agents edit better with CodeNexus.
That is a separate claim, requires a separate eval, and the eval has to
be written before the build to isolate motivation.

## Scope boundary

| In scope | Out of scope |
|----------|--------------|
| Agent edit-task outcomes with vs without CodeNexus memory layer | Retrieval precision (covered by REQ-10 B1-B7 + FSC F1-F10) |
| Required-symbol inspection during edit prep | A2A endpoint latency (covered by REQ-06 acceptance) |
| Forbidden-edit avoidance via ADR constraints | UI usability (no UI claims tied to this contract) |
| Cross-baseline relative comparison | Standalone CodeNexus capability claims |

If a question is "did retrieval precision change?" -- it is a REQ-10
question, not a contract question. If a question is "did the agent
edit better?" -- it is a contract question.

## Locked fields (frozen 2026-05-02)

### Task taxonomy (counts + categories locked, instances NOT yet)

30 frozen tasks total, distributed:

| Category | Count | What it measures |
|----------|-------|------------------|
| bugfix | 10 | Agent inspects right symbols + edits right files for a bug |
| refactor | 10 | Agent surfaces caller/callee fan-out before structural change |
| API behavior change | 5 | Agent identifies all consumers of a behavior change |
| forbidden-edit-because-ADR | 5 | Agent declines or warns on edits an ADR forbids |

Counts and categories LOCKED. Specific task wording, expected files,
expected symbols, and forbidden bad edits ride **task-instance authoring
(Week 4-6)**, governed by separate authoring discipline that this contract
gates against backsliding.

Authoring constraint applied at instance time:

- Each task carries: starting prompt + expected touched files + required
  inspected symbols + forbidden bad edits + ground-truth ADR references
  for forbidden-edit category.
- Forbidden-edit category is only authorable after 04.5-03 lands stable
  symbol identity AND minimal arch metrics ship. (See risk R2 below.)

### Baselines (4, locked names)

Each task is run under all four to make relative comparison meaningful:

| Baseline | What the agent has access to | Why included |
|----------|------------------------------|--------------|
| B0 no-tool | Pure prompt + filesystem read | Establishes raw model floor |
| B1 rg + manual reads | Prompt + ripgrep + Read | Establishes "competent dev with grep" floor |
| B2 current-CodeNexus | Prompt + `query` + `list_callers` + `get_symbol` (today's A2A surface) | Establishes pre-memory CodeNexus uplift |
| B3 CodeNexus-MVP-with-memory | B2 surface + `query_constraints` + `remember_symbol_note` + `get_edit_context` (memory-MVP A2A surface) | The proposition under test |

Names LOCKED. Tool wrapping for each baseline rides instance authoring
(specifically: Claude Code MCP config that exposes only the baseline's
allowed tools per run).

### Metrics (6, locked definitions)

Per task per baseline, compute:

| Metric | Type | Definition |
|--------|------|------------|
| M1 required-symbol-inspection recall | mechanical | (inspected symbols intersect expected symbols) / (expected symbols). Inspected = symbols the agent actually opened/read during the run, captured from tool-call logs. |
| M2 forbidden-edit rate | mechanical | Boolean per task: did the agent emit any edit operation (Write, Edit, NotebookEdit, or equivalent file-mutating tool call) targeting a file in the forbidden set? "Emit" means the tool call appears in the run record, regardless of whether the harness ultimately applied it. Aggregate as rate across all tasks under one baseline. |
| M3 ADR constraint recall | judgment | (relevant ADRs surfaced by agent during run) / (relevant ADRs in ground truth). Graded 0-3 with N=3 seeds, see judge-method lock. |
| M4 wrong-file edit rate | mechanical | Boolean per task: did the agent emit any edit operation (per M2 emit definition) targeting a file outside the expected touched set? Aggregate as rate. |
| M5 task completion | bifurcated | If task ships unit/integration tests: pass/fail mechanical. Else: rubric-graded 0-3 by judge with N=3 seeds. Bifurcation rule LOCKED here -- which tasks ship tests vs use rubric is locked at instance authoring time, not flippable post-run. |
| M6 tool-use latency budget | mechanical | Wall-clock seconds from task start to last edit operation emitted (per M2 emit definition), or to agent-declared completion if no edits emitted. Median + p90 across tasks under one baseline. |

Definitions LOCKED. Threshold tuning (e.g., "what counts as inspected")
is part of the definition above and cannot be relaxed silently.

### Run protocol (locked)

Within one eval run, the agent driving each baseline must be held
constant. Specifically:

- Same agent model id (e.g., `claude-opus-4-7`) across B0/B1/B2/B3.
  Switching agent model between baselines is a confound; prohibited.
- Same temperature / sampling parameters across B0/B1/B2/B3. Locked
  default: temperature=0 if the agent supports deterministic mode;
  else lowest available + record the value in the run record.
- Each (task, baseline) cell runs N=3 agent seeds (independent
  invocations). Per-cell metric values aggregate as median across the
  3 seeds for mechanical metrics and as the median-of-3-seed-judge-
  median for judgment metrics. The 3 agent seeds are independent of
  the 3 judge seeds (so M3/M5-rubric judgment is at most 9 judge
  calls per (task, baseline) cell when both rubric branch and judgment
  apply).
- The agent model id, temperature, and CodeNexus build commit SHA are
  recorded in every run record. Cross-run comparison requires these
  three to match; otherwise comparison is methodological and requires
  a methodology note.

Run-protocol fields LOCKED. Specific agent-model choice rides instance
authoring (Week 4-6) and is locked at that time before any task runs.

### Success gate (numbers locked)

The memory-MVP claim succeeds against current-CodeNexus IF AND ONLY IF
all three conditions hold simultaneously when comparing B3 to B2:

```
gate_a: forbidden_edit_rate(B3) <= 0.75 * forbidden_edit_rate(B2)
        AND wrong_file_edit_rate(B3) <= 0.75 * wrong_file_edit_rate(B2)
        (>= 25% relative reduction in BOTH bad-edit metrics)

gate_b: required_symbol_recall(B3) - required_symbol_recall(B2) >= 0.20
        (>= 20pp absolute improvement in required-symbol recall)

gate_c: median_latency(B3) <= 2.0 * median_latency(B2)
        (no greater than 2x median task wall-clock)
```

Numbers LOCKED. If gate_a or gate_b fails, the memory-MVP DID NOT MEET
the contract -- regardless of secondary metrics. If gate_c fails, the
memory-MVP shipped at unacceptable latency cost regardless of quality
gains, also a contract failure.

Numbers were chosen for falsifiability (a real win shows clearly; a
marginal win does not pass) not for predicted feasibility. They may
prove too aggressive; that finding is itself information. Relaxation
post-freeze requires changelog entry naming the relaxation, the run that
exposed it, and the new threshold. Silent relaxation is a contract
violation.

### Repo selection criteria (rules locked, specific repos NOT yet)

When task instances are authored, the 3 chosen repos must collectively
satisfy:

- C1 anti-self-eval-bias: at least 1 repo where Curry is NOT primary
  author (no commit-history dominance). The other 2 may be
  author-authored if they cover language gaps the non-author repo
  misses.
- C2 size band: each repo between 5k and 100k LoC. Smaller repos make
  graph navigation trivial; larger repos make eval runtime infeasible.
- C3 multi-language coverage: across the 3 repos, all of {TypeScript,
  Python, Go} must appear, matching the cut-down language ambition
  (TS+Python+Go per audit Decision 4 direction).
- C4 license compatibility: repos must allow read access for eval
  purposes. Public OSS or explicit author consent.
- C5 ADR availability: at least 1 repo must have markdown decision
  records (ADR / RFC / design doc) that the forbidden-edit category
  can ground truth against. If no candidate repo has ADRs, instance
  authoring inserts ADRs first as a separate step (NOT as eval
  authoring shortcut -- they must be real decisions documented before
  eval design).

Rules LOCKED. Specific repo choices ride instance authoring; choices
must be recorded in `.planning/EVAL-INSTANCES.md` (sibling file, written
Week 4-6) with how each criterion is satisfied.

### Judge-method lock

LOCKED position (the audit said "do not punt past contract"):

- Mechanical metrics (M1, M2, M4, M6): no judge. Captured from tool-call
  logs and edit diff. Implementation must emit a structured run record
  (one JSON object per task per baseline) that contains all evidence
  for these metrics. If a metric cannot be computed mechanically from
  the run record alone, the run record is incomplete and the run is
  invalid.
- Judgment metrics (M3, M5-rubric branch): LLM-as-judge graded 0-3 with
  N=3 seeds (matches existing R5/R6/R6c LLM-judge methodology pattern --
  see PROJECT.md "Graded LLM-judge eval pipeline" differentiator).
  Final grade per (task, baseline, metric) = median across 3 seeds.
- Reliability monitor: per eval run, Curry hand-grades a stratified
  20% sample of judgment-metric records (1 per task category per
  baseline minimum). If hand-grade vs LLM-judge disagreement on the
  sample exceeds 25% (where "disagreement" = grade delta >= 2 on a 0-3
  scale, or polarity flip on pass/fail rubric), the eval run is
  INVALIDATED. Re-run requires rubric tightening (changelog entry) +
  re-judging. Reliability monitor is non-optional.
- Judge-model selection: rides instance authoring (Week 4-6). The
  contract locks the method, not the model. Whichever model is chosen
  must be locked before any judging starts and used for all 3 seeds in
  a given eval run; comparing across runs that used different judge
  models requires a methodology note.

Note: the bifurcation rule for M5 (tests vs rubric) means rubric judging
is bounded -- not every task triggers it. Tasks that ship tests bypass
LLM-judge for M5.

## Acknowledged risks (carry forward to instance authoring)

R1 self-eval bias if all 3 repos end up being Curry's. Mitigation: C1
above forces at least one non-author repo. Residual risk: even one
non-author repo, the other two may dominate the aggregate. Instance
authoring must report aggregate AND non-author-repo-only metrics
separately. A claim that fails on non-author-repo-only is a contract
failure even if aggregate passes.

R2 forbidden-edit ground truth depends on ADR set, which depends on
04.5-03 (stable symbol identity) plus minimal arch metrics (constraint
attachment to symbol/file/module scope). Mitigation: forbidden-edit
category authoring is gated on 04.5-03 land + Week 2 minimal arch
metrics ship. If instance authoring proceeds before either gate clears,
that batch of 5 forbidden-edit tasks is provisional and re-authored
post-gate. Contract treats this as a known temporal dependency, not a
contract weakness.

R3 LLM-as-judge cost vs reliability at scale. Mitigation: 20% human
spot-check above bounds reliability cost. Cost ceiling: at 30 tasks * 4
baselines * N=3 seeds * 2 judgment metrics = 720 judge calls per eval
run. At one judge run per Week 6 milestone + roughly one per major
config change, this is bounded and cheap relative to engineering time.
Residual risk: if the cost-ceiling assumption fails at scale (e.g.
multi-thousand task expansion), this contract is the wrong tool and a
new eval design replaces it; the contract does not silently sample down.

## Coexistence with retrieval-layer evals

Retrieval-layer evals continue unchanged:

- spike-001 B1-B7 retrieval eval: REQ-10 acceptance gate, regression
  band [65.9%, 69.9%] for any retrieval-layer change including 04.5-03.
  Governs retrieval precision claims.
- FSC F1-F10 hand-eval: cross-corpus generous-denominator regression
  guard, gate >= 50%. Governs cross-corpus generalization claims.

EVAL-CONTRACT.md governs agent-outcome claims (different beast). A
single change can touch all three; it must clear all three's relevant
gates separately. Retrieval gates failing does not invalidate
agent-outcome gates and vice versa, but a release claiming "memory MVP
helps agents" requires this contract to pass while the retrieval gates
also remain green.

## Changelog discipline

Any change to a LOCKED field above (taxonomy counts/categories, baseline
names/tools, metric definitions, success gate numbers, repo selection
criteria, judge-method) requires:

1. A row appended to the changelog table below, with date + summary +
   rationale + (if relaxation) explicit acknowledgment that this is a
   relaxation.
2. The change committed in the same commit as the modified contract
   field.
3. If the change is a relaxation triggered by a failing run, a
   reference to the run record. Silent relaxation = contract violation.
4. New eval runs after a changelog entry are reported with the changelog
   version they were judged under (e.g., "Run R3 against contract v1.2").

Tightenings (raising a gate, adding a metric, narrowing a definition)
follow the same discipline. The contract version increments on any
change.

| Date | Version | Change | Rationale | Triggering run (if any) | Relaxation? |
|------|---------|--------|-----------|-------------------------|-------------|
| 2026-05-02 | v1.0 | Initial freeze | Audit Decision 2 LOCKED gamma-split: preregister CONTRACT Week 0 to isolate motivation, populate DATA per Codex Week 4-6 cadence. R1 #2 critique addressed by lock-before-build discipline; R2 prescription ("Frozen eval v1 created BEFORE any tuning lands") satisfied. | n/a | n/a |

## Glossary

- **Agent-outcome eval**: measures whether an LLM agent (e.g., Claude
  Code via MCP) edits source code better when CodeNexus memory layer is
  available, vs without. Contrast with retrieval-layer eval which
  measures top-K precision against a known query set.
- **Memory-assisted MVP / memory MVP**: the system delivered by Codex's
  6-week cadence -- minimal arch metrics + ADR extraction +
  query_constraints + remember_symbol_note + get_edit_context + MCP
  wrapping (audit synthesis, lines 60-67 of strategic review).
- **Run record**: structured JSON emitted per (task, baseline) execution
  capturing tool calls, files inspected, edits committed, wall-clock
  timing, and judge inputs/outputs. Implementation lives in
  task-instance harness; contract requires sufficient detail to
  mechanically compute M1/M2/M4/M6 and to feed M3/M5-rubric to judge
  with reproducibility (seed, prompt, model id).
- **Stratified sample (reliability monitor)**: 20% human spot-check
  drawing at least 1 sample per task category (bugfix, refactor, API,
  forbidden-edit) per baseline. With 30 tasks * 4 baselines = 120 cells,
  20% = 24 cells minimum, satisfied by 1-per-category-per-baseline = 16
  cells expanded to 24 by adding 8 extras drawn proportional to
  category size.

## Provenance

- Contract authored 2026-05-02 in next-session opening per audit
  Decision 2 LOCKED gamma-split.
- Locked fields trace 1:1 to audit lines 126-141 of
  `.planning/audits/2026-05-02-codex-strategic-review.md`.
- 04.5-03 PRE-PLAN-NOTES (commit f303d64) and PROJECT.md "Graded
  LLM-judge eval pipeline" differentiator (line 56) inform the
  judge-method lock.
- Audit basis: Codex R1 attack #2 + R2 prescription + decision walk
  through alpha/beta/gamma with gamma selected (commit 525d317).

End of contract.
