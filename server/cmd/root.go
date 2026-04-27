// SPDX-License-Identifier: Apache-2.0

// Package cmd holds cobra command definitions for the codenexus binary.
// Each subcommand lives in a sibling file (serve, index, query, mcp); this
// file owns the root command and the persistent flags shared across them.
package cmd

import (
	"github.com/spf13/cobra"
)

// rootCmd is the top-level cobra command. Subcommands attach in init().
var rootCmd = &cobra.Command{
	Use:   "codenexus",
	Short: "CodeNexus — code+knowledge graph",
	Long: "CodeNexus combines a Rust core (parsing, indexing, hybrid search) " +
		"with a Go service layer (HTTP, MCP, CLI). The Go binary supervises " +
		"the Rust core and proxies A2A requests to it.",
	SilenceUsage: true,
}

// Persistent flag values populated by cobra. Subcommands read them via the
// exported accessors below to keep the cobra-specific globals contained here.
var (
	flagPort     int
	flagRustBin  string
	flagLogLevel string
)

// Port returns the --port flag value (Go HTTP server port). Used by serve.
func Port() int { return flagPort }

// RustBin returns the --rust-bin override path (empty = auto-discover/env).
func RustBin() string { return flagRustBin }

// LogLevel returns the --log-level flag value (e.g. "info", "debug").
func LogLevel() string { return flagLogLevel }

// Execute runs the root cobra command and returns its error. main() wraps it.
func Execute() error {
	return rootCmd.Execute()
}

func init() {
	pf := rootCmd.PersistentFlags()
	pf.IntVar(&flagPort, "port", 8080, "Go HTTP server port (used by serve)")
	pf.StringVar(&flagRustBin, "rust-bin", "", "override path to codenexus-core binary; empty = auto-discover or CODENEXUS_RUST_BIN")
	pf.StringVar(&flagLogLevel, "log-level", "info", "slog level: debug | info | warn | error")

	rootCmd.AddCommand(serveCmd, indexCmd, queryCmd, mcpCmd)
}
