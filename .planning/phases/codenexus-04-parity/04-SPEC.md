# Phase 4 (First Slice): First-Run UX Cluster + P2 Resilience Same-Crate -- Specification

**Created:** 2026-04-28
**Ambiguity score:** 0.156 (gate: <= 0.20) -- PASS
**Requirements:** 5 locked
**Phase number:** 4 (first slice; subsequent Phase 4 slices for multi-language tree-sitter / multi-repo registry / git overlay / CodeFlow port get their own SPECs)

## Goal

Replace Phase 03.6's silent first-run failure mode with explicit first-run UX (download prompt + failure recovery link + offline-bootstrap doc + HF revision pin) so an Apache-2.0 open-source user on a clean machine (or behind a Clash-down network) can either (a) succeed within 60s with explicit progress, or (b) fail with a one-click recovery path -- AND opportunistically apply the same `consecutive_fails` resilience pattern from `main.rs:Index` to the two other call sites in the same crate (`server.rs:198` A2A Index handler, `search.rs:31` Query path).

## Background

After Phase 03.6 candle migration (commit `67320ec` 2026-04-28), `embedder.rs:67-71` already prints one `eprintln!` line before HF download and `:73-74` already adds a one-line `.context()` on failure -- but there is no progress indicator during the ~1.2GB download, no link to recovery documentation in the failure path, no HF revision pin (supply-chain drift risk: silent re-uploads on HuggingFace invalidate cache), no `docs/embedder-offline-bootstrap.md` for offline / Clash-China-down recovery, and no link from `README.md` "Quick start" to recovery steps. Plus, the Phase 3.5b `--max-consecutive-fail` counter pattern landed only in `main.rs:Index`; `server.rs:198` (A2A endpoint Index handler) inherits the embedder retry but lacks the fail counter / structured abort, and `search.rs:31` (Query path) silently retries 5 times with backoff (~7.75s sleep budget per failed query) when a single clean error would be the right UX.

This SPEC is the **first slice of Phase 4 Parity**. The other four Phase 4 deliverable groups (multi-language tree-sitter, multi-repo registry, git overlay, CodeFlow MIT port) get their own SPECs after this slice closes.

## ASSUMPTION CORRECTIONS (read before Requirements)

These reverse three claims in `PROJECT.md` Phase 4+ Backlog (line 71-94) that scout-phase grep verified to be wrong. Planner MUST NOT inherit the original PROJECT.md claims; this SPEC supersedes them for the first slice.

1. **tracing**: PROJECT.md line 75 assumes "project already wires tracing elsewhere". VERIFIED FALSE -- 0 uses of `tracing` in `experiments/poc-retrieval/src/`, 33 uses of `eprintln!`. **SPEC IMPACT**: first-run UX improvements use `eprintln!`, NOT `tracing::info!`. Tracing migration is explicitly out of scope; if it ever happens, separate quick task or phase.

2. **revision API**: PROJECT.md line 73 assumes `Qwen3TextInitOptions` exposes a `revision: "<sha>"` parameter. VERIFIED FALSE -- actual signature is `Qwen3TextEmbedding::from_hf(MODEL_REPO, &Device::Cpu, DType::F32, MAX_LEN)` (4-arg, no revision; verified at `experiments/poc-retrieval/src/embedder.rs:72` and Phase 03.6 `03.6-01-SUMMARY.md:153-188` cargo-cache spelunking). **SPEC IMPACT**: this SPEC locks WHAT only ("revision MUST be pinned"). HOW (use hf-hub crate / huggingface_hub_rust crate / fork fastembed) is planner's job. See `04-PRE-PLAN-NOTES.md` for planner hints (non-locking).

3. **embedder.rs first-run state**: PROJECT.md line 75 says "silent until HF cache hit". VERIFIED PARTIALLY FALSE -- `embedder.rs:67-71` already has 1 `eprintln!` start prompt (mentions ~1.2 GB and HF cache path), `:73-74` already has 1 line `.context("model download failed -- check internet to huggingface.co")`. **SPEC IMPACT**: requirements R2 are framed as "augment existing messaging with progress + recovery link", NOT "add first-run messaging from zero".

## Requirements

### R1: HF revision pin

Pin the Qwen3-Embedding-0.6B model load to a specific HuggingFace Hub commit SHA, recorded in `docs/ARCHITECTURE.md` §9.8 history row, so silent re-uploads of the model on HuggingFace cannot invalidate the local cache without a project-controlled version bump.

- **Current**: `embedder.rs:39` defines `const MODEL_REPO: &str = "Qwen/Qwen3-Embedding-0.6B";` with no revision; `from_hf(MODEL_REPO, ...)` resolves to whatever HEAD commit HuggingFace serves. Phase 03.6 03.6-RESEARCH.md (line 560) explicitly flagged this as deferred to Phase 4 hardening.
- **Target**: `embedder.rs` defines `const QWEN3_REVISION: &str = "<40-char-sha>"`; the model load path uses this SHA via the fetch primitive chosen by planner (see `04-PRE-PLAN-NOTES.md`); `docs/ARCHITECTURE.md` §9.8 gets a new history row appending the pinned SHA (per §9.8 protocol's "version-hash-affecting changes" discipline).
- **Acceptance** (all three required):
  - (a) `grep -E 'const QWEN3_REVISION = "[a-f0-9]{40}"' experiments/poc-retrieval/src/embedder.rs` → 1 hit
  - (b) `grep -E '<sha-from-(a)>' docs/ARCHITECTURE.md` → 1 hit (in §9.8 history row)
  - (c) **Reload test**: `rm -rf ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/<sha>/` followed by `./poc-retrieval index --repo <small-test-repo>` re-creates the same SHA snapshot directory and exits 0; verifier confirms `ls ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/` lists exactly the const SHA.

### R2: First-run download UX

Augment the existing `embedder.rs:67-71` start-prompt and `:73-74` failure-context lines so a first-run user understands what is happening (size + URL + ETA) and where to recover when network fails (link to `docs/embedder-offline-bootstrap.md`).

- **Current**: One `eprintln!` start prompt (mentions ~1.2 GB + HF cache path, no URL, no ETA). One `.context()` failure line ("check internet to huggingface.co", no link to recovery doc).
- **Target**: Start-prompt mentions `huggingface.co` URL explicitly and "30-60s on broadband" ETA wording; failure path emits a second line linking to `docs/embedder-offline-bootstrap.md` for recovery (HF_HOME pre-seeding, HF_HUB_OFFLINE mode, Clash-China-down walkthrough).
- **Acceptance** (a)+(b) required, (c) deferred:
  - (a) `grep -nE 'first-run download|huggingface\.co' experiments/poc-retrieval/src/embedder.rs` → >= 1 hit (start-prompt contains both URL and ETA wording)
  - (b) `grep -n 'embedder-offline-bootstrap' experiments/poc-retrieval/src/embedder.rs` → >= 1 hit (failure path links to recovery doc)
  - (c) DEFERRED -- progress indicator: fastembed `Qwen3TextEmbedding::from_hf` does NOT expose a progress callback. If R1 plan path naturally exposes hf-hub fetch (which has `download_with_progress` or equivalent), implementer can deliver it as a side benefit and update R2 acceptance to include (c). Otherwise progress indicator stays an open follow-up.

### R3: Offline / Clash-down recovery doc

Create `docs/embedder-offline-bootstrap.md` covering the four documented offline recovery paths (manual safetensors download, HF_HOME pre-seeding, HF_HUB_OFFLINE=1 mode, Clash-China-down walkthrough), and link it from `README.md` "Quick start" so failure-path users find recovery in one click.

- **Current**: `docs/` contains only `ARCHITECTURE.md` and `origin-spec.md`. No offline-bootstrap doc exists. README.md "Quick start" section (lines 53-58) shows `make build` + `./bin/codenexus serve --port 8080` only, no link to recovery.
- **Target**: `docs/embedder-offline-bootstrap.md` exists with at least 4 H2 sections covering the documented recovery paths; `README.md` "Quick start" or "Build" section contains a relative link to the new doc.
- **Acceptance** (all three required):
  - (a) `test -f docs/embedder-offline-bootstrap.md` → file exists
  - (b) `grep -cE '^## ' docs/embedder-offline-bootstrap.md` → >= 4 (sections must include "Manual download", "HF_HOME pre-seeding", "HF_HUB_OFFLINE mode", "Clash-China-down recovery" -- titles can vary, but all four topics MUST be present as H2 sections, verifier reads the doc once to confirm topic coverage)
  - (c) `grep -nE 'embedder-offline-bootstrap' README.md` → >= 1 hit (link from Quick start / Build section, NOT buried in a Notes / Appendix area)

### R4: P2 server.rs A2A Index handler -- consecutive_fails counter (mechanical patch)

Copy the `--max-consecutive-fail` counter pattern from `main.rs Index` (Phase 3.5b commit `8f4da66`) into `server.rs:198` A2A Index handler. Same risk profile as `main.rs Index` -- both call `embedder.embed()` in a long loop, both can hit silent partial state without abort. NO `EmbedError` enum introduction in this slice (Q5=B locked: enum design is deferred to a future production-grade phase; this slice does only mechanical patch).

- **Current**: `experiments/poc-retrieval/src/server.rs:198` (A2A endpoint Index handler) inherits the 5-attempt embedder retry but has no consecutive-fails counter, no structured abort. A 4-symbol fail-cluster would burn ~20min wall-clock with zero recovery and silent partial state in the response.
- **Target**: `server.rs` Index handler includes a counter (default = 5, configurable via the same mechanism as `main.rs --max-consecutive-fail` -- prefer config-file or A2A request param so A2A clients can override; CLI flag does not apply to a server handler) that increments on `embedder.embed()` Err, resets on Ok, and bails (returns A2A task FAILED state with structured error message containing the consecutive-fails count) when threshold is reached.
- **Acceptance** (both required):
  - (a) `grep -nE 'consecutive_fails|max_consecutive_fail' experiments/poc-retrieval/src/server.rs` → >= 1 hit (counter pattern present in Index handler scope)
  - (b) Synthetic-failure test (verifier-runnable): start `./poc-retrieval serve`, send A2A Index request against a path where embedder is forced to fail (e.g. `JINA_API_KEY=` empty + a config that swaps embedder to a HTTP backend, OR easier: temporarily inject a panic via env var hook), confirm A2A task transitions to `failed` state with consecutive-fails count surfaced in error message; task does NOT hang waiting for max-attempts × max-delay accumulation.

### R5: P2 search.rs Query path --砍 5-attempt retry storm (mechanical patch)

The current `embedder.embed()` retry wrapper (5 attempts × exponential 250ms-base backoff = ~7.75s sleep budget) is appropriate for Index path but punishes Query path UX -- a single Query embedding failure causes the user-facing search to wait ~7.75s before returning an error. Q5=B locked: NO EmbedError enum split in this slice; instead, mechanically reduce the Query call site's retry to 2 attempts max with 250ms delay (≤500ms total budget), so a Query embedder failure surfaces as a clean error within 1 second.

- **Current**: `experiments/poc-retrieval/src/search.rs:31` (Query path) calls `embedder.embed(query, Role::Query)` which uses the shared 5-attempt retry wrapper in `embedder.rs:84-101` (`MAX_ATTEMPTS = 5`, `BASE_DELAY_MS = 250`, exponential backoff = ~7.75s total sleep on failure).
- **Target**: Query path uses a Query-specific retry budget of MAX 2 attempts with MAX 250ms total delay (no exponential backoff). Implementation options for planner: (i) introduce `embed_query()` method on `Embedder` that wraps `embed_once()` with the lighter retry policy, leaving the heavier wrapper for Index callers, OR (ii) expose `MAX_ATTEMPTS` / `BASE_DELAY_MS` as parameters to a new `embed_with_policy(text, role, policy)` method, with `embed()` retaining current defaults for back-compat. Planner picks; either gives the Query path its budget cap.
- **Acceptance** (both required):
  - (a) `grep -nE 'embed_query|embed_with_policy|MAX_ATTEMPTS\s*[:=]\s*2' experiments/poc-retrieval/src/{embedder,search}.rs` → >= 1 hit
  - (b) Synthetic-failure test (verifier-runnable): force `embedder.embed_once` to error; measure wall-clock from `./poc-retrieval query "x"` invocation to error return → MUST be < 1.0s (single 250ms sleep + processing). Compare to old Query path which would take ≥ 7.5s.

## Boundaries

**In scope (this SPEC):**
- HF revision pin (R1) including SHA const + ARCH §9.8 row + reload test
- First-run download UX (R2) augmenting existing `embedder.rs:67-71/73-74` lines (NOT building from zero, NOT introducing tracing)
- Offline / Clash-down recovery doc (R3) including 4 documented recovery paths + README.md link
- `server.rs:198` A2A Index handler counter (R4) -- same-crate mechanical patch
- `search.rs:31` Query path retry budget cap (R5) -- same-crate mechanical patch
- E2E smoke harness for first-run UX (Q6=B locked acceptance, see Acceptance Criteria below)

**Out of scope (deferred to later slices / phases):**
- Multi-language tree-sitter (Phase 4 group 2, separate SPEC) -- this slice does not touch parser
- Multi-repo registry (Phase 4 group 3, separate SPEC) -- one DB per repo stays the assumption
- Git overlay via gix (Phase 4 group 4, separate SPEC) -- no git-blame / git-log integration here
- Pattern detection / security scanners ported from CodeFlow MIT (Phase 4 group 5, separate SPEC) -- no NOTICE attribution required by this slice
- `tracing` framework migration -- ASSUMPTION CORRECTION #1; use `eprintln!`. If tracing ever happens, separate quick task or phase
- `EmbedError` enum (Transient / Permanent / Timeout) design and 33 caller-site arm-match -- Q5=B locked deferred to a future production-grade phase
- Progress indicator (R2 (c)) -- DEFERRED unless implementation path naturally exposes it
- Fork of fastembed-rs to add revision support -- explicit anti-pattern (see `04-PRE-PLAN-NOTES.md`)
- Any change to `core/`, `server/` (Go), `ui/` -- co-location rule: this slice modifies ONLY `experiments/poc-retrieval/src/{embedder,server,search}.rs` + `docs/embedder-offline-bootstrap.md` + `docs/ARCHITECTURE.md` (§9.8 row) + `README.md` (Quick start link)

**Co-location boundary protector**: Any future request to extend "P2 resilience" / "first-run UX" beyond `experiments/poc-retrieval/src/` belongs in a separate SPEC. Same-crate is the geometric anti-scope-creep rule for this slice.

## Constraints

- **No new external dependencies for R1 are negotiable but bounded**: planner picks between `hf-hub` (already a transitive dep of fastembed-rs) and `huggingface_hub_rust` (new, may need adding). Adding huggingface_hub_rust is acceptable if `cargo doc` shows clean snapshot_download API; otherwise stick with hf-hub.
- **No tracing crate adoption** -- ASSUMPTION CORRECTION #1.
- **Single-fat-binary distribution invariant preserved** -- ARCH §3 line 89; this slice does not change packaging or distribution shape (specifically: the safetensors blob MUST NOT be vendored into the Go-embedded binary; ARCH constraints on binary size cap apply).
- **Apache 2.0 license compliance** -- if R1 adopts huggingface_hub_rust crate, verify license is Apache 2.0 / MIT / BSD before adding (NO GPL/AGPL deps per PROJECT.md Out of Scope).
- **Eval baseline preserved** -- this slice MUST NOT change embedder model weights, dim, prefix, or instruction string. Pinning revision to the SHA already in use post-03.6 (the SHA whose model produced poc.db's 67.9% B1-B7) is required, NOT pinning to a newer SHA. Verifier confirms post-pin REQ-10 mean precision_at_5 stays within ±2pp of 67.9%.

## Acceptance Criteria

Verifier runs each checkbox as a single command (or short command sequence) and records PASS / FAIL.

**R1 -- HF revision pin:**
- [ ] (R1.a) `grep -E 'const QWEN3_REVISION = "[a-f0-9]{40}"' experiments/poc-retrieval/src/embedder.rs` returns 1 hit
- [ ] (R1.b) Same SHA appears in a new `docs/ARCHITECTURE.md` §9.8 history row (`grep -F '<sha>' docs/ARCHITECTURE.md` returns >= 1 hit; row format matches §9.8 protocol)
- [ ] (R1.c) Reload test: delete the snapshot subdir, rerun `index`, observe the same SHA subdir reborn, exit code 0

**R2 -- first-run download UX:**
- [ ] (R2.a) `grep -nE 'first-run download|huggingface\.co' experiments/poc-retrieval/src/embedder.rs` returns >= 1 hit on a start-prompt line that includes both the URL and an ETA word ("30-60s" or "broadband" or equivalent)
- [ ] (R2.b) `grep -n 'embedder-offline-bootstrap' experiments/poc-retrieval/src/embedder.rs` returns >= 1 hit on a failure-path line

**R3 -- offline recovery doc:**
- [ ] (R3.a) `test -f docs/embedder-offline-bootstrap.md` returns 0
- [ ] (R3.b) `grep -cE '^## ' docs/embedder-offline-bootstrap.md` returns >= 4 AND verifier read confirms 4 topic coverage: manual download, HF_HOME pre-seeding, HF_HUB_OFFLINE mode, Clash-China-down recovery
- [ ] (R3.c) `grep -nE 'embedder-offline-bootstrap' README.md` returns >= 1 hit in Quick-Start / Build / Install section (NOT in Notes / Appendix)

**R4 -- server.rs A2A Index handler counter:**
- [ ] (R4.a) `grep -nE 'consecutive_fails|max_consecutive_fail' experiments/poc-retrieval/src/server.rs` returns >= 1 hit in the Index handler scope
- [ ] (R4.b) Synthetic-failure test: A2A Index task transitions to `failed` state with consecutive-fails count in error message; does NOT hang for max-attempts × max-delay accumulation

**R5 -- search.rs Query path budget cap:**
- [ ] (R5.a) `grep -nE 'embed_query|embed_with_policy|MAX_ATTEMPTS\s*[:=]\s*2' experiments/poc-retrieval/src/{embedder,search}.rs` returns >= 1 hit
- [ ] (R5.b) Synthetic-failure test: forced single-failure Query returns error in < 1.0s wall clock

**E2E smoke (Q6=B locked, applies to R1+R2+R3 together):**
- [ ] (E2E) On a clean cache (`rm -rf ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/`), run `./poc-retrieval query "x"` (small test repo pre-indexed with the pinned SHA) and observe: (1) start prompt with URL + ETA appears, (2) download completes, (3) query returns results, exit 0
- [ ] (E2E) Block `huggingface.co` (e.g. add `0.0.0.0 huggingface.co` to hosts file or kill Clash) on a clean cache, rerun the same `query`, observe: (4) failure message appears containing the link to `docs/embedder-offline-bootstrap.md`, (5) command exits with non-zero; (6) restoring network and rerunning succeeds (E2E (1)-(3) re-occur)

**Eval no-regression (constraint enforcement):**
- [ ] Post-pin REQ-10 B1-B7 mean precision_at_5 within ±2pp of 67.9% (Phase 03.6 baseline) -- run `eval/req10_alpha06.json` harness, compare delta

## Ambiguity Report

| Dimension          | Score | Min  | Status | Notes                                                                 |
|--------------------|-------|------|--------|-----------------------------------------------------------------------|
| Goal Clarity       | 0.85  | 0.75 | ✓      | Scope = first-run UX cluster + P2 same-crate; 5 R derived             |
| Boundary Clarity   | 0.85  | 0.70 | ✓      | Co-location rule + 7 explicit Out-of-Scope items                      |
| Constraint Clarity | 0.82  | 0.65 | ✓      | Q5=B locks "no enum, mechanical patch only"; eval no-regression locked |
| Acceptance Criteria| 0.85  | 0.70 | ✓      | 13 pass/fail checkboxes incl. E2E smoke + eval no-regression          |
| **Ambiguity**      | **0.156** | <=0.20 | ✓ PASS | All 4 dimensions above minimum                                        |

## Interview Log

| Round | Perspective                       | Question summary                                                  | Decision locked                                                                          |
|-------|-----------------------------------|--------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| 0     | Researcher (scout)                | Current state of first-run UX in embedder.rs; tracing usage; revision API in fastembed | 33 eprintln! / 0 tracing; from_hf 4-arg no revision; 1 start-prompt + 1 failure-context already exist (3 ASSUMPTION CORRECTIONS) |
| 1     | Boundary Keeper (Q1)              | SPEC range: whole Phase 4 vs first slice                          | B = first-run UX cluster + P2 resilience same-crate; co-location rule added              |
| 1     | Boundary Keeper (Q2)              | tracing framework introduction in this slice                      | B = NOT introduced; 33 eprintln! sanyong; tracing migration deferred                     |
| 1     | Boundary Keeper (Q3)              | HF revision pin implementation depth in SPEC                       | A = lock WHAT only ("revision MUST be pinned"); HOW = planner via PRE-PLAN-NOTES         |
| 2     | Failure Analyst (Q4 R1/R2/R3)     | Acceptance check shape per requirement                            | R1=(a)+(b)+(c) reload test; R2=(a)+(b) progress deferred; R3=(a)+(b)+(c) all required    |
| 2     | Failure Analyst (Q5)              | P2 acceptance shape: enum vs mechanical                            | B = no EmbedError enum in slice; mechanical patch only (counter copy + retry budget cap) |
| 2     | Failure Analyst (Q6)              | E2E smoke as acceptance                                           | B = 5 R + 1 E2E smoke (clean cache + Clash-down failure path)                            |

## PROJECT.md backlog cross-reference

This SPEC supersedes PROJECT.md Phase 4+ Backlog lines 71-94 for the first slice scope. After this slice closes, the PROJECT.md backlog entries can be marked `[CLOSED via Phase 4 first slice]` (parallel to how line 70 was marked `[CLOSED via Phase 03.6]`).

## Phase 03.6 cross-reference

Phase 03.6 closure (commit `67320ec` 2026-04-28) shipped candle in-process embedder via fastembed-rs 5.13. This SPEC builds on top of the 03.6 baseline -- the locked SHA in R1 MUST be the SHA whose model produced 03.6's 67.9% B1-B7 baseline (NOT a newer SHA), to preserve the eval no-regression constraint.

---

*Phase: codenexus-04-parity (first slice)*
*Spec created: 2026-04-28*
*Next step: /gsd-discuss-phase 4 -- discuss-phase will detect this SPEC.md, treat all 5 R + boundaries + acceptance as locked, and focus on HOW (which fetch path / which retry refactor / how to wire E2E smoke harness). Then /gsd-plan-phase 4 to break into executable plans.*
