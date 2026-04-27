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
