---
phase: 5
title: "Plan-checker iter 1 -- amended PLANs (post-CCG round 2)"
status: NEEDS-FIX (P0=2 / P1=4 / P2=8)
ran_at: 2026-05-03T~16:30Z
verifier: gsd-plan-checker
parent_artifacts:
  - 05-CCG-ROUND-2-FINDINGS.md (status: AMENDMENTS-LANDED)
  - 05-DISCUSS-SUMMARY.md (round-3 amendment block at end)
  - 05-W{0..6}-PLAN.md (each carries Round-2 Amendment Block)
  - commit 3db99ca (amendments) + 8532da4 (state)
---

# Plan-checker iter 1 result

## Headline

P0=2 / P1=4 / P2=8

The amended PLANs are MUCH closer to executable than pre-amendment. The cross-wave architecture (W0 absorbs CI-1/CI-2/CI-4/MC-1/MC-2; W2/W3/W4 inherit cleanly) is sound. **Dominant defect:** each PLAN's Round-2 Amendment Block lives at the top, but the original `<objective>`, `<plan_time_decisions>`, `<interfaces>`, `<tasks>`, `<acceptance_criteria>`, `<verify>`, `<must_haves>`, `<verification>` sections all remain UNCHANGED below it, still describing the OLD architecture. An executor reading these PLANs cannot mechanically know which side wins for any given grep/test/SQL fragment. The prompt says "treat Amendment Block as authoritative; treat original sections as audit-trail-only" -- the actual PLAN files do not say that to the executor.

## Per-PLAN findings

### W0 (Storage layer -- HEAVIEST)

- **[P0] amendment-vs-original conflict in deliverable surface.** Amendment block (lines 33-204) drops `adrs` table + `adrs_fts` + `constraints_fts` and replaces with Symbol kind='ADR' + adr_metadata sidecar + notes_fts + idx_symbols_fnk + body_text + migration framework. Original `<objective>` (lines 207-243), `<plan_time_decisions>` D-W0-02 (lines 252-257), `<interfaces>` SQL block (lines 320-383), `<tasks>` Task 1 Step B (lines 490-503), `<acceptance_criteria>` (lines 580-600), `<must_haves>.truths` and `.artifacts.contains` (lines 805-844), `<verification>` (lines 846-853) ALL reference the OLD architecture. Executor running `grep -nE 'CREATE TABLE IF NOT EXISTS adrs' storage.rs` will fail. **Fix:** add `> **SUPERSEDED by Round-2 Amendment Block; original retained for audit only**` directive at top of each conflicting subsection, OR rewrite `<tasks>` / `<acceptance_criteria>` / `<must_haves>` / `<verification>` in-place to match amended SQL DDL block (lines 76-155). Recommended: rewrite, because mechanical executors do not safely consume conditional sections.

- **[P0] FK target asymmetry on legacy DBs (MC-2 framework but not test).** Amendment §1 invents `Store::migrate(&mut Connection)` + `schema_version`. Amendment §2 says `CREATE UNIQUE INDEX idx_symbols_fnk ON symbols(path, name, kind)`. Will FAIL on existing fsc.db / poc.db with duplicate `(path, name, kind)` triples (older corpora pre-04.5-03 had file-Symbols with same fnk on overload-style symbols). No acceptance test for "legacy DB with duplicate fnk fails loud-error vs. silently corrupts". W0 will compile-and-test-green on `:memory:` but break on first poc.db re-open. **Fix:** add migration test against fsc.db (or synthetic legacy fixture) -- `Store::migrate()` either succeeds with no dups or returns clear "schema migration failed: fnk uniqueness violated for (path=..., name=...)" error.

- **[P1] amended Rust API additions list (lines 159-173) incomplete.** Amendment lists 7 fns. Missing: `Store::list_notes_for_symbol_active_only` (active-leaf semantics filter), `Store::adr_at_line` (W4 needs this for supersede dance), `Store::count_symbols_kind` (W2 D-W2-04-amended needs this for graceful-degrade flag), `Store::clear_*` helpers (W0 original `<action>` lists these). **Fix:** explicitly enumerate post-amendment storage API surface; add `adr_at_line` + `count_symbols_kind` declarations so W2/W4 can rely on them.

- **[P2] notes_fts BM25-only (Amendment §4) consistent with W2 D-W2-03-amended but contradicts original `<must_haves>.truths` (line 819) which still claims `embedding BLOB` populated on notes/adrs.**

- **[P2] Amendment §6 supersede unique-index discipline good but doesn't add Test 12 in Task 1.** **Fix:** add Test 12 -- insert A, attempt two supersedes of A, verify second fails with constraint error.

- **[P2] doc gap: ON DELETE policy on `adr_metadata.symbol_id REFERENCES symbols(id)`.** SQLite default NO ACTION = silent. **Fix:** add `ON DELETE RESTRICT` explicitly OR document why default is acceptable.

### W1 (remember_symbol_note write)

- **[P1] D-W1-04 (TaskStore.export_dir threading) left to executor.** Amendment block adds W1-specific tests but doesn't lock the `state` threading question. **Fix:** lock now (recommend extend TaskStore with `export_dir: Option<PathBuf>` + `repo_root: PathBuf`, modify `dispatch()` signature) so W1 executor doesn't burn context window deciding.

- **[P2] supersede-fork tests (Amendment lines 53-57) outside `<task>` element.** Executor parsing `<behavior>` block won't see them. **Fix:** add three amendment-block tests to Task 1 `<behavior>` block explicitly.

- **[P2] D-W1-05 max-length 2000 chars: char count not byte count.** Acceptable for V1.0 ASCII-heavy notes; CJK can pass char check + exceed byte budget. Flag for visibility.

### W2 (query_constraints + list_notes)

- **[P0 inherited]** Same amendment-vs-original split as W0. Amendment "Replaces original Output items" (lines 42-89) drops `corpus_scope` + `CorpusScope` enum + `all_constraint_embeddings` + per-row embeddings. Original `<plan_time_decisions>` D-W2-01 / D-W2-02 (lines 175-198), `<interfaces>` block lines 245-275, Task 1 Step A/D/E (lines 360-490), `<acceptance_criteria>` lines 491-499, `<done>` (lines 507-516) ALL reference dropped APIs. Executor will create code amendment forbade. **Fix:** rewrite Task 1 around amended API surface (`kind_filter: Option<Vec<String>>`, `Store::search_notes_fts`, RRF in handler).

- **[P1] kind_filter: Option<Vec<String>> breaking change to all `search::search` call sites.** Adding 7th positional param breaks Query handler at server.rs:113. **Fix:** in rewritten Task 1, explicitly require `grep -nE 'search::search' core/src/` audit + add `None` (or `Some(vec![])`) at all call sites. Verified single call site today.

- **[P1] LOC re-sizing (Amendment lines 72-82) honest but Task 1 `<acceptance_criteria>` doesn't constrain it.** Original Task 1 sized for ~110 LOC; amended ~300-400. **Fix:** rewrite Task 1+Task 2 split. Recommend: Task 1 = `search.rs::search` add `kind_filter` + `Store::search_notes_fts` + tests (~150 LOC); Task 2 = `query_constraints + list_notes` handlers + a2a.rs variants + RRF + tests (~250 LOC).

- **[P2] D-W2-04-amended graceful degradation needs `Store::count_symbols_kind`.** Not in W0 amended API list. **Fix:** add to W0 amended Rust API additions; W2 amendment block test 6 consumes it.

- **[P2] D-W2-02 namespace reuse confusion.** Original was about embedding population (now dropped); Amendment uses D-W2-02 for RRF k=60. **Fix:** rename to D-W2-02b in amended block.

### W3 (get_edit_context composite)

- **[P0 inherited]** Same amendment-vs-original split. Amendment §CI-3a (lines 33-58) extracts 5 `_internal()` fns as W3 prerequisite; §CI-3b (lines 60-74) adds `warnings: Vec<String>` to EditContextBrief. Original Task 1 `<action>` Step B (lines 350-352) says EditContextBrief is new, but original `<interfaces>` lines 270-291 defines it WITHOUT `warnings` field. Original `<acceptance_criteria>` line 426 only verifies struct exists, not warnings field. **Fix:** rewrite `<interfaces>` + `<acceptance_criteria>` to include `warnings` + partial-failure tests from Amendment lines 142-147.

- **[P1] Internal-fn extraction prerequisite NOT broken out as separate task.** Amendment §CI-3a lists 5 `handle_*_internal` to extract; original Task 1 Step A says one sentence covering only 2 (`query_constraints_internal`, `list_notes_internal`). Real scope is ~120 LOC across 5 handlers (server.rs:100-197 inline match arms). **Fix:** split into Task 1a (internal-fn extraction across all 5 handlers; existing tests stay green) + Task 1b (composite handler builds on top). 1a must pass before 1b.

- **[P1] LOC sizing ~480 honest (was 80) -- split into W3a/W3b recommended.** **Fix:** split into W3-Rust (Tasks 1a+1b) + W3-Go (Task 2). W3-Go can run partly in parallel after W3-Rust internal-fn extraction green.

- **[P2] Imports edge handling (MC-1) sample warning text uses `{edge_id}` literal.** Should be `format!("...{}", edge_id)`. **Fix:** add acceptance `grep -F 'skipped Imports edge' server.rs` >= 1 hit + runtime test asserting warning contains edge id.

- **[P2] EdgeView.confidence type consistent (f64) -- flag only because Amendment doesn't restate.**

### W4 (extract_adrs harness)

- **[P0 inherited]** Same amendment-vs-original split. Amendment block (lines 38-152) writes go to `symbols` (kind='ADR') + `adr_metadata`; FTS rides `symbols_fts`. Original `<objective>` lines 156-205 still references `adrs` + `adr_symbol_links`. Original Task 2 Step B (lines 596-723) handler calls `store.insert_adr(...)` (the OLD W0 API, not amended `insert_adr_symbol` + `insert_adr_metadata`). Original `<acceptance_criteria>` line 780, `<honest_gap_list>` line 893 still reference "adrs". **Fix:** rewrite Task 2 handler to call amended two-step transactional insert (Amendment lines 47-69); update acceptance + must_haves to match.

- **[P1] D-W4-amended-03 transactional insert requires `Connection::transaction()`.** Original handler used single-statement `INSERT OR IGNORE`. Amended needs two statements wrapped. **Fix:** in rewritten Task 2 handler use `let tx = self.conn.transaction()?; ... tx.commit()?;`. Add acceptance test: `insert_adr_symbol` succeeds but `insert_adr_metadata` raises -> both rollback (no orphan kind='ADR' Symbol).

- **[P1] D-W4-amended-01 ADR Symbol naming brittle on heading rename.** If `## 9.4 Reranker policy` -> `## 9.4 Reranker policy (revised)`, same paragraph at same line gets NEW Symbol row + adr_metadata, OLD row orphaned (`superseded_by_symbol_id = NULL`). Supersede dance in original Task 2 Step B keys on `(source_path, source_line)` not name. **Fix:** clarify -- when same `(source_path, source_line)` has new doc_version_sha, supersede regardless of heading_anchor change. Confirm `adr_at_line` queries symbols+adr_metadata join returning most recent non-superseded by line, regardless of name change.

- **[P2] tree-sitter-md vs tree-sitter-markdown crate name unresolved (OQ-W4-01).** Pin specific working version at execution time (cargo search at run time).

- **[P2] doc_version_sha shells out to git.** Two paths (git available / fallback to content sha2). Original tests don't cover fallback. **Fix:** add Test 10 -- mask git binary, verify content_sha fallback fires + warning emitted.

### W5 (MCP polish)

- **[P2] amendment genuinely light (lines 25-65).** Only deltas: one-sentence addition to `get_edit_context` description about warnings field; BETA-V1-SPEC line 213 amendment unchanged. No conflict with original sections.

- **[P2] BETA-V1-SPEC amendment (D-W5-06) replaces line 213 with multi-line block.** Verified line 213 is `- A2A operations: query_constraints(file|symbol|topic),`. ASCII-safe replacement.

- **[P2] OQ-W5-01 mcp-go API resolved in favor of WithDescription + raw strings.**

### W6 (eval harness)

- **[P2] amendment genuinely light (lines 24-61).** Only delta: per-task result row gains `warnings_observed`; aggregate `warnings_rate` non-penalizing. No conflict.

- **[P2] D-W6-08: W6 ships SKELETON, not first eval run.** B3 vs B2 gate (MUST 6 cost) is post-V1.0 separate step. Acceptable per honest_gap_list 605-607 but worth Curry visibility.

- **[P2] Third-repo lock (OQ-W6-01) deferred; 10/30 task placeholders.** Harness should default to running 20 tasks + skip placeholders gracefully.

- **[P2] B3-min ablation: Go MCP server `--description-mode` flag.** Correctness asserts experimentally; cannot pre-verify.

## Cross-PLAN findings

- **[P0 across all 7 PLANs] amendment-vs-original conflict is dominant defect.** Each Round-2 Amendment Block correct, but original `<tasks>` / `<acceptance_criteria>` / `<must_haves>` / `<verification>` not edited in-place. Executor processing `<task>` elements + verifying via `<acceptance_criteria>` produces code amendments forbid. **Fix:** for W0/W2/W3/W4 (substantive amendments), in-place rewrite of `<tasks>` / `<acceptance_criteria>` / `<must_haves>` / `<verification>` to match amendment block; original prose moved to `## Audit Trail (pre-amendment, not authoritative)` appendix. W1/W5/W6 amendments light enough -- add one-line "see Amendment Block above for X" pointer at each affected subsection.

- **[P1] depends_on chain still respected.** W0:[]; W1:[W0]; W2:[W0,W1]; W3:[W0,W1,W2]; W4:[W0,W3]; W5:[W3,W4]; W6:[W3,W4,W5]. Wave numbers match max(deps)+1.

- **[P1] cross-wave assumptions about W0 deliverables.** W2 needs notes_fts (covered §4). W3 needs has_imports_edges (covered §5). W4 needs body_text + adr_metadata (covered §2 + §4). W4 also needs `Store::adr_at_line` -- NOT in W0 amended API list. **Fix:** add `adr_at_line` + `count_symbols_kind` to W0 API additions explicitly.

- **[P1] W4 depends_on=[W0, W3] structurally correct but W4 doesn't actually use W3 internal-fn.** W4 Task 2 Step B handler is standalone. W3 dep is for MCP tool stub replacement. **Fix:** confirm dep reason is "W3 ships extract_adrs stub registration, W4 fills body" + update depends_on rationale comment.

- **[P2] LOC budget cumulative ~2500-2700 honest across W0-W6 (vs original ~600).** W0/W3 at high-end (~400-480 LOC each). Acceptable but tight. Consider splitting W3 if executor reports context pressure.

- **[P2] cross-plan data contracts.** No conflicting transforms. JSONL export shape consistent (W1+W4). EditContextBrief consumed only by W3. ConstraintHit consumed only by W3 composite.

- **[P2] CONTEXT.md vocab compliance.** Amended PLANs use Symbol/Edge/EdgeKind/AliasDecl. Avoid-list check: W2 amendment uses "score" in RRF context (acceptable algo term). EdgeView.kind matches EdgeKind variants per CONTEXT.md:24-28. Imports lift to AliasDecl honored via MC-1.

## Goal-backward verdict

**Probably yes**, conditional on three things:

1. **Amendment-vs-original conflict resolved (P0 above).** Without fix, executors build OLD architecture passing original acceptance but failing amended; net result: tools register but storage wrong; B3 eval conflated with broken-backend not skipped-tool.

2. **W5 G6-grade descriptions actually work as advertised.** CodeCompass evidence (58% -> 5% from description quality) is load-bearing. Verbatim prose authored by Claude with Claude reader in mind; effect on Sonnet agent testable in W6, not pre-verifiable.

3. **Cross-session intelligence actually fires in 30-task evals.** Multi-turn or multi-session simulation needed for "prior agent left note that current agent reads" pattern. W6 30-tasks.jsonl includes 5 post-edit annotation tasks but single-session. Cross-session value prop (PROJECT.md:107) is asserted, not measured. Plan-checker P2.

Architecture sound. Amendments correctly addressed CCG round 2 issues without introducing new flaws. 9-tool surface realistic. Migration framework + fnk-FK + body_text approach correct cost trade vs separate adrs table. LOC sizing honest after amendments fits W4-W6 timeline. Principal risk = execution mechanical fidelity (P0), not architectural soundness.

## Recommendation

**NEEDS-FIX-AND-RE-ITER**

2 P0 issues block execution. 4 P1 issues should fix in same revision pass. 8 P2 issues can land same revision OR open as W0-Task-3 / honest-gap-list at executor discretion.

Recommended sequencing:

1. **Rewrite W0/W2/W3/W4 `<tasks>` / `<acceptance_criteria>` / `<must_haves>` / `<verification>` in-place to match Round-2 Amendment Blocks.** Move original prose to `## Audit Trail (pre-amendment)` appendix at end. (~1.5-2 hr per PLAN; 4 PLANs = ~6-8 hr.)

2. **Add missing items per P1:** W0 Rust API additions completeness (`adr_at_line`, `count_symbols_kind`, `clear_*` helpers); W0 legacy-DB migration test fixture; W3 split into Tasks 1a (internal-fn extraction) + 1b (composite); W4 transactional insert + heading-rename supersede policy clarification; W4 `adr_at_line` FK to amended W0.

3. **Re-run plan-checker iter 2.** Expected convergence to 0/0/0 because architectural ambiguity is dominant defect; rewriting in-place removes it. Iter 2 should NOT need CCG round 3 -- amendments themselves sound, just need to land in right textual location.

Total time to executable: ~8-12 hr planner work + ~30 min plan-checker iter 2 = under 2 sessions. Phase 5 W4 milestone (~2026-05-30 per BETA-V1-SPEC § 3) remains achievable.

NOT recommended: NEEDS-CCG-ROUND-3-FIRST. Amendments don't have new architectural defects worth re-litigating; defect is purely structural (amendments alongside, not replacing, original sections).

## Audit trail

Plan-checker iter ran 2026-05-03 ~16:30Z by gsd-plan-checker subagent. All 7 amended PLANs read; cross-referenced with BETA-V1-SPEC sec 8, PROJECT.md:106-108, 05-DISCUSS-SUMMARY.md, 05-CCG-ROUND-2-FINDINGS.md, CONTEXT.md, and live source at experiments/poc-retrieval/core/src/{storage.rs (456 LOC), search.rs (164 LOC), server.rs (361 LOC), a2a.rs (157 LOC)}.
