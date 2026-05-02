# Codex Strategic Adversarial Audit -- 2026-05-02 (R1 + R2)

**Status:** Archived for next-session decision. Not yet acted upon. PROJECT.md / ROADMAP.md / STATE.md unmodified.
**Reviewer:** Codex (gpt-5.5 via codex CLI, adversarial mode, reasoning effort actually low though high requested)
**Operator:** Curry (via Claude Code)
**Trigger:** `/mine:codex adversarial -- zoom-out CodeNexus` after 04.5-03 PRE-PLAN session closed (commits 4ebffa9 / a0fa4d8 / f303d64 / 81e81d7).
**Raw transcripts:**
  - R1: [`2026-05-02-codex-r1-raw.txt`](2026-05-02-codex-r1-raw.txt) (1935 lines, ~175 KB)
  - R2: [`2026-05-02-codex-r2-raw.txt`](2026-05-02-codex-r2-raw.txt) (2880 lines, ~225 KB)

This audit is the synthesized output. Use it to drive next-session decisions; consult raw transcripts only if you need to reconstruct reasoning chains.

## Process

R1 = strategic zoom-out at the artifact level (PROJECT.md, ROADMAP.md, STATE.md, ARCHITECTURE.md, CONTEXT.md, 04.5-03-PRE-PLAN-NOTES.md). 10 attacks + Single Biggest Risk + Actionable Fix.

R2 = pushback on R1's strongest internal contradictions and weakest framings. Codex was forced to defend, update, or concede each push. Result: **2 CONCEDE / 4 UPDATE / 0 DEFEND.** Codex moved on every push.

## Verdict evolution table

| # | Decision | R1 verdict | R2 verdict (after pushback) |
|---|----------|------------|------------------------------|
| 1 | Core value: 67.9% top-5 on spike-001 7 queries | RIGHT FOR WRONG REASON | (not pushed back; verdict holds) |
| 2 | Graded LLM-judge eval as moat | RIGHT BUT FRAGILE | (not pushed back; verdict holds) |
| 3 | Single fat-binary identity | RIGHT BUT FRAGILE | (not pushed back; verdict holds) |
| 4 | A2A endpoint as differentiator | RIGHT FOR WRONG REASON | **CONCEDE** -> "BAD FRAMING OF RIGHT ARCHITECTURE" (architecture matches Codex's prescription; prose oversells) |
| 5 | Software 3.0 strategic bets | RIGHT BUT MIS-SEQUENCED | (not pushed back; verdict holds) |
| 6 | Sentrux lift strategy | WRONG (cut to 600 LoC native) | **CONCEDE** -> "RIGHT BUT HIGH-RISK, NOT WRONG" (600 LoC framing was misleading; ~2300 LoC native estimate was directionally correct; lift justified IF aggressively bounded) |
| 7 | Phase 04.5 metrics/DSM/rules expansion | WRONG (cut or defer) | **UPDATE** -> "WRONG AS BROAD PRODUCT SCOPE, RIGHT AS THIN INFRASTRUCTURE" (keep minimal Snapshot + arch metrics for ADR substrate; DSM/evo/Leiden/Infomap/rules-DSL-expansion still cut) |
| 8 | Phase 5 Bridge sequencing | WRONG (move before parity) | **UPDATE** -> "WRONG IF Bridge WAITS FOR FULL PARITY; RIGHT THAT 04.5-03 IS A PRECONDITION" (Codex collapsed two boundaries; 04.5-03 must precede memU bridge) |
| 9 | Solo-dev capacity / doom march | WRONG | **UPDATE** -> "WRONG UNDER ADOPTION METRIC; FRAGILE UNDER PERSONAL-RESEARCH METRIC" (PROJECT.md does claim solo-dev/small-team adoption, so the warning still applies under that metric) |
| 10 | Competitive positioning | RIGHT FOR WRONG REASON | (not pushed back; verdict holds; best positioning is "agent risk-reduction memory layer" not "open code search") |

R2 also produced a falsifiable concrete prescription where R1's prescription was hand-wavy.

## Revised Single Biggest Risk + Actionable Fix (R2 final)

**Risk:** Persisting sophisticated memory and decision intelligence on top of unstable graph semantics, then spending months expanding generic analysis surfaces before proving agent edit outcomes improve.

**Fix:** Finish 04.5-03 first, keep only the thin metrics substrate needed for ADR/risk scoping, then pull forward a memory-assisted agent edit MVP with a frozen eval. Defer broad DSM/evo/rules/clustering/security/pattern parity until that eval proves CodeNexus changes agent behavior, not just graph richness.

## Codex's 6-week falsifiable cadence

```
Week 1 -- 04.5-03 land (graph_build.rs split, alias_decls table, CallResolver
          native, T1-T7 still green). NO memory writes before stable symbol
          identity.

Week 2 -- Minimal arch metrics (blast radius, cycle/reachability, StoreSnapshot
          projection, constraint attachment to symbol/file/module scope).
          Internal API only. NO product surface yet.

Week 4 -- ADR extraction from markdown + query_constraints op + MCP tool
          wrapping it. Frozen eval v1 created BEFORE any tuning lands.

Week 6 -- Symbol memory store + get_edit_context op + eval run against all
          baselines + release note (including failures honestly).
```

### MVP scope (IN)

- Stable symbol identity post-04.5-03 (alias_decls, CallResolver, ResolutionMethod)
- `query_constraints(file|symbol|topic)` returns ADR/MUST/MUST-NOT/SHOULD scoped to symbol/file/module
- `remember_symbol_note(symbol_id, note, source_session, confidence)` for agent-discovered intelligence
- `get_edit_context(symbol_id|file)` returns callers + callees + blast radius + relevant ADR constraints + prior memory notes
- MCP tools wrapping the above (MCP is the actual agent interface)
- Minimal Obsidian / vault import: markdown decision extraction only

### MVP scope (OUT, even if previously planned)

- Broad DSM / evo / clustering product surfaces (defer / cut)
- Rules DSL unless adapted directly to ADR-derived constraints
- Security scanners
- Pattern detection
- Multi-repo registry unless eval needs it
- 30-language ambition (3 is enough: TS / Python / Go)
- Polished three-way cytoscape viz

### Eval set (concrete)

- 30 frozen edit-prep tasks across 3 repos: 10 bugfix / 10 refactor / 5 API behavior change / 5 "do not edit because ADR forbids"
- Each task: expected touched files + required inspected symbols + forbidden bad edits
- 4 baselines: no tool / `rg` + manual reads / current CodeNexus query+list_callers / CodeNexus memory-assisted MVP
- 6 metrics: required-symbol inspection recall / forbidden-edit rate / ADR constraint recall / wrong-file edit rate / task completion (tests or rubric) / tool-use latency budget
- Success gate: >=25% relative reduction in forbidden/wrong-file edits + >=20pp improvement in required-symbol recall + no >2x median task time

## What this means for the current locked plan (04.5-03 PRE-PLAN-NOTES.md)

**04.5-03 work is JUSTIFIED, with caveat.** R2 confirmed:
- 04.5-03 graph split + alias_decls + native CallResolver = precondition for any durable memU bridge (R2 Push 2).
- Lift scope (~2200 verbatim + ~400 cherry-pick + ~300 native) is the right size, NOT a "WRONG" decision (R2 Push 3 CONCEDE).
- Caveat: lift must be aggressively bounded; do not metastasize into Sentrux-as-product. The currently-locked middle path is at the edge of acceptable.

**04.5-04 (DSM + evo wiring) status changes from queued to RECONSIDERED.** R2 says minimal Snapshot + arch traversal + cycle/blast IS needed for ADR moat -- but DSM and evo are NOT prerequisites for it. Plan 04.5-04 should be re-spec'd at the boundary of "thin substrate for ADR" vs "broad metrics product."

**04.5-05 (Rules DSL decision a/b/c) status changes to LIKELY-DEFER.** R2 says rules DSL expansion is "cut as roadmap mass" unless directly adapted to ADR-derived constraints. Decision (c) -- "ADR is input, rules engine is the runtime" -- aligns with R2's prescription. Decisions (a) lift-as-is or (b) skip-entirely both lose under R2 framing.

**04.5-06 (NOTICE + license audit) unchanged.** Mechanical, low cost, no roadmap implication.

**04.5-07 (Multi-language framework activation) status DOWNGRADES.** R2 explicitly says "30-language ambition" is OUT. Activate TS+Python+Go via plugin.toml, not 30 languages. Cut sentrux's plugin/* breadth from 30+ to 3 maintained configs.

**Phase 4 (broader Parity) and Phase 04.1 (Graph Clustering and Evolution Layer) status DEFERRED.** R2's 6-week cadence skips them entirely in favor of memory-assisted agent edit MVP.

**Phase 5 (Bridge to obsidian-llm-wiki / memU) PROMOTED to next-after-04.5-03.** Specifically as "memory-assisted agent edit MVP" with the falsifiable eval gate, not as "three-way viz integration."

**PROJECT.md "Strategic" section moves from Phase 4+ aspirational to Week 4-6 concrete.** The agent risk-reduction memory layer becomes the actual product, not the long-tail.

**PROJECT.md A2A framing needs surgery.** Architecture stays; prose stops calling A2A "half the core value." MCP becomes the public face per actual REQ-07 implementation.

## Decisions to make next session (NOT acted on this session)

1. **Accept R2's revised roadmap or partial?** Three concrete options:
   - (A) Full pivot: rewrite ROADMAP.md / PROJECT.md to R2's 6-week cadence; cut Phase 4 / 04.1 / 04.5-04 / 04.5-05 / 04.5-07 (30-lang ambition) from the active plan; commit roadmap revision before any new code.
   - (B) Partial: keep 04.5-03 as currently planned (it's already aligned); rewrite Phase 4+ section per R2 prescription; defer 04.5-04+ until eval proves edit-outcome lift.
   - (C) Reject: stay on current ROADMAP; treat R2 as advisory; revisit at Phase 4 entry.
   The "incremental" pick is (B). The "decisive" pick is (A).

2. ~~**Eval methodology overhaul -- when?**~~ **LOCKED 2026-05-02 -- gamma position: pre-register CONTRACT now, populate DATA per Codex Week 4-6.** Decision walked through three positions:

   - alpha (full front-load contract + 30 tasks pre-04.5-03): rejected -- 30-task authoring is SPEC-slice scale, parallel-with-04.5-03 = direct doom-march hit; current-CodeNexus baseline measured against known-broken resolver (T3/T4 pinned bugs) puts "improvement" on noise floor.
   - beta (full back-load per Codex Week 4): rejected -- "eval written after build" is exactly the endogenous-eval failure mode R1 #2 named. R2 itself said "Frozen eval v1 created BEFORE any tuning lands" -- that discipline applies pre-04.5-03 too, not just pre-MVP.
   - gamma (split): pre-register CONTRACT Week 0, populate DATA Week 4-6. Selected.

   Rationale: R1 #2 keyword is "preregistered scoring rubric" -- preregistration isolates motivation, not time order. Contract authoring is ~1-2hr (pure doc, no code), runs serial-not-parallel with 04.5-03 land. Net effect: 04.5-03 G-D gate quality upgrades from "REQ-10 +/- 2pp band" (the narrow endogenous eval R1 #2 critiqued) to "REQ-10 + contract-locked baseline".

   Contract scope to write next-session opening (~1-2hr, then close before 04.5-03 work resumes):

   - Task taxonomy: 10 bugfix / 10 refactor / 5 API behavior change / 5 forbidden-edit-because-ADR (numbers + categories locked, specific task instances NOT yet)
   - 4 baselines (locked names): no-tool / `rg` + manual reads / current-CodeNexus query+list_callers / CodeNexus-MVP-with-memory
   - 6 metrics (locked definitions): required-symbol-inspection recall / forbidden-edit rate / ADR constraint recall / wrong-file edit rate / task completion (tests-or-rubric) / tool-use latency budget
   - Success gate (locked numbers): >=25% reduction in forbidden/wrong-file edits AND >=20pp improvement in required-symbol recall AND <=2x median task time
   - Repo selection criteria (locked, but specific repos NOT yet): >=1 non-author primary repo (anti self-eval-bias); size band; multi-language coverage
   - Judge-method lock (LLM-judge vs human grader -- to be decided in contract; do not punt past contract)
   - Meta-rule: contract changes after lock require changelog entry + rationale; relaxation never silent

   Three known-unsolved-by-contract risks acknowledged (revisit at instance authoring):
   - Self-eval bias if all 3 repos are author's
   - Forbidden-edit task ground-truth needs ADR set, which needs 04.5-03 + minimal arch metrics first
   - Judge-method (LLM-as-judge) cost vs reliability tradeoff at scale

   File path target: `.planning/EVAL-CONTRACT.md` at repo root of `.planning/` for discoverability (sibling to PROJECT.md / ROADMAP.md / STATE.md). Format: markdown with frontmatter `frozen_at: <date>`, sections matching the locked-fields above, explicit changelog table at bottom.

   Coexistence note: spike-001 B1-B7 retrieval eval and FSC F1-F10 hand-eval continue as retrieval-layer regression guards. EVAL-CONTRACT.md governs agent-outcome eval (different beast). Both run; one does not replace the other.

   Status: position locked this session 2026-05-02; contract authoring deferred to next-session opening before 04.5-03 work resumes. ~1-2hr budgeted.

3. **A2A framing surgery in PROJECT.md -- do now or wait?** Two-paragraph edit, low cost. Could land in next session opening as a clarity-fix not a strategic decision.

4. **30-language ambition cut -- impact on 04.5-07 plan.** If R2 is accepted, 04.5-07 spec collapses from "30+ languages via plugin.toml" to "TS+Python+Go via plugin.toml; 27+ other configs stay in sentrux source tree as available-but-unmaintained."

5. **Frozen eval task set -- who/when authors it?** R2 says 30 tasks across 3 repos with expected behaviors. This is real authoring work, comparable to the SPEC slices. Not free.

## Honest framing notes

- Codex used model `gpt-5.5` with reasoning effort actually `low` despite `-c reasoningEffort=high` flag. Codex CLI either ignored or downgraded. Conclusions may have additional depth available at higher effort -- worth a third round only if R2 itself becomes contested.
- R2 conceded twice (#3, #4) and updated four times (#1, #2, #5, #6 of pushes -- which mapped to original verdicts #6, #8, #9, #4+#7+#5). Zero DEFEND outcomes. This is unusually clean for an adversarial second round; either R1 was sloppy on those points (the more likely read) or R2's pushes were genuinely strong.
- Curry's role this session: drove the deepening, made the locked decisions on 04.5-03, then asked for adversarial pushback on his own work. The pattern of asking Codex to attack the work YOU JUST DID is a high-leverage discipline. R1 was correct that the locked decisions had problems; R2 was correct that the framing of the problems was sloppy.
- Open-source first-impression risk (CLAUDE.md feedback rule 37) is implicitly reinforced by R2 Push 5: PROJECT.md's open-source/small-team framing means adoption pressure is real, doom march warning applies. The locked Phase 4 first-run UX P1 cluster was the right call by that rule's lights.

## Provenance

- R1 invocation: 2026-05-02 ~12:10 UTC; codex CLI v0.128.0; D:/projects/codenexus workdir; adversarial mode prepended
- R2 invocation: 2026-05-02 ~12:35 UTC; same CLI/workdir; adversarial mode prepended
- Both runs prompts persisted at C:/Users/Administrator/AppData/Local/Temp/codex_adv_zoomout.txt + codex_adv_r2.txt (ephemeral; raw transcripts in `.planning/audits/` are canonical)
- Curry confirmed "3+4" choice (re-challenge + archive) at 2026-05-02 12:30 UTC

End of audit.
