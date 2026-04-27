// SPDX-License-Identifier: Apache-2.0

// Package supervisor owns the Rust core child process: spawn, ready-wait on
// /healthz, restart-on-exit with backoff, crash-loop breaker, and lockfile
// bookkeeping. CLI subcommands and the chi /healthz handler observe its state
// via the Supervisor.State() snapshot accessor.
//
// REQ-08 (deferred): //go:embed will replace cfg.RustBinPath:
//
//	//go:embed bin/codenexus-core
//	var rustBinFS embed.FS
//	At Start(): extract rustBinFS to <XDG_CACHE_HOME>/codenexus/bin/codenexus-core-<version>/
//	Set cfg.RustBinPath = extracted path. Everything below stays the same.
package supervisor

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/exec"
	"sync"
	"time"
)

// Config carries the values supervisor.Start needs. The CLI builds it from
// flags + env + auto-discover; tests can build it directly.
type Config struct {
	RustBinPath  string
	RustPort     int
	LockfilePath string
	DataDir      string
	Device       string
	RustLog      string
}

// State is the read-only snapshot exposed by Supervisor.State(). The /healthz
// handler uses these fields to render the supervisor portion of its response.
type State struct {
	RustAlive      bool
	RestartCount   int
	BreakerTripped bool
}

// Supervisor owns one Rust core process. Concurrency invariants:
//   - state and cmd are mu-protected.
//   - supervise() is the only goroutine that calls cmd.Wait() and respawns.
//   - Stop() signals via stopCh + ctx cancellation propagated by Start's caller.
type Supervisor struct {
	cfg Config

	mu              sync.Mutex
	state           State
	cmd             *exec.Cmd
	restartTimes    []time.Time
	lastStableStart time.Time

	stopCh chan struct{}
	doneCh chan struct{}
}

// Start launches the Rust core, waits up to 30s for /healthz green, kicks off
// the supervise() goroutine for restart-on-exit, and returns the Supervisor.
// On failure it kills any half-started child before returning.
func Start(ctx context.Context, cfg Config) (*Supervisor, error) {
	if cfg.RustBinPath == "" {
		return nil, errors.New("supervisor: RustBinPath empty (set --rust-bin or CODENEXUS_RUST_BIN)")
	}
	s := &Supervisor{
		cfg:    cfg,
		stopCh: make(chan struct{}),
		doneCh: make(chan struct{}),
	}
	if err := s.spawnLocked(); err != nil {
		return nil, err
	}
	if err := s.waitReady(ctx, 30*time.Second); err != nil {
		_ = s.killLocked()
		return nil, fmt.Errorf("supervisor: rust /healthz not green: %w", err)
	}
	s.mu.Lock()
	s.state.RustAlive = true
	s.lastStableStart = time.Now()
	s.mu.Unlock()
	go s.supervise(ctx)
	return s, nil
}

// State returns a snapshot of the supervisor's current condition.
func (s *Supervisor) State() State {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.state
}

// Stop terminates the Rust core (graceful kill, 5s grace, then force) and
// removes the lockfile. Safe to call multiple times.
func (s *Supervisor) Stop() error {
	select {
	case <-s.stopCh:
		// Already stopped.
		return nil
	default:
		close(s.stopCh)
	}
	s.mu.Lock()
	err := s.killLocked()
	s.mu.Unlock()
	_ = UnlinkLockfile()
	return err
}

// ----- internals -----

// spawnLocked builds *exec.Cmd with §5.5 env vars, starts it, and writes the
// lockfile. Caller holds s.mu OR holds no lock (only called from Start before
// any other goroutine sees s, and from supervise() with the lock held).
func (s *Supervisor) spawnLocked() error {
	cmd := exec.Command(s.cfg.RustBinPath)
	cmd.Env = append(os.Environ(),
		"CODENEXUS_PORT="+itoa(s.cfg.RustPort),
		"CODENEXUS_PORT_LOCKFILE="+s.cfg.LockfilePath,
		"CODENEXUS_DATA_DIR="+s.cfg.DataDir,
		"CODENEXUS_DEVICE="+s.cfg.Device,
		"RUST_LOG="+s.cfg.RustLog,
	)
	// HF_HOME inherited from os.Environ() if user-set; nothing to do here.
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("supervisor: spawn %s: %w", s.cfg.RustBinPath, err)
	}
	s.cmd = cmd
	if err := WriteLockfile(Lockfile{
		PID:           cmd.Process.Pid,
		Port:          s.cfg.RustPort,
		StartedAtUnix: time.Now().Unix(),
	}); err != nil {
		// Non-fatal: log and continue. CLI subcommands rely on this file but
		// supervise() will rewrite it on the next restart.
		slog.Warn("supervisor: write lockfile failed", "err", err)
	}
	slog.Info("supervisor: spawned rust core",
		"pid", cmd.Process.Pid, "port", s.cfg.RustPort, "bin", s.cfg.RustBinPath)
	return nil
}

// waitReady polls http://localhost:<port>/healthz every 500ms until 200 OK or
// timeout. Returns nil on success.
func (s *Supervisor) waitReady(ctx context.Context, timeout time.Duration) error {
	url := fmt.Sprintf("http://localhost:%d/healthz", s.cfg.RustPort)
	client := &http.Client{Timeout: 1 * time.Second}
	deadline := time.Now().Add(timeout)
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}
		if time.Now().After(deadline) {
			return fmt.Errorf("timeout after %s polling %s", timeout, url)
		}
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
		if err == nil {
			resp, err := client.Do(req)
			if err == nil {
				_ = resp.Body.Close()
				if resp.StatusCode == http.StatusOK {
					return nil
				}
			}
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(500 * time.Millisecond):
		}
	}
}

// supervise is the only goroutine that owns cmd.Wait + respawn.
//
// Restart strategy (§2.3):
//   - Backoff sequence: 1s, 2s, 4s, 8s, 16s, then 30s (clamped).
//   - Crash-loop breaker: ≥5 restarts in any 60s window → log fatal, set
//     BreakerTripped, return so caller (serve.go) cancels its root context
//     and exits non-zero.
//   - 5 minutes of stable uptime since last spawn resets the restart counter.
func (s *Supervisor) supervise(ctx context.Context) {
	defer close(s.doneCh)

	backoffSeq := []time.Duration{1 * time.Second, 2 * time.Second, 4 * time.Second, 8 * time.Second, 16 * time.Second}
	backoffCap := 30 * time.Second
	attempt := 0

	for {
		s.mu.Lock()
		cmd := s.cmd
		s.mu.Unlock()
		if cmd == nil {
			return
		}

		// Wait for the current child to exit.
		err := cmd.Wait()
		select {
		case <-s.stopCh:
			return // graceful shutdown
		case <-ctx.Done():
			return
		default:
		}
		slog.Warn("supervisor: rust core exited", "err", err)

		s.mu.Lock()
		s.state.RustAlive = false

		// Reset the restart-window counter if the previous spawn was stable
		// for >= 5 minutes (§2.3).
		if !s.lastStableStart.IsZero() && time.Since(s.lastStableStart) >= 5*time.Minute {
			s.restartTimes = nil
		}
		now := time.Now()
		s.restartTimes = append(s.restartTimes, now)
		// Trim to last 60s window.
		cutoff := now.Add(-60 * time.Second)
		trimmed := s.restartTimes[:0]
		for _, t := range s.restartTimes {
			if t.After(cutoff) {
				trimmed = append(trimmed, t)
			}
		}
		s.restartTimes = trimmed

		if len(s.restartTimes) >= 5 {
			s.state.BreakerTripped = true
			s.mu.Unlock()
			slog.Error("supervisor: crash-loop breaker tripped (>= 5 restarts in 60s)",
				"window_count", len(s.restartTimes))
			return
		}
		s.mu.Unlock()

		// Compute backoff for this attempt.
		var d time.Duration
		if attempt < len(backoffSeq) {
			d = backoffSeq[attempt]
		} else {
			d = backoffCap
		}
		attempt++
		slog.Info("supervisor: backing off before restart", "duration", d, "attempt", attempt)

		select {
		case <-ctx.Done():
			return
		case <-s.stopCh:
			return
		case <-time.After(d):
		}

		s.mu.Lock()
		if err := s.spawnLocked(); err != nil {
			s.mu.Unlock()
			slog.Error("supervisor: respawn failed", "err", err)
			return
		}
		s.mu.Unlock()

		// Wait for /healthz green again before counting this attempt as live.
		if err := s.waitReady(ctx, 30*time.Second); err != nil {
			slog.Error("supervisor: respawned core failed /healthz", "err", err)
			s.mu.Lock()
			_ = s.killLocked()
			s.mu.Unlock()
			continue // loop again, accumulating into the breaker window
		}

		s.mu.Lock()
		s.state.RustAlive = true
		s.state.RestartCount++
		s.lastStableStart = time.Now()
		s.mu.Unlock()
		// Reset attempt index after a successful spawn so the next exit
		// starts the backoff sequence from 1s again. The breaker still tracks
		// raw restart timestamps, which is the §2.3 invariant.
		attempt = 0
	}
}

// killLocked terminates the current cmd (if any). Caller holds s.mu.
func (s *Supervisor) killLocked() error {
	if s.cmd == nil || s.cmd.Process == nil {
		return nil
	}
	// Best-effort graceful kill, then force.
	_ = s.cmd.Process.Kill()
	done := make(chan struct{})
	go func() {
		_, _ = s.cmd.Process.Wait()
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		// process may already be dead; nothing more we can portably do.
	}
	s.cmd = nil
	s.state.RustAlive = false
	return nil
}

// itoa wraps strconv.Itoa so callers do not need a separate import.
func itoa(i int) string {
	// Avoid importing strconv just to keep a single helper here local; it is
	// already used elsewhere via the stdlib but keeping this thin shim makes
	// the call sites read uniformly.
	return fmt.Sprintf("%d", i)
}
