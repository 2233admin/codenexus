# Phase 4 (First Slice): First-Run UX Cluster + P2 Resilience Same-Crate -- Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `04-CONTEXT.md` -- this log preserves the alternatives considered.

**Date:** 2026-04-28
**Phase:** codenexus-04-parity (first slice)
**Areas selected for discussion:** R1 fetch path, R5 retry refactor, E2E smoke harness, R4 server.rs threshold source (added in follow-up)
**Areas NOT selected (defaulted via PRE-PLAN-NOTES / SPEC):** None unselected at gray-area phase; R4 was added when user requested deeper discussion after the first batch.

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| R1 fetch path (Recommended) | hf-hub vs huggingface_hub_rust vs cargo-doc-then-decide | check |
| R4 server.rs threshold source | config.toml vs A2A envelope per-call vs hybrid vs hardcoded | check (added in follow-up) |
| R5 retry refactor shape | embed_query method vs embed_with_policy parameterized | check |
| E2E smoke harness shape & location | bash vs Rust #[test] vs hybrid; HTTPS_PROXY vs hosts vs Clash kill | check |

**Notes:** User selected 3 of 4 in initial multiSelect (skipped R4 = "go to PRE-PLAN-NOTES default"); after first-batch decisions captured, user asked to revisit R4 in follow-up.

---

## R1 fetch lib

| Option | Description | Selected |
|--------|-------------|----------|
| hf-hub (transitive dep) | 0 new top-level dep, fastembed already uses it; cache layout matches R1.c reload test; cost = manual file enumeration over 5 files (config/model.safetensors/tokenizer/tokenizer_config/special_tokens_map) via repo.get(). | |
| huggingface_hub_rust (NEW) | HF official 2026-04 release; snapshot_download(repo, revision) one-call returns dir; risk = 0.x version / license / maturity must verify. | check |
| cargo-doc-then-decide (Recommended) | Follow PRE-PLAN-NOTES Hint 1 decision criteria: planner does 5-min cargo doc check, prefer huggingface_hub_rust if clean, fall back to hf-hub otherwise. Aligns with informational-not-binding design intent. | |

**User's choice:** huggingface_hub_rust (pre-commit, overriding cargo-doc-then-decide recommendation)
**Notes:** User signal = preference for Python-convention parity API + HF-official maintenance (compatible with PROJECT.md `feedback_tool_selection_filter` "new > mature" rule). Pre-commit risk mitigated by binding-on-failure Plan-Time Gates G-01 (license), G-02 (version), G-03 (API existence) -- if any gate fails, planner falls back to hf-hub without re-asking user.

---

## R5 retry refactor shape

| Option | Description | Selected |
|--------|-------------|----------|
| embed_query() method (Recommended) | New method on Embedder impl with internal const QUERY_MAX_ATTEMPTS=2 + QUERY_DELAY_MS=250; search.rs:31 changes from embed(q,Role::Query) to embed_query(q). Aligns Q5=B mechanical, intent-at-call-site clear. | check |
| embed_with_policy parameterized | RetryPolicy { max_attempts, base_delay_ms, exponential } struct; future tunability without adding new methods, but introduces hidden state machine that may violate Q5=B mechanical lock. | |

**User's choice:** embed_query() method (Recommended)
**Notes:** Aligned with PRE-PLAN-NOTES Hint 3 recommendation. Q5=B mechanical-only spirit preserved.

---

## E2E smoke harness shape

| Option | Description | Selected |
|--------|-------------|----------|
| bash script (Recommended) | experiments/poc-retrieval/eval/e2e_first_run_smoke.sh; parallel to eval/req10_alpha06.json; cache rm -rf + HTTPS_PROXY env in bash is cleanest. | |
| Rust #[test] integration | tests/e2e_first_run.rs; type-safe but cache mutation pollutes sibling tests, env override needs serial_test crate, P2 complexity mismatch for first-slice mechanical. | |
| Hybrid (bash drives, Rust runs) | bash clears cache + sets env, Rust release binary runs the actual scenario. | check |

**User's choice:** Hybrid (bash drives, Rust runs)
**Notes:** User pre-committed to Hybrid over the workflow-recommended bash-only. Signal = willing to accept extra production texture in first slice (D-03b clarification follow-up locked Rust side = `./bin/codenexus query` not `cargo run` -- tests release artifact + `make build` + `//go:embed` chain).

---

## E2E network-block mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| HTTPS_PROXY=http://0.0.0.0:1 (Recommended) | PRE-PLAN-NOTES Hint 4 recommended; one-line export + one-line unset; CI / 5090 windows / Curry's Shanghai Clash machine all behave identically. | check |
| hosts file edit (0.0.0.0 huggingface.co) | Windows requires admin; Linux/macOS sudo; cleanup forgotten = host DNS pollution; CI hard to reproduce. | |
| kill Clash process | Only Curry's Shanghai box (Clash is the only huggingface.co route there); not portable; CI cannot reproduce. | |
| Multiple fallback chain | HTTPS_PROXY -> hosts -> kill Clash sequentially; redundancy increases verifier debug surface for no actual benefit. | |

**User's choice:** HTTPS_PROXY=http://0.0.0.0:1 (Recommended)
**Notes:** Aligned with PRE-PLAN-NOTES Hint 4. Portable, undoable, no privilege escalation.

---

## R4 server.rs:198 max_consecutive_fail threshold source (added in follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Hardcoded const = 5 (Q5=B aligned) | server.rs adds const MAX_CONSECUTIVE_FAIL: usize = 5; zero A2A schema change, zero config loading; most mechanical. Future tunability deferred to next slice. | |
| A2A envelope per-call override | PRE-PLAN-NOTES Hint 3 recommended; A2A Index task params extends with Optional<max_consecutive_fail>; server falls back to hardcoded 5 if absent. Pro: A2A clients tune per-batch. Con: schema surface + client must remember to set. | |
| config.toml startup value | server reads config.toml [embedder].max_consecutive_fail at startup (5 if absent). Mirrors main.rs CLI flag thinking. Restart-required to tune; not per-request. | |
| Hybrid (config default + envelope override) | config.toml or hardcoded provides default; A2A envelope allows per-request override. Most flexible but 2 sources of truth + priority logic. | check |

**User's choice:** Hybrid (config default + envelope override)
**Notes:** Second pick of Hybrid in this discussion (after E2E shape). Signal = accepts more plumbing for first-slice production texture, as long as architectural locks (Q5=B no enum, no tracing) hold. D-05 records priority order (envelope > config > hardcoded 5); D-05b adds Plan-Time Gate G-04 (A2A v0.2 spec must allow vendor params extension).

---

## Claude's Discretion

- Plan grouping (2 vs 3 plans): SPEC un-locks; planner default 3 (UX cluster / P2 patches / E2E harness), can collapse to 2 if R4/R5 trivially small.
- R4 envelope field naming (vendor-prefix `x-codenexus-max-consecutive-fail` vs free-form `max_consecutive_fail`): depends on Gate G-04 outcome.
- R2 messaging exact wording: SPEC R2.a verifier grep is the contract; phrasing is planner/executor's call within those grep constraints.

## Deferred Ideas

(None -- discussion stayed within first-slice scope.)

## Plan-Time Verification Gates (canonical list)

| Gate | What | Action on failure |
|------|------|-------------------|
| G-01 | huggingface_hub_rust license in {Apache 2.0, MIT, BSD} | Fall back to hf-hub (no re-ask user) |
| G-02 | huggingface_hub_rust version != pre-release / 0.0.x | Fall back to hf-hub (no re-ask user) |
| G-03 | huggingface_hub_rust exposes snapshot_download(repo, revision) -> PathBuf | Fall back to hf-hub if API materially missing (no re-ask user) |
| G-04 | A2A v0.2 spec allows vendor params extension on Index task envelope | Drop D-05 envelope-override layer; threshold source = (config.toml + hardcoded) only |
