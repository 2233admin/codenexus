---
phase: 4
slice: first-slice-ux-resilience
reviewers: [gemini, codex]
reviewers_failed: [opencode, claude]
reviewer_failure_reasons:
  opencode: "Invalid token (Copilot subscription auth expired) — exit 0 / 0 bytes output"
  claude: "Skipped — running inside Claude Code (CLAUDE_CODE_ENTRYPOINT=sdk-ts), self-review excluded for independence per gsd-review workflow rule"
reviewed_at: 2026-04-28T07:26:14
plans_reviewed:
  - 04-01-PLAN.md
  - 04-02-PLAN.md
  - 04-03-PLAN.md
consensus_classification: SPLIT
consensus_recommendation: "Replan via /gsd-plan-phase 4 --reviews. Codex flagged 3 HIGH issues that are verifiable from plan text. Plans should NOT execute as-is."
---

# Cross-AI Plan Review — Phase 4 First Slice

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

## OpenCode Review

**FAILED.** Exit 0, 0 bytes output.

**Error from stderr:** `Invalid token (request id: 20260427232548841111198268d9d67wGjbE5Q)` — Copilot subscription auth token expired. To re-enable: refresh GitHub Copilot subscription token via OpenCode CLI auth flow, then re-run `/gsd-review --phase 4 --opencode`.

---

## Claude Review

**SKIPPED for independence.** Running inside Claude Code (`CLAUDE_CODE_ENTRYPOINT=sdk-ts`). Per gsd-review workflow rule, self-review of own plans is excluded — would be self-confirming bias.

The orchestrator (Claude Opus 4.7 = me) provides the consensus synthesis below as the synthesizer role per Curry's CCG pattern (Codex + Gemini → Claude synthesis), not as a fourth independent reviewer.

---

## Consensus Synthesis (Claude as synthesizer, per CCG pattern)

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

### Codex-only concerns (Gemini missed)

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

### Gemini-only concerns (Codex missed)

| Concern | Severity | Synthesizer call |
|---------|----------|------------------|
| R4 envelope override is dead code (Go side OOS) | MEDIUM | MEDIUM — confirmed; SUMMARY note required for follow-up Go-side work |
| File list verification (merges.txt / vocab.json) | LOW | LOW — sanity-check during executor, not blocking plan |
| Role unused import in search.rs | LOW | LOW — executor cleanup |

### Divergent Views (worth flagging)

- **Severity of SHA provenance:** Gemini says MEDIUM (eval gate catches it); Codex says HIGH (provenance overclaim). Synthesizer split: the *implementation* risk is MEDIUM (eval gate is real safety), but the *plan-text honesty* is HIGH — Plan 04-01 Task 1 should say "SHA recovered from cache, eval no-regression at Plan 04-03 Task 2 is the canonical proof of model-identity" rather than implying the cache+timestamp is the proof.
- **R4 envelope override:** Gemini calls it MEDIUM dead code; Codex doesn't flag it as dead code but flags the G-04 justification as muddled. Both views point at the same root cause: the rationale for adding the field isn't well-grounded. Synthesizer call: the field SHOULD be added (Rust-side A2A contract is the right scope for this slice; Go-side parity is later phase work), but the plan-time verification rationale should be rewritten per Codex's suggestion.

---

## Recommendation

**Replan via `/gsd-plan-phase 4 --reviews`.** The 3 Codex HIGH issues + the agreed-MEDIUM concerns require concrete plan text changes — not just SUMMARY footnotes at executor time. Specifically:

### Changes the replan must produce

1. **04-03 Task 1 rewrite (3 HIGH issues, top priority):**
   - Replace `rm -rf "$HF_CACHE_DIR/..."` with `export HF_HOME="$(mktemp -d -t codenexus-e2e-XXXXXX)"; trap 'rm -rf "$HF_HOME"' EXIT`
   - Move pre-index step INSIDE the isolated `HF_HOME` (or use a committed tiny test DB built ahead of time and ship with the harness)
   - Add fault injection mechanism (`CODENEXUS_EMBED_FAIL=1` checked inside `embed_once`) and add R4.b + R5.b synthetic-failure tests that USE the fault injection
   - Closure language: replace "phase complete" with "phase complete: R1/R2/R3/R4.a/R5.a/E2E/EVAL_NO_REGRESSION verified; R4.b/R5.b verified via fault injection"

2. **04-01 Task 1 hardening:**
   - Add post-fetch assertion: `assert!(snapshot.ends_with(QWEN3_REVISION))` and `for path in fetched_paths { assert!(path.starts_with(&snapshot)); }`
   - Add unit smoke test that `snapshot_dir()` returns path containing `/snapshots/<QWEN3_REVISION>`
   - Reword SHA provenance language: cache+timestamp is *recovery method*, eval no-regression is *proof of model-identity*

3. **04-01 Task 3 doc cleanup:**
   - Remove or clarify `refs/main` instructions in offline-bootstrap.md (pinned snapshot dir is the contract; refs/main is cache-internal)

4. **04-02 Plan-Time G-04 rewrite:**
   - Reframe: `max_consecutive_fail` is added to `OperationRequest::IndexRepo` as a CodeNexus operation-schema field. A2A metadata pass-through is a separate concept. Justification should reference operation-schema versioning (semver-compatible field addition with `serde(default)`), not A2A metadata.

5. **04-02 Task 3 bound check:**
   - Change `1..=1000` → `1..=100` (operationally sane; even pathological repos rarely have >100 consecutive embedder failures before the user kills the job) OR add a named constant `MAX_RAISED_THRESHOLD: usize = 1000` with a comment explaining the upper bound

6. **04-03 Task 2 eval gate:**
   - Either tighten to "deterministic equality" (mean precision_at_5 must equal Phase 03.6 exactly; any drift = wrong SHA) OR widen to ±5pp with explicit acknowledgment that ±2pp is below metric granularity for n=7

### Changes for SUMMARY (not requiring replan)

- 04-02-SUMMARY.md must note the Go-side `OperationRequest` parity work as P3 follow-up (Gemini's R4 dead-code concern)
- Executor cleanup: remove `Role` unused import in search.rs (Gemini)

### Verifier-time checks (no plan change needed)

- `merges.txt` / `vocab.json` actually required by fastembed Qwen3 loader — sanity-check during R1 implementation; if loader fails, add to FILES list

---

*Review completed: 2026-04-28T07:26:14*
*Synthesis classification: SPLIT (resolved in favor of Codex's HIGH calls)*
*Recommended next step: `/gsd-plan-phase 4 --reviews` to incorporate this feedback before execute*
