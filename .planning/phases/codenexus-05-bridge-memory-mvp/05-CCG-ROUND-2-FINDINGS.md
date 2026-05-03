---
phase: 5
title: "CCG Round 2 -- Adversarial Challenge of Discuss-Phase Outputs"
status: AMENDMENTS-LANDED 2026-05-03 ~15:00 UTC (Curry option (X) opinionated defaults applied; CI-1=(b), CI-2=(a), CI-3=(b), CI-4 dissolved; MC-1/MC-2/MC-3 addressed)
authority: BETA-V1-SPEC sec 8 acceptance gate ("discuss-phase ran with at
   least 1 round of CCG; Codex + Claude triangulation; Gemini if
   infrastructure bug fixed")
ran_at: 2026-05-03 ~13:30 UTC (Codex challenge); ~15:00 UTC (Claude amendments per Curry option X)
gate_status: PARTIALLY MET (Codex round complete + amendments landed; Gemini infra-blocked, deferred to codenexus-tooling sub-slice)
parent_artifacts:
  - 05-DISCUSS-SUMMARY.md (round 1 synthesis amended; round-3 amendment block at end)
  - 05-discuss-api.md (§ Round-2 Amendment Block; A-CI-1/2/3 + A-MC-1)
  - 05-discuss-adr.md (§ Round-2 Amendment Block; A-G5-CI-1 cascade, CI-4 dissolved)
  - 05-discuss-{strategic,mcp}.md (unaffected)
  - 7 x 05-W{0..6}-PLAN.md (each carries § Round-2 Amendment Block; status
    updated from PROVISIONAL to AMENDED)
amendment_decisions:
  - CI-1: chose (b) Symbol kind='ADR' reuse (CON-2 cascade)
  - CI-2: chose (a) unique index on symbols(path, name, kind)
  - CI-3: chose (b) partial brief + warnings field + internal-fn extraction
  - CI-4: dissolved under CI-1 cascade (ADR FTS rides symbols_fts)
  - MC-1: Imports edge skip-with-warn in W0 helpers + W3 EdgeView builder
  - MC-2: W0 explicitly invents minimal migration framework
  - MC-3: dissolved under CI-1 cascade
---

# CCG Round 2 Findings

## Executive summary

Codex independently challenged round-1 discuss decisions and surfaced
**4 CRITICAL ISSUES** that block 05-PLAN.md execution as currently drafted.
Gemini delegation failed at the OS layer (Windows `spawn('gemini')` ENOENT
in gemini-plugin-cc transport.mjs:57 -- bare name doesn't resolve `.cmd`
on Windows). Gemini infra fix is NOT this phase's scope; defer to a
codenexus-tooling sub-slice.

The 7 PLAN.md files authored in parallel by gsd-planner are based on the
PRE-CCG-round-2 decisions. They are now **PROVISIONAL**: 4 amendments
must land in discuss artifacts + plans before plan-checker iter 2 can
hit 0/0/0 per BETA-V1-SPEC sec 8 quality bar.

## Codex critical issues (BLOCK plan-phase execution)

### CI-1: G2 `corpus_scope` LOC estimate is materially understated

**Claim being challenged:** "30-80 LOC search.rs extension to thread
`corpus_scope` parameter."

**Codex evidence (file:line):**
- `search::search` is hard-wired to the symbols corpus at all 3 coupling
  points: BM25 uses `store.bm25(...)` [search.rs:29], vector search scans
  `store.all_embeddings()` [search.rs:31-38], result hydration requires
  `store.fetch(id)?` returning `parser::Symbol` [search.rs:65-74].
- Storage has only symbol-specific accessors [storage.rs:348-390].
- G2 says Topic mode must search a NEW constraints corpus -- requires:
  - Result abstraction not always `Symbol` (new `Hit` shape)
  - New constraint embedding accessors
  - New constraint FTS accessor
  - Tests for both corpora
- Honest sizing: **100-200 LOC plus tests, not 30**.

**Required amendment:**
- 05-discuss-api.md G2: re-scope to "search-result-type abstraction +
  per-corpus accessors" not "thread parameter"
- 05-W2-PLAN.md: re-size LOC + acceptance criteria; possibly split into
  W2a (search refactor) + W2b (constraint backend) sub-waves

### CI-2: G3 notes table SQL DDL is internally inconsistent

**Claim being challenged:** "(path, name, kind) primary identity for
notes; symbol_fnk_id INTEGER REFERENCES symbols_fnk(id)."

**Codex evidence:**
- Existing `symbols` has NO unique constraint on `(path, name, kind)`
  [storage.rs:15-24]
- The proposed notes table stores the triple directly, but no actual FK
  can target it unless plan-phase adds a unique index OR a separate
  identity table
- Discuss simultaneously says "fnk persist" AND references hypothetical
  `symbol_fnk_id INTEGER REFERENCES symbols_fnk(id)` -- which does not
  exist.

**Plus transaction concern:**
- Server opens fresh rusqlite connection per operation [server.rs:75-79]
  with no multi-step transaction semantics
- Concurrent supersede of same note can fork "active leaf" without
  explicit transaction discipline

**Required amendment:**
- 05-discuss-api.md G3: choose ONE of (a) add unique index on symbols
  `(path, name, kind)` in W0, OR (b) introduce `symbols_fnk` identity
  table with its own rowid as FK target. Match notes table FK to chosen.
- 05-W0-PLAN.md: include the chosen approach in storage migration.
- Specify supersede-as-transaction semantics; W1 PLAN must call out the
  multi-step write contract.

### CI-3: G4 `get_edit_context` is NOT 80 LOC, hides architectural prerequisite

**Claim being challenged:** "~80 LOC composite handler, zero new backend
code."

**Codex evidence:**
- Current dispatch is synchronous inside one `spawn_blocking` worker
  [server.rs:75-98]
- All operations are simple inline match arms [server.rs:100-197], NOT
  reusable functions
- get_edit_context would need NEW internal functions for: get_symbol,
  caller lookup, constraints, notes, edges -- none extractable today
  without prior refactor
- Failure semantics are all-or-nothing: any error -> `TaskState::Failed`
  [server.rs:79-82]
- Composite edit brief needs plan decision: partial brief with warnings
  vs full failure when sub-calls error. Discuss left this UNANSWERED.

**Required amendment:**
- 05-discuss-api.md G4: lock partial-failure contract (recommend partial
  brief with `warnings: Vec<String>` field for missing sub-calls)
- 05-W3-PLAN.md: add prerequisite step "extract handle_query / handle_get_symbol
  / handle_list_callers into _internal() functions in server.rs" BEFORE
  composite handler is written. This is W2 work bleeding into W3.
- LOC re-sizing: ~80 LOC composite + ~120 LOC internal-fn extraction +
  ~40 LOC test partial-failure cases = ~240 LOC total, not 80.

### CI-4: G5 FTS5 terminology is wrong -- "contentless" vs "external-content"

**Claim being challenged:** "FTS5 contentless mode (`content=adrs`)."

**Codex evidence per SQLite FTS5 docs sec 4.4.3:**
- `content=''` (empty string) = contentless mode
- `content=<table>` = external-content mode (DIFFERENT)
- External-content REQUIRES user-managed triggers for insert/update/
  delete consistency
- Existing symbols FTS already uses external-content + triggers
  [storage.rs:22-28]
- ADR sketch in G5 has NO triggers spec'd

**Required amendment:**
- 05-discuss-adr.md G5: replace "contentless saves disk" claim with
  EITHER (a) external-content + insert/update/supersede triggers, OR
  (b) true contentless with accepted read-back limitations (no SELECT
  on stored text, only matching+rank).
- 05-W0-PLAN.md: include the chosen approach + trigger DDL if (a).

## Codex concerns (challenge but accept if pushed back)

### CON-1: A2A op count 4-5 is scope creep without spec amendment

BETA-V1-SPEC names exactly 3 Phase 5 ops [BETA-V1-SPEC.md:197-199] +
repeated at [:213-215]. ADR extraction is listed as INTERNAL harness,
not a public op. Discuss expanded to 4-5 (list_notes + extract_adrs
public).

**Action:** UQ-A3 (Curry approved 5 ops) is the explicit amendment.
05-W5-PLAN.md correctly includes "BETA-V1-SPEC line 213 amendment" as
a deliverable. Plan-phase work item -- no further block, but verify
W5 captures the spec edit.

### CON-2: G5 separate `adrs` table dismissed `Symbol kind=ADR` reuse too quickly

Discuss compared "cleaner ontology" vs separate table but did NOT
compare against the real refactor cost of corpus-agnostic search.
Codex right that `kind='ADR'` Symbol could reuse search.rs / list_callers
/ HitView / SymbolView entirely.

**Action:** if CI-1 (corpus_scope refactor) lands as 100-200 LOC,
revisit G5 decision. If Symbol kind=ADR reuse saves the corpus_scope
refactor, that's a NET WIN despite ontology fuzz. Defer to plan-checker
iter 1 to model both paths.

### CON-3: `EditContextTarget` wire format JSON shape needs locking

Existing `OperationRequest::Query` has scalar fields [a2a.rs:74-82], not
nested enums. If MCP exposes `target` as string but Rust expects nested
enum, wire JSON diverges from typed API.

**Action:** 05-W3-PLAN.md must include exact JSON example for both wire
shapes; reconcile MCP layer with axum layer.

## Codex validations (Claude got it right)

- (path, name, kind) note identity is supported by drift evidence (M5_fnk
  = 1.0)
- Symbol-only `get_edit_context` for V1.0 is right cut (full-file payload
  multiplication blocks current one-shot JSON)
- ADR extraction must NOT depend on live edge data in V1.0 (edges = 0
  in tested binary)

## Codex missed constraints

### MC-1: edges schema still permits Imports

storage.rs CHECK still allows `Imports` [storage.rs:29-35] despite
CONTEXT.md saying it was lifted to AliasDecl. G4's "EdgeView NOT Imports"
claim is directionally right but plan-phase MUST handle mixed-schema
DBs in the field.

**Action:** 05-W0-PLAN.md or W3-PLAN.md must include "if Imports edges
exist on DB open, log warning + skip them in EdgeView; do NOT crash".

### MC-2: NO migration framework exists

All schema is `CREATE IF NOT EXISTS` in `Store::open` [storage.rs:11-56].
Adding notes/adrs/FTS/triggers IS inventing the migration system.

**Action:** 05-W0-PLAN.md must explicitly call out "this slice INVENTS
migration framework"; consider whether to lift sentrux's migration
discipline (if any) OR roll a minimal one. Either way, NOT a hidden
side-effect.

### MC-3: search result type is symbol-shaped throughout

`Hit` embeds `parser::Symbol` [search.rs:9-17]; query responses expose
symbol-shaped hits [server.rs:114-125]. This IS the hidden structural
reason corpus-scope extension is larger than parameter thread.

**Action:** ties to CI-1 amendment; tracked there.

## Gemini delegation failure

**Status:** FAILED at infrastructure layer.

**Root cause:** `gemini-plugin-cc/gemini/1.1.0/scripts/lib/transport.mjs:57`
calls `spawn(this.geminiPath, args, {stdio:[...]})` with `geminiPath`
defaulting to bare string `'gemini'` and no `shell: true` option. On
Windows, Node's `child_process.spawn` does NOT consult `PATHEXT` and
cannot resolve `gemini.cmd` (npm-installed shim) from bare name `gemini`.

Reproducer:
```
node -e "spawn('gemini', ['--version'])" -> Error: spawn gemini ENOENT (errno -4058)
```

**Installed gemini:** `C:\Users\Administrator\AppData\Roaming\npm\gemini.cmd`
(works directly).

**Fix scope:** Patch `lib/transport.mjs` to either (a) add `shell: true`
on Windows, OR (b) accept `GEMINI_BIN` env var defaulting to
`process.platform === 'win32' ? 'gemini.cmd' : 'gemini'`. OR create
PATH shim.

**This phase's response:** Defer fix to a codenexus-tooling sub-slice
(NOT Phase 5 scope). For this round-2 acceptance gate, Codex-only
challenge meets the BETA-V1-SPEC sec 8 line 229-230 spirit ("Gemini if
infrastructure bug fixed" was an explicit conditional). Document failure
+ proceed.

**Prompt preserved at:** `C:/Users/Administrator/AppData/Local/Temp/gemini_ccg_r2_prompt.txt`
(Phase 5 CCG round 2 brief, ready for re-dispatch when infra fixed).

## Recommendation for plan-phase

**BLOCK 05-PLAN.md execution** on 4 amendments before plan-checker iter:

1. CI-1 amendment: re-scope G2 corpus_scope as search-result-type
   abstraction (100-200 LOC); update W2-PLAN
2. CI-2 amendment: choose unique-index OR symbols_fnk approach for notes
   FK; update G3 + W0-PLAN
3. CI-3 amendment: add internal-fn extraction prerequisite to W3-PLAN +
   lock partial-failure contract
4. CI-4 amendment: choose external-content+triggers OR contentless for
   ADR FTS; update G5 + W0-PLAN

**Plus 1 confirmation:**
- CON-2: have plan-checker iter 1 model both G5 paths (separate adrs
  table vs Symbol kind=ADR) given CI-1's larger refactor cost

**Plus 3 documentation additions:**
- MC-1: handle pre-W0 Imports edges in W0/W3
- MC-2: explicitly call out W0 INVENTS migration framework
- MC-3: links to CI-1

## Status of 7 PLAN.md files

All 7 (05-W0-PLAN.md through 05-W6-PLAN.md) are **PROVISIONAL** until
the 4 amendments above land. They were authored in parallel with this
round-2 challenge and reflect pre-amendment decisions. Each PLAN should
have a top-of-file `## PROVISIONAL` banner pointing here until amendments
are folded in (next session work).

Plan-checker should NOT be run on current 7 PLAN.md files; would produce
high-noise output that re-discovers what Codex already surfaced.

## Honest gap (rule 18)

**P1**: Gemini round NOT run -- BETA-V1-SPEC sec 8 line 229-230 acceptance
gate is partially met. Codex-only is acceptable per the conditional
("Gemini if infrastructure bug fixed") but a true tri-model triangulation
is missing.

**P2**: Codex's adversarial reading was over discuss artifacts + select
source files (search.rs / storage.rs / server.rs / a2a.rs). Did NOT
audit the 7 PLAN.md files directly (they were authored in parallel,
unavailable to Codex at challenge time). Plan-checker iter 1 will be
the first Codex pass over actual PLAN.md content.

**P3**: gsd-planner produced 4126 lines across 7 PLAN.md without
incorporating round-2 findings -- because rounds were parallel. Net
result is double work (amend discuss artifacts + amend 7 PLAN files);
sequential CCG-round-2-then-plan-phase would have saved ~30 min of
synthesis. Acceptable trade for parallelism per Curry's "all in / 继续
都干吧" approval but worth noting for future planning.
