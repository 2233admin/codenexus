// SPDX-License-Identifier: Apache-2.0

package supervisor

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"time"
)

// Lockfile is the on-disk record at ~/.codenexus/port describing the running
// Rust core (PID, port, start time). It is written by the Go supervisor after
// a successful spawn and consumed by CLI subcommands (index/query/mcp).
type Lockfile struct {
	PID           int   `json:"pid"`
	Port          int   `json:"port"`
	StartedAtUnix int64 `json:"started_at_unix"`
}

// LockfileDir returns ~/.codenexus, creating it (0700) if missing.
func LockfileDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", fmt.Errorf("portscan: user home: %w", err)
	}
	dir := filepath.Join(home, ".codenexus")
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", fmt.Errorf("portscan: mkdir %s: %w", dir, err)
	}
	return dir, nil
}

// LockfilePath returns ~/.codenexus/port (best-effort: returns empty string
// only if user home is unavailable, which is also fatal upstream).
func LockfilePath() (string, error) {
	dir, err := LockfileDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "port"), nil
}

// ReadLockfile loads the on-disk lockfile. Returns an error if missing or
// malformed; CLI subcommands surface this as "is `codenexus serve` running?".
func ReadLockfile() (*Lockfile, error) {
	path, err := LockfilePath()
	if err != nil {
		return nil, err
	}
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("portscan: read lockfile %s: %w", path, err)
	}
	var lf Lockfile
	if err := json.Unmarshal(b, &lf); err != nil {
		return nil, fmt.Errorf("portscan: parse lockfile %s: %w", path, err)
	}
	return &lf, nil
}

// WriteLockfile atomically (write+rename) records the current Rust core state.
func WriteLockfile(lf Lockfile) error {
	path, err := LockfilePath()
	if err != nil {
		return err
	}
	b, err := json.MarshalIndent(lf, "", "  ")
	if err != nil {
		return fmt.Errorf("portscan: marshal lockfile: %w", err)
	}
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, b, 0o600); err != nil {
		return fmt.Errorf("portscan: write tmp lockfile: %w", err)
	}
	if err := os.Rename(tmp, path); err != nil {
		return fmt.Errorf("portscan: rename lockfile: %w", err)
	}
	return nil
}

// UnlinkLockfile removes the lockfile if present. Errors other than "not
// exists" are returned; "not exists" is ignored.
func UnlinkLockfile() error {
	path, err := LockfilePath()
	if err != nil {
		return err
	}
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("portscan: remove lockfile: %w", err)
	}
	return nil
}

// AcquireRustPort scans 9876..9999 for a free TCP port and returns the chosen
// port plus the lockfile path. Stale lockfile handling: if the existing
// lockfile points to a process that no longer looks alive, it is removed.
//
// The caller (supervisor.Start) is responsible for writing a fresh lockfile
// AFTER spawning the Rust core, since only then is the real PID known.
func AcquireRustPort() (int, string, error) {
	lockPath, err := LockfilePath()
	if err != nil {
		return 0, "", err
	}

	if existing, err := ReadLockfile(); err == nil {
		if isLikelyAlive(existing) && portInUse(existing.Port) {
			// Live previous instance — reuse its port. Caller will detect
			// during spawn that the port is busy and fail visibly; this is
			// preferable to silently double-spawning the Rust core.
			return existing.Port, lockPath, nil
		}
		// Stale: remove and fall through to fresh scan.
		_ = UnlinkLockfile()
	}

	for p := 9876; p <= 9999; p++ {
		if !portInUse(p) {
			return p, lockPath, nil
		}
	}
	return 0, "", fmt.Errorf("portscan: all ports 9876..9999 in use")
}

// portInUse returns true if 127.0.0.1:p cannot be bound.
func portInUse(p int) bool {
	addr := fmt.Sprintf("127.0.0.1:%d", p)
	l, err := net.Listen("tcp", addr)
	if err != nil {
		return true
	}
	_ = l.Close()
	return false
}

// isLikelyAlive applies the zero-dep heuristic from §D-S4: if the lockfile is
// less than 24h old AND the PID is plausibly alive (POSIX: signal-0; Windows:
// os.FindProcess never errors so we fall back to age-only), treat as alive.
func isLikelyAlive(lf *Lockfile) bool {
	if lf == nil {
		return false
	}
	startedAt := time.Unix(lf.StartedAtUnix, 0)
	if time.Since(startedAt) > 24*time.Hour {
		return false
	}
	proc, err := os.FindProcess(lf.PID)
	if err != nil {
		return false
	}
	// signalZero is platform-specific (see portscan_unix.go / portscan_windows.go).
	return signalZero(proc) == nil
}
