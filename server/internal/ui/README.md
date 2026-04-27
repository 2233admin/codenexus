# UI

Vanilla JS + cytoscape.js. Served by Go via `//go:embed` from this directory
(`server/internal/ui/`). No build step — edit a file, `go build`, refresh.

## Files

- `embed.go` — `//go:embed all:.` exports `UIFS embed.FS` for serve.go.
- `index.html` — single-page shell: search box, results table, graph viewport.
- `app.js` — vanilla JS; calls `/api/v1/query` + `/api/v1/list_callers`.
- `style.css` — minimal two-pane layout + confidence color bands.
- `cytoscape.min.js` — vendored cytoscape 3.x (MIT). Pinned: see top-of-file comment.

## Anti-scope

No React/Vue/Svelte, no build step, no package.json, no npm/yarn/pnpm, no TypeScript.
Vendored deps only — no CDN at runtime so the single fat-binary stays self-contained.

HTMX was considered and dropped — the app.js shape (fetch + JSON in/out + DOM render)
doesn't benefit from HTMX's form-replace-fragment patterns. Re-add later only if an
actual form-driven flow appears.

## Updating cytoscape

```bash
curl -L -o cytoscape.min.js https://cdn.jsdelivr.net/npm/cytoscape@<version>/dist/cytoscape.min.js
# update top-of-file comment with new version
go build ./...   # picks up via embed.FS automatically
```
