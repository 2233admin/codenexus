// CodeNexus server — Go service layer.
//
// Phase -1 / 0 will fill:
//   - Spawn embedded codenexus-core binary as A2A daemon (//go:embed)
//   - chi HTTP router serving UI (//go:embed of ui/) + REST API
//   - mark3labs/mcp-go MCP stdio handler, tools wrap A2A calls into Rust core
//   - cobra CLI: codenexus index <repo>, codenexus query <text>, codenexus serve, codenexus mcp
//   - subprocess lifecycle: spawn on serve start, healthcheck via A2A GET, restart on crash

package main

import "fmt"

func main() {
	fmt.Println("codenexus: pre-MVP placeholder. See .planning/ for status.")
}
