---
frozen_at: 2026-05-02
status: SPEC (proposal; runs once prerequisites verified)
authority: Codex tactical pick 2026-05-02 ("First: (1) drift cheap probe") via session 019de7d2-b3a8-7081-883e-487cb353c88f
parent_audit: .planning/audits/2026-05-02-codex-strategic-review.md (Codex W1 weakness + I1 priority improvement: "Drift Evidence Gate")
governs: Whether 04.5-03 is a "Phase 5 precondition" or a "quality improvement"
type: cheap-probe-evidence-generation (feedback rule 36)
budget: 4-8 hours of focused session work; do NOT inflate
---

# Drift Evidence Probe -- 04.5-03 precondition test

## Purpose

The strategic zoom-out claims Phase 04.5-03 is a precondition for Phase 5
memory MVP because "without 04.5-03, the eval would measure noise (memory
annotations break when symbol identity drifts under refactor / re-index)."
That claim is unproven in the project's current evidence base. Codex W1
called this out as dependency inflation -- turning an implementation cleanup
into a strategic gate without quantifying whether the gate blocks the outcome.

This probe quantifies symbol identity drift on TODAY's pre-04.5-03 codebase
so the next strategic decision (sequencing 04.5-03 vs Phase 5 memory MVP)
can ride evidence, not assumption.

The probe runs against the EXISTING monolithic `graph_build.rs` indexer. It
does NOT validate that 04.5-03 fixes drift -- that validation rides 04.5-03's
G-D acceptance gate. This probe answers exactly one question: "Is current
drift bad enough that Phase 5 memory MVP would be undermined by it?"

## Scope (in)

- Re-index `poc.db` corpus 5 times pairwise drift measurement
- Re-index `fsc.db` corpus 5 times pairwise drift measurement
- Compare `symbols` table contents and `edges` table contents across runs
- Report 3 metrics: % stable symbol identity / % fallback-resolvable / %
  memory attachment loss (under a synthetic Phase-5-style attachment policy)

## Scope (OUT)

- Will NOT measure post-04.5-03 drift (that rides W5 G-D gate naturally)
- Will NOT measure embedding drift (Phase 03.6 cosine equivalence already
  proves embedding determinism; mean=0.9994, p10=0.9993)
- Will NOT modify indexer code (cheap probe, not feature work)
- Will NOT extend to multi-language (TS-only corpus today)
- Will NOT compare against alternative tools (graphify / code-review-graph)

## Prerequisites (verify before running, ABORT if any miss)

| Prereq | Verification command (run from D:/projects/codenexus) | Pass evidence |
|--------|------------------------------------------------------|---------------|
| poc-retrieval cargo workspace builds clean | `cargo build --release -p codenexus-core --bin codenexus-core` | exit code 0, binary at `experiments/poc-retrieval/target/release/codenexus-core(.exe)` |
| poc.db source corpus is locally available | identify the obsidian-llm-wiki source dir from `.planning/research/` or `.planning/spikes/001-*` (the corpus poc.db indexes) | source dir path exists; `ls` returns >=50 .ts files |
| fsc.db source corpus is locally available | identify the full-self-coding source dir from progress.txt or PROJECT.md "Quick Tasks Completed" entries | source dir path exists; `ls` returns >=100 .ts files |
| HF cache pre-seeded for embedder (per Phase 03.6 + Phase 4 first slice docs) | `ls ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3/` | model.safetensors + tokenizer.json + config.json present |
| Working dir has >=10 GB free (5 db copies x 2 corpora x ~1 GB each in worst case) | `df -h $(pwd)` | available column shows >10 GB |

If any prereq fails, document the gap in this file's "Prereq failure log"
section (added at run time) and switch to Codex's flip-condition: "do (2)
EVAL-CONTRACT v1.1 amendment proposal second immediately" instead of (1).

## Run matrix (exact)

```
For each corpus C in [poc, fsc]:
  For each run R in [r1, r2, r3, r4, r5]:
    1. Backup any existing C.db to C.db.preprobe.bak (once per corpus, before r1)
    2. cargo run --release -p codenexus-core -- index \
         <C source dir> \
         --db <C>.db.<R> \
         --max-consecutive-fail 5
    3. Wait for completion (poc ~5min/run; fsc ~8-10min/run per Phase 03.6 history)
    4. Capture per-run JSON dump:
       sqlite3 <C>.db.<R> -json \
         "SELECT id, file, name, kind, line FROM symbols ORDER BY file, name, line" \
         > <C>.<R>.symbols.json
       sqlite3 <C>.db.<R> -json \
         "SELECT src_id, dst_id, kind, confidence FROM edges ORDER BY src_id, dst_id, kind" \
         > <C>.<R>.edges.json
       (NOTE: schema field names may differ from above -- verify with .schema first
        and adapt the SELECT to actual column names. If `alias_decls` table exists
        post-W0, ALSO dump it. If not, skip.)

After all 10 runs (5 per corpus) complete:
  Run drift_compare.py (separate spec; see "Output schema" below for input/output)
```

Total runtime estimate: 5 * 5 = 25 min for poc + 5 * 8.5 = 42 min for fsc =
~67 min total wall-clock. Plus ~30 min for analysis script + writeup. Total
~1.5-2 hr -- inside Codex's 4-8 hr probe budget.

## Metrics (exact definitions)

For each adjacent run pair `(R_i, R_i+1)` per corpus:

**M1 -- % stable symbol identity (path+name+kind level):**
```
let A = set of (file, name, kind) tuples in run R_i
let B = set of (file, name, kind) tuples in run R_i+1
M1 = |A intersect B| / |A union B|         # Jaccard similarity
```
Symbol identity at the (file, name, kind) level is what memU bridge would
attach `remember_symbol_note` to. If M1 < 1.0 across runs of the SAME
unchanged source, the indexer is non-deterministic in a way memory MVP
cannot tolerate.

**M2 -- % stable rowid (true symbol_id stability):**
```
let A_id = map (file, name, kind) -> id from run R_i
let B_id = map (file, name, kind) -> id from run R_i+1
M2 = (count of keys K in (A_id intersect B_id) where A_id[K] == B_id[K]) /
     |A_id intersect B_id|
```
Even if (file, name, kind) is stable, rowid ordering may not be. A memU bridge
keying on rowid would break under M2 < 1.0; one keying on (file, name, kind)
would survive.

**M3 -- % stable edge resolution:**
```
let E_A = set of (src_file, src_name, dst_file, dst_name, kind) tuples in R_i
       (resolved by joining edges to symbols on src_id and dst_id)
let E_B = same in R_i+1
M3 = |E_A intersect E_B| / |E_A union E_B|
```
Edge resolution stability is what makes `query_constraints` and
`get_edit_context` ops reproducible. If M3 << M1, the resolver is the unstable
layer (which is exactly what 04.5-03 rewrites).

**M4 -- T3+T4 PINNED bug stability check (sanity):**
```
For both R_i and R_i+1:
  Run cargo test -p codenexus-core graph_build::tests::T3
  Run cargo test -p codenexus-core graph_build::tests::T4
M4 = both T3 + T4 PASS (PINNED bug behavior preserved) on every run
```
Sanity bound: confirms the probe environment is healthy. If M4 fails, abort
probe -- something else changed about the test setup, not the indexer.

**M5 -- synthetic memory attachment loss:**
```
Pretend a Phase 5 memU bridge attached `remember_symbol_note` to 30 random
symbols after run R_1. Compute fraction of those notes that would still
correctly attach after R_2..R_5 under three keying policies:
  - rowid only:                    M5_rowid    = (1 - drift)
  - (file, name, kind):            M5_fnk      = (1 - drift)
  - (file, name, kind) + path-aware fallback if path renamed:
                                   M5_fnk_fb   = (1 - drift)
M5 = report all three; pick worst as headline number for decision rule
```
This is the load-bearing metric. If M5 < 90% under (file, name, kind), then
04.5-03 IS a Phase 5 precondition. If M5 >= 95% under (file, name, kind),
then the audit's claim ("would measure noise") is overcalibrated and 04.5-03
demotes to "quality improvement."

**M6 -- absolute symbol count delta:**
```
M6 = |B| - |A|         # signed; should be 0 across runs of unchanged source
```
Should be 0 if indexer is deterministic. Non-zero is a red flag to investigate.

## Output schema

Single JSON file at `experiments/poc-retrieval/eval/drift_evidence_probe_results.json`:

```json
{
  "probe_version": "1",
  "ran_at": "<UTC ISO 8601>",
  "ran_against_commit": "<HEAD SHA at probe time>",
  "indexer_runtime_total_min": 67.4,
  "corpora": {
    "poc": {
      "runs": ["poc.db.r1", "poc.db.r2", "poc.db.r3", "poc.db.r4", "poc.db.r5"],
      "pairs": [
        {
          "from": "r1", "to": "r2",
          "M1_jaccard_fnk": 0.998,
          "M2_rowid_stable_among_matched": 0.92,
          "M3_edge_resolution_stable": 0.94,
          "M4_t3_t4_pinned_pass": true,
          "M5_attachment": {
            "rowid_only": 0.92,
            "fnk": 0.998,
            "fnk_with_path_fallback": 0.999
          },
          "M6_count_delta": 0
        },
        ... (4 more pairs: r2-r3, r3-r4, r4-r5, r1-r5 long-range)
      ],
      "summary": {
        "M1_min": 0.997,
        "M5_fnk_min": 0.996,
        "M5_rowid_min": 0.91,
        "M6_max_abs": 0
      }
    },
    "fsc": { ... same shape ... }
  },
  "decision": "<see decision rule below>",
  "decision_evidence": "<one paragraph explaining which thresholds tripped>",
  "next_actions": [
    "<one or more action items derived from the decision>"
  ]
}
```

## Pass / fail interpretation per metric

| Metric | "Healthy" range | "Concerning" range | "04.5-03 IS necessary" range |
|--------|----------------|---------------------|-------------------------------|
| M1 (Jaccard fnk) | >= 0.999 | 0.99 - 0.999 | < 0.99 |
| M2 (rowid stable) | >= 0.95 | 0.85 - 0.95 | < 0.85 (memU keying on rowid would break; doc this as gotcha) |
| M3 (edge stable) | >= 0.99 | 0.95 - 0.99 | < 0.95 |
| M5 fnk | >= 0.99 | 0.95 - 0.99 | < 0.95 |
| M5 rowid | (informational; not gating) | n/a | n/a |
| M6 count delta | 0 always | rare 1-2 | persistent non-zero |

## Decision rule

```
IF M5_fnk_min (worst across all pairs and corpora) >= 0.99
   AND M3_min >= 0.99
   AND M6_max_abs == 0
THEN
  decision = "04.5-03 demotes to QUALITY IMPROVEMENT"
  next_actions = [
    "Update STATE.md and ROADMAP.md to reflect that 04.5-03 is no longer
     gating Phase 5 memory MVP",
    "Re-evaluate Codex 6-week cadence: Phase 5 memory MVP can start in
     parallel with 04.5-03, not sequentially after",
    "EVAL-CONTRACT v1.1 amendment proposal becomes higher priority"
  ]

ELIF M5_fnk_min < 0.99 OR M3_min < 0.99
THEN
  decision = "04.5-03 CONFIRMED PHASE 5 PRECONDITION"
  next_actions = [
    "Proceed with W0 execution as planned",
    "Document the empirical drift evidence in 04.5-03 SUMMARY when it ships",
    "Audit's load-bearing premise validated; ROADMAP cadence stands"
  ]

ELIF probe runtime exceeded 8 hours OR prereqs failed
THEN
  decision = "PROBE INCOMPLETE -- defer drift evidence question"
  next_actions = [
    "Document blocker in this file's 'Prereq failure log' section",
    "Flip to Codex's stated flip-condition: do EVAL-CONTRACT v1.1 amendment
     proposal next instead",
    "Re-attempt drift probe after blocker resolved"
  ]
```

## Acceptance criterion (for the runner agent)

A subsequent agent (Claude or executor sub-agent) can produce a runnable
script `experiments/poc-retrieval/scripts/drift_evidence_probe.sh` (or .ps1)
directly from this spec without:

- Asking Curry which corpora to use (poc + fsc are named here)
- Asking Curry which metrics to compute (M1-M6 are defined here)
- Asking Curry what the decision rule is (encoded above)
- Asking Curry where to write output (path named here)
- Asking Curry whether to commit results to git (results are evidence; commit
  to `.planning/probes/runs/<UTC date>-drift-evidence.md` as a SUMMARY +
  attach the JSON; do NOT commit the per-run db files)
- Re-reading the audit / EVAL-CONTRACT / PROJECT.md for context

The runner agent's only legitimate "ask Curry" cases are:
- A prereq fails and the workaround is non-trivial
- One of the corpora's source dir cannot be located (the spec didn't pin the
  exact path because progress.txt referenced it inconsistently across entries)
- A metric definition produces NaN or unbounded result on real data
  (defensive: spec assumes the indexer is deterministic enough that ratios
  are well-defined; if not, that IS the result)

## Prereq failure log (filled in at run time, empty until probe is attempted)

(Empty)

## Run log (filled in at run time)

(Empty)

## Decision (filled in after run)

(Empty)
