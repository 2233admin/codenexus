---
phase: 5
title: "Phase 5: Bridge -- Memory-Assisted Edit Surface (MVP)"
status: PRE-PLAN-NOTES (scaffold for discuss-phase)
authority: BETA-V1-SPEC v1.0 § 8 (frozen 2026-05-02) + drift probe DEMOTE
   (`.planning/probes/runs/2026-05-03-drift-evidence.md` 2026-05-03)
parent_artifacts:
  - .planning/BETA-V1-SPEC.md § 8 (Phase 5 PLAN.md scope, 70% locked)
  - .planning/probes/runs/2026-05-03-drift-evidence.md (M5_fnk = 1.0
    vindicates (path, name, kind) keying)
  - .planning/audits/2026-05-02-codex-strategic-review.md (Phase 5 PROMOTED
    to "memory-assisted agent edit MVP" per audit synthesis lines 60-67)
unblocked_by: drift probe DEMOTE 2026-05-03 (commit d5e5eb0)
critical_path: TRUE (BETA-V1-SPEC § 8 calls this "the load-bearing hole
   that exists TODAY"; W4 Scenario A milestone)
budget_pre_plan: ~4-8 hr (per § 8 line 200-202; matches 04.5-03 prior cycle)
---

# Phase 5 Bridge -- Pre-Plan Notes

## Why this phase exists (one paragraph)

Per BETA-V1-SPEC § 8: Phase 5 ships the memory-assisted edit surface that
makes per-symbol agent annotations persist across sessions. Three A2A
operations land here (`query_constraints`, `remember_symbol_note`,
`get_edit_context`) plus storage key policy + ADR extraction harness +
note lifecycle. This is the W4 milestone in Scenario A and the W1-W2
compressed milestone in Scenario B (drift demote scenario, NOW ACTIVE
per 2026-05-03 probe outcome).

**MUST 5 + MUST 7 of BETA-V1-SPEC depend on this PLAN.md existing as an
executable plan, not a section header.** If W1 starts and PLAN.md is not
authored within the first session, Scenario D (W10+ hard stop) activates
near-certainly.

## Imported scope (from BETA-V1-SPEC § 8, verbatim)

> Phase 5 Bridge PLAN.md scope (proposed -- finalize at W1 discuss-phase):
>
> - A2A operations: `query_constraints(file|symbol|topic)`,
>   `remember_symbol_note(symbol_id, note, source_session, confidence)`,
>   `get_edit_context(symbol_id|file)` -- per audit synthesis lines 60-67
> - Storage key policy: (file, name, kind) primary + path-aware fallback
>   (per drift probe M5_fnk_with_path_fallback metric)
> - ADR extraction harness (markdown headers + MUST/MUST-NOT/SHOULD
>   pattern matching)
> - Note lifecycle: write / read / list / supersede; NO delete-without-audit
> - MCP tool wrapping (the actual public surface)
> - Explicit OUT of Phase 5: Obsidian wiki graph, shared PG, IDE
>   affordances, remote A2A, clustering -- all live in V1.1+ per § 6

## Drift probe outcome anchor (2026-05-03)

Storage key decision is now **evidence-backed**, not hypothetical:
- M5_fnk = 1.0 across 10 pairs both corpora -> (path, name, kind) keyed
  notes survive re-index 100% of the time
- M5_rowid = 1.0 -> rowid is also stable on this binary, but rule should
  prefer (path, name, kind) for cross-binary-version safety (rowid stability
  is not contractual, fnk identity is parser-output canonical)
- M5_fnk_with_path_fallback = 1.0 (vacuously, since fnk already 100%) ->
  fallback is unused on current corpora but spec keeps it for file-rename safety

PLAN.md MUST encode (path, name, kind) primary + path-aware fallback as
the storage key policy. This is a derived requirement from drift probe
M5 metric, not a free design choice.

## Gray areas to resolve in discuss-phase (proposed, not exhaustive)

### G1: memU integration mode -- self-contained vs shared PG vs hybrid
- ADR-PG decision (PROJECT.md line 190, currently "Pending Phase 5")
- Self-contained = Phase 5 owns its SQLite for notes; memU runs separately
- Shared PG = both write into same Postgres for cross-system queries
- Hybrid = self-contained today, shared PG opt-in for power users
- Locked default in BETA-V1-SPEC: "stay self-contained" (REQ-04 inherits)
  but Phase 5 may revisit
- **Decision needed**: confirm self-contained for MVP, document why, defer
  shared PG to V1.1+ explicitly

### G2: query_constraints scope and signature
- Audit synthesis says input = `file|symbol|topic` (3 modalities)
- "topic" is the loosest -- semantic NL search? tag-based? both?
- Output shape: ranked list of constraint texts? structured (severity,
  source, location)? cytoscape-renderable?
- BM25 + vector hybrid? reuse search.rs::search? or new path?

### G3: remember_symbol_note schema -- minimal vs rich
- Minimal: (symbol_id, note_text, source_session, confidence)
- Rich: + tags + supersedes_id + author + created_at + last_accessed
- Lifecycle: write -> read -> list -> supersede; NO delete-without-audit
- Question: is "supersede" a write of new note + flag old as superseded,
  or in-place mutation with append-only history? (audit log discipline)
- Migration path: notes from V1 -> V1.1 if schema evolves

### G4: get_edit_context output format
- "Context for editing symbol X" = ?
- Constraints (from query_constraints) + notes (from list_notes) +
  callers (from existing list_callers) + recent changes (git overlay,
  Phase 4 territory but maybe lift)?
- Single op or composite of existing ops + new aggregation?

### G5: ADR extraction harness scope
- Markdown headers + MUST/MUST-NOT/SHOULD pattern matching is the spec
- Source: project's ARCHITECTURE.md / docs/*.md / planning files only?
  Or arbitrary user-pointed dirs?
- Storage: as constraint nodes in the graph? Separate table? Both
  (graph node + searchable text blob)?
- Re-extraction trigger: on-demand vs scheduled vs file-watch?

### G6: MCP tool surface naming + descriptions
- Per CodeNexus PROJECT.md "Agent behavioral alignment" deliverable:
  MCP tool descriptions must score on retrieval-as-affordance, not just
  "tool exists". CodeCompass paper says agents skip graph tools 58% of
  the time; target <= 5%
- Implication: tool descriptions must include trigger phrasing + worked
  examples + when-NOT-to-call hints
- Question: does Phase 5 PLAN scope this rigorously, or defer to a separate
  "MCP affordance polish" sub-slice?

### G7: V1.0 vs V1.1+ cut line clarity
- BETA-V1-SPEC § 8 explicitly OUTs Obsidian wiki, shared PG, IDE
  affordances, remote A2A, clustering
- But "memU integration" is in/out? Notes-flow into memU's recall layer?
  Or pure self-contained with eventual ADR?
- If memU integration is V1.1+, what's the integration shape (FFI?
  HTTP A2A? shared SQLite mount?)

## Plan structure (proposed wave breakdown -- finalize after discuss)

W0: Storage layer (notes table + ADR extraction table + queries)
W1: A2A op `remember_symbol_note` (write + read + list + supersede)
W2: A2A op `query_constraints` (file/symbol/topic dispatch)
W3: A2A op `get_edit_context` (composite aggregator)
W4: ADR extraction harness (markdown scanner + pattern matcher)
W5: MCP tool surface (3 ops + agent affordance copy + first-run docs)
W6: 30-task eval harness skeleton (per BETA-V1-SPEC W6 milestone, may
    actually live in EVAL-INSTANCES.md not Phase 5 PLAN)

## Open questions for next discuss-phase round

- UQ1: Is W6 (30-task eval) in Phase 5 PLAN scope or separate
  EVAL-INSTANCES.md authoring? (BETA-V1-SPEC implies separate but
  W6 milestone implies bundled)
- UQ2: Does memU CURRENT actually expose an interface that "self-contained"
  Phase 5 storage can talk to (eventual ADR write), or is memU integration
  100% V1.1+?
- UQ3: Should Phase 5 MVP wait for cross-version drift probe (P1
  follow-up from 2026-05-03 SUMMARY) before locking storage key, or is
  M5_fnk = 1.0 evidence enough to commit (path, name, kind) keying now?
- UQ4: Phase 04.5-03 W1-W5 is "demoted to QUALITY IMPROVEMENT" but still
  ships. Does Phase 5 PLAN need to assume edges/alias_decls populated
  (i.e. wait for W3) or can it ship with edges=0 today and add edge-aware
  ops later?
- UQ5: ADR extraction overlaps with `query_constraints` -- is it the
  same thing under two names? Audit synthesis frames them as separate
  but they may collapse.

## Acceptance for "Phase 5 PLAN.md is no longer a load-bearing hole"
(from BETA-V1-SPEC § 8 line 225-233)

- [ ] File exists at `.planning/phases/codenexus-05-bridge-memory-mvp/05-PLAN.md`
- [ ] Discuss-phase ran with at least 1 round of CCG (Codex + Claude
   triangulation; Gemini if infrastructure bug fixed)
- [ ] Plan-checker iter 2 = 0/0/0 (matches today's 04.5-03 quality bar)
- [ ] Storage key policy makes drift probe M5 metrics actionable as
   Phase 5 acceptance gates
- [ ] All 7 G1-G7 gray areas resolved with locked decision + rationale
- [ ] All UQ1-UQ5 open questions either answered or explicitly deferred
   with deferral rationale
- [ ] Honest gap list (rule 18) appended

## What this PRE-PLAN-NOTES file does NOT do

- Does NOT lock G1-G7 (those are discuss-phase decisions)
- Does NOT decide W0-W6 wave granularity (proposed only; finalized after
  discuss + plan-checker pass)
- Does NOT replace BETA-V1-SPEC § 8 -- this is the scaffold that
  consumes § 8 + drift probe outcome and stages the next discuss round
- Does NOT commit to a Phase 5 timeline -- BETA-V1-SPEC W4 (Scenario A)
  vs W1-W2 compressed (Scenario B drift-demote, currently active) are
  both possible; PLAN authoring is what ratifies the scenario lock

## Honest gap list (rule 18 -- this is PRE-PLAN, gaps expected)

**P1**: 7 gray areas (G1-G7) all unresolved. Discuss-phase MUST run
before plan-phase to lock these.

**P2**: 5 open questions (UQ1-UQ5) some of which depend on external
state (memU current API, cross-version drift probe future result).
Some may be deferrable to V1.1+ instead of resolved now.

**P3**: W0-W6 wave breakdown is one-author proposal. CCG triangulation
during plan-phase may collapse / split waves. Treat as starting point
not commitment.
