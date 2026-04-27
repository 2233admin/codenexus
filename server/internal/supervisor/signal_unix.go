// SPDX-License-Identifier: Apache-2.0

//go:build !windows

package supervisor

import (
	"os"
	"syscall"
)

// signalZero sends signal 0 to the process — a no-op delivery that returns an
// error iff the process is dead or the caller lacks permission. Per POSIX it
// is the canonical liveness probe.
func signalZero(p *os.Process) error {
	return p.Signal(syscall.Signal(0))
}
