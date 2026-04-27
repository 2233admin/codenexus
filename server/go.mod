module github.com/2233admin/codenexus

go 1.25.5

// Phase 0 spike will fill:
//   - github.com/go-chi/chi/v5 (HTTP router)
//   - github.com/mark3labs/mcp-go (MCP server SDK)
//   - github.com/spf13/cobra (CLI)
//   - encoding/json + net/http (A2A client)

require (
	github.com/go-chi/chi/v5 v5.2.5
	github.com/google/uuid v1.6.0
	github.com/mark3labs/mcp-go v0.49.0
	github.com/spf13/cobra v1.10.2
)

require (
	github.com/google/jsonschema-go v0.4.2 // indirect
	github.com/inconshreveable/mousetrap v1.1.0 // indirect
	github.com/spf13/cast v1.7.1 // indirect
	github.com/spf13/pflag v1.0.9 // indirect
	github.com/yosida95/uritemplate/v3 v3.0.2 // indirect
)
