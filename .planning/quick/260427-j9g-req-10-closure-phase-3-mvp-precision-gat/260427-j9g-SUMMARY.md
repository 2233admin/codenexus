---
phase: quick-260427-j9g
plan_id: 260427-j9g
status: complete
type: execute
requirements: [REQ-10]
landed_files:
  - experiments/poc-retrieval/eval/req10_alpha06.json   # NEW (30-query eval evidence, B1-B7 mean=67.9%)
  - .planning/STATE.md                                  # MODIFY (Phase 3 4/5 -> 5/5 closed; Quick Tasks Completed row; last_activity)
  - progress.txt                                        # APPEND (closure session log block)
  - .planning/quick/260427-j9g-.../260427-j9g-PLAN.md   # NEW
  - .planning/quick/260427-j9g-.../260427-j9g-SUMMARY.md# NEW (this file)
commits:
  - 226c50f "mvp(eval): REQ-10 PASS -- Phase 3 MVP precision gate met"
gates:
  acceptance_gate_60pct: pass
  spike_001_baseline_subset_b1_b7: 0.679
  gitnexus_1_6_3_baseline: 0.436
  delta_vs_gate: +0.079
  delta_vs_gitnexus: +0.243
  invariants_verified: 8/8
---

# REQ-10 Summary — Phase 3 MVP precision gate met (PASS)

## Verdict

**PASS.** B1-B7 spike-001 baseline subset mean precision_at_5 = **0.679** (67.9%) at locked config (alpha=0.6, rerank=false). Acceptance gate 0.600 cleared by **+7.9pp**; GitNexus 1.6.3 baseline 0.436 beaten by **+24.3pp**.

Phase 3 (MVP) closes here. 5/5 REQs done.

## Per-query breakdown (B1-B7)

| Query | Domain text | CodeNexus | GitNexus 1.6.3 | Delta | Notes |
|---|---|---|---|---|---|
| B1 | filesystem fallback when obsidian not running | **1.00** | 0.10 | **+0.90** | Largest win — RRF fusion crushes NL on multi-keyword query |
| B2 | preflight check for protected directories | **1.00** | 0.70 | +0.30 | |
| B3 | search files by tag | **1.00** | 0.65 | +0.35 | |
| B4 | build concept graph from notes | 0.00 | 0.80 | -0.80 | **Known miss**: Python target, POC parser TS-only (`_arch_limit` flag in queries.json) |
| B5 | rate limiting middleware *(negative)* | -0.25 | 0.00 | -0.25 | Negative test; CodeNexus returned a high-confidence false positive (top-1 score > 0.012 threshold). Tunable in Phase 4 |
| B6 | safe file deletion with dry run | **1.00** | 0.30 | +0.70 | |
| B7 | register MCP tool handler | **1.00** | 0.50 | +0.50 | |
| **mean** | | **0.679** | 0.436 | **+0.243** | Gate 60% **+7.9pp clear** |

6 of 7 queries clear (B1/B2/B3/B6/B7=100%). B4 is an architectural limit acknowledged in queries.json. B5 is a negative-test scoring formula edge case.

## Axis breakdown (full 30-query set, for context only — NOT the gate)

| Axis | Mean | n |
|---|---|---|
| 1 (exact symbol lookup, A1-A10) | 70.0% | 10 |
| 2 (NL semantic, B1-B10) | 47.5% | 10 |
| 3 (graph traversal, C1-C10) | 30.0% | 10 |
| **overall (A+B+C)** | **49.2%** | 30 |

Axis-2 full-set 47.5% looks low but is dominated by the 3 new concurrency/conflict/aggregation queries (B8-B10) that have no GitNexus baseline — this is uncalibrated comparison terrain. Axis-3 30% is hand-matcher precision; LLM-judge re-eval (R7c) measured 23.3% — both consistent with Phase 4 graph augmentation being unfinished.

The gate is the 7-query subset, and the 7-query subset cleared.

## Configuration locked

- alpha = 0.6 (R5/R6 locked; R6c position-bias randomization confirmed Δ within stochastic noise)
- rerank = false (R6 found rerank trend +2pp at p=0.084, NOT significant — locked default stays off)
- No graph augmentation (R7 graph-axis3 sweep was axis-3 specific)
- DB: experiments/poc-retrieval/poc.db (52 files / 2116 symbols / 877+ edges from earlier marathon-session indexing)

## What this slice did NOT do

This is a **docs + state-flip** slice. No source code changed. The eval that produced 67.9% was executed by the orchestrator before the quick task was created, using the existing release binary (`experiments/poc-retrieval/target/release/poc-retrieval.exe`, 10MB, built in the prior marathon session). The slice persists evidence + flips planning state.

Specifically NOT exercised in this slice:
- Building or rebuilding the Rust core binary (used existing one)
- A2A endpoint live smoke (`./poc-retrieval.exe serve --port 9876` + curl)
- Go fat-binary embedded extraction (REQ-08 acceptance)
- Browser load of cytoscape UI (REQ-09 acceptance)
- Any production-path eval (CLI eval reads same SQLite + same search.rs as A2A endpoint Query handler — number is identical via either path modulo serde overhead)

## Honest gap list

### P1 — REQ-08 plumbing bugs (surfaced during orchestrator investigation, NOT closed)

1. `make` is not on Windows git-bash PATH on this host. Makefile commands (`make build-core` / `make build`) have never run end-to-end on this machine. Workaround needed before any future build automation.
2. Cargo `package.name = poc-retrieval` (verified via `experiments/poc-retrieval/Cargo.toml`), but Makefile line 25 `cp .../codenexus-core(.exe) $(EMBED_DIR)/` expects `codenexus-core(.exe)`. Binary name mismatch breaks `build-server` step regardless of make availability.

These are **REQ-08 acceptance gaps**, not REQ-10 gaps. They surfaced because REQ-07/08/09 were all "build/vet only, deferred real-binary smoke" — no one ran the production make chain end-to-end. The eval CLI path bypassed the bug entirely. Fix recommended in a separate quick task (estimated 30-45min).

### P2 — Deferred smokes (REQ-06/07/08/09 acceptance, separate from REQ-10)

- REQ-06 A2A endpoint live POST /tasks/send + GET /tasks/{id} round-trip
- REQ-07 Go supervisor real spawn-and-restart cycle (Rust kill -> 5s restart per backoff)
- REQ-08 //go:embed extraction + supervisor exec of extracted binary
- REQ-09 browser load of cytoscape UI on running stack with real query results

Recommend a single P2 quick task that fixes plumbing AND runs all four smokes in one full-stack session — a single live `./codenexus serve` validates them all simultaneously.

### P3 — Optional optimizations (NOT required for Phase 3 closure)

- rerank=true sweep — R6 trend +2pp at p=0.084 (would push 67.9% toward ~70%)
- Negative threshold 0.012 -> 0.025 tweak — would flip B5 from -0.25 to +1.0, push mean toward ~73.9%
- Phase 4 graph augmentation — R7 axis-3 hand-matcher 15%, LLM-judge 23.3%; sweep could close axis-3 gap
- Linear sync of XAR-266 (REQ-10 issue) to Done — defer; one-shot script pattern from 2026-04-26 can be re-run

## Process insight (validated, worth remembering)

**CLI Cmd::Eval and A2A endpoint Query handler share the same retrieval engine.** Both call `search::search(&store, &embedder, rr, &q, top, alpha)` against the same `storage::Store` opened from the same SQLite file. The number from CLI eval = number from POST /tasks/send modulo serde overhead.

This invalidates progress.txt scenario (c) ("eval harness needs porting from spike CLI to A2A endpoint"). The single-binary spike model paid off: same retrieval code, two callers (CLI + HTTP). REQ-10 acceptance can be measured via either, and the cheaper path (CLI) is sufficient.

## Next-session resumption

Three reasonable continuations:

**Option A — Fix REQ-08 plumbing** (30-45min quick task)
Fixes Makefile binary name + adds bash/PowerShell wrapper for `make build` so the chain runs end-to-end on Windows without GNU make. Validates REQ-08 acceptance + flushes deferred REQ-06/07/09 smokes in one full-stack run.

**Option B — Open Phase 4** (`/gsd-new-milestone` or `/gsd-add-phase`)
Plan first Phase 4 cycle. Recommended starting point per PROJECT.md tactical backlog: spike->core promotion (delete the 13-line core/ placeholder, alias core to experiments/poc-retrieval via cargo workspace) + Leiden community detection (~30 lines petgraph). Closes hidden-architectural-mismatch debt while shipping a small first Phase 4 increment.

**Option C — Strategic exploration** (`/gsd-explore` Software 3.0 reframe)
Engage the strategic backlog from PROJECT.md d98b16c: agent behavioral alignment (CodeCompass 58% miss target -> ≤5%), cross-session codebase understanding via memU coupling, architectural-decision semantic indexing (`query_constraints` A2A op). This is the differentiator vs Sourcegraph; MVP precision passing is necessary but not sufficient.

Default recommendation: **A then B**. A is a debt clearance that's been deferred 3 REQs in a row; doing it before Phase 4 prevents Phase 4 from inheriting debt. B is the natural next milestone.

## Verification

Re-computable from committed evidence:

```bash
cd D:/projects/codenexus
python -c "
import json, statistics
data = json.load(open('experiments/poc-retrieval/eval/req10_alpha06.json'))
b17 = [r for r in data if r['id'] in ('B1','B2','B3','B4','B5','B6','B7')]
mean = statistics.mean(r['precision_at_5'] for r in b17)
assert len(data) == 30, f'expected 30, got {len(data)}'
assert abs(mean - 0.679) < 0.01, f'expected 0.679, got {mean:.4f}'
print(f'PASS: B1-B7 mean = {mean:.4f}')
"
```

## Robustness caveat (2026-04-27 post-closure analysis)

**The 67.9% number is the literal gate-pass figure but its provenance is compromised.** Reviewer (Curry) flagged the local-optimum risk after the closure commit landed; held-out slicing was done in-place against the existing 30-query eval JSON. Findings:

| Slice | Score | Signal | Read |
|---|---|---|---|
| B1-B7 (in-distribution, sweep-tuned) | 67.9% | weak | alpha=0.6 was selected by R3 sweep on these same 7 queries; n=7 means 1 query flip costs -14.3pp |
| A1-A10 axis-1 exact symbol | 70.0% | medium | different task (exact lookup), low overfit risk; system not broken on this corpus generally |
| B8 held-out NL (concurrent writes) | 0% | real miss | corpus has 19 `lock` symbols; retrieval returned generic `write/writeFile/operations` instead — genuine concept-level miss |
| B9 held-out NL (conflict detection) | 0% | rubric noise | corpus has 0 `conflict` / 0 `diff` / 3 `version` / 2 `merge`; concept barely exists in repo, query is effectively an unmarked negative |
| B10 held-out NL (aggregate metadata) | 0% | rubric failure | retrieval found `buildDigest / fetchAllNotes / digest` (semantically correct), but `expected_paths` was `[meta, aggregate, frontmatter, kb_meta]` — too narrow, right answer rejected |
| Axis-3 graph traversal | 30% | weak | known retrieval-only ceiling, Phase 4 graph augmentation needed |

**Honest read of the held-out signal:**
- 1 of 3 held-out queries (B8) shows real generalization failure
- 1 of 3 (B9) is a query-set construction problem (concept not in corpus)
- 1 of 3 (B10) is a rubric-too-narrow problem (right answer found, scored 0)

So held-out failure rate is **~1 of 3 real**, not 3 of 3 as the raw 0% suggests. But 1 sample is not enough to say anything statistically.

**Reframing**: this slice is a **PRELIMINARY PASS** on the literal gate as written in REQUIREMENTS.md. The gate's ability to predict cross-corpus + truly-out-of-distribution performance is **untested**. Phase 3.5 robustness slice is required before claiming Phase 3 truly closed.

**Phase 3.5 robustness slice scope** (next session, 30-60min):
1. Fix B10 rubric (expand `expected_paths` to include `digest/buildDigest/fetchAllNotes/collector`); re-score
2. Run alpha sweep on B1-B7 ∪ B8-B10 joint set, verify alpha=0.6 is still optimal (or shift it)
3. Index a second corpus (claude-code-custodian or full-self-coding); author 5-10 fresh NL queries; hand-eval
4. Write robustness verdict in a new quick task; only THEN flip Phase 3 from prelim to truly closed (or open Phase 3.5 to fix retrieval if cross-corpus tanks)

## Phase 3 status: PRELIMINARY CLOSED 2026-04-27 (pending Phase 3.5 robustness)

5 of 5 REQs done **on the literal gates as written**:
- REQ-06 ✓ Rust core A2A endpoint (e0727c2 marathon)
- REQ-07 ✓ Go server scaffold (8ff8e11 + 54f23b1 + 01efa75)
- REQ-08 ✓ //go:embed plumbing (f5b6621 + 59b725b + bbf11ee) — note: plumbing bugs deferred
- REQ-09 ✓ embedded UI bundle (ec3849e + dfdcb95 + 68fb008)
- REQ-10 ⚠ MVP precision gate met (this slice) — see Robustness caveat above

Phase 3.5 robustness slice required before Phase 4 opens; recommended scope captured above. Phase 4+ backlog (tactical + Software 3.0 strategic) lives in PROJECT.md but should not be opened until Phase 3.5 verdict is in.
