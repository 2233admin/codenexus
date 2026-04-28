# Plan 04-06 Summary -- IndexRepo Transactional Safety + r4b_probe.sh Warning

**Plan:** 04-06 (Phase 4 first slice -- P1 finding mitigation, post-04-05)
**Date:** 2026-04-28
**Status:** **PASS** -- IndexRepo handler now preserves pre-existing data when all embeds fail; R4.b probe semantics intact; r4b_probe.sh head comment carries explicit destructive warning until lazy-clear was wired up. Hot-fix slice, no separate plan file (scope ~1 sentence; spec was the P1 finding section in 04-05-SUMMARY).

## Executive summary

The 04-05-SUMMARY P1 finding identified that `OperationRequest::IndexRepo` in `server.rs` called `Store::clear()` (DELETE FROM symbols + symbols_fts) **before** the embed loop, with no transaction wrapping. Result: any synthetic-fail / network-down / parse-fine-but-embed-bail scenario that returned `Err` after `clear()` left the symbols table empty, destroying pre-existing data.

This slice ships the **deferred-clear fix**: the `clear()` call moves from handler entry (line 203 pre-fix) to a lazy gate inside the embed loop, fired only after the first successful embed. Mechanically, this is the same pattern `main.rs CLI Index` already uses -- the consecutive_fails counter operates on streaming inserts, never on a "wipe-and-pray" clear up front. This is a behaviour fix, not an architectural redesign; no transaction primitive added (rusqlite tx wrapping was an alternative considered but skipped because it would require holding an open tx for the full ~70 min reindex of a 2000-symbol corpus, with no progress visibility for other A2A clients).

## Acceptance evidence

### Mechanical test: marker survival across synthetic-fail R4.b probe

```
PRE-PROBE  symbols count: 1
PRE-PROBE  marker present: ('MARKER_BEFORE_R4B',)

[run r4b_probe.sh against poc.db with CODENEXUS_EMBED_FAIL=always]
[R4.b] state=failed elapsed=20s
[R4.b] PASS: A2A failed state with consecutive count in 20s

POST-PROBE symbols count: 1
POST-PROBE marker present: ('MARKER_BEFORE_R4B',)
VERDICT: PASS -- fix preserved data on synthetic-fail IndexRepo
```

Two invariants both hold:
- **R4.b semantics preserved** -- fault injection still triggers `consecutive_fails >= 5` bail; A2A task transitions to `failed` state with structured "5 consecutive embed failures" error (same as 04-04 followup gate).
- **Pre-existing data preserved** -- the marker row is still in the symbols table post-probe.

### Build status

```
$ cargo build --release
warning: poc-retrieval (bin "codenexus-core") generated 3 warnings (pre-existing)
   Finished `release` profile [optimized] target(s) in 18.11s
BUILD_EXIT=0
```

3 warnings are pre-existing (count_edges_by_kind_conf, etc.); not introduced by this slice.

## What landed

### Code change -- `experiments/poc-retrieval/src/server.rs`

**Diff scope:** 3 surgical edits totalling ~25 lines (~12 lines net additions, with comment blocks documenting the rationale + 04-05-SUMMARY P1 findings cross-reference). All other code paths untouched.

1. **Handler entry (line 199-218):** removed `store.clear()?` from line 203; replaced with multi-line comment explaining the deferred-clear rationale and pointing at Plan 04-05 P1 finding.

2. **Loop initialization (lines 234-239):** added `let mut cleared = false;` flag alongside `indexed` and `consecutive_fails` counters, with comment.

3. **Insert site (lines 272-280):** added `if !cleared { store.clear()?; cleared = true; }` block immediately before `store.insert()`. Brief non-atomic window between clear and insert is acceptable since IndexRepo is one-A2A-client-per-request.

### Documentation -- `experiments/poc-retrieval/eval/r4b_probe.sh`

Added a multi-line WARNING block at the top of the script header explaining:
- The destructive-on-failure behaviour pre-04-06.
- The 04-05 T2 design moment when this surfaced.
- The 04-06 fix landing reference.
- A "do not run against a poc.db / fsc.db you rely on" guidance line for the period before the fix lands (now historical, but kept as audit trail of how the issue was discovered).

This warning is now technically obsolete (the fix has landed), but kept as a Chesterton's-Fence breadcrumb -- if a future contributor edits IndexRepo and accidentally re-introduces eager clear, the warning resurfaces in the probe header to flag the regression.

## Why deferred clear, not transaction wrapping

Both options were considered:

| Option | Pros | Cons |
|--------|------|------|
| **(A) Wrap entire IndexRepo in a single rusqlite Transaction** | Cleanest atomicity; full rollback on any error including parse failures | Requires holding an open tx for the full ~70 min reindex of a 2000-symbol corpus; other A2A clients see stale data until commit; rusqlite buffers tx state in memory (~symbols × 1KB graph state); harder to reason about concurrent access |
| **(B) Deferred clear, gated by `cleared = false` flag** | Minimal code change (~12 lines); same pattern as main.rs CLI Index already uses; no long-held tx; new data still replaces old once first embed succeeds | Brief non-atomic window between clear and first insert (single statement gap; one-client-per-request mitigates); does NOT protect against mid-reindex failures (if first 100 embeds succeed then next 5 fail, partial state remains -- same as pre-04-06 main.rs CLI Index behaviour) |

Chose (B) because it ships the data-preservation property the P1 finding actually flagged (synthetic-fail / network-down don't destroy data) without architectural rework. Mid-reindex partial state is a separate property (failure-recovery semantics) that 04-02 R4 consecutive_fails counter already addressed at the policy layer; (A) would over-engineer this slice.

Future Plan candidate: full rusqlite Transaction wrap if A2A clients ever need atomic-reindex semantics (e.g. "either the new index is complete or the old one is intact"). Not blocking any active work.

## Honest gap list

### P2 -- known follow-ups

- **Mid-reindex partial state** -- if first N embeds succeed (clear fires) then next M consecutive fail (bail), the DB is left with N inserted rows + 0 of the remaining (total - N - M) rows. This is the same behaviour as pre-04-06 `main.rs` CLI Index. Not a regression; not in this slice's scope. Fix candidate: rusqlite Transaction wrapping the entire reindex op.
- **No automated test for the destructive scenario** -- this slice verified the fix manually via Python sqlite3 marker injection + R4.b probe re-run. A unit test (`cfg(test)`-only fault injection or temporary DB harness) would lock the regression. Pre-existing `cargo test` linker conflict (esaxx-rs/ort RuntimeLibrary mismatch LNK2038/LNK1319, noted in 04-02-SUMMARY) blocks this; deferred until linker fix lands.

### P3 -- out of scope

- **rusqlite Transaction wrap** -- atomic reindex semantics; deferred until needed.
- **Audit other call sites** -- main.rs Index doesn't call clear() at all (CLI semantics differ: append-only). Server.rs IndexRepo was the only destructive entry. No other call sites need this fix.

## Files changed

- `experiments/poc-retrieval/src/server.rs` (3 edits, ~25 lines net change)
- `experiments/poc-retrieval/eval/r4b_probe.sh` (warning block in head comment, ~16 lines)
- `.planning/phases/codenexus-04-parity/04-06-SUMMARY.md` (THIS FILE, NEW)

## Wall-clock budget actuals

- server.rs read + design (Option A vs B trade-off): ~5 min
- 3 server.rs edits + comment blocks: ~5 min
- cargo build --release: ~3 min (incremental, only server.rs touched)
- Python sqlite3 marker harness + R4.b probe re-run: ~3 min
- r4b_probe.sh warning block: ~2 min
- This SUMMARY: ~10 min
- **Total: ~28 min** -- well under the 30-45 min plan estimate from 04-05-SUMMARY recommended next actions.

## Slice closure cross-references

- `04-05-SUMMARY.md` -- P1 finding "R4.b probe destructive on existing DB" introduced this work.
- `04-04-FOLLOWUP-SUMMARY.md` -- R4.b probe addendum (commit 9a326d1) is the diagnostic that surfaced the bug.
- `r4b_probe.sh` head comment -- now carries the historical record of how the bug was discovered.

## Net Phase 4 first slice gates after 04-04 + 04-05 + 04-06

| Gate | Status |
|------|--------|
| EVAL_NO_REGRESSION (deterministic equality) | PASS (04-04) |
| R1.d offline-mode probe | PASS (04-04) |
| R5.b synthetic-failure Query <1s | PASS (04-04) |
| R4.b synthetic-failure A2A IndexRepo | PASS (04-04 followup, R4.b semantics now non-destructive per 04-06) |
| R1.c reload test | PASS (04-05, file-level sha256) |
| FIRSTRUN-UX-PRE-SEED | PASS (04-05) |
| FIRSTRUN-UX-DOC-LINK | PASS (04-05) |
| **IndexRepo non-destructive on failure** (NEW gate from P1 finding) | **PASS (04-06)** |
| E2E harness gates 1b/4/5/6 | DEFERRED -> P3 Linux/macOS smoke (04-05 framing) |

**Phase 4 first slice runtime gates: 8 of 9 PASS, 1 P3-deferred cluster.**

---

*Phase 4 first slice closure addendum: P1 IndexRepo transactional safety bug fixed via deferred-clear pattern. R4.b semantics preserved; existing data preserved. Hot-fix slice, ~28 min wall-clock, no architectural redesign needed.*
*Closure timestamp: 2026-04-28T18:55+08:00*
