---
frozen_at: 2026-05-02
status: FROZEN
governs: CodeNexus Beta V1 definition + timeline + scope cuts
basis: Codex strategic analysis 2026-05-02 (CCG PARTIAL; Gemini infrastructure bug unfixed) + Curry synthesis (α drift compression / β parallel decisions / γ named V1.1+ backlog / δ private-beta hybrid)
parent_audit: .planning/audits/2026-05-02-codex-strategic-review.md
related_artifacts: .planning/EVAL-CONTRACT.md (v1.0 frozen 49bae0d) + .planning/probes/drift_evidence_probe.md (commit 2582dae)
---

# CodeNexus Beta V1 Spec

> **Frozen artifact.** This document locks the definition of Beta V1 for CodeNexus.
> Authored: 2026-05-02. CCG verdict: PARTIAL (Codex-only; Gemini down).
> Do not silently expand scope. Any MUST change requires explicit commit + reason.

---

## § 0  What This Document Locks

Beta V1 = locked feature set + eval evidence + public release hygiene.
NOT "whatever ships first to get feedback" (that's a private alpha).
This frame is intentionally rigorous. If evidence fails, we ship honestly as
"evidence-failed Beta V1", not by lowering the bar.

---

## § 1  8 MUSTs (all required for Beta V1 to be declared)

| # | Requirement | Anchored in | Notes |
|---|-------------|-------------|-------|
| 1 | REQ-10 no-regression (stays in 67.9% band) | PROJECT.md REQ-10 + 04.5-03 G-D | Baseline must be re-verified after every model swap |
| 2 | Stranger on clean machine reaches queryable server state | README + offline-bootstrap doc + pre-seed script | UX is P1 per CLAUDE.md rule 37 |
| 3 | 04.5-03 lands OR drift probe demotes it to quality | W0-W5 plans + drift probe spec (commit 2582dae) | Drift probe outcome is schedule leverage, not a blocker |
| 4 | Language scope DECIDED AND DOCUMENTED before W4 entry; default: TS + Python + Go | open audit D4 + EVAL-CONTRACT C3 | Curry has explicit override window W1-W3. After W4 entry: locked. |
| 5 | Phase 5 Bridge ships memory-assisted edit surface (`query_constraints` + `remember_symbol_note` + `get_edit_context`) | EVAL-CONTRACT B3 baseline + PROJECT.md Strategic | No PLAN.md exists today -- writing it is W1 critical path (see § 8) |
| 6 | EVAL-INSTANCES.md frozen (30 tasks / 3 repos / agent + judge model lock / harness) | EVAL-CONTRACT v1.0 | ~15 hrs human-in-loop authoring (30 tasks * ~30 min). Block time. |
| 7 | Beta passes B3 vs B2 gate (>=25% / >=20pp / <=2x) OR ships as "Beta evidence failed" | EVAL-CONTRACT success gate | If gate missed: README first screen MUST contain `WARN: Beta V1 shipped without meeting B3 eval gate (see EVAL-INSTANCES results).` -- not buried in footnotes. |
| 8 | Public release hygiene (LICENSE / NOTICE / A2A framing surgery / no overclaim) | LICENSE + NOTICE + open audit D3 | Easiest in calendar terms (~few hours). Trickiest in tone. README must match eval evidence exactly. |

---

## § 2  Critical Path

```
drift probe outcome
  -> [A] ships as planned     -> Phase 5 Bridge PLAN.md (W4)
  -> [B] demotes to quality   -> Phase 5 Bridge PLAN.md (W1-W2, compressed)
       |
       v
30-task EVAL-INSTANCES authoring (~15 hrs block time)
       |
       v
full B3 vs B2 eval run (720 judge calls * N=3 seeds, multi-day)
       |
       v
Beta V1 declared (or "evidence-failed" declared honestly)
```

D1 (roadmap pivot scope) and D3 (PROJECT.md A2A framing surgery) are
**parallel**, not blocking. They can happen during 04.5-03 W3-W5
implementation. Net: ~3 days off critical path.

---

## § 3  4-Scenario Timeline

| Scenario | Trigger | Target | Confidence |
|----------|---------|--------|-----------|
| A -- planned | 04.5-03 ships W0-W5 as written | **W8 ~ 2026-06-27** | p50 |
| B -- drift compression | drift probe demotes 04.5-03 to quality | **W5-W6 ~ 2026-06-06 to 2026-06-13** | p50 if probe fires early |
| C -- eval stall | <3 repos satisfy C1-C5 OR ADR ground truth missing | **W9-W10 ~ 2026-07-04 to 2026-07-11** | p80 if risk #1 fires |
| D -- Phase 5 unspecced at W4 | no PLAN.md exists at W4 entry | **W10+ hard stop** | near-certain if Phase 5 stays unspecced |

**Point estimate W8 +/- 2w (p50). W10 (p80 if any single risk fires).**
Week 6 vs Week 8 split is bought by drift probe outcome, NOT by working harder.

### Milestone sketch (Scenario A)

- **W1 (~2026-05-09):** drift probe + D1/D3/D4 + 04.5-03 W0-W2 (or demote)
- **W2 (~2026-05-16):** 04.5-03 W3-W5 closure + REQ-10 no-regression
- **W4 (~2026-05-30):** Phase 5 Bridge PLAN.md + first slice; EVAL-INSTANCES skeleton
- **W6 (~2026-06-13):** memory MVP impl + 30 tasks authored + first eval run -> release CANDIDATE
- **W8 (~2026-06-27):** Beta V1 SHIPPABLE -- full eval pass + spot-check + release hygiene

---

## § 4  Top 3 Risks * Mitigations

| Risk | Mitigation |
|------|-----------|
| EVAL-INSTANCES authoring stalls (<3 repos satisfy C1-C5 OR forbidden-edit tasks lack real ADR ground truth) | Don't fake ADRs. Defer to W8 or declare evidence-failed. |
| Phase 5 Bridge unspecced at W4 entry (no PLAN.md today) | Scope Phase 5 to per-symbol notes + ADR retrieval. Cut Obsidian wiki / shared PG / IDE affordances to V1.1. |
| 04.5-03 absorbs framework sprawl (workspace-weave / DSM / evo / 30-lang) | Enforce W0-W5 plan as written. Demote unfinished parts to Beta V1.1. |

---

## § 5  Private Beta Option (δ)

Repo flips public per release-hygiene rule (MUST 8), but adoption stays
invite-only / link-shared for first 2-4 weeks. No registry / package /
marketing surface until feedback loop closes. Reduces first-impression risk
(CLAUDE.md rule 37). Orthogonal to all 8 MUSTs -- compatible with any
timeline scenario.

---

## § 5.5  Trade-off Ledger: What Curry Gains vs Loses by Choosing This Frame

This section exists because the rigorous-Beta frame has real costs. Naming
them up front prevents post-hoc regret if MUST 7 ships as evidence-failed
or if scope cuts (§ 6) frustrate users. Future Curry / future Codex /
future Claude: this ledger is the contract -- if any line item below
proves wrong, propose an amendment, do not silently override.

### Gains (what Curry gets by adopting this frame)

| # | Gain | Why it matters |
|---|------|---------------|
| G1 | **Falsifiable shipping bar.** 8 MUSTs are testable, not aspirational. | No goalpost drift mid-quarter. Either gates hit or the gates fail loudly. The "evidence-failed Beta V1" exit (MUST 7) is also bounded -- you ship honestly, you don't endlessly delay. |
| G2 | **Eval-evidence-backed claims = real differentiation moat.** | Per find-skills survey: code-review-graph (762 installs) + graphify (226) ship the same tree-sitter+SQLite+MCP pattern. Differentiation cannot come from feature richness; it has to come from "this thing measurably changes agent edit behavior" which only the EVAL-CONTRACT B3 vs B2 gate can prove. |
| G3 | **Decision fatigue cut.** | workspace-weave / 30-lang / IDE plugin / clustering / multi-repo / Obsidian-wiki integration are all answered in advance (deferred to V1.1+). Future Curry doesn't have to re-litigate them when a flash-of-inspiration hits at 2am. |
| G4 | **Schedule legibility (8 weeks +/- 2).** | Solo-dev with multi-machine workload (this + QT quant on other devices). A point estimate with a band tells Curry when to push and when to stop without burning judgment on "should I keep going". |
| G5 | **Drift probe outcome can BUY back schedule.** | Scenario B compresses Beta V1 to W5-W6 if drift evidence demotes 04.5-03 to quality work. That's pure upside conditional on probe results -- a free schedule lever the rigorous frame creates by encoding 04.5-03 as MUST 3 ("lands OR demotes"), not just "lands". |

### Losses (what Curry gives up by adopting this frame)

| # | Loss | Why it matters |
|---|------|---------------|
| L1 | **Scope cuts hurt real users.** | 30-language activation, IDE plugin, Phase 4 broad parity, Phase 04.1 clustering+evolution -- all deferred. Adopters who wanted those features get a "soon" that may take Beta V1.1 (post-W8) or never. Some will close the tab and pick a competitor. |
| L2 | **~8 weeks of focused capacity locked to CodeNexus.** | QT quant + other projects on other devices keep moving but local-machine attention belongs here. If a higher-priority opportunity emerges in the 8-week window, switching costs the Beta date. |
| L3 | **~15 hrs eval authoring is sunk cost if MUST 7 fails.** | Authoring 30 tasks + 3 repo nominations + agent/judge model selection + run-harness implementation is real work even if the eval ultimately ships as evidence-failed. The work is not transferable to a different framing without re-authoring. |
| L4 | **"Evidence-failed Beta V1" path costs trust capital with some audiences.** | Honest framing is right ethically + technically. But OSS adopters skim README first impressions; "shipped without meeting eval gate" reads as failure to many even when the integrity move is shipping it that way. Mitigation: § 5 private-beta hybrid limits blast radius. |
| L5 | **Audit's reframe is load-bearing.** | The whole MUST 5 + MUST 7 architecture rests on Codex's R2 reframe ("CodeNexus = LLM external long-term memory + structured perception layer"). Codex's own premise challenge from earlier today (the 3-arm A/B/C test where C = rg-bundle + hand-maintained NOTES.md baseline) flagged this as testable. If the EVAL-CONTRACT v1.1 amendment proposal lands AND that B1.5 baseline beats B3, the audit's reframe collapses and Beta V1 positioning collapses with it. |

### Synthesis (what the trade actually is)

This frame trades **feature breadth + flexibility** for **evidence quality
+ schedule legibility + decision closure**. The trade is right IF
**(a)** you believe the eval gate is achievable inside the 6-9 week
budget AND **(b)** the load-bearing premise ("memory > poor-man's-memory")
is testable in W6 budget. If either is wrong, you've burned 8 weeks for
a clearer "we don't know" instead of for a successful Beta.

The probe spec at `.planning/probes/drift_evidence_probe.md` (commit
`2582dae`) tests **(a)** indirectly (drift evidence makes the eval
meaningful or proves the substrate doesn't need 04.5-03's full investment).
The EVAL-CONTRACT v1.1 amendment proposal -- not yet written -- would
test **(b)** directly by adding the B1.5 NOTES.md-poor-man's-memory
baseline. **Both should land before MUST 7 commits real eval cycles.**

---

## § 6  Beta V1.1+ Named Backlog

Everything below is explicitly OUT of Beta V1 scope. Future Curry and
future Codex: do not silently re-admit. Each item gets a discuss-phase
+ insert-phase if/when re-prioritized.

- 30-language activation (beyond TS + Python + Go)
- Phase 4 broad parity (security scanners / CodeFlow port / cross-corpus)
- Phase 04.1 clustering + evolution layer (Leiden / Infomap / DF-Leiden / CoDAEN-NeGMA)
- VS Code extension
- Shared-PG memU coupling (currently self-contained store; see Phase 5 ADR-PG decision)
- Multi-repo `workspace-weave` aliasing
- IDE affordances beyond MCP tool surface
- Remote A2A mesh (cross-host agent discovery)
- Obsidian wiki graph integration (defer to obsidian-llm-wiki side)
- Telemetry / analytics / cloud sync surfaces
- Enterprise controls (RBAC / audit logs / SSO)

---

## § 7  Freeze Metadata

- Authored: 2026-05-02
- Basis: Codex strategic analysis (CCG PARTIAL) session
  `019de7d9-63bd-7ef1-8c66-9500116c14a5` + Curry synthesis (alpha drift
  compression / beta parallel decisions / gamma named V1.1+ backlog /
  delta private-beta hybrid) + Curry's trade-off ledger directive
- Commit hash for this freeze: filled at commit time
- Next scheduled review: W4 entry (~2026-05-30), triggered by Phase 5
  Bridge PLAN.md authoring decision (see § 8)
- Amendment discipline: any MUST change requires `docs(beta-v1):` commit
  with explicit rationale + version bump (this is v1.0). Tightenings
  follow same discipline as relaxations -- no silent goalpost moves.

---

## § 8  Immediate Priority: Phase 5 Bridge PLAN.md

> **Phase 5 Bridge PLAN.md is the load-bearing hole that exists TODAY.**
> W1's first task is to close it. Otherwise Scenario D (W10+ hard stop)
> activates at W4 with near-certain probability.

**Why this is the W1 first task, not W4:**

- MUST 5 requires Phase 5 ships memory-assisted edit surface with
  `query_constraints` + `remember_symbol_note` + `get_edit_context`.
- W4 milestone in Scenario A says "Phase 5 Bridge PLAN.md + first slice".
- A PLAN.md takes ~4-8 hrs to write properly with discuss-phase +
  CCG-locked decisions + plan-checker iter 2 = 0/0/0 verification (today's
  04.5-03 plan-phase round took ~6 hrs across two iterations).
- If W1 starts with no PLAN.md authored, the eval-instance authoring at
  W4 has no implementation to evaluate, and the Bridge first slice has
  no scope -- hence W4 entry slips, hence Scenario D triggers.
- Phase 5 Bridge spec is also where decisions like "memU storage key"
  (rowid vs (file, name, kind) vs path-aware fallback) get locked --
  those decisions interact directly with the drift probe results, so
  W1 is the right window to author them.

**Phase 5 Bridge PLAN.md scope (proposed -- finalize at W1 discuss-phase):**

- A2A operations: `query_constraints(file|symbol|topic)`,
  `remember_symbol_note(symbol_id, note, source_session, confidence)`,
  `get_edit_context(symbol_id|file)` -- per audit synthesis lines 60-67
- Storage key policy: (file, name, kind) primary + path-aware fallback
  (per drift probe M5_fnk_with_path_fallback metric)
- ADR extraction harness (markdown headers + MUST/MUST-NOT/SHOULD
  pattern matching)
- Note lifecycle: write / read / list / supersede; NO delete-without-audit
- MCP tool wrapping (the actual public surface)
- Explicit OUT of Phase 5: Obsidian wiki graph, shared PG, IDE
  affordances, remote A2A, clustering -- all live in V1.1+ per § 6

**Acceptance for "Phase 5 PLAN.md is no longer a load-bearing hole":**

- File exists at `.planning/phases/codenexus-05-bridge-memory-mvp/05-PLAN.md`
  (or similar phase-dir convention)
- Discuss-phase ran with at least 1 round of CCG (Codex + Claude
  triangulation; Gemini if infrastructure bug fixed)
- Plan-checker iter 2 = 0/0/0 (matches today's 04.5-03 quality bar)
- Storage key policy makes drift probe M5 metrics actionable as
  Phase 5 acceptance gates

If W1 starts and Phase 5 PLAN.md is not authored within the first
session, the rigorous-Beta frame (§ 0) starts breaking. The audit's
load-bearing reframe (Phase 5 PROMOTED) needs Phase 5 to actually
exist as an executable plan, not a section header.

---

End of spec.
