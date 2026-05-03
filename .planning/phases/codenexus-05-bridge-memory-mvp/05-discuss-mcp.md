# G6: MCP Tool Surface Naming + Descriptions for 3 New Ops

(Authored by gsd-advisor-researcher 2026-05-03 -- file written by parent
synthesizer because the advisor role contract returns deliverable as message
rather than disk-write.)

## 1. MCP description quality bar

CodeCompass (arxiv 2602.20048) measured agents skipping graph tools 58% of
the time even when prompts explicitly tell them to use the tool; G2-task
adoption was 0/30 trials despite the tool being designed for exactly that
case. The mechanism is rational: when the default approach (Glob+Read) hits
~80% on most tasks, the agent cannot detect ahead of time which task needs
the graph tool, so the tool's overhead isn't justified by the agent's prior.
This is the failure mode CodeNexus PROJECT.md:106 commits to fixing (target:
drive 58% skip rate to <= 5%).

Companion paper "MCP Tool Descriptions Are Smelly" (arxiv 2602.14878v2)
measured 97.1% of 856 real-world MCP tools have at least one description
smell; 56% have "Unclear Purpose". Six tool description components matter:
Purpose, Guidelines (when to use), Limitations (when NOT to use), Parameter
Explanation, Length/Completeness (3-4+ sentences for complex tools),
Examples (both success + failure cases).

**5 quality criteria for CodeNexus Phase 5 tool descriptions:**

1. **Trigger phrasing precedes mechanics.** First sentence answers "when does
   the agent reach for this tool?" not "what does this tool do?". Existing
   CodeNexus tools (`server.go:35,44,54,63`) currently fail this -- they all
   open with mechanics ("Index a repository...", "Hybrid BM25+vector
   search...") and never tell the agent the *symptom* that should trigger
   them. Phase 5 tools must invert.

2. **Worked example with realistic input + output.** CodeCompass G3 trial
   adoption only hit 100% after the prompt included a worked checklist
   (positioned at end of prompt, mitigating "Lost in the Middle"). MCP tool
   descriptions face the same attention problem -- an abstract description
   alone gets skipped. Concrete `{"input": ..., "output": ...}` JSON pair
   anchors the tool in the agent's pattern-matching.

3. **Explicit when-NOT-to-call hint.** Per smell #3 ("Unstated Limitations"):
   if the tool description doesn't bound itself, the agent over-calls
   (spurious cost) or skips (under-call). For Phase 5 specifically:
   `query_constraints` must NOT be called for "find a function by name"
   (that's `query`); `remember_symbol_note` must NOT be called for arbitrary
   scratch notes (only when editing the symbol surfaces a non-obvious
   gotcha worth persisting); `get_edit_context` must NOT be called as a
   default warmup (only when about to edit a specific symbol).

4. **User-visible value, not capability brag.** Per the Software 3.0 reframe
   in PROJECT.md:102 -- these tools exist so the LLM stops re-deriving the
   same insight every session. Description must say "the previous agent left
   a note here" or "an ARCHITECTURE.md MUST applies to this code" -- not
   "returns notes from the notes table."

5. **Vocab consistency with CONTEXT.md.** Tool descriptions must use
   **Symbol**, **Edge**, **EdgeKind**, **AliasDecl**, **Confidence** as
   defined in CONTEXT.md:13-65 -- the LLM has seen these terms when calling
   `query`/`get_symbol`/`list_callers`, and lexical consistency reinforces
   the conceptual model. Avoid "node", "entity", "link", "score"
   (CONTEXT.md "_Avoid_" list).

**Anti-patterns to avoid (with examples):**

- BAD `"Returns notes for a symbol."` -- pure mechanics, no trigger, no value, no example. Smell: Unclear Purpose.
- BAD `"Powerful semantic search over architectural decisions."` -- marketing prose, zero trigger, zero parameter explanation. Smell: Underspecified.
- BAD `"Use when you need constraint info."` -- circular ("constraint info" = unexplained jargon). Smell: Opaque Purpose.
- GOOD `"Call this BEFORE editing a symbol when you suspect a non-obvious invariant or 'do not change X' rule applies. Returns prior agents' notes + ADR clauses (MUST/MUST-NOT/SHOULD) anchored to this symbol or its file. Skip if you've already retrieved this symbol's context this session."` -- trigger + value + bounds.

## 2. `query_constraints` MCP description

**Tool name (final):** `query_constraints`

**Rejected alternatives:**
- `find_constraints` -- verb mismatch with sibling `query` (existing). Stay parallel.
- `get_adr` -- too narrow (also returns symbol-scoped notes' constraint-flavored ones).
- `search_rules` -- "rules" overloads with linter rules; "constraints" tracks PROJECT.md:108 vocabulary.
- `list_constraints` -- `list_*` in CodeNexus conventionally returns all (`list_callers`); this is filtered/ranked, so `query_*` is right.

**Description text (verbatim prose for MCP):**

> Call this BEFORE editing or refactoring code when an architectural rule, prior decision, or "do not do X" constraint might apply to this file/Symbol/topic. Returns ranked constraint clauses extracted from the project's ARCHITECTURE.md, planning docs, and prior `remember_symbol_note` annotations -- specifically MUST / MUST-NOT / SHOULD statements anchored to the queried scope. This is the tool that surfaces things like "MUST NOT introduce reranker without LLM-judge" automatically when an agent edits retrieval code, instead of requiring the agent to remember those constraints exist. Args: `repo_hash` (string), `scope` (one of `file` / `symbol` / `topic`), `target` (string: file path / symbol_id / NL topic). Returns: `[{text, severity: MUST|MUST-NOT|SHOULD, source: {kind: ADR|note, path, line}, anchor: {symbol_id?, file?}, confidence}]` ranked by relevance x severity. Do NOT call this for "find a function by name" (use `query`) or "what calls X" (use `list_callers`); this returns *prose constraints*, not Symbols. Skip if you've already retrieved constraints for this scope earlier in the session.

**Worked example:**

```json
// Input
{
  "repo_hash": "a1b2c3",
  "scope": "file",
  "target": "experiments/poc-retrieval/core/src/search.rs"
}

// Output
{
  "constraints": [
    {
      "text": "MUST NOT introduce reranker without LLM-judge eval first.",
      "severity": "MUST-NOT",
      "source": {"kind": "ADR", "path": "docs/ARCHITECTURE.md", "line": 412},
      "anchor": {"file": "experiments/poc-retrieval/core/src/search.rs"},
      "confidence": 0.94
    },
    {
      "text": "Counter location: consecutive_fails belongs in caller's loop, not embedder.",
      "severity": "MUST",
      "source": {"kind": "note", "path": "notes/symbol/embed_query.md", "line": 1},
      "anchor": {"symbol_id": "search.rs::embed_query"},
      "confidence": 0.81
    }
  ]
}
```

**When-NOT-to-call:** Symbol-name lookup -> use `query`. Caller graph -> use
`list_callers`. Free-text README search -> use external grep. Already
retrieved constraints for this scope this session -> skip (idempotent
re-call wastes tokens; results are stable within a session unless
`remember_symbol_note` fired).

## 3. `remember_symbol_note` MCP description

**Tool name (final):** `remember_symbol_note`

**Rejected alternatives:**
- `add_note` -- too generic; doesn't signal that the note is symbol-scoped + persistent across sessions.
- `annotate_symbol` -- closer, but "annotate" reads as IDE-tooling; "remember" carries the cross-session persistence semantic that PROJECT.md:107 names as the value prop.
- `save_finding` -- "finding" is bug-tracker jargon; notes are broader (gotchas, invariants, design decisions).
- `write_symbol_note` -- paired with hypothetical `read_symbol_note`; rejected because reads happen via `get_edit_context` (composite), not a separate read op.

**Description text (verbatim prose for MCP):**

> Call this AFTER editing a Symbol when you discovered a non-obvious invariant, gotcha, or design constraint that future agents (including future-you in next session) would benefit from seeing BEFORE they touch this code. Examples of good notes: "this function is called from the hot path; benchmark before adding allocations", "the consecutive_fails counter MUST stay in the caller's loop, not the embedder -- moved it once and broke Query path semantics, see commit X", "edges to this symbol carry confidence < 1.0 because the resolver falls back to GlobalUnique for cross-package calls". The note becomes part of `query_constraints` and `get_edit_context` output for this Symbol -- this is how CodeNexus accumulates per-codebase intelligence across agent-hours instead of letting it die at session end. Args: `repo_hash` (string), `symbol_id` (string from prior `query` or `get_symbol`), `note` (string, prose, max 2000 chars), `source_session` (string, opaque session id for audit), `confidence` (float 0..1, your own confidence in the note's correctness), optional `supersedes` (note_id, if this corrects a prior note). Do NOT call this for ephemeral scratch ("trying X next"), session-scoped TODOs (use your own todo list), user preferences ("Curry likes tabs"), or notes lacking a specific Symbol anchor (use the project's general notes file). Notes are append-only; supersede via `supersedes`, never delete.

**Worked example:**

```json
// Input
{
  "repo_hash": "a1b2c3",
  "symbol_id": "search.rs::embed_query",
  "note": "Retry budget here is intentionally 2 attempts / 250ms total. Index path swallows EmbedError into consecutive_fails; Query path must surface fast (user is waiting). Do not unify with Index path retry policy -- see PROJECT.md:88 EmbedError taxonomy.",
  "source_session": "claude-2026-05-15-abc",
  "confidence": 0.92
}

// Output
{
  "note_id": "n_7f3a2",
  "stored_at": "2026-05-15T14:32:11Z",
  "anchor": {"symbol_id": "search.rs::embed_query", "path": "experiments/poc-retrieval/core/src/search.rs", "name": "embed_query", "kind": "function"},
  "supersedes": null
}
```

**When-NOT-to-call:** Session-local scratch ("try Y next"), user preferences
("Curry prefers X"), notes without a Symbol anchor (use git commit message
or a project doc), high-confidence facts already obvious from the code (the
note must add information the next agent can't trivially re-derive from
`get_symbol`), notes you wouldn't stake `confidence >= 0.7` on
(low-confidence speculation pollutes future retrieval).

## 4. `get_edit_context` MCP description

**Tool name (final):** `get_edit_context`

**Rejected alternatives:**
- `get_symbol_context` -- true but understates the *editing* trigger; agent reads "context" as optional warmup rather than pre-edit gate.
- `prepare_edit` -- verb implies side effects (lock acquisition, branch creation); this is read-only.
- `inspect_symbol` -- IDE-flavor; doesn't signal that this composes constraints + notes + callers + recent changes (i.e., more than `get_symbol` already gives).
- `brief_for_edit` -- closest in intent; rejected for reading as overly novel verb that an agent might not associate with a tool call.

**Description text (verbatim prose for MCP):**

> Call this IMMEDIATELY BEFORE editing a Symbol or file. Returns a single composite brief: (1) the Symbol's current definition (same as `get_symbol`), (2) all callers (same as `list_callers`, depth=1), (3) all applicable constraints (same as `query_constraints` scope=symbol), (4) all prior `remember_symbol_note` annotations on this Symbol, (5) Edges in/out by EdgeKind. This is the one call to make before touching code -- it replaces the manual sequence of `get_symbol` + `list_callers` + `query_constraints`, and surfaces the cross-session intelligence (constraints + notes) that the agent doesn't know to ask for individually. The point is to reduce "I'll just edit and see what tests break" by making the constraint surface visible *before* the edit. Args: `repo_hash` (string), `target` (string: symbol_id from prior `query`, OR file path for file-scope edit-context), `caller_depth` (int, default 1, max 3). Returns: `{symbol, callers, constraints, notes, edges_in, edges_out}` as a single JSON object. Do NOT call this as a generic "tell me about this code" warmup; only call when an edit to this specific Symbol/file is the next concrete action. For pure exploration use `query` or `get_symbol`. Skip if you've called this on this exact target this session AND no `remember_symbol_note` has fired since (the result is stable).

**Worked example:**

```json
// Input
{
  "repo_hash": "a1b2c3",
  "target": "search.rs::embed_query",
  "caller_depth": 1
}

// Output
{
  "symbol": {
    "id": "search.rs::embed_query",
    "kind": "function",
    "path": "experiments/poc-retrieval/core/src/search.rs",
    "line": 31,
    "signature": "async fn embed_query(text: &str) -> Result<Vec<f32>, EmbedError>"
  },
  "callers": [
    {"id": "search.rs::query", "confidence": 1.0},
    {"id": "server.rs::handle_query", "confidence": 0.95}
  ],
  "constraints": [
    {
      "text": "Query path retry budget MUST stay 2 attempts / 250ms total.",
      "severity": "MUST",
      "source": {"kind": "ADR", "path": "docs/ARCHITECTURE.md", "line": 731}
    }
  ],
  "notes": [
    {
      "note_id": "n_7f3a2",
      "text": "Retry budget here is intentionally 2 attempts / 250ms total...",
      "confidence": 0.92,
      "stored_at": "2026-05-15T14:32:11Z"
    }
  ],
  "edges_in": [{"from": "search.rs::query", "kind": "Calls", "confidence": 1.0}],
  "edges_out": [{"to": "embedder.rs::embed", "kind": "Calls", "confidence": 1.0}]
}
```

**When-NOT-to-call:** Generic exploration (use `query`); just need the
function body (use `get_symbol`); just need callers (use `list_callers`
directly to skip the constraint+note overhead); already called for this
exact `target` this session with no intervening `remember_symbol_note`
(re-call is pure waste).

## 5. Affordance polish -- V1.0 scope vs V1.1+ scope

**Decision: V1.0 ships rigorous descriptions. NOT minimal MVP.**

**Rationale:**

The Phase 5 deliverable per PROJECT.md:106 is *agent behavioral alignment*
(drive 58% skip -> <=5%) and per BETA-V1-SPEC.md MUST 5 + MUST 7 the eval
gate (B3 vs B2, >=25% improvement / >=20pp / <=2x cost) is what declares
Beta V1 shipped vs evidence-failed. **The eval cannot pass with smelly
descriptions** -- it would measure whether agents-with-rigorous-descriptions
outperform agents-with-no-tools, which conflates "tool quality" with
"description quality". CodeCompass's own G2 = 0% adoption result proves
description framing is the dominant variable. Shipping minimal descriptions
in V1.0 and "polishing in V1.1" defeats the gating purpose of MUST 7 -- the
eval done in V1.0 would be on the wrong artifact.

The cost is small. Each tool description is ~200-300 words of prose. Three
tools = ~1 page of writing. Compare to ~15 hrs of EVAL-INSTANCES authoring
(BETA-V1-SPEC sec 5.5 L3): description authoring is <=5% of that budget.
Skipping it to "save time" for a follow-up V1.1 is false economy.

**What stays V1.1+:**
- A/B comparison of competing description prose variants (we ship one rigorous version in V1.0; iterate based on real-world tool-invocation telemetry in V1.1).
- Per-agent-model description tuning (Claude vs GPT-5 may benefit from different framings -- out of scope for V1.0; ship one version that targets Claude since that's the primary eval agent).
- Localized descriptions (Chinese / Japanese MCP descriptions for non-English agents).
- Dynamic descriptions that adapt based on repo language / project conventions.

## 6. Eval harness for affordance quality

**The metric**: `tool_invocation_rate = (calls_to_tool_X_when_X_was_correct) / (total_tasks_where_X_was_correct)`. CodeCompass's G2/G3 split is the template -- ground truth labels each task with which tool *should* fire.

**Phase 5 sketch (W6 milestone, may live in EVAL-INSTANCES.md per BETA-V1-SPEC sec 8 line 215):**

```
Curated task set: 30 tasks across 3 repos (per BETA-V1-SPEC MUST 6).
For each task, 3 ground-truth labels:
  - expected_tools: subset of {query, get_symbol, list_callers,
                                query_constraints, remember_symbol_note,
                                get_edit_context}
  - constraint_anchor: file/symbol where a real ADR or note applies
                       (NULL if task should NOT trigger query_constraints)
  - edit_target: symbol_id the task ends up editing (NULL if read-only task)

Run modes (A/B):
  - B2 (control): old 4 tools only (index_repo, query, get_symbol, list_callers)
  - B3 (treatment): old 4 + new 3 tools with rigorous descriptions
  - B3-min (ablation): old 4 + new 3 with minimal "tool exists" descriptions
                       -- proves description-quality is the lever, not tool existence

Metrics per run:
  1. Final task success (did the agent produce correct edit / answer)
  2. Tool-invocation rate per tool, vs ground truth (precision + recall per tool)
  3. Spurious-call rate (tool called when ground truth says it shouldn't fire)
  4. Constraint-surfaced rate: of tasks where constraint_anchor != NULL,
     what fraction had query_constraints invoked AND got the relevant clause?
  5. Cost ratio (token + wall-clock) vs B2

Pass gate: B3 beats B2 by >=25% on metric 1 AND >=20pp on metric 4 AND
           cost <= 2x B2 (per EVAL-CONTRACT v1.0 frozen success gate).
B3-min ablation expectation: B3-min should NOT beat B2 by the same margin
           -- if it does, description quality didn't matter, which would
           invalidate this G6 deliverable's premise.
```

**Phase 5 vs Phase 4+ scope split:**
- **Phase 5 (V1.0):** ship harness skeleton, run B2 / B3 / B3-min on 30 tasks x 3 repos x N=3 seeds. Lock judge model + agent model in EVAL-INSTANCES.md.
- **Phase 4+ / V1.1+:** expand to 100+ tasks; multi-agent evaluation (Claude + GPT-5 + Sonnet); auto-mine new tasks from real session traces; description A/B on competing prose variants.

The tool-invocation rate metric is the load-bearing one for the
PROJECT.md:106 deliverable. Final task success is the user-visible metric
but is downstream -- if invocation rate stays low, success can't move. Keep
both; report both.

## 7. Open questions for Curry

1. **Tool naming: `remember_symbol_note` vs `remember_note`?** "Symbol" prefix makes scope explicit (anti-misuse) but is slightly verbose. Recommend keeping `remember_symbol_note` for V1.0 -- explicitness > brevity at the affordance layer.

2. **Should `get_edit_context` accept file scope OR symbol-only in V1.0?** Spec from PRE-PLAN-NOTES.md G4 says `symbol_id|file`. File scope multiplies output size (all symbols in a 500-line file ~= 20+ symbols x full context). Recommend: V1.0 = symbol-only; file-scope deferred to V1.1 (pending eval evidence that file-scope gives net positive vs N x symbol-scope calls).

3. **Note write authentication / authorship.** `source_session` is opaque per MUST 5 spec. Should V1.0 capture WHICH agent model wrote the note (Claude vs GPT-5 vs human)? Useful for retrieval ranking ("trust human notes > Claude notes > GPT-5 notes" or vice versa). Defer to V1.1 unless you want it now.

4. **Spurious-call penalty in B3 eval.** Should the eval penalize calling `query_constraints` when ground truth says it shouldn't fire (over-call cost), or only reward correct calls (under-call cost)? Recommend: penalize both. CodeCompass only measures under-call; over-call inflates token cost which threatens MUST 7 cost gate.

5. **Minimum confidence floor on `remember_symbol_note`.** Should V1.0 reject notes with `confidence < 0.5` to prevent low-quality pollution? Or accept all and rank-by-confidence at retrieval time? Recommend: accept all, rank at retrieval (matches append-only discipline; supersede is the correction mechanism).

6. **Description vocabulary check: is "Symbol" too jargon-y for non-CodeNexus-native agents?** Claude has seen "Symbol" via the existing 4 tool descriptions, so consistency wins. But a fresh agent (first MCP call) sees "Symbol" cold. Mitigation: first sentence of each new description should be self-contained ("...the Symbol -- a parsed function/class/method/etc."). Already done in worked examples; verify in final prose pass.

## Self-review (analysis-triforce)

1. **Precision:** the 58% / 0% / 100% / 97.1% / 56% numbers are sourced from the two arxiv papers cited; rest is project-doc citations (file:line). No invented numbers.
2. **Framework adaptation:** the "rigorous V1.0 descriptions" recommendation depends on the Software 3.0 reframe in PROJECT.md:102 holding. If that reframe collapses (per BETA-V1-SPEC sec 5.5 L5), the description-quality investment shifts from load-bearing to nice-to-have. Flagged.
3. **Feasibility:** ~1 page of prose x 3 tools is achievable in a single W5 sub-slice. Eval harness skeleton is the heavier lift but already scoped to W6 in PRE-PLAN-NOTES.md:135.

## Sources

- [CodeCompass: Navigating the Navigation Paradox in Agentic Code Intelligence](https://arxiv.org/abs/2602.20048)
- [Model Context Protocol (MCP) Tool Descriptions Are Smelly!](https://arxiv.org/abs/2602.14878)
- [How are AI agents used? Evidence from 177,000 MCP tools](https://arxiv.org/html/2603.23802v1)
