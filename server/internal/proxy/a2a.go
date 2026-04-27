// SPDX-License-Identifier: Apache-2.0

// Package proxy is the A2A v0.2 client used by the Go service layer. It
// targets the Rust core's /tasks/send + /tasks/{id} endpoints, materializes
// the §3.5 envelope (skill_id="code-graph"; operation discriminator inside
// parts[].data), and decodes responses while preserving all four meta scores.
package proxy

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"time"

	"github.com/google/uuid"
)

// Client is a thin HTTP wrapper around the Rust core's A2A endpoint.
type Client struct {
	httpc   *http.Client
	baseURL string
}

// New returns a Client targeting http://localhost:<rustPort>.
func New(rustPort int) *Client {
	return &Client{
		httpc:   &http.Client{Timeout: 60 * time.Second},
		baseURL: fmt.Sprintf("http://localhost:%d", rustPort),
	}
}

// ----- §3.5 envelope shapes -----

// Envelope is the wire-level body for POST /tasks/send.
type Envelope struct {
	TaskID   string    `json:"task_id"`
	SkillID  string    `json:"skill_id"`
	Messages []Message `json:"messages"`
}

// Message is one chat-style turn inside an Envelope.
type Message struct {
	Role  string `json:"role"`
	Parts []Part `json:"parts"`
}

// Part is a single content part. We use Type="data" for structured args/results
// and Type="text" for human-readable error narration on failure (§3.3).
type Part struct {
	Type string          `json:"type"`
	Data json.RawMessage `json:"data,omitempty"`
	Text string          `json:"text,omitempty"`
}

// taskCreateResponse is the immediate POST /tasks/send body: task_id + state.
type taskCreateResponse struct {
	TaskID string `json:"task_id"`
	State  string `json:"state"`
}

// taskPollResponse is the polled GET /tasks/{id} body: state + agent message
// once finished. The agent message's Parts carry the operation's data part.
type taskPollResponse struct {
	TaskID  string  `json:"task_id"`
	State   string  `json:"state"`
	Message Message `json:"message"`
}

// errorPayload is the §3.3 machine-readable failure body inside parts[].data.
type errorPayload struct {
	Code      string          `json:"code"`
	Retryable bool            `json:"retryable"`
	Details   json.RawMessage `json:"details,omitempty"`
}

// ----- Operation argument types -----

// IndexRepoArgs is the payload for operation=index_repo.
type IndexRepoArgs struct {
	RepoPath    string `json:"repo_path"`
	Incremental bool   `json:"incremental"`
}

// QueryArgs is the payload for operation=query.
type QueryArgs struct {
	RepoHash string `json:"repo_hash"`
	Q        string `json:"q"`
	K        int    `json:"k"`
}

// GetSymbolArgs is the payload for operation=get_symbol.
type GetSymbolArgs struct {
	RepoHash string `json:"repo_hash"`
	SymbolID string `json:"symbol_id"`
}

// ListCallersArgs is the payload for operation=list_callers.
type ListCallersArgs struct {
	RepoHash string `json:"repo_hash"`
	SymbolID string `json:"symbol_id"`
	Depth    int    `json:"depth"`
}

// ----- Operation result types (§3.5.1 — §3.5.4) -----

// Range is a [start_line, end_line] inclusive line range.
type Range struct {
	StartLine int `json:"start_line"`
	EndLine   int `json:"end_line"`
}

// Symbol is the dense form returned by get_symbol; QueryHit is the sparse form
// used inside query results. They overlap on identity fields.
type Symbol struct {
	SymbolID string   `json:"symbol_id"`
	Kind     string   `json:"kind"`
	Name     string   `json:"name"`
	Path     string   `json:"path"`
	Range    Range    `json:"range"`
	Parent   string   `json:"parent,omitempty"`
	Snippet  string   `json:"snippet,omitempty"`
	Children []string `json:"children,omitempty"`
	Imports  []string `json:"imports,omitempty"`
}

// QueryHit is a single search hit; ALL FOUR meta scores are preserved per §3.4.
type QueryHit struct {
	SymbolID    string  `json:"symbol_id"`
	Kind        string  `json:"kind"`
	Name        string  `json:"name"`
	Path        string  `json:"path"`
	Range       Range   `json:"range"`
	Parent      string  `json:"parent,omitempty"`
	Snippet     string  `json:"snippet,omitempty"`
	Bm25Score   float64 `json:"bm25_score"`
	VectorScore float64 `json:"vector_score"`
	RrfScore    float64 `json:"rrf_score"`
	FinalScore  float64 `json:"final_score"`
}

// QueryResult is the full operation=query response body.
type QueryResult struct {
	Operation string     `json:"operation"`
	Results   []QueryHit `json:"results"`
}

// IndexResult is the operation=index_repo response body.
type IndexResult struct {
	Operation         string `json:"operation"`
	RepoHash          string `json:"repo_hash"`
	FilesIndexed      int    `json:"files_indexed"`
	SymbolsIndexed    int    `json:"symbols_indexed"`
	DurationMs        int    `json:"duration_ms"`
	LastIndexedCommit string `json:"last_indexed_commit,omitempty"`
}

// SymbolResult is the operation=get_symbol response body.
type SymbolResult struct {
	Operation string `json:"operation"`
	Symbol    Symbol `json:"symbol"`
}

// CallerHit is one row of operation=list_callers results.
//
// Confidence is the highest edge confidence observed on any Calls edge from
// this caller to the queried target (per ARCHITECTURE.md §9.7; default filter
// ≥ 0.5). Surfacing it lets agents distinguish high-confidence direct calls
// (e.g. resolver step 1, conf 1.0) from softer matches (e.g. step 3 same-file
// fallback, conf 0.9). Phase 4 Leiden community detection can reuse this as
// edge weight.
type CallerHit struct {
	SymbolID   string  `json:"symbol_id"`
	Name       string  `json:"name"`
	Path       string  `json:"path"`
	EdgeKind   string  `json:"edge_kind"`
	Confidence float64 `json:"confidence,omitempty"`
}

// CallersResult is the operation=list_callers response body.
type CallersResult struct {
	Operation string      `json:"operation"`
	Callers   []CallerHit `json:"callers"`
}

// ----- Core method -----

// SendTask materializes the §3.5 envelope, POSTs it, then polls until the task
// reaches a terminal state. On state=completed it returns the agent message's
// data part as raw JSON for the caller to decode into its operation-specific
// result struct. On state=failed it parses §3.3 and returns a wrapped error.
//
// `args` may be any value that JSON-marshals into an object; SendTask merges
// {"operation": op} on top of those fields. Pass nil if the operation has no
// args (none currently — but kept for forward compatibility).
func (c *Client) SendTask(ctx context.Context, op string, args any) (json.RawMessage, error) {
	taskID, err := newUUIDv7()
	if err != nil {
		return nil, fmt.Errorf("a2a: generate task_id: %w", err)
	}

	dataPart, err := mergeOperationData(op, args)
	if err != nil {
		return nil, fmt.Errorf("a2a: build data part: %w", err)
	}
	envelope := Envelope{
		TaskID:  taskID,
		SkillID: "code-graph",
		Messages: []Message{{
			Role:  "user",
			Parts: []Part{{Type: "data", Data: dataPart}},
		}},
	}

	body, err := json.Marshal(envelope)
	if err != nil {
		return nil, fmt.Errorf("a2a: marshal envelope: %w", err)
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.baseURL+"/tasks/send", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("a2a: build request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := c.httpc.Do(req)
	if err != nil {
		return nil, fmt.Errorf("a2a: post /tasks/send: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode/100 != 2 {
		return nil, fmt.Errorf("a2a: /tasks/send status %d", resp.StatusCode)
	}
	var created taskCreateResponse
	if err := json.NewDecoder(resp.Body).Decode(&created); err != nil {
		return nil, fmt.Errorf("a2a: decode /tasks/send response: %w", err)
	}
	if created.TaskID == "" {
		created.TaskID = taskID
	}

	pollDeadline := time.Now().Add(60 * time.Second)
	ticker := time.NewTicker(250 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("a2a: context done while polling: %w", ctx.Err())
		case <-ticker.C:
		}
		if time.Now().After(pollDeadline) {
			return nil, fmt.Errorf("a2a: poll wall-timeout after 60s (task_id=%s)", created.TaskID)
		}
		polled, err := c.pollOnce(ctx, created.TaskID)
		if err != nil {
			return nil, err
		}
		switch polled.State {
		case "completed":
			return extractDataPart(polled.Message)
		case "failed":
			code, text := extractFailure(polled.Message)
			return nil, fmt.Errorf("a2a %s failed: %s (code=%s)", op, text, code)
		case "submitted", "working", "":
			continue
		default:
			return nil, fmt.Errorf("a2a: unknown task state %q", polled.State)
		}
	}
}

func (c *Client) pollOnce(ctx context.Context, taskID string) (*taskPollResponse, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/tasks/"+taskID, nil)
	if err != nil {
		return nil, fmt.Errorf("a2a: build poll request: %w", err)
	}
	resp, err := c.httpc.Do(req)
	if err != nil {
		return nil, fmt.Errorf("a2a: poll: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode/100 != 2 {
		return nil, fmt.Errorf("a2a: poll status %d", resp.StatusCode)
	}
	var p taskPollResponse
	if err := json.NewDecoder(resp.Body).Decode(&p); err != nil {
		return nil, fmt.Errorf("a2a: decode poll: %w", err)
	}
	return &p, nil
}

// ----- Convenience wrappers -----

// IndexRepo invokes operation=index_repo and decodes the result.
func (c *Client) IndexRepo(ctx context.Context, args IndexRepoArgs) (IndexResult, error) {
	var out IndexResult
	raw, err := c.SendTask(ctx, "index_repo", args)
	if err != nil {
		return out, err
	}
	if err := json.Unmarshal(raw, &out); err != nil {
		return out, fmt.Errorf("a2a: decode index_repo result: %w", err)
	}
	return out, nil
}

// Query invokes operation=query and decodes the result, preserving all four
// meta scores per §3.4.
func (c *Client) Query(ctx context.Context, args QueryArgs) (QueryResult, error) {
	var out QueryResult
	raw, err := c.SendTask(ctx, "query", args)
	if err != nil {
		return out, err
	}
	if err := json.Unmarshal(raw, &out); err != nil {
		return out, fmt.Errorf("a2a: decode query result: %w", err)
	}
	return out, nil
}

// GetSymbol invokes operation=get_symbol and decodes the result.
func (c *Client) GetSymbol(ctx context.Context, args GetSymbolArgs) (SymbolResult, error) {
	var out SymbolResult
	raw, err := c.SendTask(ctx, "get_symbol", args)
	if err != nil {
		return out, err
	}
	if err := json.Unmarshal(raw, &out); err != nil {
		return out, fmt.Errorf("a2a: decode get_symbol result: %w", err)
	}
	return out, nil
}

// ListCallers invokes operation=list_callers and decodes the result.
func (c *Client) ListCallers(ctx context.Context, args ListCallersArgs) (CallersResult, error) {
	var out CallersResult
	raw, err := c.SendTask(ctx, "list_callers", args)
	if err != nil {
		return out, err
	}
	if err := json.Unmarshal(raw, &out); err != nil {
		return out, fmt.Errorf("a2a: decode list_callers result: %w", err)
	}
	return out, nil
}

// ----- Helpers -----

// mergeOperationData turns (op, args) into the JSON object that goes inside
// parts[].data — i.e. {"operation": op, ...args fields flat}. args may be nil,
// a struct, a map, or anything else json.Marshal accepts as an object.
func mergeOperationData(op string, args any) (json.RawMessage, error) {
	merged := map[string]any{"operation": op}
	if args != nil {
		// Marshal args, then unmarshal into a map so we can flatten it.
		b, err := json.Marshal(args)
		if err != nil {
			return nil, fmt.Errorf("marshal args: %w", err)
		}
		// Empty struct case marshals to "null" if args is a typed nil.
		if string(b) != "null" && len(b) > 0 {
			var asMap map[string]any
			if err := json.Unmarshal(b, &asMap); err != nil {
				return nil, fmt.Errorf("args is not a JSON object: %w", err)
			}
			for k, v := range asMap {
				if k == "operation" {
					continue // protect the discriminator
				}
				merged[k] = v
			}
		}
	}
	return json.Marshal(merged)
}

// extractDataPart pulls the first parts[].Type=="data" data field from msg.
func extractDataPart(msg Message) (json.RawMessage, error) {
	for _, p := range msg.Parts {
		if p.Type == "data" && len(p.Data) > 0 {
			return p.Data, nil
		}
	}
	return nil, errors.New("a2a: completed task has no data part in agent message")
}

// extractFailure returns (code, humanText) from a failed agent message per §3.3.
// Both a text part (human) and a data part with errorPayload are expected.
func extractFailure(msg Message) (string, string) {
	var code, text string
	for _, p := range msg.Parts {
		switch p.Type {
		case "text":
			if text == "" {
				text = p.Text
			}
		case "data":
			var ep errorPayload
			if err := json.Unmarshal(p.Data, &ep); err == nil {
				if ep.Code != "" {
					code = ep.Code
				}
			}
		}
	}
	if text == "" {
		text = "(no human-readable error text)"
	}
	if code == "" {
		code = "UNKNOWN"
	}
	return code, text
}

// newUUIDv7 returns a UUIDv7 string suitable for trace_id / task_id (§5.4).
// google/uuid added NewV7 in v1.6.0.
func newUUIDv7() (string, error) {
	id, err := uuid.NewV7()
	if err != nil {
		return "", err
	}
	return id.String(), nil
}
