---
phase: 5
slice: 05-W5
plan_id: 05-W5
title: "W5: MCP tool surface polish -- 5 production-grade descriptions per G6 + first-run agent affordance docs + BETA-V1-SPEC line 213 amendment"
wave: 5
depends_on: [05-W3, 05-W4]
status: PLAN-AUTHORED (awaits plan-checker iter)
files_modified:
 - server/internal/mcpsrv/server.go
 - docs/MCP-AFFORDANCE.md
 - .planning/BETA-V1-SPEC.md
locked_decisions_honored:
 - G6  # 5-criterion description quality bar; production-grade prose for ALL 5 ops; V1.0 ships rigorous descriptions, NOT minimal MVP
 - UQ-A3  # 5 public A2A ops surfaced through MCP (extract_adrs, list_notes, query_constraints, remember_symbol_note, get_edit_context)
 - UQ-B8  # vocab self-contained for non-CodeNexus-native agents (first sentence defines Symbol)
gates:
 - G-A  # build clean (Go); descriptions render correctly via mcp client tool list
 - G-B  # all 5 descriptions meet G6 5-criterion quality bar (verified by checklist)
 - G-C  # first-run agent affordance docs ship at docs/MCP-AFFORDANCE.md
 - G-D  # BETA-V1-SPEC section 8 line 213 amendment shipped (3 ops -> 5 ops named explicitly)
---

> **!! AMENDED 2026-05-03 per CCG round 2 (light) !!** W5 amendments are
> minimal because MCP descriptions are wire-protocol-facing -- they describe
> the tool surface, not the backend. The only round-2 cascade affecting W5
> is the new `warnings: Vec<String>` field in `EditContextBrief` (per W3 §
> Round-2 Amendment Block CI-3b) -- the `get_edit_context` description text
> should mention this field briefly so agents know partial briefs are
> possible. All other description prose unchanged. See
> `05-DISCUSS-SUMMARY.md § Round-3 Amendments LANDED`.

## Round-2 Amendment Block (W5 -- minimal; warnings field surface)

### get_edit_context description prose addition

The G6-locked description for `get_edit_context` (in 05-discuss-mcp.md § 4)
should be augmented with one sentence about the `warnings` field:

> ... (existing prose) ... Returns `{symbol, callers, constraints, notes,
> edges_in, edges_out, warnings}` as a single JSON object. **The `warnings`
> field is non-empty when sub-calls returned partial data** (e.g., callers
> timeout, legacy Imports edges skipped) -- inspect it before relying on
> a missing or empty list. Do NOT call this as a generic ...

W5 executor adds the bolded sentence to the existing prose. Idiomatic
phrasing acceptable variation; substance is "warnings field exists, check
it".

### Other ops -- unchanged

- query_constraints: backend swap (kind_filter + notes_fts) is invisible
  to MCP description (still returns ranked constraint hits)
- remember_symbol_note: supersede semantics unchanged at API surface
- list_notes: unchanged
- extract_adrs: storage shift (Symbol kind='ADR' + sidecar) invisible to
  MCP (still returns extraction stats)

### BETA-V1-SPEC line 213 amendment (still authoritative)

Original W5 D-W5 deliverable still applies: amend BETA-V1-SPEC sec 8 line 213
from "3 ops" to "5 ops named explicitly (query_constraints,
remember_symbol_note, list_notes, get_edit_context, extract_adrs)" per
UQ-A3. No change.

---


<objective>
Replace the W3 first-cut 1-sentence MCP descriptions with G6
production-grade prose (~200-300 words each, ~1 page per tool, ~5 pages
total). Per G6 section 1: agents skip graph tools 58% of the time when
descriptions are smelly (CodeCompass arxiv 2602.20048); 97.1% of MCP
tools have at least one description smell (arxiv 2602.14878). Phase 5
deliverable per PROJECT.md:106 is agent behavioral alignment. The
B3-vs-B2 eval gate (W6 + EVAL-CONTRACT v1.0) cannot pass with smelly
descriptions because the eval would conflate tool quality with
description quality.

W5 ships exactly the prose authored verbatim in 05-discuss-mcp.md
sections 2 / 3 / 4 (query_constraints, remember_symbol_note,
get_edit_context). For list_notes and extract_adrs, W5 authors
descriptions at the same quality bar following G6 section 1's 5 criteria:
1. Trigger phrasing precedes mechanics
2. Worked example with realistic input + output (JSON pair)
3. Explicit when-NOT-to-call hint
4. User-visible value, not capability brag
5. Vocab consistency with CONTEXT.md (Symbol, Edge, EdgeKind,
  AliasDecl, Confidence)

W5 also ships `docs/MCP-AFFORDANCE.md` -- a one-page agent-facing doc
explaining when to reach for which of the 5 new tools, how they
compose, and the key insight (cross-session intelligence) per
PROJECT.md:107.

W5 amends BETA-V1-SPEC section 8 line 213 from "3 ops" to explicit naming of
all 5 ops per UQ-A3 and SUMMARY's recommendation that this is a
Phase 5 deliverable.

Out of scope: A/B prose iteration via telemetry (V1.1+ per G6 section 5);
per-agent-model tuning (V1.1+); localized descriptions (V1.1+);
EVAL-INSTANCES.md authoring (W6 -- separate plan).

Output:
- `server/internal/mcpsrv/server.go`: replace W3 first-cut
 descriptions for all 5 new tools with G6-grade prose. Each
 description is multi-paragraph and uses `mcp.WithDescription` (or
 whatever the long-form description API is in mcp-go; verify).
- `docs/MCP-AFFORDANCE.md` (NEW): one-page agent affordance guide.
- `.planning/BETA-V1-SPEC.md`: line ~213 amendment naming 5 ops.
</objective>

<plan_time_decisions>
- **D-W5-01 (description verbatim source):** For query_constraints,
 remember_symbol_note, get_edit_context: copy the description text
 block verbatim from 05-discuss-mcp.md sections 2, 3, 4 (the prose
 in `> ...` quote blocks). Do NOT paraphrase. The prose was
 authored to G6's 5-criterion bar by the discuss-advisor; W5's job
 is mechanical insertion + light formatting for Go string literals.
- **D-W5-02 (list_notes + extract_adrs prose):** Author at execution
 time following G6 section 1 5-criterion bar. Use the same structural
 template as the 3 verbatim ones (see Step B template).
- **D-W5-03 (Go string literal mechanics):** Use Go raw strings
 (backtick-delimited) for multi-paragraph descriptions to avoid
 escape sequence noise. mcp-go library should accept raw strings as
 arguments to mcp.WithDescription; verify.
- **D-W5-04 (worked example placement):** Per G6 section 1 criterion 2,
 worked example is part of the description text. Embed JSON pairs
 as Markdown code blocks (\`\`\`json) inside the description string.
 Some MCP clients render Markdown in tool descriptions; some don't.
 Per G6 line 39: "anchors the tool in the agent's pattern-matching"
 -- LLM agents process the JSON regardless of rendering.
- **D-W5-05 (MCP-AFFORDANCE.md scope):** ONE PAGE (~80-150 lines).
 Sections: (1) When to call each of the 5 new tools (5 paragraphs);
 (2) How they compose (typical session flow: query -> get_edit_context
 -> [edit] -> remember_symbol_note); (3) Cross-session intelligence
 insight per PROJECT.md:107; (4) When NOT to call any of the 5 new
 tools (avoid for: pure exploration, scratch notes, generic
 warmup). Audience = agent + human onboarding to CodeNexus MCP.
- **D-W5-06 (BETA-V1-SPEC amendment):** Edit line 213 of
 .planning/BETA-V1-SPEC.md (current text:
 "A2A operations: query_constraints(file|symbol|topic),
 remember_symbol_note(symbol_id, note, source_session, confidence),
 get_edit_context(symbol_id|file)") to:
 ```
 A2A operations (5 public ops, finalized 2026-05-03 Phase 5 discuss):
  - query_constraints(scope: file|symbol|topic, target)
  - remember_symbol_note(symbol_id, note, source_session, confidence,
   tags?, supersedes_note_id?)
  - list_notes(symbol_id, include_history?)
  - get_edit_context(symbol_id, caller_depth?) -- file-scope deferred to V1.1
  - extract_adrs(scope?, dry_run?)
 ```
 Add commit message: `docs(beta-v1): finalize Phase 5 op count (3 -> 5) per UQ-A3`. Per BETA-V1-SPEC line 183-185 amendment discipline.
</plan_time_decisions>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-mcp.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W3-PLAN.md
@.planning/phases/codenexus-05-bridge-memory-mvp/05-W4-PLAN.md
@.planning/BETA-V1-SPEC.md
@CONTEXT.md
@server/internal/mcpsrv/server.go

<interfaces>
<!-- W3 stub descriptions in server.go -->
```go
// query_constraints (W3 first-cut):
mcp.NewTool("query_constraints",
  mcp.WithDescription("Returns ranked constraint clauses (MUST/MUST-NOT/SHOULD) extracted from project ADRs and per-symbol notes for a file, symbol, or NL topic. Use BEFORE editing code when prior decisions might apply."),
  /* params */)

// W5 replaces with verbatim G6 prose (~200-300 words; see Step B for full text)
```

<!-- G6 verbatim prose anchors (READ from 05-discuss-mcp.md sections 2 / 3 / 4) -->
<!-- - query_constraints prose at lines 80-114 (description + worked example + when-NOT) -->
<!-- - remember_symbol_note prose at lines 130-161 -->
<!-- - get_edit_context prose at lines 173-224 -->
</interfaces>
</context>

<tasks>

<task type="auto" tdd="false">
 <name>Task 1: Replace W3 first-cut descriptions with G6 production-grade prose for all 5 new tools</name>
 <files>server/internal/mcpsrv/server.go</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-mcp.md sections 2-4 (verbatim prose for 3 of 5 tools)
  - server/internal/mcpsrv/server.go (full file -- locate the 5 W3 tool registration blocks)
  - mcp-go library docs (Context7 if available) -- confirm `mcp.WithDescription` accepts raw multiparagraph strings; check if mcp-go has a `mcp.WithLongDescription` separate API
  - CONTEXT.md vocab list (Symbol, Edge, EdgeKind, AliasDecl, Confidence; AVOID node/entity/link/score per CONTEXT.md _Avoid_ list)
 </read_first>

 <action>

**Step A -- copy verbatim prose for 3 tools.** From 05-discuss-mcp.md
section 2 (query_constraints) lines 80-119, section 3
(remember_symbol_note) lines 131-161, section 4 (get_edit_context)
lines 173-224. Convert each to a Go raw string:

```go
const queryConstraintsDesc = `Call this BEFORE editing or refactoring code when an architectural rule, prior decision, or "do not do X" constraint might apply to this file/Symbol/topic. Returns ranked constraint clauses extracted from the project's ARCHITECTURE.md, planning docs, and prior remember_symbol_note annotations -- specifically MUST / MUST-NOT / SHOULD statements anchored to the queried scope. This is the tool that surfaces things like "MUST NOT introduce reranker without LLM-judge" automatically when an agent edits retrieval code, instead of requiring the agent to remember those constraints exist.

Args: repo_hash (string), scope (one of file / symbol / topic), target (string: file path / symbol_id / NL topic).

Returns: [{text, severity: MUST|MUST-NOT|SHOULD, source: {kind: ADR|note, path, line}, anchor: {symbol_id?, file?}, confidence}] ranked by relevance x severity.

Worked example:
Input: {"repo_hash":"a1b2c3","scope":"file","target":"experiments/poc-retrieval/core/src/search.rs"}
Output: {"constraints":[{"text":"MUST NOT introduce reranker without LLM-judge eval first.","severity":"MUST-NOT","source":{"kind":"ADR","path":"docs/ARCHITECTURE.md","line":412},"confidence":0.94}, ...]}

Do NOT call this for "find a function by name" (use query) or "what calls X" (use list_callers); this returns prose constraints, not Symbols. Skip if you've already retrieved constraints for this scope earlier in the session.`
```

Same pattern for `rememberSymbolNoteDesc` and `getEditContextDesc`. The full prose is in 05-discuss-mcp.md; copy verbatim, escape backticks if any (replace ``` blocks with quoted JSON).

**Step B -- author list_notes + extract_adrs prose at G6 quality bar.** Template per G6 section 1 5 criteria:

list_notes:
```go
const listNotesDesc = `Call this when you need to see ALL prior remember_symbol_note annotations on a specific Symbol, in supersede-chain order. Common trigger: you're about to edit a Symbol and want to see what previous agents recorded about it -- but you do NOT want the broader composite context (constraints + callers + edges) that get_edit_context provides. list_notes is the focused, lightweight read.

By default returns active leaves only -- the latest version of each note after supersede chain resolution. Pass include_history=true to see the full audit trail (all versions including superseded ones). The is_active_leaf field on each NoteView indicates which is current.

Args: repo_hash (string), symbol_id (string from prior query or get_symbol), include_history (bool, default false).

Returns: [{note_id, path, name, kind, note_text, tags, confidence, source_session, supersedes_note_id, created_at, is_active_leaf}].

Worked example:
Input: {"repo_hash":"a1b2c3","symbol_id":"42"}
Output: {"notes":[{"note_id":7,"path":"src/search.rs","name":"embed_query","kind":"function","note_text":"Retry budget MUST stay 2 attempts / 250ms total...","confidence":0.92,"is_active_leaf":true,"created_at":"2026-05-15T14:32:11Z"}]}

Do NOT call this for: notes across multiple symbols (call get_edit_context per symbol instead), discovering whether a Symbol HAS notes (call get_edit_context which returns notes plus everything else in one trip), or as a debug dump of the entire notes table (no API for that intentionally -- notes are per-Symbol, not project-wide). Skip when get_edit_context has already returned the notes you need for the same Symbol this session.`
```

extract_adrs:
```go
const extractAdrsDesc = `Call this to (re-)extract ADR-style constraints (MUST / MUST-NOT / SHOULD prose) from the project's markdown documentation into the constraints store, where query_constraints and get_edit_context can find them. By default this runs automatically as part of index_repo -- so you usually don't need to call it explicitly.

Manual triggers worth calling extract_adrs for: (1) you just edited docs/ARCHITECTURE.md or another tracked spec and want the new MUST-clauses retrievable immediately without a full re-index; (2) you changed the [adr] config in plugin.toml and want re-extraction without bumping file mtimes; (3) you want to see what would be extracted (pass dry_run=true).

Args: repo_hash (string), scope (optional array of paths to limit extraction to a subset; default = use plugin.toml include globs which cover docs/**/*.md, .planning/*.md one-level, .planning/phases/**/*-PLAN.md, README.md), dry_run (bool, default false; true = scan and report counts but persist nothing).

Returns: {files_scanned, adrs_inserted, adrs_superseded, adrs_skipped_duplicate, warnings: []}.

Worked example:
Input: {"repo_hash":"a1b2c3","scope":["docs/ARCHITECTURE.md"],"dry_run":true}
Output: {"files_scanned":1,"adrs_inserted":12,"adrs_superseded":0,"adrs_skipped_duplicate":0,"warnings":[]}

Do NOT call this as a generic "warm the cache" warmup -- index_repo already does that. Do NOT call this with dry_run=false on every session start; it's idempotent (UNIQUE constraint on path/line/sha) but wastes tokens. Skip if you just ran index_repo this session.`
```

**Step C -- replace W3 first-cut WithDescription calls.** In server.go, locate each of the 5 `s.AddTool(mcp.NewTool("...", mcp.WithDescription("..."), ...))` calls added in W3. Replace the WithDescription argument with the corresponding constant (queryConstraintsDesc / rememberSymbolNoteDesc / etc.).

If mcp-go has a separate `mcp.WithLongDescription` option for multi-paragraph prose vs `mcp.WithDescription` for short strings, use the long variant. Verify via Context7 or by reading the existing 4 tools' registrations.

**Step D -- Go vet + build:**
```bash
cd D:/projects/codenexus/server
go build ./...
go vet ./...
```

**Step E -- 5-criterion quality checklist** (run per description; document in SUMMARY):
| Criterion | Description |
|---|---|
| 1 | Trigger phrasing precedes mechanics? (First sentence answers "when does the agent reach for this?") |
| 2 | Worked example with realistic JSON input + output? |
| 3 | Explicit when-NOT-to-call hint? |
| 4 | User-visible value (not capability brag)? |
| 5 | Vocab uses CONTEXT.md terms (Symbol, Edge, ...) and avoids node/entity/link/score? |

All 5 descriptions MUST score 5/5. SUMMARY documents the check.

 </action>

 <acceptance_criteria>
  - `grep -nE 'const (queryConstraintsDesc|rememberSymbolNoteDesc|listNotesDesc|getEditContextDesc|extractAdrsDesc)' server/internal/mcpsrv/server.go` returns 5 hits (one per tool)
  - `grep -cF 'Call this BEFORE editing or refactoring' server/internal/mcpsrv/server.go` returns 1 (query_constraints verbatim opening)
  - `grep -cF 'Call this AFTER editing a Symbol' server/internal/mcpsrv/server.go` returns 1 (remember_symbol_note verbatim opening)
  - `grep -cF 'Call this IMMEDIATELY BEFORE editing' server/internal/mcpsrv/server.go` returns 1 (get_edit_context verbatim opening)
  - `grep -cF '"severity":"MUST-NOT"' server/internal/mcpsrv/server.go` >= 1 hit (worked example for query_constraints includes the canonical example from G6 section 2)
  - `grep -cF 'Worked example' server/internal/mcpsrv/server.go` >= 5 hits (one per tool, criterion 2)
  - `grep -cF 'Do NOT' server/internal/mcpsrv/server.go` >= 5 hits (one per tool, criterion 3)
  - `cd server && go build ./... && go vet ./...` exits 0 (G-A)
  - All 5 descriptions verified against G6 5-criterion checklist; SUMMARY documents PASS for each (G-B)
 </acceptance_criteria>

 <verify>
  <automated>cd server && go build ./... && go vet ./... && grep -cE 'const (queryConstraintsDesc|rememberSymbolNoteDesc|listNotesDesc|getEditContextDesc|extractAdrsDesc)' internal/mcpsrv/server.go</automated>
 </verify>

 <done>
  server.go has 5 multi-paragraph const description strings (one per
  new tool). 3 of 5 are verbatim from 05-discuss-mcp.md G6
  sections; 2 (list_notes, extract_adrs) authored at the same G6
  quality bar. Each description satisfies all 5 G6 criteria. Go
  build + vet clean.
 </done>
</task>

<task type="auto" tdd="false">
 <name>Task 2: docs/MCP-AFFORDANCE.md (first-run agent affordance guide) + BETA-V1-SPEC line 213 amendment</name>
 <files>docs/MCP-AFFORDANCE.md, .planning/BETA-V1-SPEC.md</files>

 <read_first>
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-discuss-mcp.md (full -- frame for the affordance guide)
  - .planning/PROJECT.md lines 102-110 (Software 3.0 reframe + strategic bets 2 & 3)
  - .planning/BETA-V1-SPEC.md lines 188-238 (section 8 in full -- find the exact text to amend)
 </read_first>

 <action>

**Step A -- create `docs/MCP-AFFORDANCE.md`** (~100-150 lines).
Outline:

```markdown
# CodeNexus MCP -- Agent Affordance Guide

This guide tells you (the LLM agent) when to reach for each of the 5
new MCP tools shipped in Phase 5 (Bridge), how they compose into a
typical edit session, and the underlying insight: you are not the
first agent to touch this codebase.

## The 5 new tools at a glance

| Tool | Trigger | Returns |
|------|---------|---------|
| query_constraints | "About to edit, what rules apply?" | Ranked MUST/MUST-NOT/SHOULD clauses |
| list_notes | "What did prior agents say about this Symbol?" | Per-Symbol notes (active leaves) |
| get_edit_context | "About to edit Symbol X" | Composite brief (definition + callers + constraints + notes + edges) |
| remember_symbol_note | "I learned a non-obvious thing while editing" | note_id (persisted append-only) |
| extract_adrs | "Just changed docs/ARCHITECTURE.md, want it in constraints store NOW" | Counts (auto-runs on index_repo) |

## When to call each

[5 paragraphs, one per tool, ~30 words each, focused on the trigger.
Pull from G6 verbatim prose first sentences.]

## Typical session flow

```
1. query "embed_query"        -> get symbol_id
2. get_edit_context(symbol_id)    -> see callers + constraints + notes + edges
3. [edit the code]
4. [verify edit works]
5. remember_symbol_note(symbol_id, "Discovered: X reason for Y choice", confidence=0.85)
                   -> next agent (or future-you) sees this
```

## The cross-session insight (PROJECT.md:107)

CodeNexus persists per-Symbol annotations across sessions. The point
is: you are not the first agent to touch this codebase. When you
discover a non-obvious invariant ("the consecutive_fails counter MUST
stay in the caller's loop"), recording it via remember_symbol_note
lets the NEXT agent (which may be you in next session) discover it
via query_constraints / get_edit_context BEFORE breaking the
invariant.

This converts agent-hours-of-debugging-the-same-thing into
agent-minutes-of-reading-the-prior-note.

## When NOT to call any of these 5 tools

- Pure exploration with no edit intent: use `query` + `get_symbol` (the existing tools).
- Scratch / TODO notes: use your own todo list; do NOT pollute the persistent store.
- User preferences: use git config or project README; not Symbol-anchored.
- High-confidence trivial facts: skip remember_symbol_note (the next agent can derive from get_symbol).

## V1.0 limits

- get_edit_context file-scope is deferred to V1.1 (use Symbol target).
- ADR extraction includes docs/**/*.md + .planning/*.md (one-level) + .planning/phases/**/*-PLAN.md + README.md by default; configure via plugin.toml [adr] section if your repo has different conventions.
- Notes are append-only; supersede via the supersedes parameter, never delete.

## See also

- BETA-V1-SPEC section 8: Phase 5 scope contract
- 05-DISCUSS-SUMMARY.md: design decisions (G1-G7, UQ-A1..5)
- 05-discuss-mcp.md: full G6 description authoring rationale
```

(EXECUTOR: keep ASCII-safe per rule 17; no Unicode arrows or em-dashes; use ASCII `->` and `--`.)

**Step B -- amend BETA-V1-SPEC section 8.** Open `.planning/BETA-V1-SPEC.md`. Locate line ~213 ("A2A operations: query_constraints..."). Replace with the multi-line block from D-W5-06 above. Make sure the amendment is INSIDE section 8's bullet list so the surrounding markdown stays valid.

**Step C -- commit discipline (executor).** Per BETA-V1-SPEC line 183-185: amendment requires `docs(beta-v1):` commit message + explicit rationale. Use:
```
docs(beta-v1): finalize Phase 5 op count (3 -> 5) per UQ-A3

Per Phase 5 discuss UQ-A3 lock (Curry approved 2026-05-03 "all in"
path), V1.0 ships 5 public A2A ops not 3. extract_adrs and list_notes
now first-class per BETA-V1-SPEC section 8 acceptance gate.
```

**Step D -- verify markdown renders cleanly:**
```bash
test -f docs/MCP-AFFORDANCE.md
wc -l docs/MCP-AFFORDANCE.md # expect 80-200 lines
grep -F 'query_constraints' docs/MCP-AFFORDANCE.md # >= 1
grep -F 'remember_symbol_note' docs/MCP-AFFORDANCE.md # >= 1
grep -F 'get_edit_context' docs/MCP-AFFORDANCE.md # >= 1
grep -F 'list_notes' docs/MCP-AFFORDANCE.md # >= 1
grep -F 'extract_adrs' docs/MCP-AFFORDANCE.md # >= 1
grep -F 'extract_adrs' .planning/BETA-V1-SPEC.md # >= 1 (amendment landed)
grep -F 'list_notes' .planning/BETA-V1-SPEC.md # >= 1
```

 </action>

 <acceptance_criteria>
  - `test -f docs/MCP-AFFORDANCE.md` exits 0
  - `wc -l docs/MCP-AFFORDANCE.md` returns >= 80 AND <= 200
  - All 5 tool names present in MCP-AFFORDANCE.md (>= 1 mention each)
  - Section headings present: "When to call each", "Typical session flow", "The cross-session insight", "When NOT to call any of these 5 tools" (or close variants)
  - `grep -F 'extract_adrs' .planning/BETA-V1-SPEC.md` >= 1 hit (amendment shipped)
  - `grep -F 'list_notes' .planning/BETA-V1-SPEC.md` >= 1 hit
  - BETA-V1-SPEC section 8 line block now names 5 ops (verify via grep `5 public ops` or by reading the section 8 block)
  - No Unicode arrows / em-dashes in either file (ASCII-safe per rule 17): `grep -P '[\x80-\xff]' docs/MCP-AFFORDANCE.md` returns 0 (or matches only intentional CJK from quoted CONTEXT vocab)
 </acceptance_criteria>

 <verify>
  <automated>test -f docs/MCP-AFFORDANCE.md && wc -l docs/MCP-AFFORDANCE.md && grep -cF 'extract_adrs' .planning/BETA-V1-SPEC.md && grep -cF 'list_notes' .planning/BETA-V1-SPEC.md</automated>
 </verify>

 <done>
  docs/MCP-AFFORDANCE.md exists (~100-150 lines, ASCII-safe). All 5
  new tools covered with trigger descriptions, typical session
  flow, cross-session insight, and explicit when-NOT bounds.
  BETA-V1-SPEC section 8 amended to name 5 ops explicitly per UQ-A3 +
  D-W5-06. Commit follows BETA-V1-SPEC section 1.0 amendment discipline.
  G-C + G-D verified.
 </done>
</task>

</tasks>

<gates>
- **G-A** (Go build clean): `cd server && go build ./... && go vet ./...` clean. [Task 1]
- **G-B** (G6 5-criterion bar met): all 5 descriptions PASS the 5-criterion checklist; SUMMARY documents per-tool PASS evidence. [Task 1]
- **G-C** (affordance docs ship): docs/MCP-AFFORDANCE.md exists, ~100-150 lines, covers all 5 new tools. [Task 2]
- **G-D** (BETA-V1-SPEC amendment): section 8 line 213 amended from 3 ops to 5 named ops; commit message follows discipline. [Task 2]
</gates>

<must_haves>
truths:
 - "All 5 new MCP tools have G6 production-grade descriptions (~200-300 words each) embedded in server/internal/mcpsrv/server.go"
 - "Each description satisfies G6 5-criterion bar: trigger-first, worked example, when-NOT, user-visible value, CONTEXT.md vocab"
 - "query_constraints / remember_symbol_note / get_edit_context descriptions are VERBATIM from 05-discuss-mcp.md sections 2/3/4"
 - "list_notes / extract_adrs descriptions authored at same quality bar following G6 section 1 template"
 - "docs/MCP-AFFORDANCE.md exists as one-page agent affordance guide with all 5 tools + typical flow + cross-session insight + when-NOT bounds"
 - "BETA-V1-SPEC section 8 amended: 3 ops -> 5 ops named (extract_adrs, list_notes, query_constraints, remember_symbol_note, get_edit_context) per UQ-A3"
 - "Amendment commit follows BETA-V1-SPEC section 1.0 discipline: docs(beta-v1): prefix + rationale"
artifacts:
 - path: "server/internal/mcpsrv/server.go"
  provides: "5 G6-grade const description strings + WithDescription wiring"
  contains: "queryConstraintsDesc"
 - path: "docs/MCP-AFFORDANCE.md"
  provides: "Agent affordance guide (~100-150 lines)"
  contains: "When to call each"
 - path: ".planning/BETA-V1-SPEC.md"
  provides: "section 8 line ~213 amendment naming 5 public A2A ops"
  contains: "5 public ops"
key_links:
 - from: "server/internal/mcpsrv/server.go (s.AddTool calls)"
  to: "5 description constants"
  via: "mcp.WithDescription(<const>)"
  pattern: "WithDescription\\(.*Desc\\)"
 - from: "docs/MCP-AFFORDANCE.md"
  to: ".planning/BETA-V1-SPEC.md section 8 + 05-DISCUSS-SUMMARY.md"
  via: "see also section"
  pattern: "BETA-V1-SPEC|DISCUSS-SUMMARY"
</must_haves>

<verification>
1. `cd server && go build ./... && go vet ./...` clean (G-A)
2. 5 description constants present in server.go (G-B verified by grep + 5-criterion checklist in SUMMARY)
3. docs/MCP-AFFORDANCE.md exists with all 5 tool names mentioned (G-C)
4. .planning/BETA-V1-SPEC.md section 8 contains all 5 op names (G-D)
5. Commit message follows `docs(beta-v1):` prefix (G-D)
</verification>

<open_questions>
- **OQ-W5-01:** mcp-go API for multi-paragraph descriptions -- is `mcp.WithDescription` the right entry point or does mcp-go have a separate long-form variant? Plan-checker confirms via Context7 lookup or by reading existing 4 tools' registrations + mcp-go release notes.
- **OQ-W5-02:** docs/MCP-AFFORDANCE.md placement -- under docs/ alongside ARCHITECTURE.md? Or under .planning/? Locked: docs/ (per affordance file naming convention; agent-facing user docs live in docs/).
</open_questions>

<honest_gap_list>
**P1**:
- (none)

**P2**:
- 5-criterion checklist verification is human-judgment (Task 1 acceptance criterion: "SUMMARY documents PASS for each"). plan-checker may want a more mechanical check. Mitigation: SUMMARY embeds the checklist with grep evidence per criterion; criterion 1 (trigger-first) verifiable by checking opening sentence of each description starts with "Call this" or equivalent imperative.
- list_notes + extract_adrs descriptions are AUTHORED at execution time. They go through the same plan-checker iteration as the rest of the plan; if iter 1 finds them sub-G6 quality, fix loops back here. Mitigation: D-W5-02 template constrains structure tightly.

**P3**:
- mcp.WithDescription char limit -- some MCP clients truncate descriptions over N chars. Plan-checker confirms target client (Claude Desktop / Claude Code) does not truncate at the typical ~200-300 word range.
- BETA-V1-SPEC.md amendment is the only doc-only change in W5; if commit hooks require code changes alongside doc-only commits, executor adjusts (split into separate commits).
- MCP-AFFORDANCE.md has potential to drift from server.go descriptions over time (V1.1+ description tuning); SUMMARY notes this as a maintainability concern. Mitigation: the 1-page affordance doc is high-level; it points to server.go descriptions for details.
</honest_gap_list>
</content>
