// SPDX-License-Identifier: Apache-2.0

// CodeNexus Go service entrypoint. See cmd/ for subcommand wiring,
// internal/supervisor for Rust core lifecycle, internal/proxy for A2A
// client, and internal/mcpsrv for the MCP stdio handler.
package main

import (
	"fmt"
	"os"

	"github.com/2233admin/codenexus/cmd"
)

func main() {
	if err := cmd.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "codenexus: %v\n", err)
		os.Exit(1)
	}
}
