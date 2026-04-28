---
phase: 4
plan: "04-02"
subsystem: poc-retrieval
tags: [resilience, embed-query, fault-injection, a2a, server]
depends_on: [04-00, 04-01]
provides: [R4-counter, R5-embed-query, FAULT_INJECTION]
affects: [embedder.rs, search.rs, a2a.rs, server.rs]
tech_stack:
  patterns:
    - "caller-policy split: embed_query (2-attempt) vs embed (5-attempt wrapper)"
    - "env-gated fault injection via AtomicUsize counter (CODENEXUS_EMBED_FAIL)"
    - "A2A operation-schema versioning via #[serde(default)] Option<usize>"
    - "consecutive_fails counter + bail -> A2A failed task state"
key_files:
  modified:
    - experiments/poc-retrieval/src/embedder.rs
    - experiments/poc-retrieval/src/search.rs
    - experiments/poc-retrieval/src/a2a.rs
    - experiments/poc-retrieval/src/server.rs
decisions:
  - "M2: bound 1..=100 (MAX_RAISED_THRESHOLD), not 1..=1000 -- tighter sanity bound"
  - "M6: max_consecutive_fail is operation-schema versioning, not A2A metadata pass-through"
  - "FAULT_INJECTION lives in production code (not cfg(test)) so Plan 04-03 E2E harness can use it against release binary"
  - "Q5=B locked: EmbedError enum deferred; embed_query is option (i) plain method"
metrics:
  duration: "~10 minutes"
  completed: "2026-04-28T06:46:00Z"
  tasks_completed: 3
  files_modified: 4
---

# Phase 4 Plan 04-02: P2 Same-Crate Resilience v2 Summary

R5 embed_query (2-attempt fast-fail) + env-gated CODENEXUS_EMBED_FAIL fault injection + R4 A2A IndexRepo consecutive_fails counter with M2 bound 1..=100.

## What Landed

### Task 1 (commit d09d3a9): R5 embed_query + fault injection + search.rs switch

**embedder.rs:**
- `pub fn embed_query(&self, text: &str) -> Result<Vec<f32>>` added between `embed` and `embed_once`
- `QUERY_MAX_ATTEMPTS: u32 = 2`, `QUERY_DELAY_MS: u64 = 250` (flat, no exponential backoff)
- Env-gated fault injection at top of `embed_once`: `CODENEXUS_EMBED_FAIL=always|once|after_N`
- Static `FAULT_COUNTER: AtomicUsize` for `once` and `after_N` modes
- Two new unit tests: `embed_query_works` (compile-time signature pin + dim check) and `embed_query_fault_injection` (env-var + <900ms budget check)
- Shared 5-attempt wrapper (`MAX_ATTEMPTS: u32 = 5`) byte-identical -- untouched

**search.rs:**
- Line 31: `embedder.embed(query, Role::Query)?` -> `embedder.embed_query(query)?`
- Import trimmed: `use crate::embedder::{cosine, Embedder, Role}` -> `use crate::embedder::{cosine, Embedder}`

### Task 2 (commit 4c7694d): R4 OperationRequest::IndexRepo envelope extension

**a2a.rs:**
- `IndexRepo` variant extended with `#[serde(default)] max_consecutive_fail: Option<usize>`
- M6-corrected doc comment: operation-schema versioning, NOT A2A metadata pass-through
- Back-compat: A2A clients without the field deserialize to `None`

### Task 3 (commit fafda6e): R4 server.rs counter loop + bound check

**server.rs:**
- `MAX_CONSECUTIVE_FAIL_DEFAULT: usize = 5` and `MAX_RAISED_THRESHOLD: usize = 100` named consts
- `OperationRequest::IndexRepo { repo, max_consecutive_fail }` destructured
- Bound check `1..=MAX_RAISED_THRESHOLD`: `Some(0)` and `Some(>100)` return Err immediately
- `consecutive_fails: usize` counter reset on success, incremented on failure
- On threshold breach: `return Err(anyhow!("aborting a2a indexer: ..."))` -> existing tokio worker maps to A2A `failed` task state via `store_for_worker.fail()`
- Old `Err(_) => continue` best-effort skip removed (0 hits confirmed)

## G-04 Outcome: PASS (M6 corrected framing)

A2A 1.0 spec defines `metadata` as free-form key/value on `Task` and `Message` -- that would have allowed envelope extension too. But our chosen path is CodeNexus's own `OperationRequest::IndexRepo` operation-schema extension via `#[serde(default)]`. This is operation-schema versioning, not A2A metadata pass-through. The A2A spec is not violated -- it simply doesn't govern our internal enum fields.

v1's G-04 framing was muddled (presented A2A metadata pass-through as the rationale). v2 reframes correctly per M6: the typed `OperationRequest` enum is CodeNexus's own schema; the A2A layer transports it opaquely.

## M2 Fix: bound 1..=100

v1 chose `1..=1000`. v2 uses `1..=MAX_RAISED_THRESHOLD` where `MAX_RAISED_THRESHOLD: usize = 100`. Rationale: a fully-broken embedder hits 100 consecutive failures within ~13 minutes (5 attempts x 7.75s x 100 / 60 = ~12.9 min). Still useful as a sanity bound. `1..=1000` would allow ~130 min wall-clock burn before bail -- defeating the safety bound's purpose.

## Verification Commands Run

```
cd experiments/poc-retrieval && cargo check
# Result: 3 warnings (pre-existing), Finished dev profile. PASS.

cd experiments/poc-retrieval && cargo check --tests
# Result: 3 warnings (pre-existing), Finished dev profile. PASS.
# New test signatures (embed_query_works, embed_query_fault_injection) compile cleanly.
```

`cargo test` deferred: pre-existing linker conflict (esaxx-rs/ort RuntimeLibrary mismatch LNK2038/LNK1319) prevents test execution. Pre-existing, not introduced by this plan. `cargo check` and `cargo check --tests` both pass.

## Acceptance Grep Verification

| Check | Result |
|-------|--------|
| `pub fn embed_query` in embedder.rs | 1 hit (line 356) |
| `QUERY_MAX_ATTEMPTS: u32 = 2` | 1 hit (line 357) |
| `QUERY_DELAY_MS: u64 = 250` | 1 hit (line 358) |
| `MAX_ATTEMPTS: u32 = 5` (wrapper preserved) | 1 hit (line 332) |
| `CODENEXUS_EMBED_FAIL` in embedder.rs | 9 hits (env var read + comment + tests) |
| `static FAULT_COUNTER: AtomicUsize` | 1 hit (line 387) |
| `after_` in fault-injection block | 3 hits |
| `embedder.embed_query` in search.rs | 1 hit (line 31) |
| `embedder.embed(query` in search.rs | 0 hits |
| Role import trimmed in search.rs | `{cosine, Embedder}` only -- 1 hit |
| `max_consecutive_fail: Option<usize>` in a2a.rs | 1 hit |
| `operation-schema versioning` in a2a.rs | 1 hit |
| `consecutive_fails` or `max_consecutive_fail` in server.rs | 10 hits |
| `MAX_CONSECUTIVE_FAIL_DEFAULT: usize = 5` | 1 hit |
| `MAX_RAISED_THRESHOLD: usize = 100` | 1 hit |
| `1..=MAX_RAISED_THRESHOLD` (bound uses named const) | 1 hit |
| `1..=1000` in server.rs | 0 hits |
| `aborting a2a indexer` | 1 hit |
| `Err(_) => continue` in IndexRepo arm | 0 hits |
| `store_for_worker.fail` | 2 hits (intact) |

## R4.b + R5.b Synthetic-Failure Timing Tests: DEFERRED to Plan 04-03

The fault-injection scaffolding (`CODENEXUS_EMBED_FAIL=always`) is landed in this plan. Plan 04-03 E2E harness exercises:
- R5.b: `CODENEXUS_EMBED_FAIL=always` + `embed_query` -> Err in <1s wall clock against release binary
- R4.b: `CODENEXUS_EMBED_FAIL=always` + A2A `IndexRepo` -> A2A task state `failed` with consecutive count in error message

The `embed_query_fault_injection` unit test is a smoke check (validates env-var path + <900ms budget) but doesn't run via `cargo test` due to the linker conflict.

## Deviations from Plan

None -- plan executed exactly as written. Three tasks in dependency order, each committed atomically.

## Known Stubs

None. All wired paths are functional: `embed_query` calls `embed_once` directly; fault injection fires on first env-var read; counter loop is live in server.rs dispatch.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: env-var leakage | embedder.rs | `CODENEXUS_EMBED_FAIL` must be unset before production deployment. Test-only feature. Plan 04-03 E2E harness uses `trap 'unset CODENEXUS_EMBED_FAIL' EXIT`. |
| threat_flag: untrusted-usize | server.rs | `max_consecutive_fail` from A2A JSON is bound-checked `1..=100` before use; out-of-bounds returns Err immediately without entering embed loop (T-04-06 mitigated). |

## Honest Gap List

- **P1**: linker conflict `esaxx-rs/ort RuntimeLibrary mismatch (LNK2038/LNK1319)` prevents `cargo test` execution. Pre-existing (not introduced here). Deferred to Plan 04-03 E2E harness which uses release binary (linker conflict doesn't apply to `cargo build --release`).
- **P3**: config.toml middle layer for `max_consecutive_fail` -- out of scope; no config infrastructure exists in poc-retrieval. D-05 simplified: envelope > hardcoded.
- **P3**: `EmbedError` enum + classified retry policies -- Q5=B locked deferred. embed_query is option (i) plain method.
- **P3**: Go-side parity -- Go `IndexRepoArgs` struct needs `max_consecutive_fail` field before Go A2A clients can use the envelope override. Out of scope per CONTEXT.md co-location boundary (Go layer is a separate slice).

## Self-Check: PASSED

- FOUND: experiments/poc-retrieval/src/embedder.rs
- FOUND: experiments/poc-retrieval/src/search.rs
- FOUND: experiments/poc-retrieval/src/a2a.rs
- FOUND: experiments/poc-retrieval/src/server.rs
- FOUND commit: d09d3a9 (Task 1)
- FOUND commit: 4c7694d (Task 2)
- FOUND commit: fafda6e (Task 3)
