# 04-08 Spike Notes — Phase 4 Group 2 Multi-Language Tree-Sitter

**Date:** 2026-04-28
**Spike scope:** TypeScript + Python (1 new language) end-to-end through `parse_repo`
**Result:** PASS — architecture extends cleanly; SPEC for full group 2 can now be written from concrete evidence
**Commits:** `84f1e97` (API migration) + `c9d31a5` (multi-language spike)

## Spike acceptance bars (locked pre-run)

| Bar | Status | Notes |
|-----|--------|-------|
| `tree-sitter-python = "0.25"` dep added | PASS | Cargo.lock +11 |
| parser.rs has `Language` enum + `detect_language` | PASS | enum: Typescript, Python |
| Python `QUERY_SRC` covers def / class / async def | PASS | 2-line query catches all 3 + nested methods |
| ≥1 unit test parses Python fixture, asserts ≥3 symbols | PASS | delivered 2 tests; first asserts ≥3, actual=5 |
| `cargo test` 14+ PASS (no regression) | PASS | 16/16 in 55.82s under `--test-threads=1` |
| TS query unchanged | PASS | server test still passes; REQ-10 retrieval untouched |

## Architecture lessons — what extended cleanly

### `LangCtx` struct is the right abstraction

Bundles `tree_sitter::Language` + compiled `Query` + capture indices (`name_idx`, `body_idx`). Compiled **once** at the top of `parse_repo`, used **N times** in the file loop. Pre-spike code recompiled the query inside the call, which would scale O(files × languages) instead of O(languages).

```rust
struct LangCtx { lang, query, name_idx, body_idx }
let ts_ctx = LangCtx::new(LANGUAGE_TYPESCRIPT.into(), QUERY_SRC_TS)?;
let py_ctx = LangCtx::new(LANGUAGE.into(), QUERY_SRC_PY)?;
```

### `detect_language` from file extension is sufficient (for now)

Spike used a flat extension match: `.ts/.tsx → Typescript`, `.py → Python`. Plus an ignore-dir filter unified across languages (`node_modules`, `.git`, `dist`, `build`, `__pycache__`, `.venv`, `venv`). This collapsed the pre-spike `is_ts_file` into a per-language dispatcher with **zero** new code-organization scaffolding.

**Open for SPEC:** when group 2 expands to Go/Rust/Java/C++, do we still want a flat match, or a per-language `Language::extensions(&self) -> &[&str]` method? The flat match scales fine until ~10 languages; past that, a method is more maintainable.

### Adding language #3 = ~10 lines

Concrete recipe (per spike-evidence, not estimation):

1. `cargo add tree-sitter-<lang>` (Cargo.toml +1, Cargo.lock +N)
2. `Language::<Lang>` variant in enum (parser.rs +1)
3. New `QUERY_SRC_<LANG>` const (parser.rs +1 block, ~5-15 lines depending on grammar surface)
4. `detect_language` match arm for the new extensions (parser.rs +1)
5. `LangCtx::new` line in `parse_repo` + dispatch match arm (parser.rs +2-3)
6. Optional: 1 unit test fixture + assertions (parser.rs +20-30 lines)

**Net:** ~10 lines of code + 1 test, per language. This is the spike's most important finding — the **architectural cost is bounded**.

## Architecture lessons — what's still open

### Per-grammar query maintenance burden

Each language needs its own Query string with grammar-specific node names:
- TS: `function_declaration`, `class_declaration`, `interface_declaration`, ...
- Python: `function_definition`, `class_definition`
- Go: `function_declaration`, `method_declaration`, `type_declaration`, ...
- Rust: `function_item`, `struct_item`, `impl_item`, `trait_item`, ...

These are not auto-derivable. Every grammar requires reading the tree-sitter grammar `grammar.js` to know the right node names. **Group 2 SPEC must commit to a discovery-and-validation process for each new language** — not just "ship 5 languages tomorrow".

### Symbol kind taxonomy is not unified

Currently `Symbol.kind` is the raw `bn.kind()` string from tree-sitter — e.g. `"function_declaration"` for TS, `"function_definition"` for Python. Downstream consumers (search, BM25 ranking, list_callers) currently treat `kind` as opaque, but A2A clients calling `get_symbol` see the raw value.

**Open question for SPEC:** should we normalize to `{Function, Class, Method, Variable, Type, ...}` enum at the parser layer? Or keep raw and document the cross-language node-name table? The spike doesn't answer this — production usage signal needed.

### REQ-02 graph edges are TypeScript-only

`graph_build.rs` (Calls / Imports / Implements / Extends) is hardcoded to TS. Spike intentionally does NOT extend this — it's a separate slice of work per language (Python imports use different syntax, Python doesn't have `interface`, etc.).

**Group 2 SPEC scope decision needed:** does group 2 ship **multi-language symbol extraction only**, or does it also extend graph_build per language? Recommendation from spike experience: **symbols only first** (Phase 4 group 2), edges per language is Phase 4 group 2.5 or its own slice.

### REQ-10 retrieval evaluation set is TS-only

The B1-B7 query set (REQ-10 acceptance) all target TypeScript symbols in the obsidian-llm-wiki corpus. Adding Python language doesn't move the gate, but it doesn't *prove* Python retrieval works either. **Cross-language eval set is open work** — separate spike candidate.

## Pitfalls hit during spike (record for reuse)

### Windows %TEMP% lives under hidden AppData

`std::env::temp_dir()` on Windows returns `C:\Users\<user>\AppData\Local\Temp\...`. `AppData` has the **HIDDEN file attribute** (NTFS attribute, not name-prefix). `ignore::WalkBuilder`'s `hidden(true)` default then **filters the entire subtree** — so any test that uses `temp_dir()` as a fixture root for `parse_repo` yields 0 symbols and a confusing failure mode (no error, just empty result).

**Workaround applied:** parser tests now use `target/test-tmp/{uid}/` instead. `target/` is gitignored, not hidden, and lives at the cargo project root which is the natural scratch space. Documented in the test comments.

**Pre-existing test that didn't trip on this:** `server::tests::index_repo_empty_repo_preserves_existing_data` survived because its assertion expected 0 symbols regardless (deferred-clear invariant doesn't depend on parse_repo finding anything). Coincidence, not design.

### tree-sitter API churn 0.22 → 0.25

Step A of this slice (commit `84f1e97`) handled the migration:
- `language_typescript()` (function) → `LANGUAGE_TYPESCRIPT` (const, `LanguageFn` ABI), need `.into()` for `tree_sitter::Language`
- `cursor.matches(...)` → `StreamingIterator` not `Iterator`; rewrite all 5 sites to `let mut matches = ...; while let Some(m) = matches.next()`
- Added `streaming-iterator = "0.1"` dep, `use streaming_iterator::StreamingIterator;` in parser.rs and graph_build.rs

This was prerequisite work, not "spike scope" — but if group 2 is shipped without first migrating, it would have been one big commit and harder to bisect.

## What the SPEC should lock

When promoting this spike to Phase 4 group 2 SPEC, the following are **already evidence-backed** and can be written as locked decisions, not open questions:

1. `LangCtx` struct as the per-language extraction primitive (lock, not "consider").
2. `detect_language(&Path) -> Option<Language>` from file extension as the dispatch primitive.
3. `parse_repo` retains its current shape; per-language work is parameter substitution, not control-flow restructure.
4. `target/test-tmp/{uid}/` as the parser test fixture root (Windows-safe).
5. Adding a new language is a ~10-line PR; SPEC should NOT attempt to scope all 5 languages in one execute slice — better as 5 sequential ~1hr slices, each with its own test fixture + smoke.

## What the SPEC should still discuss

1. **Language priority order** — given spike-001 is TS-heavy and FSC eval has Python signal, the order is probably `Python (done) → Go → Rust → Java/C++ (later)`. But this is a product call, not a spike call.
2. **Symbol kind normalization** — flat raw-node-name vs unified enum. Affects API stability for A2A consumers.
3. **graph_build per language scope** — group 2 vs group 2.5 vs separate phase.
4. **Cross-language eval set** — needed before claiming "multi-language retrieval works", not just "multi-language extraction works".
5. **Failure mode when grammar crate ABI drifts** — tree-sitter 0.22 → 0.25 broke 7 sites; group 2 is going to keep adding crates, increasing future ABI-drift surface. Mitigation: pin `tree-sitter`, `tree-sitter-typescript`, `tree-sitter-python` to exact versions in Cargo.toml? Worth discussing.

## Next step (recommended)

`/gsd-spec-phase` (or inline-write per bypass convention) for Phase 4 group 2, using THIS document as the input rather than a blank template. The 5 locked decisions above + 5 discussion items above are the SPEC's skeleton.

If full group 2 ships in fresh sessions, a reasonable sub-slice cadence is:
- 04-09: SPEC discuss + plan (no code)
- 04-10: Go grammar
- 04-11: Rust grammar
- 04-12: Java/C++ grammar (optional, scope-dependent)
- 04-13: graph_build per language (separate slice)
- 04-14: Cross-language eval set + benchmark refresh

This spike (04-08) closes the entry. Not a complete group 2.
