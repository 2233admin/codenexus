// SPDX-License-Identifier: Apache-2.0

package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"time"

	"github.com/2233admin/codenexus/internal/proxy"
	"github.com/2233admin/codenexus/internal/supervisor"
	"github.com/spf13/cobra"
)

// indexCmd is a one-shot CLI passthrough: it does NOT spawn the Rust core; it
// expects `codenexus serve` to already be running and writes the lockfile.
var indexCmd = &cobra.Command{
	Use:   "index <repo>",
	Short: "Index a repository via the running Rust core",
	Args:  cobra.ExactArgs(1),
	RunE:  runIndex,
}

func runIndex(cmd *cobra.Command, args []string) error {
	lf, err := supervisor.ReadLockfile()
	if err != nil {
		return fmt.Errorf("index: read lockfile (is `codenexus serve` running?): %w", err)
	}
	client := proxy.New(lf.Port)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
	defer cancel()

	out, err := client.IndexRepo(ctx, proxy.IndexRepoArgs{
		RepoPath:    args[0],
		Incremental: true,
	})
	if err != nil {
		return fmt.Errorf("index: a2a call: %w", err)
	}
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(out); err != nil {
		return fmt.Errorf("index: encode result: %w", err)
	}
	return nil
}
