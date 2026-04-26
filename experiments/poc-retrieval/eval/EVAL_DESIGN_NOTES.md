# eval/EVAL_DESIGN_NOTES.md

Engineering-asset companion to round_*_results.md. Captures rules and constraints for designing future eval query sets and ground-truth annotations. Read this **before** extending `queries.json` or starting a new spike.

## Ground Truth Design Rules

These rules emerge from Round 1-4 bugs. Violating any of them produces measurement noise that masquerades as retrieval-quality signal.

### Rule 1 — Subject exclusion (axis 3 self-match bug, R1)

`expected_paths` for a relational query (axis 3: "who calls X", "what does X return", "X implements which interface") **MUST NOT** include the queried symbol `X` itself. If POC's retrieval returns `X` as top-1, it found the subject not the answer — must score 0, not 1.

**Counter-example from R1**: C8 "what runs after MemUAdapter sync completes" had `expected_paths=["MemUAdapter", "sync", ...]`. POC top-1 = `MemUAdapter` (the subject) → eval scored 1.0 falsely. C5/C6/C8/C10 all had this bug. R2 fixed by stripping subjects from expected_paths; eval dropped from inflated 35% to honest 30%.

### Rule 2 — Negative semantics (fuzzy-negative bug, R4)

Mark a query `negative: true` only if the **concept is architecturally absent** from the corpus, not merely if the **exact symbol name** is missing.

| Query | Naive author intent | Actual semantics | Correct flag |
|-------|---------------------|------------------|--------------|
| `parseYAMLFrontmatter` | name absent → negative | `parseFrontmatter` exists, parses YAML frontmatter, **conceptually present** | `concept_absent: false` (not negative) |
| `OAuth2Provider` | class absent → negative | OAuth-token handling in gmail/feishu collectors, **conceptually present** | `concept_absent: false` |
| `rate limiting middleware` | absent | no rate-limit logic anywhere in corpus | `concept_absent: true` (true negative) |

**Reranker correctly scores fuzzy-negatives at 0.3-0.7** (semantic neighbors exist) — flagging them `negative: true` makes eval penalize correct behavior. R4 lost ~5pp on axis 1 to this category.

### Rule 3 — File-level tolerance for `expected_paths` (R3-bias bug, R4)

`expected_paths` should be **file-level OR concept-level**, not pinned to a specific symbol name that any one round happened to pick.

**Counter-example from R4**: B1 "filesystem fallback when obsidian not running" had `expected_paths=["FilesystemAdapter","ObsidianAdapter","fallback","filesystem"]`. R3 RRF picked `obsidianAdapter` (const) — matched via name. R4 reranker picked `obsidian.ts:write` (method) — a different but equally valid answer. Eval scored R3=1.0, R4=0.0. **The author's bias toward the round being authored against shows as fake regression.**

Rewrite expected_paths to be both:
- File-level: `["adapters/obsidian", "adapters/filesystem"]` (any symbol in those files counts)
- AND concept-level: known related symbol names

So either path-substring OR symbol-name-contains qualifies a hit. Already implemented in matcher; the **discipline goes in expected_paths authorship**.

### Rule 4 — Snippet length must reflect production usage

The `snippet` shipped to embedder/reranker should match **what the system will use at query time in production**. Truncating snippets in eval to game scores is overfitting.

**Counter-example from R4 Cause A consideration**: tempting to truncate snippet to 150 char for reranker input to suppress verbose-test-file dominance. But that changes what the cross-encoder sees relative to how Phase 3 MVP will deploy. Either truncate everywhere (re-measure) or filter test files at indexing time (corpus-side fix). Don't truncate just-for-rerank-input.

### Rule 5 — Path separator, casing, normalization (R4 latent bug)

Eval matcher MUST normalize:
- Backslash → forward slash (Windows path artifacts)
- Lowercase both sides (camelCase identifier matching)
- Trim whitespace

A single character mismatch silently scores 0. Discovered in R4 when `mcp-server\src\adapters\obsidian.test.ts` failed to match expected `adapters/obsidian` — the backslash-vs-slash gap had been hidden in earlier rounds because R3's top-1 had a name match that bypassed the path check.

## Known Eval Limitations

### Single-truth ground truth cannot measure reranker lift

A 30-query × hand-annotated `expected_paths` eval captures **alignment with the annotator's bias**, not objective retrieval quality. When a reranker finds an alternative-but-valid answer (R4 Cause C pattern), eval will report a regression even when retrieval improved.

**Implication**: do not introduce cross-encoder reranking in Phase 3 MVP without first deploying graded-relevance evaluation (NDCG@5 with 0-3 ratings via LLM-judge or human annotators). See ARCHITECTURE.md §9 Phase 3 Gate.

### Cross-encoder verbose-bias (R4 Cause A)

Cross-encoders reward `query` × `document` surface elaboration. A long doc-string-heavy `renderDoc` symbol can outrank the actual `register` const because the snippet describes registration extensively. This is a known ranker behavior, not a bug — but eval design must account for it. Mitigations (file filter, snippet pre-processing) are corpus-specific and require validation per corpus.

### Negative threshold scales with fusion math

Empirical thresholds discovered:
- Unweighted RRF (R1, R2): max ≈ 2/61 ≈ 0.033, threshold 0.025 ≈ 75% of max
- Alpha-weighted RRF (R3): max ≈ 1/61 ≈ 0.0164, threshold 0.012 ≈ 73% of max
- Cross-encoder rerank (R4): max ≈ 1.0, threshold 0.30 (still imperfect for fuzzy negatives — see Rule 2)

If fusion math changes, recheck threshold. Do not hardcode — derive from observed RRF distribution.

## Future Query Set Expansion Rules

When growing beyond 30 queries:

1. **Hold out 20% of queries** as a test set never used during retrieval-config tuning. Avoid overfitting alpha/threshold/weights to the dev set.

2. **Per-axis balance**: keep symbol-exact / semantic-NL / call-relation roughly even. The 1:1:1 ratio in current set is a deliberate ablation tool — preserve it.

3. **Concept-absent queries should be ≥ 20% of each axis** to keep the negative-discrimination signal strong. Currently axis 1 has 2/10 (A9, A10), axis 2 has 1/10 (B5), axis 3 has 1/10 (C9). Bump to 2/10 minimum per axis.

4. **Annotate a `expected_outcome` field** beyond just `expected_paths`:
   - `find_one_canonical` — there is one right answer (axis 1 typical)
   - `find_any_in_set` — multiple symbols are equally valid (axis 2 typical)
   - `find_all_in_set` — must return all members (rare)
   - `concept_absent` — must score below threshold
   - `architecturally_unanswerable` — no retrieval can answer (e.g. cross-language) — exclude from headline metric

5. **Cross-corpus validation**: every release of CodeNexus should run the same 30+ query template (with corpus-specific symbol names swapped in) against at least 2 other TypeScript repos. A retrieval improvement that only shows on obsidian-llm-wiki is overfitting.

## What this document is NOT

- Not a replacement for round_*_results.md — those capture per-round empirical findings; this captures invariant rules.
- Not a Phase 1 ARCHITECTURE.md substitute — architecture decisions live there; eval design rules live here.
- Not a frozen contract — when new bugs surface, this file updates. Each rule traces to a Round number.

## Provenance

- Rules 1-5 derived from `eval/baseline_v1_results.md`, `round_2_results.md`, `round_3_results.md`, `round_4_results.md`
- Phase 3 LLM-judge requirement promoted to ARCHITECTURE.md §9 Phase 3 Gate (Round 4 finding)
- This file is consumed by future spikes / query-set extensions / reviewers
