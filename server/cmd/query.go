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

// queryCmd issues a hybrid BM25+vector search via the running Rust core.
var queryCmd = &cobra.Command{
	Use:   "query <text>",
	Short: "Query a repo's code-graph (hybrid BM25+vector search)",
	Args:  cobra.ExactArgs(1),
	RunE:  runQuery,
}

var (
	queryRepoHash string
	queryK        int
)

func init() {
	pf := queryCmd.Flags()
	pf.StringVar(&queryRepoHash, "repo-hash", "", "repo hash returned by `codenexus index` (required)")
	pf.IntVar(&queryK, "k", 5, "top-k results to return")
	_ = queryCmd.MarkFlagRequired("repo-hash")
}

func runQuery(cmd *cobra.Command, args []string) error {
	lf, err := supervisor.ReadLockfile()
	if err != nil {
		return fmt.Errorf("query: read lockfile (is `codenexus serve` running?): %w", err)
	}
	client := proxy.New(lf.Port)

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	out, err := client.Query(ctx, proxy.QueryArgs{
		RepoHash: queryRepoHash,
		Q:        args[0],
		K:        queryK,
	})
	if err != nil {
		return fmt.Errorf("query: a2a call: %w", err)
	}
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(out); err != nil {
		return fmt.Errorf("query: encode result: %w", err)
	}
	return nil
}
