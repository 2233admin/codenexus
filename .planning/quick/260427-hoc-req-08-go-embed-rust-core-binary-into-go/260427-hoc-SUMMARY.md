---
phase: quick-260427-hoc
plan_id: 260427-hoc
status: complete
type: execute
requirements: [REQ-08]
landed_files:
  - server/internal/supervisor/embed.go            # NEW
  - server/internal/supervisor/bin/.gitkeep        # NEW
  - server/internal/supervisor/rust.go             # MODIFY (extraction call when RustBinPath empty)
  - server/cmd/serve.go                            # MODIFY (resolveRustBinPath priority order)
  - Makefile                                       # MODIFY (EMBED_DIR + clean target)
  - .gitignore                                     # MODIFY (2 new entries for codenexus-core binaries)
commits:
  - f5b6621 "mvp(server): REQ-08 //go:embed Rust core binary plumbing"
  - 59b725b "mvp(server): REQ-08 Makefile EMBED_DIR + .gitignore for embedded Rust binary"
gates:
  go_build: pass
  go_vet: pass
  invariants_verified: 9/9
---

# REQ-08 Summary — //go:embed Rust core binary plumbing

## Landed

**Code (commit f5b6621):**
- `server/internal/supervisor/embed.go` (~135 lines): `//go:embed all:bin` directive + `extractRustBinary()` helper. Idempotent extraction to `os.UserCacheDir()/codenexus/bin/codenexus-core-<coreVersion>/<binname>` per ARCH §5.5 line 379. Size + sha256 check before re-extract. Atomic tmp-file + rename. POSIX exec bit via `os.OpenFile(..., 0o755)` belt-and-suspenders + `os.Chmod(0o755)` (Windows skips chmod naturally).
- `server/internal/supervisor/bin/.gitkeep`: empty file keeping the directory present in fresh clones (without it, `//go:embed all:bin` fails to compile when no real Rust binary has been built yet).
- `server/internal/supervisor/rust.go`: `Start()` now calls `extractRustBinary()` when `cfg.RustBinPath == ""` (replaces previous hard-fail-on-empty). Splice point at the placeholder comment from REQ-07 closed.
- `server/cmd/serve.go`: `resolveRustBinPath(flagVal)` priority order: `--rust-bin` flag > `CODENEXUS_RUST_BIN` env > empty (which triggers Start's extraction). Dev auto-discover (`../core/target/release/...`) preserved as fallback path.

**Build infrastructure (commit 59b725b):**
- `Makefile` line 6: `EMBED_DIR := server/embed` → `server/internal/supervisor/bin` (matches `//go:embed` sibling-path requirement). `clean` target now `rm -f` only the binary files instead of `rm -rf` the entire EMBED_DIR (preserves `.gitkeep` + dir structure).
- `.gitignore`: added `server/internal/supervisor/bin/codenexus-core` and `server/internal/supervisor/bin/codenexus-core.exe` under "# CodeNexus specific" section. `.gitkeep` stays tracked.

**Dependencies:** zero new external deps. Stdlib only: `embed`, `io`, `os`, `path/filepath`, `crypto/sha256`, `errors`, `log/slog`.

## Scaffolded but NOT smoke-tested

- **End-to-end `make build`**: requires working Rust core compile (`make build-core`) + cargo toolchain. Not exercised this slice. Plumbing is build-tested only (with `.gitkeep` as the sole `bin/` content, `go build ./...` exits 0).
- **150 MB total size budget** (REQ-08 acceptance): not measured. Will need `make build` to succeed first; expected ~110-150 MB total (Rust core 80-120 MB release + Go binary ~30 MB).
- **Portability claim** ("runs on machine with no Rust/Go toolchain installed"): not validated. Would need a clean VM/container test. Embed plumbing is necessary-but-not-sufficient for this acceptance.
- **Real extraction-and-spawn smoke**: with only `.gitkeep` in `bin/`, `extractRustBinary()` writes a 0-byte file (or the `.gitkeep` content as binary, which would then fail to spawn). Real binary needs to be cp'd in via `make build-core` before `serve` works end-to-end.

## Follow-up slices

- **REQ-09 UI //go:embed**: same plumbing pattern (`//go:embed all:ui` in a `server/cmd/` or `server/internal/ui/` file). Reuse the embed-and-extract idiom; UI doesn't need extraction (just `http.FS(uiFS)` served by chi), simpler than this REQ-08.
- **Real spawn-and-restart smoke**: after `make build-core` produces a working Rust binary, run `make build` and verify (a) extraction lands a real executable in UserCacheDir, (b) supervisor spawns it, (c) Rust /healthz returns 200 within 30s, (d) external kill triggers restart per REQ-07 acceptance #2.
- **ldflags version injection**: replace `var coreVersion = "dev"` with build-time injection via `go build -ldflags "-X github.com/2233admin/codenexus/internal/supervisor.coreVersion=v0.x.y"`. Mechanism documented in embed.go line 36 comment. Not blocking MVP.
- **Total size CI gate**: add a Makefile `size-check` target or CI step that fails if `bin/codenexus(.exe)` exceeds 150 MB after `make build`.
- **Cross-platform extraction smoke**: verify on Linux (XDG_CACHE_HOME or ~/.cache) + macOS (~/Library/Caches) + Windows (%LOCALAPPDATA%\Cache). `os.UserCacheDir()` handles the path resolution but actual extraction-and-spawn unverified.

## Notable findings (deviations + gotchas)

### Execution mid-handoff

Executor returned partial after Task 1 (commit f5b6621) with Makefile already modified in working tree but NOT committed, and `.gitignore` completely untouched. Orchestrator (Opus, main session) picked up Task 2:
- Reviewed executor's pending Makefile diff: line 6 EMBED_DIR change was correct, but `clean` target (line 48) was unchanged — would have `rm -rf` the whole `bin/` dir including `.gitkeep`. Orchestrator fixed: split into `rm -rf $(BIN_DIR)` + `rm -f $(EMBED_DIR)/$(CORE_BIN) $(EMBED_DIR)/$(CORE_BIN).exe`.
- Added `.gitignore` 3 new lines (1 comment + 2 binary patterns).
- Independent build/vet verification + 9-of-9 invariant grep before commit.

This is a single-session deviation, not a process failure. Documenting because future quick-task post-mortems should know that mid-handoff is a real failure mode and orchestrator-takes-Task-2 is a valid recovery path (vs respawning a fresh executor that lacks the half-done context).

### Plan invariants verified

All 9 invariants from plan frontmatter `must_haves` independently grep-verified by orchestrator before final commits:

| # | Invariant | Verification |
|---|-----------|--------------|
| 1 | embed.go in supervisor pkg | `ls server/internal/supervisor/embed.go` ✓ |
| 2 | `//go:embed all:bin` directive | grep:29 ✓ |
| 3 | UserCacheDir + coreVersion path | grep:71,75 ✓ |
| 4 | POSIX exec permission 0o755 | grep:76,94,109 (3 places — MkdirAll + OpenFile + Chmod belt-and-suspenders) ✓ |
| 5 | rust.go calls extraction when path empty | grep:64-69 ✓ |
| 6 | serve.go priority: flag > env > extraction | grep:50,148,152 ✓ |
| 7 | Makefile EMBED_DIR = server/internal/supervisor/bin | grep:6 ✓ |
| 8 | .gitignore has both binary names | grep:46,47 ✓ |
| 9 | SPDX header on embed.go | head:1 ✓ |

### Unresolved Question outcomes

1. **`coreVersion = "dev"` hardcoded**: accepted as MVP. `embed.go:32-37` comment documents `-ldflags "-X .../supervisor.coreVersion=v0.x.y"` upgrade path.
2. **Belt-and-suspenders OpenFile + Chmod**: kept both. `os.OpenFile(0o755)` sets the mode at creation; `os.Chmod(0o755)` redundantly enforces it after rename in case the umask interferes. Cost is one syscall. Defense-in-depth justified.
3. **sha256 on cold start**: kept full sha256 read (~100ms for 80-120 MB binary). No mtime short-circuit — premature optimization for MVP. Can revisit if cold-start UX complaints surface.
4. **`errors.Is(err, fs.ErrNotExist)`**: confirmed canonical for embed.FS lookups. Used in extract.go's idempotency check.
5. **Go 1.18+ `all:` prefix**: verified — `go.mod` is at 1.23 (REQ-07 set, tidy auto-bumped to 1.25.5 toolchain). `all:` semantics stable since 1.18.

### Plan deviation: clean target structure

Plan said "update `clean` target to remove `server/internal/supervisor/bin/codenexus-core*` (NOT remove the dir itself)". Executor missed this — orchestrator fixed in Task 2. The fix split the original `rm -rf $(BIN_DIR) $(EMBED_DIR)` into two commands so `bin/` (the directory) and `.gitkeep` survive `make clean`.

## Gotchas hit

1. **`cat` aliased to `bat`** (Windows machine): `cat Makefile` returned empty + `bat: command not found` error. Recovery: switched to `Read` tool. `pitfall_cat_aliased_to_bat.md` already documents this; rule `feedback-graduated.md` P1 #33 says "use `/usr/bin/cat`" but the rule kicks in for heredoc subshells, not direct `cat <file>` reads. **Lesson for future quick tasks: prefer `Read` tool for file inspection, reserve Bash for cmds that genuinely need shell.**

2. **`//go:embed all:` semantics with .gitkeep**: confirmed `all:bin` correctly includes dotfiles like `.gitkeep`. Without `all:` prefix, dotfiles are silently excluded (Go embed default), and an empty pattern match would fail compile. The `all:` prefix is load-bearing — do NOT drop it when copying this pattern for REQ-09.

3. **Hook misroute warning on `git commit`**: PreToolUse hook flagged `git commit` as "may modify source files" with delegation suggestion. Commits don't modify source — commits write to `.git/`. Safe to ignore for git operations specifically. Not blocking.
