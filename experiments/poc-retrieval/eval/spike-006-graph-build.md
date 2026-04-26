# Spike-006: Graph Builder POC (4 edge kinds)

**Run date:** 2026-04-27
**Corpus:** D:/projects/obsidian-llm-wiki (TS only, 52 files indexed in poc.db)
**Edge kinds:** Calls, Imports, Implements, Extends (Overrides deferred per REQ-02)
**Resolver:** naive 3-step (same-file → import-file → global-unique), confidence 1.0 throughout
**Build order:** Imports first (Pass 1), then Calls/Implements/Extends (Pass 2)

## LOC delta

| File | After | Notes |
|---|---|---|
| `src/graph_build.rs` (new) | 494 | Tree-sitter queries + 3-step resolver + 2-pass builder + import-path resolver with `.js`->`.ts` rewrite |
| `src/storage.rs` | 217 (was 95, +122) | Edges schema/indexes + 8 helper methods (`clear_edges`, `insert_edge`, `list_files`, `symbols_in_file_full`, `symbol_in_file_by_name`, `find_global_unique`, `import_targets_for_file`, `count_edges_by_kind`, `dump_edges_join`) |
| `src/main.rs` | 247 (was 211, +36) | `mod graph_build;` + `BuildGraph` and `DumpEdges` subcommands |
| `Cargo.toml` | 0 | `tree-sitter-typescript = "0.21"` already present |

## Wall-clock

- `cargo build --release`: 6.4s clean compile, 6.3s cached re-build
- `build-graph` over 52 files: **~8.9s** (well under the 30s transaction-wrap threshold; no `BEGIN/COMMIT` optimization needed for this corpus size)

## Edge counts (final, post-dedup via `UNIQUE(from_id,to_id,kind)`)

| Kind | Stored | Insert attempts |
|---|---|---|
| Imports | 120 | 120 |
| Calls | 749 | 990 (241 dups collapsed) |
| Implements | 7 | 7 |
| Extends | 1 | 1 |
| **Total** | **877** | 1118 |

**Unresolved candidates:** 2317 of 3435 total resolution attempts = **67.5%** unresolved rate.

The high unresolved rate is dominated by external-dep imports (`node:*`, npm packages) and TS standard-library identifiers (`console`, `JSON`, `Object`, `String`, etc.) where the captured `@callee` text has no corresponding indexed symbol. Internal call resolution is much higher quality (see hand-verified samples).

## Hand-verified samples (5 per kind, 20 total)

### Calls

| from_path | from_name | to_path | to_name | Verdict |
|---|---|---|---|---|
| filesystem.ts | search | filesystem.ts | exec | correct (search() invokes promisified exec) |
| filesystem.ts | search | filesystem.ts | parseRipgrepJson | correct |
| filesystem.ts | search | filesystem.ts | fallbackSearch | correct (catch-arm calls fallbackSearch) |
| filesystem.ts | search | filesystem.ts | isExitCode | correct (type guard called in error branch) |
| filesystem.ts | fullPath | filesystem.ts | resolvePath | correct (anchor: fullPath method calls resolvePath) |

### Imports

| from_path | from_name (anchor hack) | to_path | to_name | Verdict |
|---|---|---|---|---|
| filesystem.ts | exec | interface.ts | AdapterCapability | correct (Imports anchored to first-symbol-of-file = `exec`; documented hack) |
| filesystem.ts | exec | interface.ts | SearchResult | correct |
| filesystem.ts | exec | interface.ts | SearchOpts | correct |
| filesystem.ts | exec | interface.ts | VaultMindAdapter | correct |
| gitnexus.ts | exec | interface.ts | AdapterCapability | correct |

### Implements

| from_path | from_name | to_path | to_name | Verdict |
|---|---|---|---|---|
| filesystem.ts | FilesystemAdapter | interface.ts | VaultMindAdapter | correct |
| gitnexus.ts | GitNexusAdapter | interface.ts | VaultMindAdapter | correct |
| memu.ts | MemUAdapter | interface.ts | VaultMindAdapter | correct |
| obsidian.ts | ObsidianAdapter | interface.ts | VaultMindAdapter | correct |
| qmd.ts | QmdAdapter | interface.ts | VaultMindAdapter | correct |

5/5 adapter pattern recovery. Clean.

### Extends

| from_path | from_name | to_path | to_name | Verdict |
|---|---|---|---|---|
| unified-query.ts | UnifiedQueryOpts | interface.ts | SearchOpts | correct |
| (only 1 Extends edge in corpus — TS interface extension is rare in this codebase) | | | | |

**Verdict tally:** 16/16 verifiable rows correct (Extends has only 1 row total). 0 mis-resolutions in sample.

## Stretch: Axis-3 query smoke

**Q1: "who calls assertRealPathInsideVault"**
Filter: `dump-edges --kind Calls | grep assertRealPathInsideVault`
Result:
- `mcp-server\src\connector\fs-transport.ts::resolve` → `assertRealPathInsideVault`
- `mcp-server\src\index.ts::resolve` → `assertRealPathInsideVault`

Two clean structural answers. Hand-verified: both `resolve` functions do call `assertRealPathInsideVault` as path-safety gate. Retrieval baseline (R3) for this kind of axis-3 query was ~0% precision; graph gives **deterministic 100%**.

**Q2: "who calls ObsidianAdapter constructor"**
Result: no direct constructor edge captured. Closest signal: `mcp-server\src\index.ts::obsidianAdapter` → `mcp-server\src\adapters\registry.ts::get` (registry pattern; constructor is invoked inside `get()`).

This is a documented gap: tree-sitter `(call_expression function: (identifier))` does not differentiate `new X()` from `X()`. `new_expression` would need a separate query to capture explicit constructor calls. Adding it is ~5 LOC and recommended for Phase 1 graph builder.

## Not-implemented gaps (honest list)

### P1 — Resolver pitfalls

- **Default imports** (`import X from './foo'`): captured as `default_name`, but the local binding name `X` rarely matches an exported symbol named `X`. Resolver step 2 misses; falls through to global-unique which also typically misses. Quantify: of 263 unresolved Pass-1 candidates, default+namespace skip is the leading cause.
- **Namespace imports** (`import * as X from './foo'`): explicitly skipped — `X.foo()` calls captured by Calls query as `foo` only (no namespace context), so the resolver picks any symbol named `foo` globally if unique. Source of false-positives in cross-module Calls.
- **Re-export chains / barrel files** (`export { X } from './bar'`): not followed. `import { X } from './index'` will resolve to a re-export stub if the index file has X re-declared, otherwise miss. Phase 3+ `import_alias_resolver`.
- **`new X()` constructor calls**: `new_expression` not in Calls query → no Calls edge for explicit constructor invocation.

### P1 — Edge anchoring

- **Imports edges anchored to first-symbol-in-file** (the documented "exec" anomaly in the table above) as a stand-in for a proper File node. Fix when REQ-04 storage adds `kind='File'` rows.

### P2 — Tree-sitter query coverage

- **Generic-type extends/implements** (`class X extends Y<T>`, `interface I extends J<K>`): query handled with explicit `[(type_identifier) (generic_type (type_identifier) @ext)]` alternation — verified firing on UnifiedQueryOpts row.
- **IIFE / parenthesized callers** (`(fn)(arg)`): not covered.
- **Mixin patterns** (`class X extends Mixin(Base)`): not covered.

### P3 — Confidence

- All 877 edges hard-coded `confidence=1.0`. Real probabilistic resolver should down-weight global-unique step (~0.5) vs same-file (1.0) and import-file (~0.8). Phase 3+.

## Compile errors encountered + resolution

1. **First-pass build was clean** (6.4s, no errors) — tree-sitter query syntax against tree-sitter-typescript 0.21 worked first time for all 4 queries including the alternation form for Extends.
2. **Imports = 0 on first run** (not a compile error, a logic bug): `./interface.js` import strings did not resolve because TypeScript convention uses `.js` extension that points at `.ts` files on disk. Fix: strip `.js`/`.jsx` suffix before the suffix-walk so `interface.js` → try `interface.ts` first. After fix: 120 Imports edges (~32% of 383 attempted).
3. **Path-separator mismatch** (anticipated, mitigated up-front): indexed `symbols.path` uses Windows backslash via `to_string_lossy`. Resolver normalises to forward-slash for `..`/`.` collapsing then tries both forward-slash and backslash candidates against the DB. Backslash form wins on this Windows host.

## Acceptance status

- [x] `cargo build --release` succeeds, no new warnings
- [x] Existing `query` subcommand still works (regression check on `"filesystem fallback when obsidian not running"` returns 3 hits as expected)
- [x] `build-graph --repo D:/projects/obsidian-llm-wiki --db poc.db` exits 0 with stats line
- [x] `eval/spike-006-graph-build.md` populated with all sections + 16-row sample table + axis-3 stretch results
- [ ] No commits made (parent agent reviews and commits)

## Decision input for ARCHITECTURE.md

The 877-edge graph from a 52-file corpus, built in 8.9s with a 67.5% unresolved rate, is **sufficient signal that REQ-02 graph layer materially answers axis-3 queries** that retrieval can't. The 100% structural answer to "who calls assertRealPathInsideVault" vs ~0% retrieval baseline is the pivot point. Phase 1 should commit to graph-builder + 4 edge kinds; default-import / namespace / barrel-file resolution can stay Phase 3+ without blocking REQ-02 acceptance.

---

## Gap-fill round (executed 2026-04-27)

Three of "Top NOT-implemented" gaps addressed; one parent-agent-driven (sub-agents permission-blocked, parent took over).

### 1. `new_expression` → Calls

`Q_CALLS` extended with two new alternates:
- `(new_expression constructor: (identifier) @callee)` — `new Foo(...)`
- `(new_expression constructor: (member_expression …))` — `new ns.Foo(...)`

Member-call alternates ALSO capture `object: (identifier) @ns_obj` to support namespace-import resolution (#3).

### 2. Per-resolver-step confidence

`storage::insert_edge_conf(from, to, kind, confidence)` added; `insert_edge` delegates with default 1.0. `graph_build::resolve_with_conf` returns `Option<(i64, f64)>`:
- Step 1 same-file → **1.0**
- Step 2 import-file → **0.9**
- Step 3 global-unique fallback → **0.7**

Calls/Implements/Extends inserts route through `insert_edge_conf`. Imports stay 1.0 (deterministic post-resolution).

ARCHITECTURE §9.7 `confidence_min=0.5` BFS filter is now meaningful (was no-op when all = 1.0).

### 3. Namespace-import handler

Previously `import * as X from "..."` was skipped. Now populates `namespace_aliases: HashMap<(file, alias), target_file>`. Calls site `X.foo()` with `X` matching alias → resolve `foo` directly in `target_file` at confidence 0.9, bypass global-unique fallback. No Imports edge for namespace-only imports.

### Result

| Edge kind | Baseline | Gap-fill | Δ |
|-----------|----------|----------|---|
| Calls | 749 | **795** | **+46** |
| Imports | 120 | 120 | 0 |
| Implements | 7 | 7 | 0 |
| Extends | 1 | 1 | 0 |
| **TOTAL** | 877 | **923** | **+46** |

Build wall 9.6s (≈ baseline 8.9s). 1040 Calls captured pre-UNIQUE-dedup, 795 stored — member-with-object + member-without-object alternates overlap, `UNIQUE(from_id, to_id, kind)` dedupes (kept first-insert confidence).

The +46 lift = constructor calls. Spot-check: `new ObsidianAdapter()` from `index.ts` now produces a Calls edge.

### Still NOT implemented (P3+)

- **Default imports** (`import X from "..."`) — local binding ≠ target's exported name on step 2; pass_imports docs the gap
- **Barrel-file re-exports** — `export { X } from "./submodule"` not followed; ARCHITECTURE §9.7 `import_alias_resolver` Phase 3 followup
- **CLI dump for confidence distribution** — `Store::count_edges_by_kind_conf` exists; subcommand wire is a 10-LOC followup
- **Confidence-weighted PPR transitions** — `graph_ppr` (sibling commit) uses confidence as hard filter only; weighted transitions are Phase 3+

---

## PPR library landed (sibling commit)

`src/graph_ppr.rs` (~210 LOC) implements Personalized PageRank as Fast-GraphRAG / HippoRAG-style traversal:
- Pure-data `ppr_from_edge_list(edges, entries, damping, iters)` — testable without DB
- DB-wrapping `personalized_pagerank(storage, entry_ids, kinds, damping, iters, conf_min)` — calls `Store::edges_of_kinds` then delegates to pure fn
- Dangling-mass redistribution to teleport (preserves Σ=1)
- 4 unit tests pass (synthetic 5-node, empty entries, isolated entry, dangling mass conservation)

NOT wired into a CLI subcommand yet — Phase 3 wires `--axis-3` query mode that fans entry symbols into PPR + returns top-N. Library-first approach lets parent decide CLI surface separately.
