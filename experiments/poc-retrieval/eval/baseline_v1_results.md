# POC Retrieval Round 1 Baseline (frozen)

**Run date:** 2026-04-27
**Corpus:** D:/projects/obsidian-llm-wiki (TS only, 474 symbols indexed)
**Embedder:** ollama qwen3-embedding:0.6b (1024d), no Instruct prefix
**BM25:** SQLite FTS5 default unicode61 tokenizer, equal column weights
**Cosine:** brute-force in Rust
**Fusion:** RRF c=60, top-50 each side

## Headline numbers (apparent)

| Axis | Apparent | Real (after diagnosis) |
|------|----------|------------------------|
| 1 — symbol exact   | 50.0% | **30%** (3 true wins, 2 lucky negatives) |
| 2 — semantic NL    | 2.5%  | **2.5%** (genuine collapse, below GitNexus baseline 43%) |
| 3 — call relation  | 35.0% | **≈0%** (inflated by self-match scoring bug) |
| Overall            | 29.2% | **≈10%** (recalculated honestly) |

## Per-query verdicts

### Axis 1 — symbol exact

| ID | Query | Score | Verdict |
|----|-------|-------|---------|
| A1 | ObsidianAdapter | 1.0 | **TRUE WIN** — top-1 hit class definition |
| A2 | VaultBrainAdapter | 1.0 | **TRUE WIN** — top-1 |
| A3 | assertRealPathInsideVault | 1.0 | **TRUE WIN** — top-1+top-2, both files |
| A4 | PROTECTED_DIRS | 0 | **PARSER GAP**: parser misses top-level `const FOO = ...` declarations |
| A5 | concept_graph run function | 0 | architectural — Python file, POC TS-only |
| A6 | validatedArgs | 0 | **PARSER GAP**: `value: (arrow_function)` constraint excluded `const x = expr` (where expr isn't an arrow fn) |
| A7 | kb_meta | 0 | architectural — Python |
| A8 | Node tags field | 0 | **PARSER GAP**: `type Node = {...}` aliases not captured |
| A9 | parseYAMLFrontmatter (NEG) | 1.0 | **LUCKY** — top-1 RRF 0.0164 < 0.025 threshold; top-1 was `parseFrontmatter` (high-similarity neighbor), threshold barely caught it |
| A10 | OAuth2Provider (NEG) | 1.0 | **LUCKY** — same story |

### Axis 2 — semantic NL (collapse)

Top-1 RRF clusters at 0.025-0.033 (theoretical max 0.033) — meaning **BOTH BM25 and vector confidently agreed** on the wrong items. Diagnosis below.

| ID | Query | Score | Top-1 (wrong) | Should have been |
|----|-------|-------|---------------|------------------|
| B1 | filesystem fallback... | 0.5 | obsidian.ts:write (top-3 has ObsidianAdapter) | top-1 ObsidianAdapter |
| B2 | preflight check protected dirs | 0 | core/operations.ts:walk | assertRealPathInsideVault |
| B3 | search files by tag | 0 | pglite-engine.ts:upsertTag | searchByTag (if it exists as fn) |
| B4 | build concept graph | 0 | circleback-collector.ts:buildDigest | concept_graph (Python miss) |
| B5 | rate limiting (NEG) | -0.25 | x-collector.ts:main (RRF 0.0301 > 0.025) | empty / weak |
| B6 | safe deletion dry run | 0 | obsidian.test.ts:cleanPortFile | dryRun handling |
| B7 | register MCP tool handler | 0 | memu-query.ts:main; `register` is at top-5 (capped at top-3) | register |
| B8 | concurrent writes | 0 | obsidian.ts:write | locking/queue logic |
| B9 | conflicting edits | 0 | makeRegistry | conflict detection |
| B10 | aggregate metadata | 0 | buildDigest | kb_meta / aggregate |

## Diagnosis

### Why axis 2 collapsed (3 layers)

**Layer 1 — Embedder prompt format**: qwen3-embedding-0.6b is instruction-tuned. Spec requires:
- query side: `Instruct: <task>\nQuery: <text>`
- document side: raw text (or matching prefix per model card)

POC sent raw text both sides. Query embeddings are out-of-distribution → vector signal is essentially random noise within the candidate set, but RRF still ranks the top-50 → produces confidently-wrong top-1.

**Layer 2 — BM25 tokenization (Round 2 review checkpoint)**: SQLite FTS5 default `unicode61` tokenizer does **NOT** split camelCase or snake_case. `walkSubtree` is one token. `PROTECTED_DIRS` is one token. This means:
- Query "protected directories" never matches `PROTECTED_DIRS` because BM25 query tokens are `protected` + `directories` (lowercase), indexed token is `PROTECTED_DIRS` (uppercase, underscored, single-token).
- Query "directories" matches `walkSubtree`'s snippet (which contains the word `directory` in code body), not the symbol name.

**Layer 3 — BM25 column weights (Round 2 review checkpoint)**: Current `bm25(symbols_fts)` uses default equal weights across `name`, `snippet`, `kind`. Snippet 500-char dominates BM25 score by sheer length. Name should be 10x weighted to ensure symbol-name matches dominate.

### Why axis 3 was inflated to 35%

Eval design bug: `expected_paths` for several axis-3 queries included the **queried subject** itself.

| ID | Wrong expected_paths | What POC matched |
|----|---------------------|------------------|
| C5 | included `concept_graph` (subject) | `compile-trigger.ts:run` (path-substring of "compile") |
| C6 | included `kb_meta` (subject) | nothing real |
| C8 | included `MemUAdapter` (subject) | top-1 = MemUAdapter (the subject itself) |
| C10 | included `GitNexusAdapter` (subject) | top-1 = GitNexusAdapter (the subject itself) |

Real axis 3 ≈ 0% — POC retrieval cannot answer "who calls X" / "what does X call" without a CALLS edge graph, which is exactly REQ-02. **This is the predicted ablation evidence**.

Plus matcher was case-sensitive — `extractTags` did not match expected token `tags` because `Tags` ≠ `tags`. Fix: `to_lowercase()` both sides.

## Fixes for Round 2

| # | Fix | Layer | LOC |
|---|-----|-------|-----|
| ① | Embedder Instruct prefix for queries | embedder.rs | ~25 |
| ② | Parser const + type-alias + enum (drop arrow_function constraint) | parser.rs | ~10 |
| ③ | Eval matcher case-insensitive + remove subjects from C5/C6/C8/C10 expected_paths | main.rs + queries.json | ~15 |

REQ-01 in PROJECT.md gets sync'd to add Const/TypeAlias/Enum to the SymbolNode kind list.

## Round 2 review checklist (DO NOT SKIP)

After Round 2 numbers come in, BEFORE concluding, audit:

1. **If axis 2 < 50% even after Instruct prefix**: BM25 tokenization is the next suspect.
   - Check: how does FTS5 tokenize `walkSubtree`? Run `SELECT * FROM symbols_fts('walkSubtree');` to see token breakdown.
   - Fix path: switch FTS5 to `tokenize='trigram'` or write a custom tokenizer that splits camelCase/snake_case.

2. **If BM25 still pulls wrong-signal-strong on axis 2**: BM25 column weights.
   - Check: top-5 of B2 — is `assertRealPathInsideVault` ranked higher than `walk` after embedding fix? If still no, name vs snippet weight needs tuning.
   - Fix path: `bm25(symbols_fts, 10.0, 1.0, 1.0)` — name 10x, snippet 1x, kind 1x.

3. **If axis 1 misses A4/A6**: parser fix didn't capture what we expected. Verify parser query is matching at all nesting depths — `lexical_declaration` inside class bodies / arrow function bodies might need explicit anchor.

4. **If axis 3 still > 10%**: matcher's `expected_paths` substring check too lenient — narrow to exact-symbol-name match for axis 3.

5. **Negative threshold 0.025**: under new symbol count (~3-5x post-fix-②), RRF distribution will shift. Recheck whether 0.025 is still the right cut.

## Provenance

- Smoke pass commit: `27b75a6` (codenexus repo)
- This baseline frozen against `eval/queries.json` Round-1 version (pre-fix)
- Round 2 will write `eval/round_2_results.md` for direct comparison
