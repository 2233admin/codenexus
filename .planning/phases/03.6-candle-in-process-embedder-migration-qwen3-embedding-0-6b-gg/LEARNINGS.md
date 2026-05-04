---
phase: "03.6"
phase_name: "candle-in-process-embedder-migration-qwen3-embedding-0-6b-gg"
extracted_at: "2026-05-04"
extracted_by: "gsd-extract_learnings (manual workflow execution)"
sources_read:
  - "03.6-SUMMARY.md"
  - "03.6-RESEARCH.md (header only)"
  - "VERIFICATION.md"
  - "03.6-01-loader-and-equivalence-snapshot-PLAN.md (referenced)"
  - "03.6-02-cross-corpus-eval-and-closure-PLAN.md (referenced)"
  - ".planning/STATE.md"
missing_artifacts: ["UAT.md"]
---

# Phase 03.6 Learnings -- Candle in-process embedder migration

Status at close: COMPLETE 2026-04-28. All 4 hard gates PASS. ollama HTTP dependency removed; fsc.db FULL 2307-symbol index achievable in 8m22s wall-clock (vs 90min ceiling).

## Decisions

1. **fastembed-rs over hand-written candle-transformers loader** -- Plan 1 path A locked. Rationale: full surface needed (dim=1024, F32, custom prefix injection via `query_text`, last-token pool + L2 normalize) all already exposed by `fastembed::Qwen3TextEmbedding`. HF Hub auto-download via `hf-hub` 0.5 (transitive dep) handled the safetensors fetch. Net code delta: ~80 LOC vs multi-day model-loader-from-scratch the Phase 4 plan had estimated. candle-transformers held in reserve as 1-import-flip fallback if equivalence check failed.
   - Source: `03.6-SUMMARY.md` "Branch shipped (Plan 1)" / Plan 1 locked-decision #3

2. **F32 weights for the equivalence milestone, F16 deferred** -- Plan 1 locked-decision #4. Rationale: F16 introduces an orthogonal source of cosine drift (precision drift); if both model-impl drift AND precision drift shift together, debugging becomes intractable. F16 revisit is Phase 4+ measured optimization with its own equivalence regression set, never a silent flip.
   - Source: `03.6-SUMMARY.md` honest gap list [P3]

3. **Source-of-truth Rust binary for version_hash, NOT bash sha256sum** -- Plan 1 Task 2.5 chose `experiments/poc-retrieval/src/bin/compute_version_hash.rs` over `echo -n | sha256sum`. Reason: POSIX/git-bash `echo -n "\nQuery: "` emits literal backslash-n bytes (0x5C 0x6E), not LF (0x0A); a bash-computed hash would silently mismatch the Rust runtime hash forever. Rust binary imports `QUERY_INSTRUCT` directly from `embedder.rs` so prefix changes auto-propagate on rebuild.
   - Source: `03.6-SUMMARY.md` "Source-of-truth tooling"

4. **Forensic backups preserved BEFORE migration** -- `poc.db.ollama.bak`, `fsc.db.ollama.bak`, `embed_snapshot_ollama.json` (60 entries x 1024 dim) all captured pre-cut. Backups gitignored on disk per Plan 1 Task 1.4 protocol. Provides rollback path + post-hoc audit material.
   - Source: `03.6-SUMMARY.md` "Forensic backups"

5. **Lock N/A flags BEFORE retrieval runs (anti-laundering)** -- F1-F10 hand-eval N/A flags locked at `2026-04-27T14:55:28Z`; retrieval started `2026-04-27T14:56:20Z` (~52s after lock). Verifier independently recomputed `generous_p_at_5` from per-query judges and confirmed byte-identical to reported value (|diff|=0 < 1e-9). The most laundering-prone gate had its denominator frozen before any data flowed.
   - Source: `VERIFICATION.md` G_f110, `03.6-SUMMARY.md` "F1-F10 note"

## Lessons

1. **Cheap probe -> locked decision is high-ROI** -- The Phase 3.5b 4-hour retry+fail-loud micro-slice (commit `8f4da66`) generated the hard evidence (`20min wall-clock retry budget x 0 recovery`, deterministic fail at i=128 vs i=127 in two consecutive runs delta=1) that elevated subjective "ollama feels unstable" to a fundable migration decision. 4 hours of probe vs ~3 days of migration = ~18x compression of decision-lock latency. Now canonical for feedback rule 36.
   - Source: `03.6-SUMMARY.md` closing paragraph

2. **Plan-stub-vs-real-API drift is a real planning hygiene gap** -- RESEARCH.md anticipated hand-writing `Qwen3Model::forward_with_pool()`. Reality: fastembed-rs already wrapped this. Speculative API stubs from training-data memory miss real surfaces. Future phase plans should pin actual API signatures via `cargo doc` or source spelunking before locking implementation strategies.
   - Source: `03.6-SUMMARY.md` honest gap list [P2]

3. **Cosine equivalence is a green herring without downstream precision check** -- The 30-query equivalence set hit mean=0.9994 / p10=0.9993, hugging 1.0. But REQ-10 B1-B7 ALSO had to come in byte-identical to ollama baseline (67.9%, +0.0pp delta) for the prefix-preservation discipline to be validated. Cosine alone could pass while precision silently drifts; only the dual gate caught it.
   - Source: `03.6-SUMMARY.md` "Equivalence note"

4. **CPU throughput is sufficient for current corpus, cuda deferred** -- 4.6 sym/sec (8m22s for 2307 symbols) sits inside the 5-15 sym/sec budget per RESEARCH §Pitfall 6. `--features cuda` deferred to AU 5090 host rotation -- premature optimization on this Win11 host.
   - Source: `03.6-SUMMARY.md` honest gap list [P3]

5. **Phase closure is a doc-rewrite event, not just a code commit** -- ARCH §9.10 (rename "Phase 4 known unknown" to "Phase 03.6 LANDED" + delete GGUF cheap-path enumeration + add Negative Rationale block), ARCH §9.8 (history row for active hash `f2b47aa16b17`), STATE.md (status flip + decision log), PROJECT.md (P0 candle entry strikethrough w/ closure marker), SUMMARY.md (this artifact). All five docs touched in the same closure batch.
   - Source: `03.6-SUMMARY.md` "Doc updates"

## Patterns

1. **Cheap probe -> locked decision** -- 4h micro-slice with explicit fail-mode generates structured negative result; multi-day migration becomes fundable with locked numbers instead of vibes.

2. **Forensic backup before destructive cut** -- snapshot JSON of pre-state + .bak files of databases preserved before any code path changes hands. Rollback + audit always possible.

3. **Single source of truth via compiled binary, not shell glue** -- when the data flowing through your tool depends on string semantics that differ across shells (POSIX/git-bash/Windows), compute the canonical hash from the same compiled artifact that runs at runtime. Bash escape rules drift silently.

4. **Anti-laundering: lock denominators before data flows** -- Rule 7 enforced via timestamp ordering (`lock_at < retrieval_started_at`) and verifier recomputes the metric from per-query judges to floor-tolerance. No "we noticed query X was N/A so excluded it post-hoc."

5. **Phase closure as multi-doc transaction** -- ARCH/STATE/PROJECT/SUMMARY/this-LEARNINGS all updated in the closure batch; partial closures (code shipped, docs deferred) lose institutional knowledge.

6. **Hard gate + informational band split** -- REQ-10 has a literal 60% floor (PASS/FAIL gate) AND a +/-5pp informational band against Phase 3 baseline (62.9%-72.9%). The literal floor is the binary contract; the informational band catches drift even when the literal floor passes. Useful for gates where "didn't regress" is as important as "above floor."

## Surprises

1. **fastembed-rs already wraps candle** -- the "candle migration" turned into "fastembed adoption" with candle as the underlying engine + fallback path. RESEARCH.md anticipated multi-day hand-written loader; reality: ~80 LOC delta. Spike-time discovery beat speculative planning.
   - Source: `03.6-SUMMARY.md` "Branch shipped (Plan 1)"

2. **Cosine equivalence came in 0.9994/0.9993 -- much tighter than the 0.97/0.95 thresholds** -- expected meaningful drift between ollama-served qwen3 and fastembed-served qwen3. Reality: hugging 1.0. Made candle-direct fallback path unnecessary.
   - Source: `03.6-SUMMARY.md` hard-gate table

3. **fsc.db FULL reindex 8m22s, 10x under the 90-min ceiling** -- Phase 3.5b's 132/2307 burst-hang ceiling completely vanished. Wall-clock budget was ceiling-tier safety margin, not realistic estimate.
   - Source: `03.6-SUMMARY.md` hard-gate table

4. **POSIX echo -n emits literal "\n" as 0x5C 0x6E in git-bash, not LF** -- silent footgun caught at SoT-design time, not at debug time. Would have produced a runtime/source version-hash mismatch indefinitely.
   - Source: `03.6-SUMMARY.md` "Source-of-truth tooling"

5. **Phase 3.5b probe deterministic fail-point at i=128 vs i=127 in two consecutive runs (delta=1)** -- ollama burst-hang isn't stochastic, it's a near-deterministic resource-cliff. Made the case for migration ironclad rather than "maybe it'll be better."
   - Source: `03.6-SUMMARY.md` "Triggered by"

## Source artifact map

- Plan 1: `03.6-01-loader-and-equivalence-snapshot-PLAN.md` (loader path A/B + cosine equivalence gate >=0.97/>=0.95)
- Plan 2: `03.6-02-cross-corpus-eval-and-closure-PLAN.md` (REQ-10 + F1-F10 + ARCH/PROJECT/STATE rewrites + SUMMARY)
- Research: `03.6-RESEARCH.md` (654 lines, safetensors-pivot rationale, candle source inspection of `quantized_qwen3.rs::ModelWeights::forward()`)
- Summary: `03.6-SUMMARY.md` (this LEARNINGS.md primary source)
- Verification: `VERIFICATION.md` (gsd-verifier 2026-04-27, HEAD `804b7ea`)
- UAT: not produced (this phase was a hard-gated infra migration; gates ARE the UAT)
