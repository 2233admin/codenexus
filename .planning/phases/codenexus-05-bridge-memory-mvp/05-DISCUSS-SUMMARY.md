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

## Headline (locked decisions, AMENDED 2026-05-03 per CCG round 2)

Round-2 amendments superseded G2 / G3 / G4 / G5 specifics; original-row
recommendations preserved at the end of this document for audit. Authoritative
table below carries amended decisions inline:

| Gray area | Locked recommendation (amended) | Source |
|-----------|--------------------------------|--------|
| **G1** memU integration mode | **Mode B**: self-contained SQLite + V1.1-ready JSONL export hook | 05-discuss-strategic |
| **G2** query_constraints scope | 3-modality enum (`File`/`Symbol`/`Topic`); search.rs gains **`kind_filter` parameter** (NOT corpus_scope); Topic mode merges symbols_fts (kind='ADR') + dedicated **notes_fts** (BM25-only) via RRF in handler; ~150-220 LOC total (per A-CI-1) | 05-discuss-api § Round-2 Amendment Block |
| **G3** remember_symbol_note schema | Minimal-plus-3 unchanged + **CI-2 (a) unique index on symbols(path,name,kind)** as FK target + supersede-fork prevention via unique index on supersedes_note_id; 2 ops ship (write + list_notes) | 05-discuss-api § Round-2 Amendment Block |
| **G4** get_edit_context format | Composite handler with **internal-fn extraction prerequisite** (handle_*_internal in W3 BEFORE composite) + **`warnings: Vec<String>` partial-failure contract** + Imports edge skip-with-warn (MC-1); ~240 LOC honest, not 80; symbol-only V1.0; single-blob; caller_depth 1..3 | 05-discuss-api § Round-2 Amendment Block |
| **G5** ADR extraction harness | Sources unchanged + RFC 2119 PRIMARY + `## ADR` SECONDARY unchanged; **storage flipped: ADRs are Symbol rows with kind='ADR' + adr_metadata sidecar + symbols.body_text column (W0 ALTER) + reuse symbols_fts** (NOT separate adrs table; CI-4 dissolved); on-demand extract_adrs auto-coupled to index_repo unchanged | 05-discuss-adr § Round-2 Amendment Block |
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

4. **search.rs needs a `kind_filter` parameter** (per A-CI-1 cascade,
   was `corpus_scope`). ~30-50 LOC change to add `kind_filter:
   Option<Vec<String>>` to search() and filter both BM25 + vector results
   to allowed Symbol kinds. Default None preserves existing behavior. Notes
   live OUTSIDE this surface -- they get a dedicated `Store::search_notes_fts`
   accessor (BM25-only via notes_fts, no shared search.rs path). Total notes
   accessor + kind_filter + query_constraints handler ~150-220 LOC per
   05-discuss-api § Round-2 Amendment Block A-CI-1.

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

## Wave breakdown (AMENDED 2026-05-03 per CCG round 2)

Original wave breakdown preserved at end of file. Authoritative breakdown
below carries amendments inline.

- **W0**: Storage layer (HEAVIEST -- absorbs MC-2 migration framework)
  - **NEW: minimal migration framework** (schema_version table + Store::migrate())
    -- MC-2 explicit, this slice INVENTS it
  - **NEW: ALTER TABLE symbols ADD COLUMN body_text** (CI-1 cascade for ADR
    text storage; populated for kind='ADR' rows, NULL for code Symbols)
  - **NEW: rebuild symbols_fts** to include body_text in indexed columns +
    triggers
  - **NEW: CREATE UNIQUE INDEX idx_symbols_fnk ON symbols(path, name, kind)**
    (CI-2; serves as FK target for symbol_notes + identity discipline)
  - notes table = symbol_notes (G3 SQL amended) + FK to symbols(path, name,
    kind) + idx_notes_no_double_supersede unique index for fork prevention
  - **notes_fts** (external-content + triggers, mirrors symbols_fts pattern)
    -- NOT constraints_fts (CI-1 cascade dropped that abstraction)
  - **adr_metadata sidecar table** (one-to-one with kind='ADR' Symbol rows)
    + adr_symbol_links table (V1.1+ populated lazily) -- NOT separate adrs
    table (CI-1 cascade)
  - **MC-1: pre-W0 Imports edges:** add Store::has_imports_edges() helper
    (mirrors 04.5-03 pattern); flag on open if Imports rows present
  - JSONL export hook scaffold (G1 Mode B) unchanged
- **W1**: A2A ops -- write side
  - remember_symbol_note (G3) + supersede unique-index discipline (CI-2 prep)
- **W2**: A2A ops -- read side
  - **search.rs::search gains `kind_filter: Option<Vec<String>>`** (A-CI-1,
    NOT corpus_scope) -- ~30-50 LOC parameter thread
  - **NEW: Store::search_notes_fts(text, top)** accessor -- BM25-only,
    notes_fts surface, ~30-50 LOC
  - query_constraints handler with two-stream RRF merge (~80-120 LOC)
    -- File / Symbol modes are SQL-only; Topic mode merges symbols_fts
    (kind=ADR) + notes_fts results
  - list_notes (G3, exposed per UQ-A3 = 5 ops)
- **W3**: A2A ops -- composite (HEAVIER than original ~80 LOC)
  - **PREREQUISITE: server.rs internal-fn extraction** (CI-3): refactor
    handle_query / handle_get_symbol / handle_list_callers match arms into
    `handle_*_internal()` functions returning typed structs (~120 LOC)
  - get_edit_context composite handler (~80 LOC) calling internals + edges
    in/out + warnings field (CI-3 partial-failure contract)
  - **MC-1: edges_in/out builders skip Imports + push warning** (do not crash)
  - Total: ~240 LOC honest, not 80
- **W4**: ADR extraction harness (CI-1 cascade rewires storage targets)
  - extract_adrs op + tree-sitter-markdown integration (G5 keywords + paragraph
    granularity unchanged)
  - **WRITES TO symbols (kind='ADR') + adr_metadata sidecar** (NOT separate
    adrs table; CI-1 cascade)
  - body_text column populated with paragraph text (W0 ALTER pre-req)
  - Symbol rows get FTS indexing automatically via existing symbols_fts triggers
  - adr_symbol_links populated empty in V1.0 (V1.1 lazy population from
    text-mention heuristic)
- **W5**: MCP tool surface
  - 5 tool descriptions (G6 prose) + first-run agent-affordance docs
  - Description prose largely unchanged by amendments (wire format stable)
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

---

## Round-2 update (2026-05-03 ~13:30 UTC, post-CCG)

**CCG round 2 PARTIAL** (Codex done; Gemini blocked by Windows infra bug
in gemini-plugin-cc transport.mjs:57). Full findings at
`05-CCG-ROUND-2-FINDINGS.md`. Headline: **4 CRITICAL ISSUES** in round-1
discuss decisions surfaced by Codex; 7 PLAN.md files authored in parallel
are now PROVISIONAL pending amendments.

**Critical issues (block plan-phase execution):**
- CI-1: G2 `corpus_scope` LOC underestimated (30 -> 100-200+); search.rs
  is symbol-shaped throughout, true refactor is search-result-type
  abstraction
- CI-2: G3 notes table FK references hypothetical `symbols_fnk` table;
  must choose unique-index-on-symbols-fnk OR new identity-table approach
- CI-3: G4 `get_edit_context` is NOT 80 LOC; needs internal-fn extraction
  prerequisite + locked partial-failure contract
- CI-4: G5 FTS5 terminology error ("contentless" written, "external-content
  with triggers" actually meant)

**Plus 3 missed constraints:**
- MC-1: Imports edges still permitted by CHECK constraint -- W0/W3 must
  handle mixed-schema DBs
- MC-2: NO migration framework exists; W0 invents it
- MC-3: search result type symbol-shaped (ties to CI-1)

**Updated next session entry:**

1a. **Amend 4 discuss decisions per CI-1/2/3/4** (~2-3 hr)
1b. **Amend 7 PLAN.md files to inherit amendments** (~1-2 hr)
2.  **Re-run Codex challenge over amended PLAN.md** (CCG round 3, partial)
3.  **Plan-checker iter to 0/0/0** -- BETA-V1-SPEC sec 8 acceptance gate
4.  **Gemini infra fix** (separate codenexus-tooling sub-slice, NOT Phase 5)
5.  **Phase 5 W0 execution entry** -- storage layer slice with migration
    framework + chosen FK approach

**Acceptance gate status:**
- BETA-V1-SPEC sec 8 line 229-230: PARTIAL ("Gemini if infrastructure bug
  fixed" was conditional; Codex-only meets the spirit but tri-model
  triangulation pending)
- 4 critical amendments not yet landed -- plans NOT execute-ready

---

## Round-3 Amendments LANDED (2026-05-03 ~15:00 UTC, opinionated defaults)

Curry approved option **(X) opinionated defaults** -- batch-amend 4 discuss +
7 PLAN files with Claude's recommended resolutions:

- **CI-1**: chose **(b) Symbol kind='ADR' reuse** (CON-2 cascade)
- **CI-2**: chose **(a) unique index on symbols(path, name, kind)**
- **CI-3**: chose **(b) partial brief + warnings field** + internal-fn
  extraction prerequisite
- **CI-4**: **dissolved** under CI-1=(b) cascade (ADR FTS reuses symbols_fts
  which is already external-content + triggers)

**MC-1 / MC-2 / MC-3** all addressed:
- MC-1: Imports edge skip-with-warn in W0 helpers + W3 EdgeView builder
- MC-2: W0 explicitly INVENTS minimal migration framework (schema_version
  table + Store::migrate())
- MC-3: dissolved under CI-1 cascade (kind_filter parameter, not corpus
  abstraction)

**Files amended (this commit batch):**
- `05-discuss-api.md` § Round-2 Amendment Block (A-CI-1/2/3 + A-MC-1)
- `05-discuss-adr.md` § Round-2 Amendment Block (A-G5-CI-1 cascade,
  CI-4 dissolved)
- `05-DISCUSS-SUMMARY.md` (this file -- locked-decisions table + cross-cutting
  + wave breakdown all amended inline; original preserved at end)
- `05-W0-PLAN.md` § Round-2 Amendment Block (heaviest -- migration framework
  + body_text + adr_metadata + notes_fts)
- `05-W1-PLAN.md` § Round-2 Amendment Block (CI-2 supersede unique index)
- `05-W2-PLAN.md` § Round-2 Amendment Block (kind_filter not corpus_scope +
  notes_fts accessor + RRF merge)
- `05-W3-PLAN.md` § Round-2 Amendment Block (internal-fn extraction +
  warnings + LOC re-sizing + Imports skip)
- `05-W4-PLAN.md` § Round-2 Amendment Block (writes Symbol+adr_metadata not
  adrs table)
- W5 / W6 PLANs unchanged (MCP wire format + eval harness unaffected)
- `05-CCG-ROUND-2-FINDINGS.md` status header updated to AMENDMENTS-LANDED

**Acceptance gate (post-amendment):**
- BETA-V1-SPEC sec 8 line 229-230: STILL PARTIAL on Gemini side (infra bug
  not fixed; deferred to codenexus-tooling sub-slice). Codex amendments
  landed; equivalent of round-3 challenge would require new run, deferred.
- Plan-checker iter 1 can now run against amended PLAN files and is
  expected to converge faster (hidden architectural costs surfaced + locked).

**Next session entry (revised):**
1. (Optional) Re-run Codex over amended PLAN files (CCG round 3) -- HIGH
   value if next session has budget; LOW priority if Curry wants to
   execute directly
2. Plan-checker iter 0/0/0 against amended PLANs
3. Phase 5 W0 execution entry (storage layer with migration framework)
4. Open: Curry MAY want to also confirm ADR Symbol naming convention
   (`{heading_anchor}#{source_line}`) before W4 execution

---

## Original (pre-amendment) headline + wave breakdown -- AUDIT TRAIL

Preserved verbatim for the record. The amended versions above are
authoritative; the originals here are historical only and SHOULD NOT be
used for plan-checker or execution.

### Original locked-decisions table (round 1 + round-2 round-1, pre-amendment)

| Gray area | Original recommendation | Source |
|-----------|------------------------|--------|
| **G2** query_constraints scope | 3-modality enum; reuse search.rs::search via new `corpus_scope` parameter (~30 LOC); ranked relevance x severity | 05-discuss-api round 1 |
| **G3** remember_symbol_note schema | Minimal-plus-3 (no FK target spec) | 05-discuss-api round 1 |
| **G4** get_edit_context format | Composite handler; ~80 LOC; symbol-only V1.0 | 05-discuss-api round 1 |
| **G5** ADR extraction harness | Separate `adrs` + `adr_symbol_links` tables (NOT Symbol kind=ADR); FTS5 contentless mode | 05-discuss-adr round 1 |

### Original wave breakdown (round 1, pre-amendment)

- **W0**: Storage layer -- notes table (G3 SQL), adrs + adr_symbol_links +
  adrs_fts5 tables (G5 SQL), JSONL export hook scaffold (G1 Mode B)
- **W2**: query_constraints (G2) + search.rs::corpus_scope extension; list_notes
- **W3**: get_edit_context (G4 composite handler, ~80 LOC)
- **W4**: extract_adrs op + tree-sitter-markdown (writes separate adrs table)
