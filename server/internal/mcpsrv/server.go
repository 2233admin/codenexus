// SPDX-License-Identifier: Apache-2.0

// Package mcpsrv hosts the MCP server that maps the four code-graph tools
// (index_repo, query, get_symbol, list_callers) onto A2A operations against
// the Rust core. Stdio is the only transport implemented this slice; an
// SSE/HTTP upgrade is wired as a placeholder pending §3.2 D-A2.
package mcpsrv

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"

	"github.com/2233admin/codenexus/internal/proxy"
	"github.com/mark3labs/mcp-go/mcp"
	"github.com/mark3labs/mcp-go/server"
)

// RunStdio constructs an mcp-go server with the four code-graph tools, each
// delegating to the supplied proxy.Client, and serves it over stdio. Returns
// when stdio closes or the underlying transport errors.
func RunStdio(_ context.Context, client *proxy.Client) error {
	if client == nil {
		return fmt.Errorf("mcpsrv: nil proxy client")
	}

	s := server.NewMCPServer("codenexus", "0.1.0",
		server.WithToolCapabilities(true),
	)

	s.AddTool(
		mcp.NewTool("index_repo",
			mcp.WithDescription("Index a repository for code-graph search. Args: repo_path (string, abs path), incremental (bool, default true)."),
			mcp.WithString("repo_path", mcp.Required(), mcp.Description("Absolute path to the repo to index")),
			mcp.WithBoolean("incremental", mcp.Description("Reuse cached symbols if true (default true)")),
		),
		makeIndexHandler(client),
	)

	s.AddTool(
		mcp.NewTool("query",
			mcp.WithDescription("Hybrid BM25+vector search for symbols. Args: repo_hash (string), q (string), k (int, default 5). Returns top-k symbols with bm25/vector/rrf/final scores."),
			mcp.WithString("repo_hash", mcp.Required(), mcp.Description("Repo hash returned by index_repo")),
			mcp.WithString("q", mcp.Required(), mcp.Description("Free-text query")),
			mcp.WithNumber("k", mcp.Description("Top-k cutoff (default 5)")),
		),
		makeQueryHandler(client),
	)

	s.AddTool(
		mcp.NewTool("get_symbol",
			mcp.WithDescription("Fetch full symbol detail by id. Args: repo_hash (string), symbol_id (string)."),
			mcp.WithString("repo_hash", mcp.Required(), mcp.Description("Repo hash returned by index_repo")),
			mcp.WithString("symbol_id", mcp.Required(), mcp.Description("Symbol id from a prior query result")),
		),
		makeGetSymbolHandler(client),
	)

	s.AddTool(
		mcp.NewTool("list_callers",
			mcp.WithDescription("List callers of a symbol. Args: repo_hash (string), symbol_id (string), depth (int, default 1)."),
			mcp.WithString("repo_hash", mcp.Required(), mcp.Description("Repo hash returned by index_repo")),
			mcp.WithString("symbol_id", mcp.Required(), mcp.Description("Symbol id whose callers to enumerate")),
			mcp.WithNumber("depth", mcp.Description("Caller-graph depth (default 1)")),
		),
		makeListCallersHandler(client),
	)

	if err := server.ServeStdio(s); err != nil {
		return fmt.Errorf("mcpsrv: serve stdio: %w", err)
	}
	return nil
}

// NewHTTPHandler returns a placeholder /mcp/* handler. SSE upgrade per §3.2
// D-A2 is out of scope this slice; until then any caller gets 501.
//
// TODO: SSE upgrade per §3.2 D-A2.
func NewHTTPHandler() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
		w.WriteHeader(http.StatusNotImplemented)
		_, _ = io.WriteString(w, "MCP-over-HTTP/SSE not implemented yet (REQ-07 followup)")
	})
}

// ----- Tool handlers -----

func makeIndexHandler(client *proxy.Client) server.ToolHandlerFunc {
	return func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		repoPath, err := req.RequireString("repo_path")
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		incremental := req.GetBool("incremental", true)
		out, err := client.IndexRepo(ctx, proxy.IndexRepoArgs{
			RepoPath:    repoPath,
			Incremental: incremental,
		})
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		return marshalToolResult(out)
	}
}

func makeQueryHandler(client *proxy.Client) server.ToolHandlerFunc {
	return func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		repoHash, err := req.RequireString("repo_hash")
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		q, err := req.RequireString("q")
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		k := req.GetInt("k", 5)
		out, err := client.Query(ctx, proxy.QueryArgs{
			RepoHash: repoHash,
			Q:        q,
			K:        k,
		})
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		return marshalToolResult(out)
	}
}

func makeGetSymbolHandler(client *proxy.Client) server.ToolHandlerFunc {
	return func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		repoHash, err := req.RequireString("repo_hash")
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		symbolID, err := req.RequireString("symbol_id")
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		out, err := client.GetSymbol(ctx, proxy.GetSymbolArgs{
			RepoHash: repoHash,
			SymbolID: symbolID,
		})
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		return marshalToolResult(out)
	}
}

func makeListCallersHandler(client *proxy.Client) server.ToolHandlerFunc {
	return func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		repoHash, err := req.RequireString("repo_hash")
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		symbolID, err := req.RequireString("symbol_id")
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		depth := req.GetInt("depth", 1)
		out, err := client.ListCallers(ctx, proxy.ListCallersArgs{
			RepoHash: repoHash,
			SymbolID: symbolID,
			Depth:    depth,
		})
		if err != nil {
			return mcp.NewToolResultError(err.Error()), nil
		}
		return marshalToolResult(out)
	}
}

// marshalToolResult encodes the operation result as compact JSON and wraps it
// in an MCP text content block. MCP clients render this as the tool output.
func marshalToolResult(v any) (*mcp.CallToolResult, error) {
	b, err := json.Marshal(v)
	if err != nil {
		return mcp.NewToolResultError(fmt.Sprintf("marshal result: %v", err)), nil
	}
	return mcp.NewToolResultText(string(b)), nil
}
