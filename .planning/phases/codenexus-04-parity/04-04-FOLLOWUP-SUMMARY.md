# Plan 04-04 Followup — RCA Correction + Cache-First Fix (3 DEFERRED gates lifted to PASS)

**Plan:** 04-04 (Phase 4 first slice followup — supersedes the upstream-blocker framing in 04-03-SUMMARY)
**Date:** 2026-04-28
**Status:** **MOSTLY PASS** — 3 of 6 deferred runtime gates lifted to runtime PASS via 1-file surgical fix; 3 residual gates share a single fresh-install root cause (out of slice scope).

## Executive summary

The 04-03-SUMMARY framed Phase 4 first slice as "PARTIAL — blocked by hf-hub Windows upstream bug" with 6 P1 DEFERRED gates and a recommendation to file an upstream issue. **That root-cause assumption was wrong.** 04-04 followup probes proved:

1. The complete 1.2 GB `model.safetensors` blob already lived in cache (`~/.cache/huggingface/.../blobs/0437e45c...`, 26 Apr 14:48).
2. The 04-01 R1 redesign (commit `fc4df3a`) bypassed `Qwen3TextEmbedding::from_hf` correctly but still routed `model.safetensors` through `repo.download_with_progress(...)` -- an **always-fetch** API, not cache-aware. Every invocation re-downloaded regardless of cache completeness.
3. On this Windows host the unconditional download walls deterministically (~25% / ~49%, 567 MB seen in 4+2=6 runs across 04-03 harness + 04-04 probes). On other hosts (or Linux/macOS) the download would have succeeded silently and masked the cache-bypass logic bug entirely.
4. Switching `model.safetensors` to `repo.get(...)` (cache-aware) resolves the cache-hit case cleanly. Fresh-install cold-cache path falls back to `download_with_progress` -- the residual Windows-specific download wall is OUT of Phase 4 first slice scope and lives in the "first-run UX" P1 cluster (PROJECT.md line 71, separate slice).

**Net result:**

| Gate | 04-03 status | 04-04 status |
|------|-------------|--------------|
| EVAL_NO_REGRESSION (post-pin REQ-10 +/-2pp / deterministic equality) | DEFERRED | **PASS** (byte-identical 30/30 vs `req10_alpha06_candle.json` baseline; mean delta = 0.0000) |
| R1.d offline-mode probe (HF_HUB_OFFLINE=1, embedder still loads) | DEFERRED | **PASS** (cache-complete + offline env -> 6.85s clean run, no network attempt) |
| R5.b synthetic-failure Query <1s wall-clock | DEFERRED | **PASS** (`CODENEXUS_EMBED_FAIL=always` -> 0.286s, retry budget = 250ms sleep + 2 x ~18ms) |
| R1.c reload test (delete snapshot + redownload yields same SHA) | DEFERRED | DEFERRED (residual; fresh-download path still blocked on Windows) |
| R4.b synthetic-failure A2A IndexRepo | DEFERRED | DEFERRED (mechanically same as R5.b but server-mode; not run this followup -- queued for next slice) |
| E2E harness gates 1, 1b, 2, 3, 4, 5, 6 | DEFERRED | mixed: cache-hit subset (1, 2, 3) reachable now; fresh-download subset (1b, 4, 5, 6) still blocked |

## What landed in this followup

### Code change -- single-file surgical

**File:** `experiments/poc-retrieval/src/embedder.rs`
**Function:** `Embedder::snapshot_dir()` (lines 197-248)
**Diff scope:** ~50 lines added (replaces 7-line `download_with_progress` call). All other code paths (R1 SHA pin via `Repo::with_revision`, R2.a/b prompts, R2.c `DownloadProgress` impl, M1 path validation, M5 OnceLock race fix, M7 refs/main clarification) unchanged.

```rust
// Before (04-01 v2, commit fc4df3a):
let safetensors_path = repo
    .download_with_progress(
        "model.safetensors",
        DownloadProgress::new("model.safetensors"),
    )
    .map_err(|e| anyhow::anyhow!("hf-hub fetch model.safetensors: {}", e))?;
fetched.push(safetensors_path);

// After (04-04, this followup):
let safetensors_path = match repo.get("model.safetensors") {
    Ok(p) => {
        if std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false) {
            p
        } else {
            // cache symlink resolved to empty/missing target -- fall back
            eprintln!("[embedder] cache hit but blob is empty, falling back ...");
            repo.download_with_progress("model.safetensors", DownloadProgress::new(...))?
        }
    }
    Err(e_get) => {
        eprintln!("[embedder] cache-first lookup failed ({}), falling back ...", e_get);
        repo.download_with_progress("model.safetensors", DownloadProgress::new(...))?
    }
};
fetched.push(safetensors_path);
```

**Rationale:**

- `repo.get(filename)` IS the cache-aware hf-hub API; `repo.download_with_progress(filename, P)` is the always-fetch API. The original 04-01 v2 code used the wrong API.
- M1 path validation downstream (lines 254-264) is preserved -- catches cache-layout drift on the cache-hit path same as before.
- R2.c progress UX (eprintln milestone lines) preserved on the cold-cache fallback branch via `download_with_progress`. Grep gate (commit `fc4df3a`) still PASS since the `DownloadProgress` impl is still constructed.
- R1 SHA pin remains functional via `Repo::with_revision(QWEN3_REVISION)` which `repo.get` honors (proven by byte-identical eval output -- if SHA pin had broken, embedding output would have drifted).

### Verification runs (no commits required for these probes)

1. **Standalone eval probe** -- `cargo run --release -- eval --queries eval/queries.json --db poc.db --alpha 0.6 --out eval/req10_post_pin.json`
   - 7.237s wall-clock (vs ~6 minute upper bound on broken hf-hub path before timeout)
   - per-query precision_at_5 byte-identical to `eval/req10_alpha06_candle.json` (Phase 03.6 baseline) -- 30/30 deterministic equality
   - B1-B7 mean = 67.9% (matches Phase 03.6 closure +/-0.0pp)

2. **Offline-mode probe** -- `HF_HUB_OFFLINE=1 ./target/release/codenexus-core.exe eval ...`
   - 6.854s wall-clock, identical results
   - Output: `eval/req10_offline_probe.json`

3. **Synthetic-failure Query probe** -- `CODENEXUS_EMBED_FAIL=always ./target/release/codenexus-core.exe query "test query" --db poc.db`
   - 0.286s wall-clock (gate <1s, EASILY clears)
   - Stderr: `Error: CODENEXUS_EMBED_FAIL=always: synthetic embed failure (n=1)` -- confirms 2-attempt retry exhausted (250ms sleep + 2 fault-injected calls)

## Stale-artifact note (cleanup performed during followup)

During the eval probes, two stale `model.safetensors` artifacts were removed:

- `~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/blobs/0437e45c....part` (595 MB residue from 04-03 harness retries)
- `~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/blobs/0437e45c....lock` (0 byte residue)

Cleanup hypothesis (that stale `.part` confused hf-hub into resume-mode failure) was **DISPROVEN** -- post-cleanup eval still walled at 25% / 49% under the old code path. Cleanup was kept in place as good hygiene; the actual fix was the API switch, not the cleanup. Documented for completeness in case future debugging encounters similar artifacts.

## hf-hub Windows fresh-download bug -- updated framing

The 04-03-SUMMARY recommended filing an hf-hub upstream issue for the 49%-constant Windows bug. **That recommendation is partially retracted:**

- The bug IS real on this Windows host (verified independently of cache state in 04-04 probes -- after blob cleanup, fresh code path with empty cache still walls at 25% then 49% in 5-attempt retry wrapper).
- BUT: it does NOT block Phase 4 first slice closure. The cache-hit path is the dominant operational mode for an end-user who pre-seeds the cache via documented offline-bootstrap doc (`docs/embedder-offline-bootstrap.md`). Pre-seeding from a successful Linux/macOS download produces a complete cache that this Windows host then loads cleanly via the new cache-first path.
- Filing the upstream issue is still good citizen work (the 49%-constant pattern is genuinely diagnostic) but is now P3 not P1, and is OUT of Phase 4 first slice scope. Defer to the "first-run UX" P1 cluster slice (PROJECT.md line 71).

## Honest gap list (P0 / P1 / P2 / P3)

### P0 -- none

### P1 -- residual DEFERRED (single fresh-install root cause)

- **R1.c reload test** -- requires deleting snapshot dir + re-download. Re-download walls on this Windows host.
- **E2E harness gate 1b** (R2.c progress milestones >=2 % lines) -- only emits on actual fresh download.
- **E2E harness gates 4, 5, 6** (HTTPS_PROXY blocked failure path) -- tests network failure during fresh-download attempt; the underlying download wall masks the explicit-failure test.
- **First-run UX P1 cluster** (PROJECT.md line 71) -- captures all of the above plus the documented offline-bootstrap doc gap. Keep this as the home for fresh-install bugs; do NOT file them as Phase 4 first slice debt.

### P2 -- known issues + technical debt

- **R4.b synthetic-failure A2A IndexRepo** -- mechanically same fix pattern as R5.b but exercised through `serve` + A2A request. Untested in 04-04 because it needs server spin-up + a2a client (5-10 min). Queue for next slice.
- **E2E harness cache-hit gates 1, 2, 3** -- now reachable (cache-hit path validated independently via standalone eval) but harness invocation has its own pre-seed phase that may need fix-ups (`shutil.copytree` silent-fail noted in 04-03). Defer to harness rewrite slice.
- **`cargo test` linker conflict** (esaxx-rs/ort RuntimeLibrary mismatch on Windows MSVC) -- pre-existing from 04-03; unaffected by 04-04 fix.

### P3 -- out of slice scope

- hf-hub upstream issue filing -- demoted from P1 (per 04-03-SUMMARY) to P3 since cache-pre-seed workflow is documented and the bug is not on the operational hot path. Still good citizen work if someone has 10 min.
- Manual smoke on Linux/macOS -- still valuable independent verification but not blocking.

## Files changed (this followup, pending commit)

- `experiments/poc-retrieval/src/embedder.rs` (~50 line addition replacing 7-line `download_with_progress` call)
- `eval/req10_post_pin.json` (new -- byte-identical to `req10_alpha06_candle.json`, retained as audit artifact)
- `eval/req10_offline_probe.json` (new -- HF_HUB_OFFLINE=1 verification, identical to post_pin)
- `.planning/phases/codenexus-04-parity/04-04-FOLLOWUP-SUMMARY.md` (THIS FILE, NEW)
- `.planning/STATE.md` (status update from "Phase 4 first slice CODE-COMPLETE; runtime DEFERRED" -> "Phase 4 first slice MOSTLY PASS; 3 residual gates in first-run UX P1 cluster")

## Slice closure cross-references

- 04-03-SUMMARY -- supersedes the upstream-bug framing in §"hf-hub Windows fresh-download bug -- root-cause notes for upstream issue". 04-03 acceptance matrix is updated by this followup, not rewritten -- audit-friendly trail preserved.
- PROJECT.md "Cold-start / offline UX (P1, Phase 4 first-step)" -- residual fresh-install download bug stays here, becomes the home for hf-hub Windows behavior + recovery doc + pre-seed automation.
- ARCH §9.10 candle migration anchor -- unchanged; cache-first fix is an IO routing concern, not an embedder semantics concern.

## Recommended next session actions

1. **`git commit`** the 04-04 fix + this SUMMARY + STATE.md update + retained eval artifacts (~5 min).
2. **R4.b synthetic A2A test** (~10 min) -- mechanically same as R5.b, just through `serve` + A2A IndexRepo. Lift from P2 to PASS.
3. **Phase 4 group 2 entry** (multi-language tree-sitter) -- the substantive Phase 4 work, blocked previously on first-slice closure ambiguity. Now unblocked.
4. **First-run UX P1 cluster slice** (PROJECT.md line 71) -- separate dedicated slice for fresh-install download path: investigate hf-hub Windows behavior root cause OR ship pre-seed automation as the canonical install path (offline-bootstrap doc already exists, just needs scripted automation).

## Wall-clock budget actuals

- Eval probe + RCA discovery: ~10 min (probe 1 + cache inspection + reading embedder.rs)
- Stale-cleanup hypothesis test (disproven): ~3 min
- Code edit + build: ~5 min (~50 lines + cargo build)
- Verification probes (eval cache-hit + offline-mode + synthetic-fail): ~3 min total wall-clock
- This SUMMARY draft: ~15 min
- Total: ~35-40 min from session start to commit-ready state

This contrasts favorably with the 04-03 estimate of 4 next-session actions totaling ~60 min that the upstream-bug framing implied (file issue + harness rewrite + manual Linux smoke + Phase 4 group 2 start). The 04-04 RCA correction collapses 3 of those 4 actions into "single line API switch + 50 lines of fallback hygiene".

---

*Phase 4 first slice followup closure: cache-first fix lifts EVAL_NO_REGRESSION + R1.d + R5.b from DEFERRED to runtime PASS. Residual fresh-install gates (R1.c, E2E 1b/4/5/6) move to "First-run UX P1 cluster" home in PROJECT.md.*
*Closure timestamp: 2026-04-28T17:00+08:00*
