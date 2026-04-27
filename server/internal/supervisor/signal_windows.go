// SPDX-License-Identifier: Apache-2.0

//go:build windows

package supervisor

import "os"

// signalZero on Windows: os.FindProcess always succeeds and there is no
// portable signal-0 equivalent in the standard library. The caller already
// gated on lockfile mtime < 24h; treating presence as "alive" is the agreed
// zero-dep fallback per §D-S4. If the heuristic is wrong, the caller will
// fail at port-bind and re-scan, which is the desired safety net.
func signalZero(_ *os.Process) error {
	return nil
}
