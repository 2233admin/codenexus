// SPDX-License-Identifier: Apache-2.0

package cmd

import (
	"context"
	"fmt"
	"os/signal"
	"syscall"

	"github.com/2233admin/codenexus/internal/mcpsrv"
	"github.com/2233admin/codenexus/internal/proxy"
	"github.com/2233admin/codenexus/internal/supervisor"
	"github.com/spf13/cobra"
)

// mcpCmd is the entry point users wire into Claude Desktop / Cursor MCP config.
// It expects the Rust core to already be running (via `codenexus serve`); this
// subcommand never spawns or supervises the core.
var mcpCmd = &cobra.Command{
	Use:   "mcp",
	Short: "Run MCP stdio handler (assumes Rust core already running)",
	RunE:  runMCP,
}

func runMCP(cmd *cobra.Command, args []string) error {
	lf, err := supervisor.ReadLockfile()
	if err != nil {
		return fmt.Errorf("mcp: read lockfile (is `codenexus serve` running?): %w", err)
	}
	client := proxy.New(lf.Port)

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	if err := mcpsrv.RunStdio(ctx, client); err != nil {
		return fmt.Errorf("mcp: stdio server: %w", err)
	}
	return nil
}
