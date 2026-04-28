---
phase: 4
slice: first-slice-ux-resilience
sessions:
  - session_id: morning-0726
    reviewed_at: 2026-04-28T07:26:14
    reviewers: [gemini, codex]
    reviewers_failed: [opencode, claude]
    consensus_classification: SPLIT
    recommendation: "Replan via /gsd-plan-phase 4 --reviews"
  - session_id: rerun-1303
    reviewed_at: 2026-04-28T13:03+08:00
    reviewers: [gemini, codex]
    reviewers_failed: [opencode]
    consensus_classification: CONFLICT
    recommendation: "HALT-AND-REPLAN — new HIGH findings strengthen morning's diagnosis"
plans_reviewed:
  - 04-01-PLAN.md
  - 04-02-PLAN.md
  - 04-03-PLAN.md
plans_replanned_after_morning_review: false
plans_unchanged_since: 2026-04-28T07:10
combined_high_findings_count: 10
high_findings_orchestrator_verified: 2  # Makefile mismatch + fastembed from_hf re-fetch
final_verdict: HALT-AND-REPLAN
---

# Cross-AI Plan Review — Phase 4 First Slice

## RE-REVIEW SUPPLEMENT — 2026-04-28T13:03 (rerun-1303 session)

**Why this supplement exists:** Morning's review (committed at SHA `6110ae6`, 07:26:14) recommended REPLAN. Plans were not replanned in the intervening hours. User invoked `/gsd-review --phase 4 --all` again at 13:00. Re-running reviewers produced **convergent REPLAN verdict with NEW HIGH findings** — the morning Codex run and current Codex run found largely DIFFERENT HIGH issues (different agentic exec paths into different parts of the source tree), but both ended at HALT/REPLAN.

This is consistent with the CCG adversarial-review gate's purpose: stochastic agentic exec catches different blind spots each run; the **union** of findings across runs is the load-bearing evidence.

### Convergence summary (across both sessions)

| HIGH Finding | Morning (07:26) | Current (13:03) | Orchestrator-verified |
|---|---|---|---|
| 04-03 silently defers R4.b/R5.b synthetic tests | ✅ flagged | ✅ flagged | n/a (plan text reads) |
| 04-03 destructive cache mutation (`rm -rf blobs`) | HIGH | MEDIUM (downgraded) | n/a |
| 04-03 pre-index ordering bug | HIGH | not flagged | n/a |
| 04-01 `snapshot_dir()` under-specified | HIGH | MEDIUM (downgraded) | n/a |
| **R1 fastembed `from_hf` re-fetches from `main`** | ❌ missed | ✅ flagged | ✅ confirmed at `qwen3.rs:1014` |
| **D-06: hf-hub 0.5 DOES expose `download_with_progress<P>`** | ❌ missed | ✅ flagged | ✅ confirmed at `sync.rs:766-799` |
| **Makefile binary name mismatch (`codenexus-core` vs `poc-retrieval`)** | ❌ missed | ✅ flagged | ✅ confirmed `Makefile:4` vs `Cargo.toml:2`; actual built binary is `poc-retrieval.exe` |
| **04-03 harness uses Go CLI signature against Rust CLI semantics** | ❌ missed | ✅ flagged | not directly verified by orchestrator |
| **Rust `/tasks/send` ↔ Go A2A client wire format incompatible** | ❌ missed | ✅ flagged | not directly verified by orchestrator |
| SHA provenance fragility (cache+timestamp ≠ baseline-identity proof) | HIGH | not flagged | n/a |

**Two HIGH findings independently verified by the orchestrator** before composing this supplement (R1 false-pass mechanic + Makefile broken pre-flight). Both are **structural false-pass scenarios**: the plan would compile clean, pass all greps, and silently fail the actual contract.

### What's new vs morning (the load-bearing additions)

Two findings the morning Codex run missed entirely, both **verified-correct by source-code inspection**:

**(1) R1 SHA pin is decorative, not functional.** `Qwen3TextEmbedding::from_hf` at `fastembed-5.13.3/src/models/qwen3.rs:1002-1014` constructs `let repo = api.model(repo_id.to_string());` — meaning whatever path you pass (local snapshot dir or repo id), it gets wrapped in a fresh unpinned `api.model(...)` call that re-fetches from `main`. Plan 04-01's `snapshot_dir()` helper correctly downloads the pinned-revision files into the cache, but `from_hf` then ignores that work and fetches `config.json` from default `main`. **The entire purpose of R1 (supply-chain control) does not get delivered.** Plan needs R1 redesign to either:
  (a) construct `Qwen3TextEmbedding::new(model, tokenizer)` directly from local files (bypassing `from_hf`),
  (b) fork/patch fastembed-rs to honor `Repo::with_revision`, or
  (c) verify whether `Qwen3TextInitOptions` exposes a `revision` parameter that fastembed honors (Codex did not confirm; planner must verify via cargo doc).

**(2) `make build` fails before E2E starts.** `Makefile:4` declares `CORE_BIN := codenexus-core` and `Makefile:25` does `cp .../target/release/$(CORE_BIN)$(...).exe ...`. But `experiments/poc-retrieval/Cargo.toml:2` declares `name = "poc-retrieval"` and `[[bin]] name = "poc-retrieval"` at line 45-47. Actual built artifact: `experiments/poc-retrieval/target/release/poc-retrieval.exe`. The Makefile's `cp` references a file that does not exist — `make build-server` fails at that step. Plan 04-03 Task 1 has a `make build` fallback (line 148-154 of 04-03-PLAN.md) which therefore cannot succeed on a clean build. This is a **pre-existing project bug** that Plan 04-03 implicitly assumes is fixed.

Plus four other new HIGH findings from current Codex run (Go CLI ≠ Rust CLI signatures, Go ↔ Rust A2A wire format incompatible, D-06 progress callback claim factually wrong, R4.b/R5.b dishonestly downgraded — see verbatim Codex review below).

### Current run — Codex verbatim (rerun-1303)

#### Summary
SHOULD HALT. The plan has good scope discipline, but 04-01's core R1 implementation is based on a false fastembed assumption, and 04-03's release-artifact E2E path is incompatible with the current Go/Rust wiring. These are not polish defects; executor will either compile/run but still fetch HEAD, or fail before reaching the intended acceptance gates.

#### Strengths
- G-02 fallback to `hf-hub` is correct: `Cargo.toml:32` already declares `hf-hub = "0.5"`.
- `Embedder::new()` exists at `experiments/poc-retrieval/src/embedder.rs:57`, so the proposed `embed_query_works` constructor call is valid.
- `Role` removal from `search.rs` is valid after switching line 31: current only use is `search.rs:4` import + `search.rs:31` `Role::Query`.
- `server.rs:64-66` does map `dispatch` `Err` into `TaskStore::fail`, and `task_state.rs:65-70` stores failed state + error string.
- Bound check `1..=1000` in 04-02 rejects `Some(0)` as planned (`04-02-PLAN.md:365-380`).
- Wave sequencing is justified: 04-01 and 04-02 both touch `embedder.rs` insertion points.

#### Concerns
- **HIGH** R1 implementation cannot work as written — `04-01-PLAN.md:199-218` passes a local snapshot path to `Qwen3TextEmbedding::from_hf`, but fastembed 5.13.3 treats the argument as a Hub repo id and immediately calls `api.model(repo_id.to_string())` at `.../fastembed-5.13.3/src/models/qwen3.rs:1002-1014`. It does not load from a local directory. This means the SHA pin wrapper is bypassed or fails by constructing a bogus HF repo from a filesystem path.

- **HIGH** R1 "snapshot_dir + from_hf" double-fetches/wrong-revision risk — even after `hf-hub::Repo::with_revision` downloads the pinned files, fastembed's `from_hf` builds a fresh unpinned `api.model(repo_id)` at `qwen3.rs:1010-1018` and fetches `config.json` from default `main`. The plan does not actually pin the model load path.

- **HIGH** D-06 progress decision is factually wrong — `04-01-PLAN.md:60` and `04-01-PLAN.md:306` say hf-hub exposes no programmable callback. Local hf-hub 0.5.0 exposes `download_with_progress<P: Progress>` at `.../hf-hub-0.5.0/src/api/sync.rs:766-799`. Per SPEC trigger, R2(c) should be promoted or the plan must explicitly choose not to.

- **HIGH** `make build` is already broken for current binary names — Makefile copies `target/release/codenexus-core(.exe)` at `Makefile:4` and `Makefile:25`, but Cargo builds `poc-retrieval` per `experiments/poc-retrieval/Cargo.toml:45-47`. Current `target/release` contains `poc-retrieval.exe`, not `codenexus-core.exe`. 04-03 will fail at `make build` before E2E.

- **HIGH** 04-03 uses the Go CLI as if it were the Rust CLI — `04-03-PLAN.md:174`, `192`, `233`, `261` call `$BIN index --repo ... --db ...` / `$BIN query ... --db ...`. Go CLI syntax is `index <repo>` and requires a running `codenexus serve` lockfile (`server/cmd/index.go:17-29`); query requires `--repo-hash`, not `--db` (`server/cmd/query.go:31-40`). The harness cannot run.

- **HIGH** Go A2A client and Rust A2A server wire formats are incompatible — Rust `/tasks/send` expects `{ "operation": ... }` via `TaskSendBody` at `experiments/poc-retrieval/src/a2a.rs:141-142`. Go sends `{task_id, skill_id, messages[].parts[].data}` at `server/internal/proxy/a2a.go:38-41` and `:208-214`. Rust response uses `id`, not `task_id` (`a2a.rs:23-34`), while Go polls fallback client-generated ID (`proxy/a2a.go:238-255`). 04-03's "release artifact" path is not currently viable.

- **HIGH** Required R4.b/R5.b tests are silently downgraded — SPEC requires synthetic-failure tests, but 04-03 explicitly excludes them at `04-03-PLAN.md:123-128` and `:290`, then allows DEFERRED in closure at `:408-410`. That contradicts Phase acceptance, not just implementation detail.

- **MEDIUM** Eval jq filter is wrong for actual baseline JSON — `req10_alpha06.json` is a top-level array, not `{summary, results}`. The planned filter at `04-03-PLAN.md:350-362` indexes `.summary`/`.results`; use `if type=="array" then map(.precision_at_5)|add/length else ... end`.

- **MEDIUM** `snapshot_dir()` root detection is fragile — `04-01-PLAN.md:149-195` only computes root from the first fetched file. Today first file is top-level `config.json`, so nested `1_Pooling/config.json` is not actually validating the depth logic. Add assertions that every returned path is under the same `snapshots/<QWEN3_REVISION>` root.

- **MEDIUM** Cache deletion is too destructive — `04-03-PLAN.md:188` and `:229` remove `$HF_CACHE_DIR/blobs`, which can break other snapshots/revisions for the same model. R1.c only needs the target snapshot; full-blob eviction should be verifier-only and opt-in.

- **MEDIUM** `OnceLock` lazy load has a duplicate-load race — `embedder.rs:63-76` does check-then-load-then-`set`; concurrent first queries can each load/download the 1.2GB model, with losers discarded. Use `get_or_try_init` equivalent or a `Mutex<Result>` guard if server concurrency matters.

- **MEDIUM** A2A threshold override not plumbed through Go — Rust enum extension in `a2a.rs:56-59` does not update Go `IndexRepoArgs` at `server/internal/proxy/a2a.go:81-85` or CLI `server/cmd/index.go:36-39`. Direct Rust A2A clients can send it; the advertised fat-binary Go CLI cannot.

- **LOW** Threat model overclaims SHA protection — `04-01-PLAN.md:481-482` says SHA pin prevents substituted weights. hf-hub validates etag/content-addressed blobs, but the planned fastembed second fetch from unpinned `main` invalidates that claim until R1 is redesigned.

- **LOW** E2E "query returns results" can pass empty corpus setup — `04-03-PLAN.md:160-166` says fallback corpus may produce 0 symbols and "empty results" are fine. That no longer tests user-visible query success, only model loading.

#### Suggestions
- Replace 04-01 R1 design. Either fork/patch fastembed to accept `Repo::with_revision`, or manually construct `Qwen3TextEmbedding::new(model, tokenizer)` using local files. Do not pass snapshot paths to `from_hf`.
- If staying with hf-hub, use `ApiRepo::download_with_progress` and a small `Progress` implementation; update R2(c) instead of deferring.
- Fix packaging before 04-03: align Cargo bin name with `codenexus-core`, or change `Makefile:25` and supervisor expectations together.
- Rewrite E2E harness around actual current interfaces: either Rust `target/release/poc-retrieval.exe query --db ...`, or start `./bin/codenexus serve` then use Go `index <repo>` and `query <text> --repo-hash ...`.
- Fix Rust/Go A2A envelope mismatch before claiming release-artifact E2E coverage.
- Add real failure injection for R4.b/R5.b, e.g. env-gated `CODENEXUS_EMBED_FAIL=always|once` in `embed_once`, guarded as test/dev-only if needed.
- Change eval mean command to handle top-level arrays: `jq '[.[].precision_at_5] | add / length'` for current files.
- Limit cache mutation to `$HF_CACHE_DIR/snapshots/$PINNED_SHA`; make blob eviction a clearly named `--destructive-redownload` mode.

#### Risk Assessment
**HIGH** — the plan's central pinning mechanism is technically invalid against fastembed's actual API, and the closure harness is aimed at a Go CLI/build path that is currently incompatible with the Rust core binary name and A2A schema. Proceeding would generate false confidence: grep checks can pass while the model load still uses unpinned `main`, and 04-03 cannot exercise the advertised release path.

#### Verdict
**HALT**

### Current run — Gemini verbatim (rerun-1303)

#### Summary
**STRONG.** The implementation plan for CodeNexus Phase 4 (First Slice) is technically sound, rigorously aligned with the SPEC, and demonstrates high defensive engineering standards. The transition from the planned `huggingface_hub_rust` to the mature `hf-hub` 0.5 (transitive via `fastembed-rs`) is a prudent fallback that minimizes dependency bloat while satisfying the SHA pinning requirement. The hybrid approach to threshold configuration in the A2A handler (envelope > hardcoded) is well-balanced. The inclusion of a release-binary E2E harness (`e2e_first_run_smoke.sh`) and an eval no-regression gate provides critical validation that unit tests would miss, specifically regarding the `//go:embed` packaging chain and model weight consistency.

#### Strengths
- **Rigorous Version Pinning**: Correctly identifies `97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3` as the canonical SHA and integrates it into both the Rust core and the `ARCHITECTURE.md` §9.8 protocol.
- **Defensive API usage**: The `snapshot_dir()` helper in `embedder.rs` correctly handles nested file structures (e.g., `1_Pooling/config.json`) by using a depth-aware `parent()` walk to find the snapshot root.
- **UX-First Retry Logic**: The introduction of `embed_query()` in Task 04-02 explicitly decouples the interactive query budget (≤500ms) from the heavy indexing budget (~7.75s), preventing search-path stalls.
- **Actionable Recovery Path**: Task 04-03's E2E harness specifically verifies the visibility of `docs/embedder-offline-bootstrap.md` in the failure path.
- **Protocol Compliance**: Strictly adheres to the version-hash history protocol in `ARCHITECTURE.md` §9.8.

#### Concerns
- **MEDIUM** Snapshot File List Fragility — Plan 04-01 Task 1 — Hardcoded 9-file list might fail if future fastembed needs more metadata files. Suggest fetching one file and using its parent.
- **MEDIUM** A2A JSON Deserialization — Plan 04-02 Task 2 — `#[serde(default)]` on `Option<usize>` correctly returns None on missing; verify it also handles explicit `null` from Go server.
- **LOW** E2E Harness Disk Space — `rm -rf blobs` forces 1.2GB download every run; suggest "local-only" skip flag.
- **LOW** Shell Portability — `set -o pipefail` is bash-specific.
- **LOW** Wait-for-Previous on Sequential Tools — executor must enforce `wait_for_previous: true` for shared-file edits.

#### Suggestions
- Resilient snapshot path: fetch `config.json` first, return `p.parent()`.
- Bound error detail: include `[1, 1000]` in error string.
- E2E cleanup: ensure trap handles SIGINT/SIGTERM specifically.

#### Risk Assessment
**LOW** — mature libraries, disjoint regions, sequential waves, high-fidelity E2E + Eval verification.

#### Verdict
**PROCEED**

### OpenCode (rerun-1303): FAILED again with auth error

`Invalid token (request id: 202604280459478380405558268d9d6EhrY8AZW)` — same auth-pool issue as morning. Per `feedback_no_autonomous_provider_swap`, did NOT attempt fix. User can refresh OpenCode auth and re-run if desired.

### Updated CCG consensus classification (rerun-1303)

**CCG = CONFLICT** (stronger than morning's SPLIT). Verdicts directly oppose: Codex HALT vs Gemini PROCEED. Two HIGH findings verified by orchestrator source-code inspection. The disagreement is grounded in evidence Codex's agentic exec mode collected (file reads into fastembed/hf-hub/Go CLI source) that Gemini's text-reasoning mode cannot produce.

### Updated recommendation (combining both sessions)

**HALT-AND-REPLAN, with concrete plan-text changes spanning BOTH sessions' findings:**

#### Top priority — false-pass blockers (must fix before any execution)

1. **R1 redesign** — current design is decorative. Pick one:
   - (a) `Qwen3TextEmbedding::new(model, tokenizer)` constructed from local snapshot files, bypassing `from_hf` entirely
   - (b) fork fastembed-rs to honor `Repo::with_revision` on `from_hf` (heavier; upstream divergence)
   - (c) verify whether `Qwen3TextInitOptions::revision` exists in fastembed 5.13.3 (Codex did not confirm; planner must check via cargo doc)

2. **Pre-flight Makefile reconciliation** — either rename Cargo `[[bin]]` `name = "poc-retrieval"` → `"codenexus-core"` (Cargo.toml + every reference site) OR change `Makefile:4` to `CORE_BIN := poc-retrieval` and update embed paths. Pick one and land it as Plan 04-00 OR Plan 04-01 Task 0 BEFORE the substantive work.

3. **04-03 harness rewrite** — use `target/release/poc-retrieval.exe query --db ...` directly, skipping the broken Go CLI chain. Codex's morning suggestion of isolated `HF_HOME=$(mktemp -d)` should also land — both protect the user's normal HF cache AND fix the pre-index ordering bug.

#### Closure honesty — synthetic tests

4. **R4.b / R5.b synthetic-failure tests** — implement env-gated fault injection (`CODENEXUS_EMBED_FAIL=always|once|after_N`) in `embed_once`, then add the SPEC-required synthetic tests using it. Don't silently DEFER. (Both sessions flagged this as HIGH — convergent.)

#### Plan accuracy fixes

5. **D-06 R2(c) progress** — promote since `download_with_progress<P: Progress>` exists at `hf-hub-0.5.0/src/api/sync.rs:766-799`. Plan 04-01's claim that "no programmatic callback exists" is factually wrong.

6. **`snapshot_dir()` per-file path validation** — `assert!(snapshot.ends_with(QWEN3_REVISION))` and `for path in fetched_paths { assert!(path.starts_with(&snapshot)); }` (Codex morning + current both flagged the under-specification).

7. **Cache eviction scope** — limit `rm -rf` to `$HF_CACHE_DIR/snapshots/$PINNED_SHA`; do NOT touch `blobs/`. Or better: isolated `HF_HOME=$(mktemp -d)` for the harness.

8. **Eval jq filter** — `req10_alpha06.json` is a top-level array; use `jq '[.[].precision_at_5] | add / length'` not `.summary.mean_precision_at_5`.

9. **Eval gate granularity** — n=7 queries × top-5 = 35 judged slots; 1 result shift = ~2.86pp; current ±2pp gate is below metric granularity. Either tighten to deterministic equality OR widen to ±5pp with explicit acknowledgment.

10. **`OnceLock` race** — switch to `OnceLock::get_or_try_init` or `Mutex<Result<...>>` guard.

#### Style / less critical

11. **04-02 G-04 rationale rewrite** — clarify this is `OperationRequest::IndexRepo` schema extension, NOT A2A metadata pass-through.
12. **`1..=1000` bound** — change to `1..=100` or document why 1000.
13. **ARCH §9.8 row wording** — fix "version-hash-affecting" / "no version-hash change" contradiction.
14. **`refs/main` instructions in offline doc** — clarify cache-internal vs user-facing.

#### Verifier-time only (no plan change needed)

15. `merges.txt` / `vocab.json` actually required by fastembed Qwen3 loader — sanity-check during R1 implementation
16. `Role` unused import cleanup in `search.rs`

---

# ORIGINAL REVIEW — 2026-04-28T07:26 (morning-0726 session, preserved for audit)

**Reviewers attempted:** 4 (gemini, claude, codex, opencode)
**Reviewers succeeded:** 2 (gemini, codex)
**Coverage:** PARTIAL (per ccg-consensus-gate rules — minimum threshold of 1 met, 2 reviewers gives meaningful adversarial coverage)

---

## Gemini Review (4153 bytes)

**Overall verdict:** Sound and technically rigorous. Risk: LOW.

### 1. One-paragraph Summary

The plans are sound and technically rigorous, demonstrating a high level of situational awareness regarding the project's current state (e.g., the `eprintln!` vs `tracing` correction). The decomposition into UX Cluster, P2 Resilience, and E2E Harness is logical. However, Plan 04-02 introduces an "envelope override" feature (R4) that is functionally dead-code in this slice because the Go service layer is out-of-scope and cannot yet populate the new field. Despite this, the plans effectively address the "first-impression" risk identified in Phase 03.6 while hardening the internal resilience of the Rust core.

### 2. Strengths

- **Protocol Discipline:** Plan 04-01 Task 3 strictly follows the ARCH §9.8 version-hash protocol, ensuring every supply-chain change is audited even if it doesn't trigger a re-index.
- **Fail-Fast Gating:** The use of Plan-Time Gates (G-01..G-04) for library selection (`hf-hub` fallback) prevents the "library-of-the-month" trap and ensures license compliance before a single line of code is written.
- **Defensive Harnessing:** Plan 04-03 Task 1 uses `HTTPS_PROXY=http://0.0.0.0:1` for network blocking. This is a brilliant, cross-platform, non-privileged way to simulate failure without messy `/etc/hosts` edits or process-killing.
- **Packaging Awareness:** Plan 04-03 D-03b correctly identifies that testing the `make build` artifact is the only way to verify the `//go:embed` chain, which is a common point of failure for fat-binary apps.

### 3. Concerns

- **R4 Envelope Override is Dead Code (MEDIUM):** Plan 04-02 Task 2/3 implements the A2A envelope override for `max_consecutive_fail`. Since the Go service layer (the A2A client) is Out-of-Scope (SPEC §Boundaries), this field will never be `Some` in real usage until a future Go-side update. The feature is "plumbed but unreachable." *Context:* Plan 04-02 Task 3.
- **SHA Recovery Fragility (MEDIUM):** The SHA `97b0c614...` is recovered from a local cache timestamp. If this SHA does not exactly match the model used for the 03.6 baseline, Task 2 of Plan 04-03 will trigger a P0 failure. *Context:* Plan 04-01 Task 1.
- **Nested Path Assumptions in hf-hub (LOW):** Plan 04-01 Task 1 `snapshot_dir` helper assumes `1_Pooling/config.json` parent popping logic works. While the math (`depth + 1`) is correct, it assumes `hf-hub` creates the directory structure precisely as expected.
- **Arbitrary Bound Check (LOW):** The `1..=1000` check in `server.rs` (Plan 04-02 Task 3) is a reasonable DoS mitigation (T-04-06) but lacks a technical justification for the "1000" ceiling beyond "it feels safe."
- **Statistical Significance (LOW):** Comparing mean precision on n=7 queries with a +/-2pp gate (Plan 04-03 Task 2) is barely a validation of weight-identity; it's more of a "did I accidentally load BERT-base?" check.

### 4. Suggestions

- **Document the Go-Gap:** In `04-02-SUMMARY.md`, explicitly note that the A2A `max_consecutive_fail` field requires a corresponding update in the Go `OperationRequest` struct and CLI/API layer before it becomes functional.
- **File List Verification:** Plan 04-01 Task 1 `FILES` list: ensure `merges.txt` and `vocab.json` are actually required by the `fastembed` Qwen3 loader. If they are missing from the fetch list but required by the model, the load will fail.
- **Unused Import:** Plan 04-02 Task 1: Ensure `Role` is removed from `search.rs` imports to keep the build warning-free as per the plan objective.

### 5. Risk Assessment

**Overall: LOW.** The primary risk is the SHA mismatch, which is mitigated by a hard-stop P0 failure in the final plan (Plan 04-03 Task 2). The technical debt of a dead-code envelope field is minor compared to the benefit of having the Rust-side A2A contract defined and ready for the Go-side parity work in subsequent slices. The plans are conservative, evidence-based, and respect all architectural invariants.

> Verifier should pay close attention to the `e2e_first_run_smoke.log` to confirm the download messaging (URL + ETA) matches the SPEC R2.a grep contract exactly.

---

## Codex Review (8635 bytes)

**Overall verdict:** Marginal, not sound. Risk: MEDIUM-HIGH.

### 1. One-paragraph Summary

These plans are marginal, not sound. The core direction is right: pin model revision, improve first-run UX, cap query retries, add A2A index failure aborts. But 04-01 and 04-03 contain several brittle assumptions that can fail in real execution: the pinned SHA provenance is weak, the `hf-hub` snapshot layout handling is under-specified, the E2E harness mutates global HF cache destructively, and 04-03 explicitly defers R4.b/R5.b while still pretending to close the slice. 同志，这不是 fatal, but closure semantics are too optimistic.

### 2. Strengths

- **04-01 Task 1** correctly rejects the nonexistent `huggingface_hub_rust` path after gate failure and falls back to `hf-hub`, avoiding a new dependency for no reason.
- **04-01 Task 2** keeps messaging in `eprintln!`, matching actual project conventions instead of dragging in `tracing`.
- **04-02 Task 1** chooses `embed_query()` over a generic retry-policy abstraction. Good scope control. Query UX needs a small hammer, not a retry framework.
- **04-02 Task 3** correctly keeps retry policy at the caller level: CLI and server have different abort semantics, and forcing that into `Embedder` would smear responsibilities.
- **04-02 bound check** on `max_consecutive_fail` is at least present. Arbitrary, but better than trusting network input.
- **04-03** correctly insists on testing `./bin/codenexus`, not `cargo run`, which is the right surface for the single-binary packaging claim.

### 3. Concerns

- **HIGH — 04-03 closes while R4.b/R5.b are deferred.** SPEC acceptance requires synthetic-failure tests for server A2A failure transition and query failure <1s. 04-03 Task 1 explicitly says these are "NOT included" and "DEFERRED". That means the slice cannot honestly close as fully accepted. The summary can mark them deferred, but the closure commit message must not claim R4/R5 fully verified.

- **HIGH — 04-01 pinned SHA provenance is fragile.** The plan says the SHA was "recovered from Curry's local cache + refs/main timestamp". That is not proof it produced the 67.9% baseline. It proves local cache had that revision near that time. Better evidence would be: existing `poc.db` stores version hash/model metadata matching that revision, or re-running eval after pin confirms no regression. Eval catches semantic drift later, but the plan still overclaims provenance.

- **HIGH — 04-01 `snapshot_dir()` parent calculation is buggy or at least misleading.** The proposed loop computes ancestors only when `snapshot_root.is_none()`, and since first file is `config.json`, it returns top-level snapshot dir. Fine by accident. The comment discusses nested files but never validates they are under the same snapshot root. If `repo.get("1_Pooling/config.json")` lands elsewhere due to cache behavior, this will not detect it. Also no check that all 9 files are under the pinned SHA directory.

- **HIGH — 04-03 E2E destructively deletes `blobs`.** `rm -rf "$HF_CACHE_DIR/blobs"` can destroy shared cached blobs for that model, not just the pinned snapshot. It is under a specific model dir, but still global user cache mutation. Use isolated `HF_HOME` in a temp dir. Current harness risks corrupting the user's normal HF cache and makes tests slow/non-repeatable.

- **HIGH — 04-03 "pre-index step" depends on cache before deleting it.** The script indexes first "using cached snapshot", then deletes snapshot/blobs. On a truly clean machine, pre-index may already trigger the first-run download before the actual E2E Phase 1, polluting the evidence. This defeats the clean-cache test ordering.

- **MEDIUM — 04-01 assumes exact 9 files are sufficient for fastembed forever.** Enumerating files is acceptable as a fallback, but it should validate from `modules.json` / sentence-transformers layout or at least fail with a doc explaining the fixed list. This is a maintenance trap when upstream changes auxiliary files.

- **MEDIUM — 04-01 docs mention `refs/main` while code uses a pinned revision.** Offline doc tells users to write `refs/main` as `<sha>`. But if code always fetches pinned revision, `refs/main` should not matter. This advice may confuse cache layout and is unnecessary.

- **MEDIUM — 04-02 A2A metadata argument is muddled.** The plan verifies A2A `metadata`, then adds `max_consecutive_fail` directly to `OperationRequest::IndexRepo`. That is not "metadata passing through"; it is CodeNexus's own operation schema. That can be fine, but the G-04 justification is mostly irrelevant unless the actual wire layer maps A2A metadata into `OperationRequest`.

- **MEDIUM — 04-02 dropping config.toml silently violates D-05.** The context locked "envelope > config.toml > hardcoded". The plan drops config because no config infra exists. That is probably pragmatic, but it should be recorded as a SPEC/context deviation requiring explicit amendment, not buried in plan-time verification.

- **MEDIUM — `1..=1000` is arbitrary and weird.** A threshold of 1000 defeats the point for most repos. If this is a safety bound, use something closer to operational intent, e.g. `1..=100`, or document why 1000 is needed.

- **MEDIUM — eval +/-2pp on 7 queries is weak.** With 7 queries × top-5 = 35 judged slots, one result shift is ~2.86pp. A +/-2pp gate is stricter than the granularity unless the metric uses fractional judge scores. This can create false failures or fake precision. The plan should state metric granularity and expected deterministic equality.

- **MEDIUM — `cargo test --lib embedder::tests` may not match Cargo filtering semantics.** Safer command is `cargo test --lib embedder::tests::` or just `cargo test --lib`. Current command may work, but plan should not rely on ambiguous test filtering.

- **LOW — sequential waves are over-conservative but acceptable.** 04-01 and 04-02 both touch `embedder.rs`, so sequencing is defensible. 04-03 must be last. No major issue.

- **LOW — ARCH row says revision pin is "version-hash-affecting" but "no version-hash change".** This language is contradictory. Say: "audit-relevant, not embedding-version-hash-changing."

- **LOW — E2E script assumes Bash/Unix paths despite Windows environment.** The repo may run under Git Bash, but plans should say that. Otherwise `chmod`, `/usr/bin/cat`, `bash`, `jq`, `awk` are portability assumptions.

### 4. Suggestions

- **04-03 Task 1:** Use isolated cache: `HF_HOME="$(mktemp -d)"` and delete that temp dir in `trap`. Do not mutate `~/.cache/huggingface`.
- **04-03 Task 1:** Move clean-cache setup before any index/query operation. If a pre-indexed DB is required, use an existing committed tiny DB or create it with the same isolated `HF_HOME` after the clean-cache assertions are designed correctly.
- **04-03 Task 1/3:** Do not close R4.b/R5.b as accepted unless implemented. Either add fault injection, e.g. `CODENEXUS_EMBED_FAIL=1` checked inside `embed_once`, or mark the slice "partial closure".
- **04-01 Task 1:** After fetching files, assert `snapshot.ends_with(QWEN3_REVISION)` and every fetched path starts with the same snapshot root.
- **04-01 Task 1:** Add a unit/smoke assertion that `snapshot_dir()` returns a path containing `/snapshots/<QWEN3_REVISION>`.
- **04-01 Task 3:** Remove `refs/main` instructions from offline docs or clearly mark them optional/cache-internal. The pinned snapshot dir is the real contract.
- **04-02 Plan-Time G-04:** Rewrite the rationale. Either actually use A2A metadata and map it into `OperationRequest`, or say this is a CodeNexus operation-field extension independent of A2A metadata.
- **04-02 Task 3:** Change threshold bound from `1..=1000` to something operationally sane, or name the constant and document why.
- **04-03 Task 2:** Treat eval as deterministic equality or one-slot tolerance depending on metric granularity. Don't use +/-2pp blindly if metric steps exceed 2pp.
- **All plans:** Replace closure language with "accepted gates passed; R4.b/R5.b pending if fault injection not implemented."

### 5. Risk Assessment

**Overall risk: MEDIUM-HIGH.** The implementation itself is not huge, but the verification story is leaky. The biggest problem is 04-03 claiming closure while skipping two locked synthetic-failure acceptance gates. Second biggest is the E2E harness damaging global HF cache and accidentally exercising the download before the clean-cache phase. Fix those, and the plan drops to medium/low. Without those fixes, this can produce a green-looking phase with untested failure behavior.

---

## OpenCode Review (morning-0726)

**FAILED.** Exit 0, 0 bytes output. Same auth issue as current run.

---

## Claude Review (morning-0726)

**SKIPPED for independence.** Running inside Claude Code.

The orchestrator (Claude Opus 4.7 = me) provides the consensus synthesis below as the synthesizer role per Curry's CCG pattern (Codex + Gemini → Claude synthesis), not as a fourth independent reviewer.

---

## Consensus Synthesis (morning, Claude as synthesizer)

### Classification: SPLIT

Both reviewers agree the plans are *directionally correct* and *technically defensible* in their main implementation choices (hf-hub fallback, embed_query() over policy-struct, eprintln! over tracing, ./bin/codenexus over cargo run). They diverge sharply on **severity calibration** for verification quality:

- Gemini: LOW overall risk, plans are "sound and technically rigorous"
- Codex: MEDIUM-HIGH risk, "marginal, not sound", "verification story is leaky"

### Adjudication: Codex's HIGH calls are correct

The synthesizer (me) sided with Codex on the 3 HIGH issues that Gemini missed entirely, because they are verifiable from plan text — not Codex over-reactions:

1. **04-03 closure dishonesty (HIGH)** — SPEC R4.b and R5.b require synthetic-failure tests; Plan 04-03 Task 1 explicitly defers them. Closing the slice while two locked acceptance gates are unverified violates `feedback_honesty_ideology` ("任务结束诚实列缺口, 不装完工").

2. **04-03 destructive global cache mutation (HIGH)** — `rm -rf "$HF_CACHE_DIR/blobs"` operates on `~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/blobs`. Even though the path is under model-specific dir, HF blob layout is content-addressed and blobs may be referenced from other snapshots of the same model family. Codex's fix (`HF_HOME=$(mktemp -d)`) is correct.

3. **04-03 pre-index ordering bug (HIGH)** — The script pre-indexes "using cached snapshot" THEN deletes the snapshot for the clean-cache test. On a truly clean machine, pre-index triggers first-run download before the test phase, polluting the evidence. The pre-index step needs to run inside the same isolated `HF_HOME` as the test.

### Agreed Concerns (raised by both reviewers)

| Concern | Gemini | Codex | Synthesizer call |
|---------|--------|-------|------------------|
| SHA provenance fragility (cache + refs/main timestamp ≠ 67.9%-baseline proof) | MEDIUM | HIGH | MEDIUM-HIGH — eval gate at 04-03 Task 2 is the actual safety net; plan should say so explicitly |
| Eval +/-2pp on n=7 statistically noisy | LOW | MEDIUM | MEDIUM — Codex correct: 1 result shift = 2.86pp, gate is stricter than metric granularity |
| `1..=1000` arbitrary | LOW | MEDIUM | LOW-MEDIUM — fix recommended but not blocking |

### Codex-only concerns (Gemini missed) — morning

| Concern | Severity | Synthesizer call |
|---------|----------|------------------|
| 04-03 closes while R4.b/R5.b deferred | HIGH | CONFIRMED HIGH — closure honesty issue |
| 04-03 destructive cache mutation | HIGH | CONFIRMED HIGH — mktemp HF_HOME fix |
| 04-03 pre-index ordering | HIGH | CONFIRMED HIGH — pre-index in isolated HF_HOME |
| 04-01 snapshot_dir() under-specified | HIGH | MEDIUM — add assertion `snapshot.ends_with(QWEN3_REVISION)` |
| 04-01 9-file enumeration brittle | MEDIUM | LOW — fastembed file list is stable for Qwen3 family per current upstream |
| refs/main in offline docs contradicts pinned-revision code | MEDIUM | MEDIUM — fix recommended (clarify cache-internal vs user-facing) |
| 04-02 A2A G-04 justification muddled | MEDIUM | MEDIUM — rewrite G-04 rationale: this is `OperationRequest` schema extension, not A2A metadata pass-through |
| Dropping config.toml without explicit amendment | MEDIUM | LOW — already documented in plan-time verification block (info-level, not buried) |
| cargo test filter syntax | MEDIUM | LOW — change to `cargo test --lib` to avoid ambiguity |
| ARCH row contradictory wording | LOW | LOW — minor cleanup |
| Bash/Unix portability assumption | LOW | LOW — Git Bash assumption is fine for the project |

### Gemini-only concerns (Codex missed) — morning

| Concern | Severity | Synthesizer call |
|---------|----------|------------------|
| R4 envelope override is dead code (Go side OOS) | MEDIUM | MEDIUM — confirmed; SUMMARY note required for follow-up Go-side work |
| File list verification (merges.txt / vocab.json) | LOW | LOW — sanity-check during executor, not blocking plan |
| Role unused import in search.rs | LOW | LOW — executor cleanup |

### Divergent Views (morning) — worth flagging

- **Severity of SHA provenance:** Gemini says MEDIUM (eval gate catches it); Codex says HIGH (provenance overclaim). Synthesizer split: the *implementation* risk is MEDIUM (eval gate is real safety), but the *plan-text honesty* is HIGH — Plan 04-01 Task 1 should say "SHA recovered from cache, eval no-regression at Plan 04-03 Task 2 is the canonical proof of model-identity" rather than implying the cache+timestamp is the proof.
- **R4 envelope override:** Gemini calls it MEDIUM dead code; Codex doesn't flag it as dead code but flags the G-04 justification as muddled. Both views point at the same root cause: the rationale for adding the field isn't well-grounded. Synthesizer call: the field SHOULD be added (Rust-side A2A contract is the right scope for this slice; Go-side parity is later phase work), but the plan-time verification rationale should be rewritten per Codex's suggestion.

---

*Morning review completed: 2026-04-28T07:26:14*
*Re-review supplement appended: 2026-04-28T13:03+08:00*
*Final verdict (combining both sessions): HALT-AND-REPLAN*
*Recommended next step: `/gsd-plan-phase 4 --reviews` to incorporate combined feedback before any execution*
