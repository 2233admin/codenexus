# Spike-007c -- Explicit Subject Field: queries.json Schema Bump

**Run date:** 2026-04-27
**Status:** **18.8% precision_at_5 on scored-8 subset (+3.8pp vs spike-007 15% all-10 baseline). C5/C6 correctly excluded from aggregate (Python/unindexed). C7 and C4 resolved to correct TS symbols. No change in PPR outcome for C4/C7/C10.**

## Goal

Eliminate the heuristic subject extraction error mode documented in spike-007.
5/10 subjects were unresolved in spike-007 due to: Python files not indexed (C5, C6),
too-generic noun (C7 "Node"), naming mismatch (C4 "vault_create" not a symbol),
and confirmed negative (C9). This spike adds an explicit `subject` field to each
axis-3 query in queries.json and updates the eval driver to use it.

## queries.json Schema Delta

New fields on axis-3 queries only (A-axis and B-axis untouched):

| Field | Type | Meaning |
|-------|------|---------|
| `subject` | string | Exact symbol name to use as PPR entry point |
| `subject_unindexed` | bool | True = subject known but not in TS POC corpus; eval driver skips from aggregate |
| `subject_relationship` | string | Relationship semantics hint for future edge kinds (C8 only: "after_completes") |
| `subject_rationale` | string | Why this symbol was chosen over the query's literal wording |

## Per-Query Subject Mapping

| ID | Query subject word | Explicit subject | Change | Rationale |
|----|-------------------|-----------------|--------|-----------|
| C1 | assertRealPathInsideVault | `assertRealPathInsideVault` | none | already correct |
| C2 | ObsidianAdapter | `ObsidianAdapter` | none | already correct |
| C3 | FilesystemAdapter | `FilesystemAdapter` | none | already correct |
| C4 | vault_create | `dispatch` | **changed** | vault_create not a symbol; VaultFs.dispatch() handles "vault.create" case string (id=870) |
| C5 | concept_graph | `concept_graph` + `subject_unindexed: true` | **marked unindexed** | Python file, TS-only POC parser |
| C6 | kb_meta | `kb_meta` + `subject_unindexed: true` | **marked unindexed** | Python file, TS-only POC parser |
| C7 | Node | `GraphNode` | **changed** | "Node" not a symbol; GraphNode is the TS interface (id=37, 766) |
| C8 | MemUAdapter | `MemUAdapter` + `subject_relationship: after_completes` | annotated | subject correct; temporal semantics unmodeled (Phase 4) |
| C9 | OAuth2Provider | `OAuth2Provider` | none | negative=true, not in symbols -- confirmed |
| C10 | GitNexusAdapter | `GitNexusAdapter` | none | already correct |

## Precision Results: Before vs After

| ID | spike-007 precision | spike-007c precision | Subject changed | Delta | Notes |
|----|--------------------|--------------------|-----------------|-------|-------|
| C1 | 0.00 | 0.00 | no | 0 | PPR finds immediate callers (resolve); hand-expected transitive (vault_*) |
| C2 | 0.00 | 0.00 | no | 0 | PPR finds interface neighbors; hand-expected method names -- schema gap |
| C3 | 0.50 | 0.50 | no | 0 | partial match; fsAdapter in top-5 |
| C4 | 0.00 | 0.00 | yes (vault_create->dispatch) | 0 | dispatch PPR finds registry.get -- still no hand-expected error symbol |
| C5 | 0.00 | SKIP | unindexed | excluded | Python file; no longer penalizes aggregate |
| C6 | 0.00 | SKIP | unindexed | excluded | Python file; no longer penalizes aggregate |
| C7 | 0.00 | 0.00 | yes (Node->GraphNode) | 0 | GraphNode PPR: zero-score results (PPR ran but no edges from GraphNode) |
| C8 | 0.00 | 0.00 | no | 0 | temporal semantics unmodeled; same result |
| C9 | 1.00 | 1.00 | no | 0 | NEG correctly identified |
| C10 | 0.00 | 0.00 | no | 0 | PPR finds adapter siblings; hand-expected internal callees |

### Aggregate

| Metric | spike-007 | spike-007c |
|--------|-----------|------------|
| n scored | 10 | 8 (C5/C6 excluded) |
| avg precision_at_5 (scored n) | 15.0% | **18.8%** |
| avg precision_at_5 (all-10 denominator) | 15.0% | 15.0% |
| unresolved subjects (heuristic fail) | 5 | 1 (C9 only -- negative, expected) |
| lift vs R3 retrieval (~0%) | +15pp | +18.8pp (scored-8 basis) |

## Top-3 Queries Where Subject Changed

1. **C4 (vault_create -> dispatch)**: Correct diagnosis -- vault_create is not in the TS symbol
   table; the MCP server uses a `dispatch()` method with string-cased `"vault.create"` inside.
   PPR from `dispatch` returns `registry.get` and `resolve` neighbors, which are architecturally
   correct but don't match hand-expected error symbols. Precision unchanged at 0.00.
   Root cause: expected_paths (`error`, `throw`, `PreflightError`, `VaultError`) are
   either string literals or exception names -- neither is a parsed symbol. This is an
   expected_paths annotation gap, not a PPR gap.

2. **C7 (Node -> GraphNode)**: The heuristic in spike-007 failed because "Node" matched nothing
   (too generic). GraphNode IS in the symbol table (id=37, interface in adapters/interface.ts;
   id=766 in core/types.ts). PPR ran but returned zero-score results -- GraphNode has no
   outbound Calls/Implements/Extends edges in the current corpus (it is an interface with no
   callers tracked). Graph edge coverage gap for interface field-readers.

3. **C5/C6 (concept_graph, kb_meta -> subject_unindexed)**: These were scored 0.00 unjustly in
   spike-007 because the Python parser was never in scope. Marking `subject_unindexed: true`
   removes them from the denominator, raising the scored-8 precision from 15% to 18.8% without
   any change to PPR behavior.

## Failure Mode Analysis (updated)

The residual failures after explicit-subject intervention split into 3 categories:

1. **Hand-matcher too strict (C1, C2, C10)** -- PPR finds semantically correct architectural
   neighbors but expected_paths lists different-layer answers. LLM-judge re-eval (spike R8+)
   would likely score these as relevant. Unchanged from spike-007.

2. **Graph schema gaps (C7, C8)** -- C7: no edges from interface nodes (field-reader
   relationships not modeled). C8: temporal/event semantics not modeled (Triggers/Listens
   edge kind). Phase 4+ work.

3. **expected_paths annotation gap (C4)** -- error handler symbols are string literals/exception
   names, not parsed TS symbols. The query intent requires exception-flow analysis beyond
   current AST parser scope.

## Recommendation: Should `subject` land permanently in queries.json schema?

**Yes, unconditionally.** Rationale:

- Heuristic subject extraction is unreliable for natural-language axis-3 queries (spike-007:
  5/10 unresolved). Explicit field drops unresolved count to 1 (C9, a confirmed negative).
- `subject_unindexed: true` gives the eval driver a principled exclude-with-reason path vs
  scoring 0.0 unjustly. This is essential for multi-language expansion (Phase 4).
- The `subject_relationship` annotation (C8) is forward-compatible with future Triggers/Listens
  edge kinds without requiring a schema break.
- Cost: one field per axis-3 query at authoring time. Low maintenance overhead.

The Rust `extract_subject` heuristic should remain as fallback for ad-hoc CLI use (`query-graph
"who calls X"` without a JSON query set), but eval-driven runs should always supply explicit
subject.

## Provenance

- queries.json: added `subject`, `subject_unindexed`, `subject_relationship`, `subject_rationale`
  to C1-C10 (axis-1 and axis-2 untouched)
- eval driver: `eval/graph_axis3_eval.py` -- `run_query_graph()` accepts `subject` kwarg
  (passes `--subject` to CLI); main loop skips `subject_unindexed=true` with `precision=None`;
  aggregate excludes None entries
- raw output: `eval/round_7c_explicit_subject.json`
- log: `eval/r7c_explicit_subject_run.log`
- DB verified: sqlite3 symbol lookup for C4 (dispatch), C7 (GraphNode), C5/C6 (NOT FOUND)
- Rust source: NOT modified (extract_subject heuristic preserved as fallback)
- spike-007-graph-axis3.md: NOT modified (original report preserved)
