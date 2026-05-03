---
phase: 5
title: "Phase 5 Bridge -- Discuss-Phase Synthesis (G1-G7 resolution)"
status: DISCUSS-DONE (input to plan-phase pending Curry input on UQ-block-A)
authority: BETA-V1-SPEC sec 8 (acceptance gate "discuss-phase ran with at
   least 1 round of CCG") + 4 parallel gsd-advisor-researcher agents
   (2026-05-03)
synthesis_round: 1 (Claude as synthesizer; Codex/Gemini CCG round NOT YET RUN
   per § 8 acceptance line 229-230)
parent_artifacts:
  - 05-PRE-PLAN-NOTES.md (G1-G7 spec)
  - 05-discuss-strategic.md (G1 + G7, 253 lines)
  - 05-discuss-api.md (G2 + G3 + G4, 350+ lines after Claude self-fill of
    truncated G4 + cross-coupling sections)
  - 05-discuss-adr.md (G5, 290 lines)
  - 05-discuss-mcp.md (G6, ~340 lines, written from agent-returned message)
unblocked_by: drift probe DEMOTE 2026-05-03 (commit d5e5eb0)
blocks: 05-PLAN.md authoring (plan-phase) -- needs Curry decision on
   UQ-block-A items below
---

# Phase 5 Bridge -- Discuss-Phase Synthesis

## Headline (locked decisions)

| Gray area | Locked recommendation | Source |
|-----------|----------------------|--------|
| **G1** memU integration mode | **Mode B**: self-contained SQLite + V1.1-ready JSONL export hook | 05-discuss-strategic |
| **G2** query_constraints scope | 3-modality enum (`File`/`Symbol`/`Topic`); reuse search.rs::search via new `corpus_scope` parameter (~30 LOC); ranked by relevance x severity | 05-discuss-api |
| **G3** remember_symbol_note schema | Minimal-plus-3: (path, name, kind, note, tags, confidence, source_session, supersedes); append-only supersede; rowid input + fnk persist; 2 ops ship (write + list_notes), no read_note(id) | 05-discuss-api |
| **G4** get_edit_context format | Composite handler over G2/G3/get_symbol/list_callers; symbol-only target in V1.0 (file-scope V1.1+); single-blob no pagination; `caller_depth` 1..3 | 05-discuss-api |
| **G5** ADR extraction harness | Default include `docs/**/*.md` + `.planning/*.md` (one-level) + `.planning/phases/**/*-PLAN.md` + `README.md`; RFC 2119 keyword scan PRIMARY + `## ADR` heading SECONDARY; separate `adrs` + `adr_symbol_links` tables (NOT Symbol kind=ADR); on-demand `extract_adrs(scope?)` auto-coupled to `index_repo`; FTS5 contentless mode | 05-discuss-adr |
| **G6** MCP tool surface | 5-criterion description quality bar; production-grade prose for all 3 ops authored verbatim; **V1.0 ships rigorous descriptions, not minimal MVP** (eval gate cannot pass with smelly descriptions); B2/B3/B3-min A/B harness sketched for W6 | 05-discuss-mcp |
| **G7** V1.0 vs V1.1+ cut line | **V1.0 wide on op surface, narrow on integration**; deferred to V1.1+: shared PG, Obsidian wiki graph, IDE affordances, remote A2A, clustering, file-scope get_edit_context, A/B description variants, per-agent-model tuning | 05-discuss-strategic + 05-discuss-mcp |

## Cross-cutting findings (across all 4 agents)

1. **memU upstream `server/` module is declared-but-unimplemented** (Agent 1
   discovered: `memU/pyproject.toml` declares `[project.scripts] memu-server
   = "memu.server.cli:main"` but `D:/projects/memU/src/memu/server/` does
   not exist in HEAD). This collapses Mode C/D options (PyO3 / HTTP-to-non-
   existent-server) for V1.0 and forces Mode B (JSONL export hook). Filing
   memU upstream issue is a Curry-priority decision (UQ-1 below).

2. **A2A op count is actually 4-5, not 3.** PRE-PLAN-NOTES designated 3 ops;
   discuss surfaced:
   - G3 splits into write (remember_symbol_note) + read (list_notes) = 2 ops
   - G5 recommends `extract_adrs(scope?)` as 4th public op (could collapse
     to internal-only if auto-coupled to index_repo, but standalone is
     useful escape hatch)
   - get_edit_context, query_constraints stay singular = 2 ops
   - **Net: 4 ops (5 if list_notes is exposed as standalone)**.
   - Spec amendment needed: BETA-V1-SPEC sec 8 line 213 names 3 ops; should
     update to 4-5 (UQ-2).

3. **Storage key (path, name, kind) keying is consistent across G3/G4/G5.**
   All three use the same fnk identity to anchor data to Symbols. Drift
   probe M5_fnk = 1.0 (commit d5e5eb0) is the load-bearing evidence.
   No conflicts surfaced.

4. **search.rs needs a `corpus_scope` parameter** (G2's recommendation).
   ~30-80 LOC change to thread the parameter through search() -> hybrid
   scoring -> rank fusion. Plan-phase W2 work. No backwards-compatibility
   concern (default = None = current behavior).

5. **CodeCompass (arxiv 2602.20048) + MCP smell paper (2602.14878) are
   load-bearing for G6.** 58% skip rate target -> 5%, 97.1% of 856 MCP
   tools have description smells. Phase 5's deliverable is "agent
   behavioral alignment" per PROJECT.md:106; smelly descriptions in V1.0
   would fail the BETA-V1-SPEC MUST 7 cost gate by conflating tool quality
   with description quality.

## Open questions for Curry (aggregated, prioritized)

### UQ-block-A (BLOCKS plan-phase)

These need Curry input BEFORE 05-PLAN.md authoring:

**UQ-A1** (G1+G7 coupling): JSONL export destination -- CodeNexus-owned dir
(`<repo>/.codenexus/notes-export/`) vs user-config? Recommend former with
`--export-dir` override.

**UQ-A2**: memU upstream relationship -- file GitHub issue requesting
`server/` module ship, OR commit to JSONL ingestion contract as the V1.1+
integration path? Recommend the latter (lower risk, no external
coordination).

**UQ-A3** (G3+G6): Final A2A op count for BETA-V1-SPEC sec 8 line 213
amendment -- 3 / 4 / 5 ops?
   - 3 ops: collapse list_notes inside get_edit_context, hide extract_adrs
   - 4 ops: keep extract_adrs public, hide list_notes
   - 5 ops: expose all (extract_adrs, list_notes, query_constraints,
     remember_symbol_note, get_edit_context)
   - Recommend: 5 ops (maximizes agent affordance per G6 research).

**UQ-A4** (G5 supersede semantics for ADRs): if a doc edit changes a MUST
clause, do we keep history (`adrs` row marked superseded + new row
inserted) or in-place mutation? Recommend history-preserving append-only
(matches G3 notes lifecycle).

**UQ-A5** (G4 file-scope deferral): confirm V1.0 ships symbol-only
get_edit_context, file-scope deferred to V1.1+? Or push file-scope into
V1.0 with the 256KB payload risk?

### UQ-block-B (does NOT block plan-phase, can resolve in W0+)

**UQ-B1** (G3): Min confidence floor on remember_symbol_note write? Recommend
no floor (accept all, rank at retrieval).

**UQ-B2** (G3): NoteView.is_active_leaf default behavior -- active leaves
only or full history? Recommend active leaves default + `?include_history`
opt-in.

**UQ-B3** (G3): Note authorship capture (which agent model wrote the note)?
Recommend defer to V1.1.

**UQ-B4** (G6): Spurious-call penalty in eval -- recommend penalize both
under-call and over-call.

**UQ-B5** (G5): markdown parser dep choice (tree-sitter-markdown vs
pulldown-cmark vs hand-rolled) -- recommend tree-sitter-markdown for
consistency with existing parser stack; plan-phase verifies cargo dep.

**UQ-B6** (G5): excluded-dirs handling -- silent ignore vs warn vs error?
Recommend silent for `.planning/audits/` etc; logger trace if
verbosity=debug.

**UQ-B7** (G2): EdgeView depth in get_edit_context -- depth=1 only for
target symbol or include callers' edges?

**UQ-B8** (G6): "Symbol" vocab in MCP descriptions for non-CodeNexus-native
agents -- self-contained first sentence per description (already done in
G6 prose).

**UQ-B9** (cross-version drift probe timing) -- block V1.0 ratification on
new probe (post-W3 ship of edges/alias_decls) or proceed with M5_fnk=1.0
evidence? Recommend proceed; cross-version probe is P1 follow-up per
2026-05-03 SUMMARY.

### UQ-block-C (V1.1+ scope, defer entirely)

**UQ-C1**: Per-agent-model description tuning (Claude vs GPT-5)
**UQ-C2**: Localized descriptions (Chinese / Japanese)
**UQ-C3**: Dynamic descriptions adapting per repo language
**UQ-C4**: Shared-PG memU coupling
**UQ-C5**: A/B description prose variants (telemetry-driven)
**UQ-C6**: File-watcher / cron extraction triggers (currently on-demand only)

## Plan-phase readiness assessment

**Ready to author 05-PLAN.md** if Curry:
- [ ] Resolves UQ-block-A (5 items, ~30 min decision time)
- [ ] OR delegates UQ-block-A to recommended defaults (single "go with
  recommendations" approval)

**NOT ready to author 05-PLAN.md** if:
- BETA-V1-SPEC sec 8 op count amendment (UQ-A3) is unresolved (W1-W5 wave
  breakdown depends on whether extract_adrs is a public op or internal)
- G1 JSONL export destination (UQ-A1) undecided (W0 storage schema
  depends on whether export hook lives in core/ or in a separate
  bridge sub-crate)

**CCG triangulation NOT yet run.** BETA-V1-SPEC sec 8 acceptance line
229-230 requires "discuss-phase ran with at least 1 round of CCG (Codex +
Claude triangulation; Gemini if infrastructure bug fixed)". Today's round
1 = Claude (synthesizer) over 4 sub-agent outputs. Round 2 = need Codex /
Gemini independent challenge. Defer to next session OR fold into
plan-checker iter 1 round.

## Wave breakdown (proposed, finalize at plan-phase)

Updated from PRE-PLAN-NOTES W0-W6 with discuss findings:

- **W0**: Storage layer
  - notes table (G3 SQL)
  - adrs + adr_symbol_links + adrs_fts5 tables (G5 SQL)
  - JSONL export hook scaffold (G1 Mode B)
- **W1**: A2A ops -- write side
  - remember_symbol_note (G3)
- **W2**: A2A ops -- read side
  - query_constraints (G2) + search.rs::corpus_scope extension
  - list_notes (G3, exposed if UQ-A3 = 5 ops)
- **W3**: A2A ops -- composite
  - get_edit_context (G4 composite handler)
- **W4**: ADR extraction harness
  - extract_adrs op + tree-sitter-markdown integration (G5)
- **W5**: MCP tool surface
  - 5 tool descriptions (G6 prose) + first-run agent-affordance docs
- **W6**: Eval harness skeleton
  - 30-task curated set + B2/B3/B3-min runner (G6 sketch)
  - May actually live in EVAL-INSTANCES.md per BETA-V1-SPEC sec 8 line 215

## Self-check (analysis-triforce)

1. **Precision**: All numbers / file:line citations sourced from sub-agent
   outputs which themselves cite project docs / arxiv papers. Two agent
   outputs were truncated/non-disk: G4 sections of API doc filled by Claude
   synthesizer (clearly marked); G6 file written from agent message
   verbatim with header noting authorship. Both flagged in "honest gap"
   below.

2. **Framework adaptation**: G6's "rigorous descriptions in V1.0"
   recommendation depends on Software 3.0 reframe in PROJECT.md:102
   holding. If reframe collapses (BETA-V1-SPEC sec 5.5 L5 risk), description
   investment becomes nice-to-have not load-bearing.

3. **Feasibility**: 6 waves x ~3-5 days each = ~3-5 weeks. Matches
   BETA-V1-SPEC W5-W6 compressed timeline (Scenario B post-drift-demote).
   Critical path = W0 -> W1 -> W2 -> W3 (storage + 4 ops). W4-W6 can run
   parallel with Phase 04.5-03 W1-W5 (sentrux lift, demoted to QUALITY
   IMPROVEMENT today).

## Honest gap list (rule 18)

**P1**:
- 5 UQ-block-A items unresolved -- need Curry input before plan-phase
- CCG round 2 (Codex + Gemini) not yet run -- BETA-V1-SPEC sec 8 acceptance
  gate. Could fold into plan-checker iter 1 OR run dedicated CCG round.

**P2**:
- 05-discuss-api.md G4 + cross-coupling + open-questions sections (lines
  ~247-end) authored by Claude synthesizer, not by the original advisor
  agent (which truncated mid-G3). Quality may be lower than agent-native
  output; plan-checker should re-verify.
- 05-discuss-mcp.md was written from agent-returned message (advisor role
  contract returns deliverable-as-message rather than disk-write). Content
  is verbatim from agent; only the file-write step was synthesizer-side.
  No content drift.
- Wave breakdown is one-author proposal. CCG triangulation may collapse /
  split waves.

**P3**:
- search.rs `corpus_scope` LOC estimate is eyeball (30-80 range).
  Plan-phase verifies.
- Cross-version drift probe (P1 deferred per 2026-05-03 SUMMARY) is also
  P1 follow-up for Phase 5 V1.0 ratification per UQ-B9; if Curry decides
  to wait for it, Phase 5 ship slips.

## Next session entry

1. **Curry resolves UQ-block-A** (5 items, ~30 min) -- this turn or async
2. **CCG round 2** -- Codex + Gemini independent challenge of this synthesis
3. **/gsd-plan-phase 5** -- author 05-PLAN.md across 6 waves with locked
   decisions baked in
4. **plan-checker iter to 0/0/0** -- BETA-V1-SPEC sec 8 acceptance gate
5. **Phase 5 W0 execution entry** -- storage layer slice
