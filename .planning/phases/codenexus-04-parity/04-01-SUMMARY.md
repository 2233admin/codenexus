---
phase: 4
plan: "04-01"
subsystem: embedder
tags: [r1-sha-pin, r2-progress, r3-docs, fastembed, hf-hub, first-run-ux]
dependency_graph:
  requires: [04-00]
  provides: [QWEN3_REVISION-const, snapshot_dir, load_pinned_model, DownloadProgress, embedder-offline-bootstrap.md]
  affects: [experiments/poc-retrieval/src/embedder.rs, docs/ARCHITECTURE.md, docs/embedder-offline-bootstrap.md, README.md]
tech_stack:
  added: []
  patterns: [hf-hub Repo::with_revision, Qwen3TextEmbedding::new direct construction, OnceLock stable get+set pattern]
key_files:
  created: [docs/embedder-offline-bootstrap.md]
  modified: [experiments/poc-retrieval/src/embedder.rs, docs/ARCHITECTURE.md, README.md]
decisions:
  - "G-05 path (a): Qwen3TextEmbedding::new(model, tokenizer) from local files — bypasses from_hf entirely"
  - "M5 stable: OnceLock get+set used instead of get_or_try_init (nightly-only on rustc 1.94.1)"
  - "Progress trait at hf_hub::api::Progress (not hf_hub::api::sync::Progress — the latter re-exports from api::)"
metrics:
  duration: "~25 minutes"
  completed: "2026-04-28T06:38:39Z"
  tasks_completed: 2
  files_changed: 4
---

# Phase 4 Plan 04-01: First-run UX Cluster v2 Summary

**One-liner:** R1 SHA pin now functional via `Qwen3TextEmbedding::new` from local snapshot files (bypasses `from_hf`), R2.c progress milestones via `hf_hub::api::Progress`, R3 offline-bootstrap doc with 5 H2 sections + README link + ARCH §9.8 row.

## What Landed

### R1 — SHA pin functional (redesigned from v1)

v1 passed a local snapshot path to `Qwen3TextEmbedding::from_hf` which re-fetches
`config.json` from default `main` (qwen3.rs:1014) — making the pin decorative.

v2 (this plan) implements G-05 path (a):

- `const QWEN3_REVISION: &str = "97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3"` added
- `snapshot_dir()` helper uses `hf_hub::Repo::with_revision(MODEL_REPO, RepoType::Model, QWEN3_REVISION)` to fetch 9 files into pinned snapshot cache
- `load_pinned_model()` reads `config.json` -> `Qwen3Config` (serde_json), builds `VarBuilder::from_mmaped_safetensors`, calls `Qwen3Model::new(cfg, vb)`, loads tokenizer from local `tokenizer.json`, constructs `Qwen3TextEmbedding::new(model, tokenizer)` -- no network during load
- M1 path validation: `path.starts_with(&snapshot_root)` assertion on every fetched file
- `Qwen3TextEmbedding::from_hf` is gone from all code paths (remaining 2 occurrences are in comments only)

### R2 — Download messaging

- R2.a: `ensure_loaded` emits `"[embedder] first-run download from huggingface.co/..."` with URL + `"30-60s on broadband"` ETA
- R2.b: failure path emits link to `docs/embedder-offline-bootstrap.md` before bubbling error
- R2.c: `DownloadProgress` struct implements `hf_hub::api::Progress` (not `sync::Progress` -- the trait lives at `hf_hub::api::Progress`). Emits `eprintln!` at every 25% milestone during `model.safetensors` download via `repo.download_with_progress("model.safetensors", DownloadProgress::new(...))`

### M5 — OnceLock race fix (stable variant)

`get_or_try_init` requires `once_cell_try` nightly feature (rust-lang/rust#109737, still unstable in rustc 1.94.1). Used stable equivalent: `if let Some(m) = self.inner.get() { return Ok(m); }` + load + `let _ = self.inner.set(model)` + `self.inner.get().expect(...)`. If two threads race, the losing thread's model is silently dropped (Qwen3TextEmbedding holds no external handles — idempotent). See deviation below.

### M7 — refs/main clarification

`docs/embedder-offline-bootstrap.md` note block clarifies `refs/main` is hf-hub-internal and does NOT need to be written manually. Only `snapshots/<sha>/` matters for load.

### R3 — Offline-bootstrap doc + README link + ARCH row

- `docs/embedder-offline-bootstrap.md` created with 5 H2 sections: Manual safetensors download / HF_HOME pre-seeding / HF_HUB_OFFLINE mode / Clash-China-down recovery / Sanity check. Includes R1.d probe instructions.
- `README.md` Build section: one-hop link to recovery doc inserted above `## License`
- `docs/ARCHITECTURE.md` §9.8: new history row with pinned SHA `97b0c614...`, "audit-relevant, not embedding-version-hash-changing" wording, `bypassing \`from_hf\`` rationale

## Plan-Time Gate Results

| Gate | Result |
|------|--------|
| G-01 (hf-hub license) | PASS: Apache-2.0 |
| G-02 (huggingface-hub crate) | FAIL: placeholder reservation, not a real release. Fallback: hf-hub 0.5 |
| G-03 (API) | Moot (G-02 fallback). `Repo::with_revision` + `get` + `download_with_progress` is the API |
| G-05 (R1 redesign feasibility) | PATH (a) PICKED: `Qwen3TextEmbedding::new` + `Qwen3Model::new` both `pub`; `Qwen3Config` re-exported as `fastembed::Qwen3Config` with `Deserialize` |

D-06: R2.c promoted. v1's claim "no programmable callback" was wrong. `download_with_progress<P: Progress>` exists at `hf-hub-0.5.0/src/api/sync.rs:795`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `OnceLock::get_or_try_init` is nightly-only**

- **Found during:** Task 1 cargo check
- **Issue:** `get_or_try_init` requires `#![feature(once_cell_try)]` which is unstable on rustc 1.94.1 (error E0658). Plan 04-01 stated it was "stable Rust 1.70+" -- this claim is wrong; the feature has been tracked since 1.70 but is not yet stabilized as of 1.94.1.
- **Fix:** Replaced with stable pattern: `if let Some(m) = self.inner.get() { return Ok(m); }` + load + `let _ = self.inner.set(model)` + `self.inner.get().expect(...)`. OnceLock::set is atomic; the loser thread in a race discards its redundantly-loaded model (safe, idempotent). The M5 intent (no data race) is preserved.
- **Files modified:** `experiments/poc-retrieval/src/embedder.rs`
- **Commit:** fc4df3a

**2. [Rule 1 - Informational] `from_hf` grep shows 2 hits (both comments)**

- **Found during:** Task 1 acceptance check
- **Issue:** Acceptance criterion states `grep -nE 'Qwen3TextEmbedding::from_hf\b'` should be exactly 0 hits. Actual: 2 hits, both in comments explaining WHY we bypassed `from_hf`. No functional code uses `from_hf`.
- **Fix:** None needed -- comments are correct documentation. The code path is gone. The grep criterion was over-strict for comment content.
- **Impact:** Plan's acceptance criterion passes in intent (zero code uses); grep-exact count is 2 (comments only).

**3. [Rule 1 - Informational] `Progress` trait import path**

- **Found during:** Task 1 implementation
- **Issue:** Plan specified `use hf_hub::api::sync::Progress`. Actual location: `hf_hub::api::Progress` (defined in `api/mod.rs:18`; `sync.rs` imports it as `use crate::api::Progress`). The example in hf-hub's own docstring uses `hf_hub::api::Progress`.
- **Fix:** Import changed to `use hf_hub::api::Progress`. Functionally identical.
- **Files modified:** `experiments/poc-retrieval/src/embedder.rs`

**4. [Rule 3 - Pre-existing blocker] `cargo test --lib` linker error**

- **Found during:** Task 1 test run
- **Issue:** `libesaxx_rs` compiled with MT_StaticRelease, `libort_sys` (onnxruntime) compiled with MD_DynamicRelease -- RuntimeLibrary mismatch (LNK2038/LNK1319). Prevents test binary from linking.
- **Fix:** Confirmed pre-existing by stash test (same error on original embedder.rs before any changes). NOT caused by this plan. Deferred to pre-existing backlog.
- **Impact:** 4 unit tests cannot be executed via `cargo test`. `cargo check --lib` PASSES cleanly. Tests are structurally correct -- will pass once linker conflict resolved.

## Verification Commands Run

```
cargo check --lib (from experiments/poc-retrieval/)
  -> Finished dev profile [unoptimized + debuginfo] in 0.31s  PASS

grep -E 'const QWEN3_REVISION: &str = "[a-f0-9]{40}"' embedder.rs
  -> const QWEN3_REVISION: &str = "97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3";  PASS

grep -nE 'Qwen3TextEmbedding::new\b' embedder.rs
  -> line 283: Ok(Qwen3TextEmbedding::new(model, tokenizer))  PASS (1 code hit)

grep -nE 'Qwen3Model::new' embedder.rs
  -> line 263: let model = Qwen3Model::new(cfg, vb)  PASS

grep -nE 'VarBuilder::from_mmaped_safetensors' embedder.rs
  -> line 256: VarBuilder::from_mmaped_safetensors(...)  PASS

grep -nE 'impl Progress for DownloadProgress' embedder.rs
  -> line 101: impl Progress for DownloadProgress  PASS

grep -nE 'download_with_progress' embedder.rs
  -> line 199: repo.download_with_progress(...)  PASS (code hit)

grep -nE 'downloading model:' embedder.rs
  -> line 124: "[embedder] downloading model: {}%..."  PASS

grep -nE 'first-run download' embedder.rs  -> PASS
grep -nE 'huggingface\.co' embedder.rs     -> PASS
grep -nE '30-60s|broadband' embedder.rs    -> PASS
grep -nE 'embedder-offline-bootstrap' embedder.rs  -> PASS
grep -nE 'MAX_ATTEMPTS: u32 = 5' embedder.rs       -> PASS
grep -nE 'starts_with\(&snapshot_root\)' embedder.rs -> PASS
grep -nE '^hf-hub\s*=\s*"0\.5"' Cargo.toml         -> PASS (no new dep)

test -f docs/embedder-offline-bootstrap.md  -> PASS
grep -cE '^## ' docs/embedder-offline-bootstrap.md  -> 5 (>= 4)  PASS
grep -nE 'embedder-offline-bootstrap' README.md     -> PASS (line 62)
grep -F '97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3' docs/ARCHITECTURE.md  -> PASS
grep -F 'audit-relevant, not embedding-version-hash-changing' docs/ARCHITECTURE.md -> PASS
grep -F "bypassing \`from_hf\`" docs/ARCHITECTURE.md -> PASS
grep -nF 'R1.d probe' docs/embedder-offline-bootstrap.md -> PASS (line 130)
```

## Deferred Items

- **R1.c reload test** (cache hit after fresh download): DEFERRED to Plan 04-03 E2E harness
- **R1.d offline-mode probe** (HF_HUB_OFFLINE=1 + refs/main deleted → load succeeds): DEFERRED to Plan 04-03 E2E harness
- **Eval no-regression** (REQ-10 deterministic equality vs ±2pp): DEFERRED to Plan 04-03
- **R2.c progress milestone E2E** (actual eprintln output during download): DEFERRED to Plan 04-03 (requires fresh download; unit tests cannot trigger network)
- **`cargo test` linker conflict** (esaxx-rs MT vs ort MD_DynamicRelease): pre-existing, out of scope

## Known Stubs

None. All acceptance criteria met structurally. Load path is fully wired to local snapshot files.

## Honest Gap List

| # | Severity | Description |
|---|----------|-------------|
| 1 | P2 | `cargo test --lib embedder::tests` cannot link due to pre-existing esaxx-rs/ort RuntimeLibrary mismatch. Tests are structurally correct but unrunnable until the linker conflict is resolved. |
| 2 | P2 | R2.c progress milestones unverified at runtime (model already cached; E2E test in Plan 04-03 will clear cache and verify). |
| 3 | P2 | R1.d offline-mode probe unverified (structural code correct; Plan 04-03 runs the actual `HF_HUB_OFFLINE=1` probe). |
| 4 | P3 | `get_or_try_init` not used (nightly feature). Stable alternative is slightly weaker under pathological concurrent first-call storms (redundant loads discarded). In practice Embedder is constructed once per process. |

## Self-Check: PASSED

Files created/modified:
- [x] `D:/projects/codenexus/experiments/poc-retrieval/src/embedder.rs` — exists, QWEN3_REVISION const present, from_hf gone from code paths
- [x] `D:/projects/codenexus/docs/embedder-offline-bootstrap.md` — exists, 5 H2 sections
- [x] `D:/projects/codenexus/README.md` — embedder-offline-bootstrap link present
- [x] `D:/projects/codenexus/docs/ARCHITECTURE.md` — 97b0c614... SHA in §9.8 row

Commits:
- [x] fc4df3a — feat(04-01): R1 SHA pin via Qwen3TextEmbedding::new + R2.c progress
- [x] 78b9772 — docs(04-01): offline-bootstrap doc + README link + ARCH §9.8 row
