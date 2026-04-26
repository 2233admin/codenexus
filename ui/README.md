# UI

Vanilla JS + HTMX + cytoscape.js. Served by Go via `//go:embed`.

Phase 1 MVP will fill:
- `index.html` — search box + cytoscape graph viewport
- `app.js` — A2A client calls (via Go HTTP proxy) + cytoscape rendering
- `style.css` — minimal layout

Anti-scope: no React/Vue/Svelte, no build step, no package.json.
