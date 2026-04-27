// SPDX-License-Identifier: Apache-2.0

// Package health renders the Go service's /healthz endpoint. It proxies the
// Rust core's own /healthz (process liveness + version + indexed_repos) and
// wraps the response with supervisor metadata (restart_count, breaker_tripped).
// When the supervisor's crash-loop breaker is tripped, this handler returns
// 503 unconditionally so external supervisors observe the failure.
package health

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/2233admin/codenexus/internal/supervisor"
)

// Probe is the minimal interface this package needs from the supervisor. It
// is satisfied by *supervisor.Supervisor and lets us avoid an import cycle if
// future tests need to substitute a fake.
type Probe interface {
	State() supervisor.State
}

// NewHandler returns the /healthz http.Handler. rustPort is the Rust core's
// HTTP port discovered via portscan (lockfile.Port).
func NewHandler(p Probe, rustPort int) http.Handler {
	c := &http.Client{Timeout: 2 * time.Second}
	rustURL := fmt.Sprintf("http://localhost:%d/healthz", rustPort)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		state := p.State()
		w.Header().Set("Content-Type", "application/json")

		if state.BreakerTripped {
			w.WriteHeader(http.StatusServiceUnavailable)
			_ = json.NewEncoder(w).Encode(map[string]any{
				"ok": false,
				"supervisor": map[string]any{
					"breaker_tripped": true,
					"restart_count":   state.RestartCount,
					"rust_alive":      state.RustAlive,
				},
			})
			return
		}

		req, err := http.NewRequestWithContext(r.Context(), http.MethodGet, rustURL, nil)
		if err != nil {
			respondRustDown(w, state, fmt.Sprintf("build request: %v", err))
			return
		}
		resp, err := c.Do(req)
		if err != nil {
			respondRustDown(w, state, fmt.Sprintf("transport: %v", err))
			return
		}
		defer resp.Body.Close()
		body, _ := io.ReadAll(resp.Body)
		if resp.StatusCode != http.StatusOK {
			respondRustDown(w, state, fmt.Sprintf("rust /healthz status %d", resp.StatusCode))
			return
		}

		var rustBody map[string]any
		if len(body) > 0 {
			_ = json.Unmarshal(body, &rustBody)
		}
		if rustBody == nil {
			rustBody = map[string]any{}
		}
		w.WriteHeader(http.StatusOK)
		_ = json.NewEncoder(w).Encode(map[string]any{
			"ok":   true,
			"rust": rustBody,
			"supervisor": map[string]any{
				"restart_count":   state.RestartCount,
				"breaker_tripped": false,
				"rust_alive":      true,
			},
		})
	})
}

func respondRustDown(w http.ResponseWriter, state supervisor.State, reason string) {
	w.WriteHeader(http.StatusServiceUnavailable)
	_ = json.NewEncoder(w).Encode(map[string]any{
		"ok":         false,
		"rust_alive": false,
		"reason":     reason,
		"supervisor": map[string]any{
			"restart_count":   state.RestartCount,
			"breaker_tripped": false,
		},
	})
}
