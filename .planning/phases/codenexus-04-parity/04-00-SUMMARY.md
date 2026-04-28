---
phase: codenexus-04-parity
plan: "04-00"
subsystem: infra
tags: [cargo, rust, makefile, build-chain, binary-rename]

requires:
  - phase: codenexus-03-retrieval
    provides: experiments/poc-retrieval crate (the Rust core being renamed)

provides:
  - "Cargo [[bin]] name aligned with Makefile: codenexus-core binary artifact"
  - "make build cp step unblocked: codenexus-core.exe present at embed dir"
  - "bin/codenexus.exe (Go wrapper) built end-to-end"

affects:
  - codenexus-04-parity/04-01
  - codenexus-04-parity/04-02
  - codenexus-04-parity/04-03

tech-stack:
  added: []
  patterns:
    - "Cargo package.name and [[bin]] name can diverge: package stays poc-retrieval (crate identity), binary artifact renamed to codenexus-core (build surface)"

key-files:
  created: []
  modified:
    - experiments/poc-retrieval/Cargo.toml

key-decisions:
  - "Rename Cargo [[bin]] to codenexus-core (option a per REVIEWS.md HIGH#2): aligns with Makefile:4 CORE_BIN, productizes the binary name for Phase 4"
  - "Preserve package.name = poc-retrieval: crate identity unchanged, only artifact name changes"
  - "Leave compute_version_hash aux binary unchanged: separate target, not used by Makefile"

patterns-established:
  - "Binary artifact name (codenexus-core) decoupled from crate name (poc-retrieval)"

requirements-completed: [INFRA]

duration: 8min
completed: "2026-04-28"
---

# Phase 4 Plan 00: Pre-flight Cargo bin name reconciliation Summary

**Renamed Cargo [[bin]] from poc-retrieval to codenexus-core, aligning with Makefile CORE_BIN and unblocking the make build cp step and Plan 04-03 E2E harness**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-28T14:20:00Z
- **Completed:** 2026-04-28T14:28:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Renamed `[[bin]] name` in `experiments/poc-retrieval/Cargo.toml` from `"poc-retrieval"` to `"codenexus-core"` (line 46 only)
- Preserved `package.name = "poc-retrieval"` (line 2, crate identity) and `compute_version_hash` aux binary (unchanged)
- `cargo build --release` exits 0, produces `target/release/codenexus-core.exe` (37 MB)
- cp step (`experiments/poc-retrieval/target/release/codenexus-core.exe` -> `server/internal/supervisor/bin/`) succeeds
- `go build` step succeeds, `bin/codenexus.exe` (50 MB) present

## Task Commits

1. **Task 1: Rename [[bin]] to codenexus-core + verify build chain** - `53313b8` (feat)

**Plan metadata:** (included in task commit)

## Files Created/Modified

- `experiments/poc-retrieval/Cargo.toml` - Line 46: `name = "poc-retrieval"` -> `name = "codenexus-core"` (1 line diff)

## Build Chain Verification

### Baseline (pre-rename)
```
warning: `poc-retrieval` (bin "poc-retrieval") generated 3 warnings
Finished `release` profile [optimized] target(s) in 1.01s
target/release\poc-retrieval.exe  <- OLD artifact
```

### Post-rename
```
warning: `poc-retrieval` (bin "codenexus-core") generated 3 warnings
Finished `release` profile [optimized] target(s) in 25.07s
target/release\codenexus-core.exe  <- NEW artifact (37 MB)
```

### cp step
```
server/internal/supervisor/bin/codenexus-core.exe (37 MB) -- present
```

### Go build
```
bin/codenexus.exe (50 MB) -- present
```

### Acceptance criteria check
- `grep -nE '^name = "codenexus-core"'` -- 1 hit (line 46) PASS
- `grep -nE '^name = "poc-retrieval"'` -- 1 hit (line 2, package) PASS
- `grep -nE '^name = "compute_version_hash"'` -- 1 hit (line 50) PASS
- `grep -rnE 'CARGO_BIN_NAME' src/` -- 0 hits PASS
- `cargo build --release` exit 0 PASS
- `codenexus-core.exe` at `target/release/` PASS
- cp step produces `server/internal/supervisor/bin/codenexus-core.exe` PASS
- `bin/codenexus.exe` present PASS

### Cross-reference scan
```bash
grep -rn "poc-retrieval\.exe|cargo run --bin poc-retrieval|target/release/poc-retrieval"
```
Zero hits in code/scripts (excluding .planning/, .git/, target/, Cargo.lock). No other consumers of old binary name.

### Note on `make` availability
`make` is not in PATH on this machine (git-bash). The Makefile steps were executed manually in sequence:
1. `cd experiments/poc-retrieval && cargo build --release` (build-core step)
2. `cp ... server/internal/supervisor/bin/` (embed cp step)
3. `cd server && go build -o ../bin/codenexus.exe .` (build-server step)

All three exited 0. The build chain logic is verified correct; `make` availability is a dev-machine configuration issue, not a code defect.

## Decisions Made

- Option (a) per REVIEWS.md HIGH#2: rename Cargo `[[bin]]` to `codenexus-core`. Rejected option (b) (rename Makefile) because it would propagate POC framing into production surface.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `make` binary not in PATH (git-bash on Windows). Executed Makefile steps manually in sequence. All steps verified to exit 0. Not a code issue.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes introduced. This is a build-system manifest rename only.

## Known Stubs

None.

## Honest Gap List

- **P0:** None
- **P1:** None
- **P2:** `make` not in PATH on dev machine -- developer should confirm `make build` works in their normal shell environment (cmd/PowerShell + GNU Make or WSL). Code is correct; tool availability is environment config.
- **P3:** Stale `target/release/poc-retrieval.exe` artifact remains in build cache (harmless -- Makefile only references `codenexus-core.exe`; cleared by `cargo clean`).

## Next Phase Readiness

- Plans 04-01, 04-02, 04-03 can all reference `./bin/codenexus.exe` and `experiments/poc-retrieval/target/release/codenexus-core.exe` -- both files present
- Build chain (Cargo -> cp -> Go embed) verified end-to-end
- No blockers

---
*Phase: codenexus-04-parity*
*Completed: 2026-04-28*

## Self-Check: PASSED

- `experiments/poc-retrieval/Cargo.toml` modified: FOUND
- Commit `53313b8`: FOUND (git log confirms)
- `experiments/poc-retrieval/target/release/codenexus-core.exe`: FOUND (37 MB)
- `server/internal/supervisor/bin/codenexus-core.exe`: FOUND (37 MB)
- `bin/codenexus.exe`: FOUND (50 MB)
