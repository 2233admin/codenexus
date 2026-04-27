---
phase: quick-260427-hoc
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - server/internal/supervisor/embed.go            # NEW
  - server/internal/supervisor/bin/.gitkeep        # NEW
  - server/internal/supervisor/rust.go             # MODIFY (replace placeholder comment + extraction call)
  - server/cmd/serve.go                            # MODIFY (priority order: flag/env → embed → dev fallback)
  - Makefile                                       # MODIFY (EMBED_DIR + clean target)
  - .gitignore                                     # MODIFY (2 new entries)
autonomous: true
requirements:
  - REQ-08
must_haves:
  truths:
    - "Go server binary embeds Rust core binary at compile time via //go:embed"
    - "On first serve, Rust binary extracts to OS user-cache-dir/codenexus/bin/codenexus-core-<version>/ idempotently"
    - "Extracted binary is exec-permissioned on POSIX (0755); Windows skips chmod"
    - "Build succeeds in fresh clone with only .gitkeep present (no real Rust binary committed)"
    - "Dev workflow preserved: --rust-bin flag + CODENEXUS_RUST_BIN env still override embed extraction"
    - "go build ./... and go vet ./... clean from server/ directory"
  artifacts:
    - path: "server/internal/supervisor/embed.go"
      provides: "//go:embed all:bin directive + extractRustBinary() helper"
      contains: "//go:embed all:bin"
    - path: "server/internal/supervisor/bin/.gitkeep"
      provides: "Keeps embed source dir present in git so //go:embed has a non-empty target in fresh clones"
    - path: "server/internal/supervisor/rust.go"
      provides: "Start() calls extractRustBinary() when cfg.RustBinPath is empty"
      contains: "extractRustBinary"
    - path: "server/cmd/serve.go"
      provides: "resolveRustBinPath returns explicit override OR delegates to extraction (no longer hard-fails on empty)"
    - path: "Makefile"
      provides: "EMBED_DIR := server/internal/supervisor/bin (line 6); clean target updated"
      contains: "EMBED_DIR := server/internal/supervisor/bin"
    - path: ".gitignore"
      provides: "Ignores built Rust binary copied into embed dir, but NOT the .gitkeep"
      contains: "server/internal/supervisor/bin/codenexus-core"
  key_links:
    - from: "server/internal/supervisor/embed.go"
      to: "server/internal/supervisor/bin/"
      via: "//go:embed all:bin pragma (path is RELATIVE TO embed.go's package dir, not main.go — user-flagged gotcha)"
      pattern: "//go:embed all:bin"
    - from: "server/internal/supervisor/rust.go Start()"
      to: "embed.go extractRustBinary()"
      via: "called when cfg.RustBinPath == \"\" (replaces current hard-fail)"
      pattern: "extractRustBinary"
    - from: "server/cmd/serve.go runServe()"
      to: "supervisor.Start()"
      via: "passes empty RustBinPath when no flag/env override (so Start triggers extraction)"
    - from: "Makefile build-server target"
      to: "server/internal/supervisor/bin/codenexus-core(.exe)"
      via: "cp from core/target/release/ before go build"
      pattern: "EMBED_DIR := server/internal/supervisor/bin"
---

<objective>
Wire `//go:embed` plumbing so the Go server can embed the Rust core binary at build time and extract it to the user cache dir on first run, replacing the current hard-fail-on-empty-RustBinPath behavior. Plumbing only — does not require a real Rust binary to compile or pass go vet.

Purpose: Unblock REQ-08 (single fat-binary distribution) without committing the Rust artifact to git. Preserves dev workflow (`--rust-bin` / `CODENEXUS_RUST_BIN` / `../core/target/release/...` auto-discover) so REQ-07 invariants stay green.

Output: 2 new files (embed.go, bin/.gitkeep), 4 modified files (rust.go, serve.go, Makefile, .gitignore). Compiles + passes vet with empty `bin/` (only .gitkeep present).
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.planning/REQUIREMENTS.md
@docs/ARCHITECTURE.md
@server/internal/supervisor/rust.go
@server/cmd/serve.go
@Makefile
@.gitignore

<critical_gotcha>
USER EXPLICITLY FLAGGED — DO NOT REGRESS:

`//go:embed` paths are relative to the **package directory containing the directive**, NOT relative to `main.go`. The Rust binary lives at `core/target/release/codenexus-core` which is `../../../core/target/release/codenexus-core` relative to `server/internal/supervisor/`. **Cross-parent-dir embed is forbidden by Go's compiler.**

Therefore:
1. Makefile MUST `cp` the Rust binary into `server/internal/supervisor/bin/` (sibling-or-deeper of embed.go) before `go build`.
2. `EMBED_DIR` MUST change from `server/embed` to `server/internal/supervisor/bin`.
3. The `//go:embed` directive must reference `bin/...` (relative to the embed.go file).
4. Use `//go:embed all:bin` form — the `all:` prefix ensures dotfiles like `.gitkeep` are included, which is critical because `//go:embed bin/codenexus-core` would FAIL TO COMPILE when the file is absent in fresh clones.
</critical_gotcha>

<interfaces>
<!-- Existing supervisor.Config (rust.go:30-37) — DO NOT change shape -->
```go
type Config struct {
    RustBinPath  string  // empty → extract from embed in REQ-08
    RustPort     int
    LockfilePath string
    DataDir      string
    Device       string
    RustLog      string
}
```

<!-- Existing supervisor.Start signature (rust.go:67) — DO NOT change shape -->
```go
func Start(ctx context.Context, cfg Config) (*Supervisor, error)
```
Current behavior at line 68-70:
```go
if cfg.RustBinPath == "" {
    return nil, errors.New("supervisor: RustBinPath empty (set --rust-bin or CODENEXUS_RUST_BIN)")
}
```
This is the splice point. Replace with: if empty, call extractRustBinary() and assign result to cfg.RustBinPath. If extraction fails, propagate error (with hint to set --rust-bin manually).

<!-- Existing serve.go resolveRustBinPath (serve.go:148-179) — auto-discover stays as a dev fallback -->
The current order: flag → env → auto-discover sibling `../core/target/release/...`. After this slice, the Go-level resolver may return "" (empty) and that's fine — supervisor.Start() handles extraction. resolveRustBinPath remains useful as a dev override path.

<!-- Stdlib API used -->
```go
import "embed"
//go:embed all:bin
var rustBinFS embed.FS

// Reading the embedded binary:
data, err := rustBinFS.ReadFile("bin/codenexus-core")           // POSIX
data, err := rustBinFS.ReadFile("bin/codenexus-core.exe")       // Windows
// ENOENT here = real Rust binary was not staged before `go build` (only .gitkeep was embedded). Return a clear error.

// XDG cache dir (cross-platform):
cacheRoot, err := os.UserCacheDir()
// → Windows: %LOCALAPPDATA%, Linux: $XDG_CACHE_HOME or $HOME/.cache, macOS: ~/Library/Caches
extractDir := filepath.Join(cacheRoot, "codenexus", "bin", "codenexus-core-"+coreVersion)
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create embed.go + .gitkeep, splice extraction into rust.go and serve.go</name>
  <files>
    server/internal/supervisor/embed.go (NEW),
    server/internal/supervisor/bin/.gitkeep (NEW, empty file),
    server/internal/supervisor/rust.go (MODIFY: replace placeholder comment block at lines 8-13 + replace empty-path hard-fail at lines 68-70),
    server/cmd/serve.go (MODIFY: relax slog.Warn at lines 51-54 — empty rustBin is now OK because supervisor extracts on demand)
  </files>
  <action>
**1. Create `server/internal/supervisor/bin/.gitkeep`** — empty file. Existence-only; commits the dir.

**2. Create `server/internal/supervisor/embed.go`** — single file holding both the embed directive and the extractor. Apache 2.0 SPDX header. Package `supervisor`. Stdlib only: `crypto/sha256`, `embed`, `encoding/hex`, `errors`, `fmt`, `io`, `io/fs`, `log/slog`, `os`, `path/filepath`, `runtime`.

Required contents (write all of this verbatim — these are the contract):
```go
// SPDX-License-Identifier: Apache-2.0

package supervisor

import (
	"crypto/sha256"
	"embed"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
)

// rustBinFS embeds the Rust core binary that the Makefile cp'd into bin/
// before `go build`. The `all:` prefix is required so .gitkeep (a dotfile)
// is included; without it //go:embed would fail to compile in fresh clones
// where only .gitkeep is present.
//
// IMPORTANT: //go:embed paths are relative to THIS .go file's directory,
// not main.go. The Makefile MUST stage the Rust binary at
// server/internal/supervisor/bin/codenexus-core(.exe) — cross-parent-dir
// embed (e.g. ../../core/target/release/) is rejected by the Go compiler.
//
//go:embed all:bin
var rustBinFS embed.FS

// coreVersion is the suffix used for the extraction dir under
// <UserCacheDir>/codenexus/bin/codenexus-core-<coreVersion>/. Hardcoded "dev"
// for the REQ-08 plumbing slice; later upgraded via:
//
//	go build -ldflags "-X github.com/2233admin/codenexus/internal/supervisor.coreVersion=v0.x.y"
var coreVersion = "dev"

// rustBinaryName returns the OS-appropriate filename of the embedded binary.
func rustBinaryName() string {
	if runtime.GOOS == "windows" {
		return "codenexus-core.exe"
	}
	return "codenexus-core"
}

// extractRustBinary writes the embedded Rust core binary to
// <UserCacheDir>/codenexus/bin/codenexus-core-<coreVersion>/<binname> and
// returns the absolute path. Idempotent: skips re-write if the existing file
// matches the embedded source by size + sha256. Sets exec permission on POSIX.
//
// Returns an error if the embedded binary is absent (only .gitkeep was staged
// before `go build`) — caller should fall back to --rust-bin / CODENEXUS_RUST_BIN.
func extractRustBinary() (string, error) {
	binName := rustBinaryName()
	embedPath := "bin/" + binName

	embedded, err := rustBinFS.ReadFile(embedPath)
	if err != nil {
		// Distinguish "not staged" from other errors so the caller can produce a
		// dev-friendly hint.
		if errors.Is(err, fs.ErrNotExist) {
			return "", fmt.Errorf("supervisor: embedded rust binary %q not present (build with `make build` or set --rust-bin / CODENEXUS_RUST_BIN): %w", embedPath, err)
		}
		return "", fmt.Errorf("supervisor: read embedded rust binary: %w", err)
	}
	if len(embedded) == 0 {
		return "", fmt.Errorf("supervisor: embedded rust binary %q is zero bytes (build pipeline staged a placeholder, not a real binary)", embedPath)
	}

	cacheRoot, err := os.UserCacheDir()
	if err != nil {
		return "", fmt.Errorf("supervisor: resolve user cache dir: %w", err)
	}
	dstDir := filepath.Join(cacheRoot, "codenexus", "bin", "codenexus-core-"+coreVersion)
	if err := os.MkdirAll(dstDir, 0o755); err != nil {
		return "", fmt.Errorf("supervisor: mkdir %s: %w", dstDir, err)
	}
	dstPath := filepath.Join(dstDir, binName)

	// Idempotency: skip re-write if size + sha256 already match.
	if fi, statErr := os.Stat(dstPath); statErr == nil && !fi.IsDir() && fi.Size() == int64(len(embedded)) {
		existingHash, hashErr := fileSHA256(dstPath)
		embeddedHash := sha256.Sum256(embedded)
		if hashErr == nil && existingHash == hex.EncodeToString(embeddedHash[:]) {
			slog.Debug("supervisor: extracted rust binary already current", "path", dstPath)
			return dstPath, nil
		}
	}

	// Write atomically: tmp file + rename. Use 0o755 so POSIX gets exec bit;
	// Windows ignores mode but still needs O_CREATE|O_TRUNC|O_WRONLY semantics.
	tmpPath := dstPath + ".tmp"
	tmp, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o755)
	if err != nil {
		return "", fmt.Errorf("supervisor: open %s: %w", tmpPath, err)
	}
	if _, err := tmp.Write(embedded); err != nil {
		_ = tmp.Close()
		_ = os.Remove(tmpPath)
		return "", fmt.Errorf("supervisor: write %s: %w", tmpPath, err)
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return "", fmt.Errorf("supervisor: close %s: %w", tmpPath, err)
	}
	// On POSIX, ensure exec bit even if umask stripped it from the OpenFile mode.
	if runtime.GOOS != "windows" {
		if err := os.Chmod(tmpPath, 0o755); err != nil {
			_ = os.Remove(tmpPath)
			return "", fmt.Errorf("supervisor: chmod %s: %w", tmpPath, err)
		}
	}
	if err := os.Rename(tmpPath, dstPath); err != nil {
		_ = os.Remove(tmpPath)
		return "", fmt.Errorf("supervisor: rename %s -> %s: %w", tmpPath, dstPath, err)
	}

	slog.Info("supervisor: extracted embedded rust binary",
		"path", dstPath, "bytes", len(embedded), "version", coreVersion)
	return dstPath, nil
}

// fileSHA256 returns the hex-encoded SHA-256 of the file at path. Used for
// idempotency check in extractRustBinary; reading the full file is acceptable
// because the binary is ~80-120 MB and this only runs on cold start.
func fileSHA256(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}
```

**3. Modify `server/internal/supervisor/rust.go`:**
- Replace the placeholder comment block at lines 8-13 (the "REQ-08 (deferred)" doc comment that says `//go:embed will replace cfg.RustBinPath`) with a one-line cross-reference: `// REQ-08: when cfg.RustBinPath is empty, Start() calls extractRustBinary() (see embed.go) to materialize the embedded Rust core binary into the user cache dir.`
- Replace lines 68-70 (the `if cfg.RustBinPath == ""` hard-fail) with extraction + clear error path:
  ```go
  if cfg.RustBinPath == "" {
      extracted, err := extractRustBinary()
      if err != nil {
          return nil, fmt.Errorf("supervisor: RustBinPath empty and embed extraction failed (set --rust-bin or CODENEXUS_RUST_BIN to override): %w", err)
      }
      cfg.RustBinPath = extracted
  }
  ```
- DO NOT touch the rest of rust.go. The lockfile, supervise(), backoff, and breaker logic stays exactly as is.

**4. Modify `server/cmd/serve.go`:**
- At lines 50-54, the current code logs a Warn when rustBin is "". After this slice, an empty rustBin is the **default and expected production path** (supervisor extracts from embed). Replace the warning block with a Debug-level log:
  ```go
  rustBin := resolveRustBinPath(RustBin())
  if rustBin == "" {
      slog.Debug("serve: no explicit rust binary path; supervisor will extract from embed",
          "hint", "set --rust-bin or CODENEXUS_RUST_BIN for dev override")
  }
  ```
- DO NOT change resolveRustBinPath() itself (lines 148-179). Its three-tier order (flag → env → auto-discover sibling) remains a useful dev override; an empty return now means "let supervisor handle it" instead of being a fatal warning.

**Why one task, not two:** embed.go alone does not compile cleanly without the rust.go splice (the unused `extractRustBinary` triggers staticcheck on some toolchains, plus the placeholder comment becomes wrong). Wiring all four files together produces one atomic logical commit.
  </action>
  <verify>
    <automated>cd D:/projects/codenexus/server && go build ./... && go vet ./...</automated>
  </verify>
  <done>
1. `server/internal/supervisor/embed.go` exists with `//go:embed all:bin` directive and `extractRustBinary()` function exported (lowercase — package-internal is fine).
2. `server/internal/supervisor/bin/.gitkeep` exists (empty file).
3. `server/internal/supervisor/rust.go` no longer hard-fails on empty `cfg.RustBinPath` — instead calls `extractRustBinary()`.
4. `server/cmd/serve.go` no longer logs Warn for empty rustBin.
5. `cd D:/projects/codenexus/server && go build ./...` exits 0.
6. `cd D:/projects/codenexus/server && go vet ./...` exits 0.
7. Apache 2.0 SPDX header on all NEW .go files.
  </done>
</task>

<task type="auto">
  <name>Task 2: Update Makefile EMBED_DIR + .gitignore + verify build</name>
  <files>
    Makefile (MODIFY: line 6 EMBED_DIR + line 48 clean target),
    .gitignore (MODIFY: append codenexus-core entries under "# CodeNexus specific" section)
  </files>
  <action>
**1. Modify `Makefile`:**
- Line 6: change `EMBED_DIR := server/embed` to `EMBED_DIR := server/internal/supervisor/bin`.
- Lines 23-27 (build-server target): NO CHANGE — `mkdir -p $(EMBED_DIR)` and `cp ... $(EMBED_DIR)/` already use the variable correctly. Verify by re-reading after edit.
- Line 48 (clean target): currently `rm -rf $(BIN_DIR) $(EMBED_DIR)`. Change to `rm -rf $(BIN_DIR)` AND add a separate line `rm -f $(EMBED_DIR)/codenexus-core $(EMBED_DIR)/codenexus-core.exe` so that the .gitkeep stays alive across `make clean`. Final clean target should look like:
  ```makefile
  clean:
  	cd core && cargo clean
  	cd server && go clean
  	rm -rf $(BIN_DIR)
  	rm -f $(EMBED_DIR)/codenexus-core $(EMBED_DIR)/codenexus-core.exe
  ```

**2. Modify `.gitignore`:**
- Append under the existing `# CodeNexus specific` section (after line 43):
  ```
  
  # REQ-08 embedded Rust binary (built artifact, do NOT commit; .gitkeep stays)
  server/internal/supervisor/bin/codenexus-core
  server/internal/supervisor/bin/codenexus-core.exe
  ```
- DO NOT add a blanket `server/internal/supervisor/bin/` rule — that would also ignore .gitkeep and break the embed pattern in fresh clones.

**3. Verify Makefile syntax** by running `make help` (does not require Rust toolchain, just a tab-indented sanity check):
```bash
cd D:/projects/codenexus && make help
```
Should print the help banner without "missing separator" or undefined variable errors.

**4. Verify .gitignore semantics** with a dry-run check:
```bash
cd D:/projects/codenexus && touch server/internal/supervisor/bin/codenexus-core && git check-ignore -v server/internal/supervisor/bin/codenexus-core && git check-ignore -v server/internal/supervisor/bin/.gitkeep ; rm -f server/internal/supervisor/bin/codenexus-core
```
Expected: first check-ignore prints a match (file IS ignored, exit 0); second exits non-zero (.gitkeep is NOT ignored).

**5. Final compile sanity** (catches any cross-task regression):
```bash
cd D:/projects/codenexus/server && go build ./... && go vet ./...
```
  </action>
  <verify>
    <automated>cd D:/projects/codenexus && grep -n "EMBED_DIR := server/internal/supervisor/bin" Makefile && grep -n "server/internal/supervisor/bin/codenexus-core" .gitignore && cd server && go build ./... && go vet ./...</automated>
  </verify>
  <done>
1. `Makefile` line 6 reads `EMBED_DIR := server/internal/supervisor/bin`.
2. `Makefile` clean target removes only the codenexus-core files (NOT the bin dir, so .gitkeep survives).
3. `make help` runs without error from the project root.
4. `.gitignore` contains TWO new lines for `codenexus-core` and `codenexus-core.exe` under `server/internal/supervisor/bin/`.
5. `.gitignore` does NOT contain a blanket `server/internal/supervisor/bin/` rule.
6. `git check-ignore` confirms `codenexus-core` is ignored, `.gitkeep` is not.
7. `cd server && go build ./...` and `go vet ./...` both clean.
  </done>
</task>

</tasks>

<verification>
End-to-end plumbing verification (no real Rust binary required):
1. **Compile:** `cd D:/projects/codenexus/server && go build ./...` exits 0.
2. **Vet:** `cd D:/projects/codenexus/server && go vet ./...` exits 0.
3. **Embed directive present:** `grep -n "//go:embed all:bin" server/internal/supervisor/embed.go` matches.
4. **Splice correct in rust.go:** `grep -n "extractRustBinary" server/internal/supervisor/rust.go` shows at least one call site (Start function).
5. **Makefile var:** `grep -n "EMBED_DIR := server/internal/supervisor/bin" Makefile` matches exactly once.
6. **.gitignore correct:** `grep -c "server/internal/supervisor/bin/codenexus-core" .gitignore` returns 2 (POSIX + .exe).
7. **.gitkeep committed:** `git ls-files server/internal/supervisor/bin/.gitkeep` returns the path.
8. **No accidental binary committed:** `git status -s server/internal/supervisor/bin/` after `make build-core` (if Rust toolchain available) shows only ignored entries — but this is a smoke test, not a hard gate for this slice.

Smoke deferred (out of scope this slice — needs working Rust core build):
- Run `make build` end-to-end and confirm `bin/codenexus(.exe)` size ≤ 150 MB.
- Run `bin/codenexus serve` on a fresh machine and confirm extraction happens, supervisor spawns the Rust child, and `/healthz` returns green.
</verification>

<success_criteria>
- All 7 plan must_haves verified by independent grep + go build + go vet
- Two atomic git commits possible (Task 1 = "feat(req-08): //go:embed Rust core binary into Go supervisor"; Task 2 = "build(req-08): point Makefile EMBED_DIR at supervisor/bin and update .gitignore")
- Dev workflow regression check: `--rust-bin /tmp/foo` flag still wins over extraction (resolveRustBinPath returns it; supervisor.Start uses it directly without calling extractRustBinary)
- Fresh-clone scenario verified: with only `.gitkeep` present in `bin/`, `go build ./...` succeeds (because `all:bin` glob is satisfied by .gitkeep alone)
</success_criteria>

<output>
After completion, create `.planning/quick/260427-hoc-req-08-go-embed-rust-core-binary-into-go/260427-hoc-SUMMARY.md` documenting:
1. Files created/modified with line-counts
2. Two atomic commit SHAs
3. Verification command output (go build / go vet / grep matches)
4. Confirmation that user-flagged path-relativity gotcha was honored (embed.go in supervisor/, references `bin/...` not `../../...`)
5. Honest gap list:
   - P1: real `make build` end-to-end smoke deferred (needs working Rust core compile)
   - P1: 150 MB size budget not yet measured (no real binary)
   - P2: ldflags-based version injection deferred (hardcoded "dev")
   - P2: cross-platform extraction smoke deferred (Linux/macOS not tested this slice)
6. Next-session entry pointer: REQ-09 embedded HTML/JS UI is the next sibling task; it has its own placeholder splice point at `server/cmd/serve.go:206-211` (uiPlaceholderHandler TODO).
</output>

<unresolved_questions>
1. **`coreVersion` constant**: hardcoded `"dev"` for this slice. Plan suggests ldflags upgrade path in a follow-up. Acceptable for MVP? (assumed yes — REQ-08 acceptance does not require version-suffixed cache dirs)
2. **POSIX `os.OpenFile(0o755)` vs explicit `os.Chmod` redundancy**: belt-and-suspenders pattern (open with 0o755 mode, then explicit Chmod on POSIX). Some umask configurations strip exec bit even with O_CREATE mode. Keep both? (assumed yes — defense in depth, ~3 lines of code)
3. **Idempotency check cost**: full sha256 of an 80-120 MB file on every cold start ≈ 100ms. Acceptable, but if too slow we could degrade to mtime + size only (sha256 only on suspect mtime). Defer the optimization — measure first.
4. **`fs.ErrNotExist` distinguishability**: `embed.FS.ReadFile` may not always return `fs.ErrNotExist` for missing entries — Go's docs say it returns an `*fs.PathError` wrapping the underlying error. The errors.Is check is correct on Go 1.16+ but the error message may not match what dev-mode users expect. Acceptable to start; refine if user reports confusing errors.
5. **`all:` prefix pre-Go-1.18 compat**: `//go:embed all:bin` syntax requires Go 1.18+. Project's go.mod (per REQ-07 scaffold) is Go 1.21+, so safe. Worth noting in the SUMMARY for future-self.
</unresolved_questions>
