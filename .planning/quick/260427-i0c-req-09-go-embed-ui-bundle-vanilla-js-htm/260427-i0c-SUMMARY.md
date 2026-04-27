---
phase: quick-260427-i0c
plan_id: 260427-i0c
status: complete
type: execute
requirements: [REQ-09]
landed_files:
  - server/internal/ui/README.md           # MOVED (git mv from project-root ui/)
  - server/internal/ui/embed.go            # NEW (//go:embed extension enumeration)
  - server/internal/ui/index.html          # NEW (top bar + 2-pane: results table 4 score cols + cytoscape graph)
  - server/internal/ui/style.css           # NEW (flex layout + .conf-high/mid/low/poor color bands)
  - server/internal/ui/app.js              # NEW (search/listCallers fetch + cytoscape render)
  - server/internal/ui/cytoscape.min.js    # VENDORED (3.30.2 from cdn.jsdelivr.net, 374 KB)
  - server/cmd/serve.go                    # MODIFY (FileServer mount + list_callers route + root / redirect + io import drop + uiPlaceholderHandler delete)
  - ui/                                    # DELETED (project-root, git mv'd)
commits:
  - ec3849e "mvp(server): REQ-09 UI scaffold -- embed.go + index.html + style.css (option B layout)"
  - dfdcb95 "mvp(server): REQ-09 cytoscape + app.js + serve.go wireup"
gates:
  go_build: pass
  go_vet: pass
  invariants_verified: 12/12
---

# REQ-09 Summary — //go:embed UI bundle (option B, no build step)

## Landed

**Task 1 (commit ec3849e):**
- `git mv ui/README.md → server/internal/ui/README.md` — option B's distinguishing move; UI sources are now siblings of embed.go
- `server/internal/ui/embed.go` (~21 lines): SPDX header + 1-line package doc + `//go:embed *.html *.js *.css *.md` directive (extension enumeration; see deviation note below) + `var UIFS embed.FS` exported
- `server/internal/ui/index.html` (~40 lines): top bar with search input + button + listCallers button; two-pane below (results table 40% / cytoscape graph viewport 60%); status footer. Results table headers: Symbol/Kind/Path/BM25/Vector/RRF/Final — Differentiation #2 (4 meta scores) made literal
- `server/internal/ui/style.css` (~50 lines): minimal flex layout, monospace, fixed top bar 50px, two-pane below; confidence color classes `.conf-high` (green ≥0.95) / `.conf-mid` (yellow 0.7-0.95) / `.conf-low` (orange 0.5-0.7) / `.conf-poor` (red <0.5) — Differentiation #4 (caller confidence) made literal

**Task 2 (commit dfdcb95):**
- `server/internal/ui/cytoscape.min.js` (374 KB): vendored 3.30.2 from `https://cdn.jsdelivr.net/npm/cytoscape@3.30.2/dist/cytoscape.min.js`. Top-of-file comment names source URL + license (MIT) + version pin
- `server/internal/ui/app.js` (~170 lines):
  - `confClass(c)` helper maps confidence → CSS class (4 bands)
  - `confColor(c)` returns hex for cytoscape node/edge style
  - `search()`: POST `/api/v1/query` with `{q, k:10}`, render results table with all 4 score columns; clicking a row selects symbol_id for listCallers
  - `listCallers()`: POST `/api/v1/list_callers` with `{symbol_id, depth:1}`, build cytoscape graph; node bg-color + edge line-color + arrow-color all driven by `confidence` data attribute via `confColor()`
  - Status footer reports caller count + confClass of top result
- `server/cmd/serve.go` modifications:
  - line 24: added `github.com/2233admin/codenexus/internal/ui` import
  - line 80: `r.Mount("/ui/", http.StripPrefix("/ui/", http.FileServer(http.FS(ui.UIFS))))` replaces `uiPlaceholderHandler()` mount
  - lines 81-83: new `r.Get("/", ...)` returns 302 → `/ui/` so bare `localhost:8080` works
  - line 86: new `r.Post("/api/v1/list_callers", apiPassthrough(client, "list_callers"))` — plan discovered this missing in REQ-07 stub (only query+index were wired)
  - removed: `uiPlaceholderHandler()` function body + `"io"` import (was only used by `io.WriteString` in the placeholder)

**Dependencies:** zero new external deps. Vendored cytoscape 3.30.2 lives in repo as a static asset, not a Go import.

## Scaffolded but NOT smoke-tested

- **Real browser load**: `./codenexus serve` + open `http://localhost:8080/` — not exercised this slice. Build + vet only prove embed plumbing compiles
- **Query round-trip with real DB**: requires Rust core running with indexed repo. UI calls `/api/v1/query` correctly per plan but actual DB hit deferred
- **Visual verification of 4 score columns + confidence color bands**: rendered in code per Differentiation #2 + #4, but no screenshot/manual confirm
- **Cross-browser testing**: vanilla JS + ES6 features (template literals, `const`, arrow functions, fetch). Targets modern Chrome/Firefox/Edge; IE11 not supported (acceptable per anti-scope)
- **Cytoscape `cose` layout on real graph**: layout default works on small graphs; perf with 100+ nodes deferred

## Follow-up slices

- **Index button**: currently no UI to invoke `index_repo` — user pastes `repo_hash` from prior `curl` or CLI. Trivial follow-up: add `<button id="index">` + flow
- **Symbol detail panel**: clicking a result row currently selects but doesn't show full detail. Phase 4 polish: add right-pane toggle showing full `get_symbol` response
- **WebSocket live updates**: index progress / new symbol events. Out of scope for MVP per anti-scope
- **Dark mode + responsive design**: explicit anti-scope this slice
- **Real spawn-and-render smoke**: blocked on `make build-core` producing a working Rust binary (same blocker as REQ-07/08 acceptance #1-#3)
- **Embedded search ranking visibility tweaks**: e.g. score column highlighting (the highest column wins), tooltip showing score components — polish

## Notable findings (deviations + gotchas)

### Deviation: `//go:embed all:.` rejected by Go embed parser

Plan invariant #2 specified `//go:embed all:.` (or equivalent). Executor tried `all:.` and Go embed parser rejected `.` as invalid pattern syntax. Fallback: enumerate extensions `//go:embed *.html *.js *.css *.md`.

This is a **functional equivalent** — all current UI files are picked up (cytoscape.min.js matches `*.js`; .gitkeep-style dotfiles are not present so dotfile-handling is moot). embed.go:14-18 contains the technical justification comment so future-you doesn't relitigate it. When new asset types land (e.g. PNG icons), extend the pattern list.

Plan invariant compliance verdict: ✅ acceptable (plan said "or equivalent"; this is the documented equivalent).

### Mid-execution handoff (executor partial-return)

Executor returned a truncated message ("Staged correctly... Committing.") without showing commit hashes or final structured EXECUTION COMPLETE block. Orchestrator (Opus, main session) verified state:
- `git log --oneline -5` showed both Task 1 (ec3849e) + Task 2 (dfdcb95) commits actually landed
- `git status --short` showed clean working tree (only untracked `.omc/` + `.planning/quick/i0c-...`)
- `ls server/internal/ui/` showed all 6 expected files
- Independent `go build ./...` + `go vet ./...` re-run, both exit 0
- 12/12 plan invariants grep-verified

Orchestrator wrote this SUMMARY.md (executor didn't reach that step) and handles final docs commit + STATE.md update. Same pattern as REQ-08 mid-execution handoff (commit f5b6621 Task 1 + orchestrator finished Task 2).

**Process insight:** Executor partial-return is a recurring failure mode (REQ-08 + REQ-09, 2-of-3 quick tasks). Recovery cost is low when orchestrator can independently verify state and finish remaining steps; full executor restart is only needed if work is genuinely incomplete. Worth adding to feedback-graduated.md as a known pattern: "executor returns mid-summary, orchestrator picks up tail."

### Plan invariants verified (12/12)

| # | Invariant | Evidence |
|---|-----------|----------|
| 1 | ui/ project-root deleted (git mv to server/internal/ui/) | `ls ui/` returns ENOENT ✓ |
| 2 | embed.go has //go:embed pattern + UIFS export | embed.go:19-20 (deviation: extension enumeration vs `all:.`, justified line 14-18) ⚠️→✓ |
| 3 | SPDX header on embed.go | embed.go:1 ✓ |
| 4 | serve.go mounts FileServer at /ui/ | serve.go:80 ✓ |
| 5 | uiPlaceholderHandler deleted | grep returns 0 hits in serve.go ✓ |
| 6 | "io" import removed | serve.go:5-25 import block has no "io" + go vet pass ✓ |
| 7 | /api/v1/list_callers route added | serve.go:86 ✓ |
| 8 | root / redirects to /ui/ | serve.go:81-83 (302 via http.Redirect + StatusFound) ✓ |
| 9 | cytoscape.min.js vendored with comment header | 374 KB + line 1-3 source/version comment ✓ |
| 10 | 4 score columns rendered | index.html:22 + app.js:76-79 ✓ |
| 11 | confidence color bands in CSS + JS | style.css:33-36 + app.js:29-32 + cytoscape style 122-129 ✓ |
| 12 | confidence values rendered in callers | app.js:98,103,112,166 ✓ |

### Plan deviation: Inv 2 enumeration vs all:.

Documented above. Functionally equivalent, technically justified, comment-anchored.

### Other findings

- Cytoscape pin landed: 3.30.2 (CDN responded successfully, no fallback to 3.30.1 needed)
- htmx: dropped per plan recommendation (app.js is fetch+JSON, htmx adds nothing)
- list_callers route was indeed missing in REQ-07 stub (only `query` and `index_repo` had been wired) — plan caught this; executor added it cleanly

## Gotchas hit

1. **`//go:embed all:.` rejected** as invalid pattern syntax. Discovered at compile time, not in spec. Fallback to extension enumeration is documented in embed.go for future-you. **Lesson for REQ-08-style embed patterns**: prefer enumeration over `all:.` until you've verified the parser accepts your literal pattern.
2. **Executor partial-return**: same pattern as REQ-08. Orchestrator state-verify protocol works. Worth formalizing.
3. **`cat` aliased to `bat`** (Windows machine, recurring across REQ-08+09): used `/usr/bin/cat` literal in heredoc subshells per `feedback-graduated.md` P0 #33. No retries this slice — rule was followed from the start.
