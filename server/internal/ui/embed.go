// SPDX-License-Identifier: Apache-2.0

// Package ui owns the embedded browser UI bundle. Files in this directory
// (HTML/JS/CSS + vendored cytoscape) are baked into the binary via
// //go:embed and served by serve.go at /ui/. No build step — editing a file
// and re-running `go build` picks up changes.
package ui

import "embed"

// UIFS is the embedded filesystem rooted at this directory. serve.go mounts
// it via http.FileServer(http.FS(ui.UIFS)).
//
// We enumerate file extensions explicitly because `//go:embed all:.` rejects
// the "." path as invalid pattern syntax in Go 1.26. Embedding README.md is
// harmless — it just becomes a fetchable static resource. When adding new
// asset types (e.g. .png, .svg) extend this list.
//
//go:embed *.html *.js *.css *.md
var UIFS embed.FS
