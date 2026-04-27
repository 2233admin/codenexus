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

### Rule 7 — N/A handling on cross-corpus blind eval: generous denominator, locked before run (Phase 3.5 finding)

When authoring a blind NL query set against a fresh corpus (e.g. F1-F10 against full-self-coding), some queries will turn out to address concepts that the indexed corpus subset does not contain (F2 worker-pool scheduling, F7 retry logic, F10 parallel merge — none in FSC's 5-file daemon subset). These are **architecturally unanswerable for this corpus**, not retrieval failures.

**Headline metric uses the generous denominator: `score / valid_queries`** (N/A excluded), not the strict `score / total_queries`. Rationale: strict scoring penalizes the choice of corpus, not the quality of retrieval. Reporting both is fine; the headline number must be the generous one.

**Discipline (must hold to keep this honest):**
- The `expected_outcome: architecturally_unanswerable` tag (Rule 5 candidate from "Future Query Set Expansion Rules" section, point 4) **MUST be locked before any retrieval results are inspected for that query**. Authoring the query, indexing the corpus, then deciding which queries to mark N/A based on the numbers is motivated reasoning — drop it.
- Acceptable workflow: write all queries → index corpus → glance at corpus structure (file list, top-level symbol counts) to mark unanswerable queries → freeze flags in queries.json with a commit → THEN run retrieval and score.
- Forbidden workflow: write queries → run retrieval → look at scores → decide which 0% scores were "actually N/A".

**Provenance & forward-locking note**: This rule was authored 2026-04-27 *after* the F1-F10 first run (Phase 3.5 sub-check 3) completed with 5/10 strict and 5/7 generous. The 7-valid count (F2/F7/F10 marked N/A) was determined by the eval author after seeing the scores, which technically violates the discipline above. The decision to use generous as headline was reaffirmed by Curry post-hoc on architectural grounds (corpus subset choice, not retrieval failure), and locked here as the standard for **all future cross-corpus runs starting with the post-Phase-3.5b full-FSC re-eval**. The first F1-F10 run remains valid as a directional signal but its 5/7=71.4% number should be re-derived under this rule against the full 2307-symbol index, not the 127-symbol partial.

### Rule 6 — LLM-judge graded 0-3 is primary metric, hand-annotation is cross-check (R5 finding)

After R5 spike (3-run LLM-judge over R4 retrieval output via okaoi MiniMax-M2.7 pool, see `round_5_results.md`), the eval primary metric is:

- **Primary**: LLM-judge graded 0-3 (NIST TREC scale), with mean (κ@t2, Spearman ρ, std) reported over **N≥3 stochastic seeds**
- **Cross-check**: hand-annotated `expected_paths` kept for cold-start, audit, and reproducibility — not for headline numbers

**Why graded > binary**: R5 found arm A (binary 0/1) and arm B@t2 (graded thresholded ≥2) tied within noise on Cohen κ (Δ = +0.069 ± 0.120 across 3 runs). The load-bearing win for graded is **Spearman ρ = 0.38 ± 0.07** — graded scores carry monotone rank information binary discards. Reranker config tuning needs rank signal, not hit/miss. Graded also catches Rule-2 fuzzy-negative cases consistently (A9 `parseYAMLFrontmatter` across all 3 runs).

**External backing** (Arize AI binary-vs-score study, 2026): "Binary judgments outperform direct numeric scoring on stability. Multi-categorical rubrics reduce variance while preserving more signal than binary." 0-3 graded sits in their "multi-categorical" sweet spot — more granularity than binary, more stability than free-form numeric.

**Reporting discipline**:
- Always report mean ± std over N≥3 seeds (LLM stochasticity at temp=0.0 still ±0.07 single-run κ noise)
- Report both κ@t2 (binary alignment) and Spearman ρ (rank correlation) — they answer different questions
- **Exclude axis-3 call-relation queries from headline metric** until static-analysis-augmented eval lands (REQ-02 CALLS edge graph, Phase 2/3) — both binary and graded LLM judges are blind to data-flow / call-graph relationships

**Raw judgment retention**: per-run `round_N_results.json` + `round_N_summary*.json` are committed (not gitignored) starting from R5 — they accumulate as future training data for a fine-tuned code-retrieval judge (Phase 4+ candidate, Prometheus / JudgeLM route).

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
