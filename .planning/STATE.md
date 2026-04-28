---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Phase 4 first slice 8 of 9 runtime gates PASS + 04-07 cargo test linker fix + IndexRepo deferred-clear unit-test regression guard + 04-08 multi-language spike (TS + Python) landed; full Phase 4 group 2 SPEC pending (04-09)
last_updated: "2026-04-28T20:30:00.000Z"
last_activity: 2026-04-28 -- 04-08 multi-language spike (commits 84f1e97 + c9d31a5 + 3585026): tree-sitter 0.22->0.25 + ts 0.21->0.23 + +tree-sitter-python 0.25 API migration; parser.rs refactor with Language enum + detect_language + LangCtx struct; 2 new parser unit tests (Python fixture + cross-lang TS+PY mixed); 04-08-SPIKE-NOTES.md distills 5 locked decisions + 5 open SPEC questions + Windows AppData hidden-attribute test pitfall. 16/16 cargo test PASS under --test-threads=1 in 55.82s (was 14/14)
progress:
  total_phases: 8
  completed_phases: 1
  total_plans: 2
  completed_plans: 2
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-26)

**Core value:** Top-5 NL search precision ≥ 60% on the spike-001 query set, exposed as an open A2A endpoint that any agent can call. **MET 2026-04-28 — B1-B7 mean=67.9% (literal gate, byte-identical to ollama baseline post-candle migration); cross-corpus FSC F1-F10 generous=72% (≥50% gate PASS).**
**Current focus:** Phase 3 (MVP) **CLOSED 2026-04-28 via Phase 03.6**. Embedder runtime migration ollama → in-process fastembed/candle COMPLETE (all 4 hard gates PASS: cosine equivalence, REQ-10 ≥60%, fsc.db FULL 2307, F1-F10 ≥50%). Next session: Phase 4 Parity entry — multi-language tree-sitter + multi-repo registry + git overlay + CodeFlow port. Phase 4+ backlog (tactical + Software 3.0 strategic) now openable.

## Current Position

Phase: 4 of 6 (Parity) — **first slice 8 of 9 runtime gates PASS 2026-04-28 (Waves 0-3 + 04-04 followup + 04-05 first-run UX workaround + 04-06 IndexRepo non-destructive fix); 1 cluster (E2E 1b/4/5/6) demoted to P3 Linux/macOS smoke**
Plan: 04-00 (Cargo bin rename) + 04-01 v2 (R1 redesign + R2.c + R3) + 04-02 v2 (R4 + R5 + fault injection) + 04-03 (E2E + EVAL_NO_REGRESSION + closure) + 04-04 followup (cache-first fix in `embedder.rs::snapshot_dir` lifts 3 deferred gates to PASS) + 04-05 (preseed-hf-cache.sh + r1c_probe.sh + offline-bootstrap doc + README + PROJECT.md line 98 reframe; lifts R1.c from DEFERRED to PASS via file-level sha256 probe) + 04-06 (server.rs IndexRepo handler deferred-clear fix; preserves pre-existing data when all embeds fail; R4.b probe now non-destructive)
Status: First slice runtime 8 of 9 gates PASS. EVAL_NO_REGRESSION + R1.d offline-mode + R5.b synthetic-fail Query + R4.b synthetic-fail A2A IndexRepo + R1.c reload + FIRSTRUN-UX-PRE-SEED + FIRSTRUN-UX-DOC-LINK + IndexRepo-non-destructive-on-failure all PASS. E2E 1b/4/5/6 cluster demoted to P3 Linux/macOS smoke regression markers (cannot be exercised on Windows where fresh-download path is broken; pre-seed bypasses entirely). Phase 4 group 2 (multi-language tree-sitter) unblocked.

## Next Session Order (post-04-06 reflection, 2026-04-28)

Recommended sequence per Curry's session-end review (2026-04-28T19:00+08:00):

1. ~~**cargo test linker conflict**~~ — **CLOSED 2026-04-28 via 04-07 micro-slice** (commits 6ba3f88 + 42f01f0). RCA: tokenizers default features include `esaxx_fast` -> `esaxx-rs/cpp`; esaxx-rs build.rs hard-codes `static_crt(true)` (`/MT`), colliding with ort's prebuilt `/MD` libs (LNK2038/LNK1319). Fix: `tokenizers = { version = "0.22", default-features = false }` strips esaxx_fast at the only direct-dep source (fastembed already imports tokenizers cleanly). Companion unit test `index_repo_empty_repo_preserves_existing_data` lands as regression guard — empty-repo path exercises deferred-clear without env var collision; mutation test confirmed it catches pre-04-06 destructive entry-clear in 0.11s. **Pre-existing gap (P2, not fixed in 04-07):** parallel `cargo test` races on CODENEXUS_EMBED_FAIL env var between two embedder tests; --test-threads=1 workaround documented in embedder.rs:557-559; future fix = static Mutex or serial_test crate. Non-blocking.
2. **Multi-language tree-sitter** (Phase 4 group 2) — **ENTRY SPIKE CLOSED 2026-04-28 via 04-08** (commits 84f1e97 + c9d31a5 + 3585026). Architecture extends cleanly: TS + Python both work, cross-lang dispatch verified via 2 new parser tests, REQ-10 baseline preserved (TS query unchanged). Spike notes at `.planning/phases/codenexus-04-parity/04-08-SPIKE-NOTES.md` distill 5 locked decisions (LangCtx primitive, file-extension dispatch, target/test-tmp/ Windows-safe fixture path) + 5 open SPEC questions (language priority, symbol kind normalization, graph_build per-lang scope, cross-lang eval set, grammar crate ABI pinning). **Remaining group 2 work** = formal SPEC + per-language sub-slices (04-09 SPEC discuss/plan, 04-10 Go, 04-11 Rust, 04-12 optional Java/C++, 04-13 graph_build per lang, 04-14 cross-lang eval). Each sub-slice is ~10 lines + 1 test per spike-evidence. Recommend opening 04-09 in a fresh session bracket since SPEC discussion benefits from clean context.
3. **Phase 04.1 Graph Clustering and Evolution Layer** — needs plan first (no PLAN.md exists in `codenexus-04.1-graph-clustering-and-evolution-layer/`, only PRE-PLAN-NOTES.md). Do NOT start execution before plan; without plan, scope drift is the primary failure mode. Run `/gsd-plan-phase 04.1` (or inline write per the bypass convention if gsd-sdk init still returns phase_dir=null) as a separate session bracket from #2.
4. **(P3 citizenship)** hf-hub upstream issue filing + Linux/macOS smoke regression + poc.db reindex from `D:/projects/obsidian-llm-wiki` to restore B1-B7 baseline — defer to a dedicated short slice when convenient. Not blocking active work.
Last activity: 2026-04-28 -- 04-04-FOLLOWUP-SUMMARY supersedes 04-03 upstream-bug framing; see `.planning/phases/codenexus-04-parity/04-04-FOLLOWUP-SUMMARY.md` for RCA correction (real cause = `download_with_progress` is always-fetch API not cache-aware, NOT hf-hub upstream bug)

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Foundation Design | 0/1 | — | — |
| 2. Stack Spike | 0/6 | — | — |
| 3. MVP | 0/5 | — | — |

**Recent Trend:**

- Last 5 plans: none yet
- Trend: —

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- 2026-04-26: Architecture pivot — pure Rust → Rust core (axum A2A) + Go service (chi/mcp-go/CLI). IPC = A2A protocol over localhost HTTP.
- 2026-04-26: License — MIT → Apache 2.0.
- 2026-04-26: Naming — Stitch → CodeNexus (locked).
- 2026-04-26: Embedder default — ollama-rs → candle.
- 2026-04-26: UI — pivoted Tauri → axum/Go-served web (option B).
- 2026-04-27: Phase 1 closed. ARCHITECTURE.md §9.1-§9.8 all locked. REQ-02 expanded to 4 edge kinds (Calls + Imports + Implements + Extends, Overrides Phase 3+). LLM-judge graded 0-3 N>=3 seeds primary metric (EVAL Rule 6). Path B Jina reranker NOT adopted (3 LLM-judge methodologies consistent — R5/R6/R6c).
- 2026-04-27: User directive — ship-it pivot. After Phase 2 storage spike, go straight to Phase 3 MVP build (axum A2A endpoint + Go MCP server + fat-binary). No more eval rounds unless explicitly requested.
- 2026-04-27: Phase 2 storage spike SKIPPED — research-conclusive verdict (rusqlite+sqlite-vec) per .planning/research/storage_backend.md; micro-bench recommended but not blocking; ship-it directive prefers building over re-validating.
- 2026-04-27: REQ-06 A2A endpoint ✓ — commit e0727c2; axum 0.7 + tokio 1.40 + 3 new src/ files (a2a, server, task_state); 4 operations (Query/GetSymbol/ListCallers/IndexRepo) working end-to-end; smoke green on existing poc.db (52 files / 2116 symbols / 877+ edges).

  **NOTE for future-you:** REQ-06 is implemented in `experiments/poc-retrieval/` (Phase 3 ship path — see `experiments/poc-retrieval/src/main.rs:71` "Phase 3 MVP entry point" comment). The `core/` directory is a 13-line `println!` placeholder that was never updated post-spike — it is **superseded**, not the build target. Makefile targets (build-core / test-core / fmt / lint / clean) point at `experiments/poc-retrieval/` as of 2026-04-27 (see commit `confidence-uplift`). Phase 4 cleanup: delete `core/` or alias it to `experiments/poc-retrieval/` via cargo workspace, whichever is cleaner.

- 2026-04-27: REQ-10 ✓ — Phase 3 MVP **CLOSED**. eval/req10_alpha06.json captures B1-B7 mean precision_at_5 = 67.9% (alpha=0.6 rerank=false) vs gate 60% (+7.9pp) and GitNexus 1.6.3 baseline 43.6% (+24.3pp). 6 of 7 queries clear (B1/B2/B3/B6/B7 all 100%; B4=0% known Python target POC TS-only miss; B5=-0.25 negative-test false-positive penalty, threshold tuneable in Phase 4). Locked config matches R5/R6 default. Plumbing bugs surfaced in REQ-08 (Makefile cp expects codenexus-core but binary is poc-retrieval; `make` not on Windows git-bash PATH) deferred to follow-up quick task — they do NOT block REQ-10 because Cmd::Eval reads same retrieval engine as A2A endpoint Query handler. Quick task: 260427-j9g.

- 2026-04-27: Phase 3.5 robustness slice (quick task 260427-nz9) — 3 of 4 sub-checks pass, 1 blocked. (1) Joint alpha sweep on 5 alphas {0.4..0.8 step 0.1}: B1-B7 plateau at 67.9% across alpha 0.6/0.7/0.8 (NOT local optimum); B1-B10 v2 joint optimum also at alpha=0.6 (57.5%); A1-A10 axis-1 stable at 70% across alpha 0.4-0.7 then collapses to 35% at 0.8 (BM25 channel materially helps exact-symbol). (2) B10 rubric corrected (expected_paths extended with digest/buildDigest/fetchAllNotes/collector — corpus-grounded, not motivated reasoning); B1-B10 lift +10pp. (3) Cross-corpus FSC eval (10 blind queries against 5/107 partial-indexed files): strict 50.0% / generous 71.4%; F5 IPC query surfaced FSC's git-as-bus coordination mechanism instead of literal IPC — exactly the kind of correct-mismatch agents need (Software 3.0 evidence). (4) BLOCKED: ollama qwen3-embedding:0.6b fails deterministically after ~130 sequential `/api/embeddings` calls (single-call works, sustained-load doesn't); two reindex attempts hit identical 127/2307 ceiling. Phase 3 stays `phase_3_prelim_complete` — local-optimum concern invalidated but cross-corpus only weakly validated on partial corpus. Next: Phase 3.5b unblock ollama (retry+backoff cheapest, candle in-process per ARCH 9.5 cleanest).

- 2026-04-27: Phase 3.5b embedder retry + fail-loud micro-slice (quick task 260427-e7r) — engineering complete, hard-evidenced negative result on retry hypothesis. Built embedder.rs retry wrapper (5 attempts × exponential 250ms-base backoff, total ~7.75s sleep budget per failed symbol BETWEEN attempts) + main.rs Cmd::Index `--max-consecutive-fail` flag (default 5, counter+anyhow::bail). Smoke run on FSC corpus failed cleanly at i=132/2307 with consecutive_fails 5/5. **Critical finding**: real ollama burst-failure mode is per-call 60s reqwest send-timeout (TCP accepts → no response body → giveup at 60s) — meaning per-failed-symbol retry cost is 5 × 60s = 5min, and a 4-symbol fail-cluster burns ~20min wall-clock with zero recovery. Boundary i=128 vs prior i=127 (Δ=1) confirms deterministic failure point. **Decision triggered**: Phase 4 candle in-process migration is required, not optional — retry budget can't compete with sustained worker-thread hang. ARCH §9.9 D-W9 locked layer ownership table (single-call retry @ Embedder, error class @ Embedder, per-error policy + counter + abort @ Caller, anti-pattern: do not move counter into Embedder struct state). PROJECT.md Phase 4 P2 backlog entry expanded with EmbedError enum (`Transient`/`Permanent`/`Timeout`) design hint + counter location rationale. EVAL_DESIGN_NOTES Rule 7 locks generous-denominator + must-lock-before-eval-runs discipline for ALL future cross-corpus runs. Phase 3 stays `phase_3_prelim_complete`; Phase 4 candle migration is the unblock path. Next: `/gsd-add-phase` to draft Phase 4 candle migration as milestone-scoped phase, NOT another quick task.

- 2026-04-28: **Phase 03.6 candle in-process embedder migration COMPLETE.** Pivoted from GGUF cheap path (per ARCH §9.10 rewrite — `quantized_qwen3::forward()` returns logits via `lm_head` not hidden states; verified via candle source inspection in RESEARCH.md §"Summary" finding #1) to safetensors via `fastembed::Qwen3TextEmbedding` (which wraps `candle-transformers::models::qwen3::Model`; direct candle held in reserve, not needed). Plan 1 spike: cosine equivalence on 30-query set mean=0.9994 / p10=0.9993 (gate ≥0.97/≥0.95 PASS). Plan 2 closure: poc.db reindexed 2116 symbols (0% drift, byte-identical to baseline), REQ-10 B1-B7=67.9% (literal 60% gate PASS, +0.0pp vs Phase 3 baseline 67.9% — inside ±5pp informational band), fsc.db FULL 2307-symbol reindex clean in 8m22s wall-clock (vs Phase 3.5b 132/2307 burst-hang at i=128 deterministic failure point), F1-F10 hand-eval generous-denominator=72% (Rule 7 gate ≥50% PASS, N/A flags locked at 2026-04-27T14:55:28Z BEFORE retrieval per locked decision #2). ARCH §9.10 rewritten with negative-rationale block (logits-via-lm_head + GGUF-tokenizer ~10× slower); ARCH §9.8 history row appended with version_hash=`f2b47aa16b17` (computed via Plan 1 Task 2.5 source-of-truth Rust binary, deterministic across re-runs). PROJECT.md Phase 4+ Backlog P0 entry CLOSED via Phase 03.6 commit `67320ec`. Phase 3 status flipped phase_3_prelim_complete → phase_3_complete. Stack shipped: candle-core/nn/transformers 0.10 + hf-hub 0.5 + tokenizers 0.22 + fastembed 5.13 (qwen3 feature, Apache-2.0), F32 weights, last-token pool + L2 normalize. ollama HTTP dependency removed from CodeNexus runtime path. Phase 03.6 SUMMARY: `.planning/phases/03.6-candle-in-process-embedder-migration-qwen3-embedding-0-6b-gg/03.6-SUMMARY.md`.

### Pending Todos

- **(P0 done) Phase 03.6 candle in-process migration** — COMPLETE 2026-04-28 via Plan 1 (loader + cosine equivalence) + Plan 2 (cross-corpus eval + closure). All 4 hard gates PASS: cosine equivalence mean=0.9994/p10=0.9993, REQ-10 B1-B7=67.9% (gate ≥60%), fsc.db FULL=2307 (no burst-hang), F1-F10 generous=72% (gate ≥50%). Shipped via fastembed-rs 5.13 wrapping candle-transformers 0.10 (NOT the GGUF cheap path the original Phase 4 plan locked — pivoted per RESEARCH.md §"Summary" finding #1: `quantized_qwen3::forward()` returns logits via lm_head, not hidden states). See `.planning/phases/03.6-.../03.6-SUMMARY.md`. ollama HTTP dependency removed from CodeNexus runtime path.
- **(P0 done) Phase 3.5b — embedder retry + fail-loud**: COMPLETE per quick task 260427-e7r. Retry wrapper landed in embedder.rs; --max-consecutive-fail flag landed in main.rs Index; bailed cleanly at 132/2307. See `.planning/quick/260427-e7r-.../260427-e7r-SUMMARY.md` for full verdict. Hard-evidence outcome: retry insufficient → triggers Phase 4 candle migration above.
- **(P1) Phase 3.5 follow-ups (epistemic gap closure)**:
  - B8 "concurrent writes" remains a real miss in original corpus even after rubric correction; investigate why (embedding mismatch? alpha insufficient? rerank needed?)
  - Cross-corpus eval should use LLM-judge or separate-person judge instead of system-author hand-judge (R5/R6 graded LLM-judge pattern; subjective bias was disclosed honestly in 260427-nz9 SUMMARY F5)
- **(P2, follow-up quick task) REQ-08 plumbing fix**: (a) Makefile line 25 cp expects `codenexus-core(.exe)` but Cargo package.name is `poc-retrieval` — name mismatch; (b) `make` not on Windows git-bash PATH on this host. Fix recommended: rename binary via Cargo `[bin]` target OR adjust Makefile cp + add bash/PowerShell wrapper for build chain. Estimated 30-45min; once fixed, naturally flushes REQ-06/07/08/09 deferred smokes in one full-stack run. Demoted from P1 to P2 after Phase 3.5b became the new acceptance blocker.
- Real spawn-and-restart smoke (REQ-07 acceptance #1-#3): blocked on plumbing fix above. With working `make build`: smoke (1) `serve --port 8080` starts both Go HTTP + extracted Rust, (2) Rust kill → 5s restart per supervisor backoff, (3) MCP query stdio round-trip
- Real browser load smoke (REQ-09 acceptance #1-#3): same blocker; verify `localhost:8080/` redirects to /ui/, search returns 4-score-column results, list_callers renders cytoscape graph with confidence color bands
- 150 MB total size budget verification (REQ-08 acceptance #2): blocked on real Rust release build. Expected ~110-150 MB Go + cytoscape 374 KB + UI text < 1 MB
- ldflags coreVersion injection: replace hardcoded `"dev"` with `-ldflags "-X .../supervisor.coreVersion=v0.x.y"` (mechanism documented in embed.go line 36 comment)
- (deferred) Phase 2 storage micro-bench: optional Criterion harness on insert/lookup/vector top-5/FTS5; skip per ship-it directive
- (deferred) Nomic Embed Code shadow re-test on AU 5090 host (CPU segfault here)
- (Phase 4+ tactical, see PROJECT.md): Leiden community detection (~30 lines petgraph), confidence-as-Leiden-weight (free), spike → core/ promotion or alias
- (Phase 4+ strategic, see PROJECT.md, Software 3.0 reframe): agent behavioral alignment (target ≤5% graph-tool miss rate vs CodeCompass 58% baseline), cross-session codebase understanding accumulation via memU integration, architectural decision semantic indexing (query_constraints A2A operation)

### Blockers/Concerns

- **A2A v0.2 spec stability** — if Google releases A2A v1.0 with breaking changes during MVP, we eat a refactor. Low-medium risk, mitigated by spec being public + small surface area we use.
- **candle model weight distribution** — Phase 2 spike must decide: bundle in binary (~80-120MB total) vs HuggingFace cache on first run. Affects cold-start UX.
- **mcp-go maintainer health** — Phase 2 spike sub-task to confirm release cadence + responsiveness.
- **Nomic Embed Code shadow eval blocked** — no-GPU on this host (per spike R5c). Retest needed on AU 5090.

### Quick Tasks Completed

| # | Description | Date | Commits | Directory |
|---|-------------|------|---------|-----------|
| 260427-h71 | REQ-07: Go server scaffold (cobra+chi+mcp-go+supervisor) per ARCH §2/§3.5/§5.5 | 2026-04-27 | 8ff8e11 + 54f23b1 | [260427-h71-req-07-go-server-scaffold-cobra-chi-mcp-](./quick/260427-h71-req-07-go-server-scaffold-cobra-chi-mcp-/) |
| 260427-hoc | REQ-08: //go:embed Rust core binary plumbing (embed.go + extraction + Makefile EMBED_DIR) | 2026-04-27 | f5b6621 + 59b725b | [260427-hoc-req-08-go-embed-rust-core-binary-into-go](./quick/260427-hoc-req-08-go-embed-rust-core-binary-into-go/) |
| (no-id) | Edge confidence on list_callers + Makefile/STATE realign + Software 3.0 reframe in PROJECT.md | 2026-04-27 | 4af9f4d + 7f6f44d + d98b16c | (no quick dir — micro-task + doc-only commits) |
| 260427-i0c | REQ-09: UI bundle (vanilla JS + cytoscape.js, no build step, option B git mv) | 2026-04-27 | ec3849e + dfdcb95 | [260427-i0c-req-09-go-embed-ui-bundle-vanilla-js-htm](./quick/260427-i0c-req-09-go-embed-ui-bundle-vanilla-js-htm/) |
| 260427-j9g | REQ-10: Phase 3 MVP precision gate met (B1-B7 mean=67.9% vs gate 60% +7.9pp; +24.3pp over GitNexus 1.6.3 baseline 43.6%) — Phase 3 CLOSED | 2026-04-27 | 226c50f | [260427-j9g-req-10-closure-phase-3-mvp-precision-gat](./quick/260427-j9g-req-10-closure-phase-3-mvp-precision-gat/) |
| 260427-nz9 | Phase 3.5 robustness slice (alpha sweep plateau confirmed, B10 rubric corrected, cross-corpus FSC strict 50%/generous 71.4% on 5-of-107 partial index; full re-index BLOCKED by ollama instability after ~130 sequential calls) | 2026-04-27 | 8f5d48c | [260427-nz9-phase-3-5-robustness-b10-rubric-fix-join](./quick/260427-nz9-phase-3-5-robustness-b10-rubric-fix-join/) |
| 260427-e7r | Phase 3.5b embedder retry + fail-loud micro-slice (engineering complete; bailed cleanly at 132/2307; retry CANNOT recover ollama 60s send-timeout × 5 = 5min/symbol = 20min/fail-cluster with zero recovery; ARCH §9.9 D-W9 + EVAL Rule 7 + PROJECT.md Phase 4 P2 backlog all locked; Phase 4 candle migration triggered by hard evidence) | 2026-04-27 | 8f4da66 | [260427-e7r-phase-3-5b-embed-retry-fail-loud-cli-index](./quick/260427-e7r-phase-3-5b-embed-retry-fail-loud-cli-index/) |
| (no-id) | Phase 4 prep: e7r PENDING backfill + ARCH §9.10 candle migration anchor (new section, §9.5 reranker untouched) + PROJECT.md Phase 4+ Backlog P0 entry with GGUF cheap-path kickoff notes (`llama.cpp/convert_hf_to_gguf.py` → `candle-transformers quantized::llama` saves day-1 spike exploration) | 2026-04-27 | 63cf312 + e553471 | (no quick dir — closure backfill + doc-only commits) |
| (no-id) | MiniMax 官方 concurrency probe: token-bucket characterized — capacity=80, refill=0.5/s (= 30 RPM steady). Sustained 2 QPS × 30s clean (60/60 ok); 4 QPS walls at exactly t=20s when 4×20=80 = bucket capacity. Initial "wall at N=64" reading retracted — was bucket-already-depleted, not concurrent ceiling. Sizing: 600-call eval = 17 min wall on 官方 (fits §9.4 30-min budget); 1500-call (3-seed) = 47 min, decision queued. Cross-validation of okaoi-vs-官方 grader agreement queued as prerequisite before any Gate-flipping run. | 2026-04-27 | af39bdc + 133a141 | (no quick dir — cheap probe per feedback rule 36) |
| 03.6-01 + 03.6-02 | Phase 03.6 candle in-process embedder migration COMPLETE. Plan 1: in-process embedder via fastembed-rs/candle-transformers 0.10 (cosine equivalence mean=0.9994/p10=0.9993 PASS); compute_version_hash source-of-truth bin (hash=f2b47aa16b17). Plan 2: poc.db reindex 2116 symbols (0% drift) + REQ-10 B1-B7=67.9% (gate PASS, byte-identical to ollama baseline) + fsc.db FULL 2307 in 8m22s (Phase 3.5b 132/2307 hang resolved) + F1-F10 hand-eval generous=72% (gate PASS) + ARCH §9.8/§9.10 rewrite + PROJECT.md backlog CLOSED + STATE.md flip phase_3_complete. | 2026-04-28 | f327d3a + 117746a + 0ff4a6a + fc9dfc6 + 3c0a323 + 0054804 + 30dcb56 + a13bf08 + b1fa94b + 19983fc + `65deee5` + `67320ec` + `67320ec` | [03.6-...](./phases/03.6-candle-in-process-embedder-migration-qwen3-embedding-0-6b-gg/) |

### Roadmap Evolution

- 2026-04-27: Phase 03.6 inserted after Phase 3 — Candle in-process embedder migration (qwen3-embedding-0.6b GGUF replacing ollama HTTP) (URGENT, INSERTED). Triggered by hard evidence in commit 8f4da66 (Phase 3.5b retry+fail-loud micro-slice: 20min wall-clock retry budget × 0 recovery on ollama 60s send-timeout hang). Phase 4 Parity unchanged.
- 2026-04-28: Phase 03.6 SHIPPED via safetensors path (fastembed-rs 5.13 wraps candle-transformers 0.10), NOT the GGUF path that the planning narrative had locked. Pivot rationale: candle's `quantized_qwen3::ModelWeights::forward()` returns vocab logits (lm_head projection), not the hidden states required for last-token pooling. RESEARCH.md §"Summary" finding #1 documents the source inspection that triggered the pivot. Phase 4 Parity entry now unblocked: full 2307-symbol FSC index achievable in <10min wall-clock with zero burst-hang.
- 2026-04-28: Phase 04.1 inserted after Phase 4 — Graph Clustering and Evolution Layer (URGENT, INSERTED). Bundles static Leiden module-boundary detection (promoted from Phase 4 tactical backlog ~30-line item) + Static Infomap call-flow refinement + DF-Leiden incremental layer (HIT-Leiden as stretch, deferred if no Rust port at plan-time) + CoDÆN-NeGMA evaluation harness + A2A `query_clusters`/`query_evolution` ops. Triggered by user research dump on dynamic/incremental community detection (HIT-Leiden 2026 Lin et al. 10²~10³× speedup; CoDÆN-NeGMA 2025 evolution-event tracking framework). Inserted as decimal (not appended Phase 7) because: Phase 6 Reach scope is plugin/IDE — clustering thematically off as Phase 7; Phase 04.1 incremental layer depends on Phase 4 file-watcher/delta-diff harness; static Leiden was already a Phase 4 backlog item so promoting + bundling avoids fragmentation. Phase 4 Parity scope unchanged structurally; Phase 4+ Backlog Leiden line will be marked promoted-to-04.1 at PROJECT.md edit time. GNN-hybrid (DLEC, neural Map Equation) explicitly deferred to Phase 6+.
- 2026-04-28: **Storage backend identity decision deferred to Phase 5 Bridge as ADR-PG** (NOT pre-Phase-4 inline migration). User proposed switching rusqlite+sqlite-vec → PostgreSQL+pgvector before Phase 4, citing pgvector maturity, cross-DB JOIN with memU, recursive CTE for Leiden, pg_trgm fuzzy search. Pushback accepted by user with reframing: **"this is an identity question, not a technical question"** — CodeNexus is "curl-and-run tool" (current ARCH §3 line 89 single-fat-binary invariant) vs "service in your local ecosystem" (PG-dependent). These are two different architectures; switching mid-Phase-4 inline is the wrong unit of change. Cost reality: 3-5 days work + REQ-10 baseline regression + first-run UX P1 regression (PROJECT.md line 71 elevated 2026-04-28 by same user). Decision: continue Phase 04.1 on SQLite (sufficient at 2116-symbol scale; petgraph Leiden runs in Rust not SQL); Phase 5 Bridge writes formal ADR-PG to decide identity question when memU integration forces it. D-R2 trait abstraction (ARCH line 629) preserves swap optionality. Adapter-pattern alternative (StorageBackend::Postgres opt-in alongside SQLite default) explicitly available if Phase 5 ADR-PG concludes "service identity" but wants to keep single-binary as opt-in default.

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| storage | redb vs rusqlite+sqlite-vec | **In progress** (spike-008 active, see .planning/research/storage_backend.md) | 2026-04-26 |
| distribution | cargo install / GH releases / homebrew | Post-MVP | 2026-04-26 |
| memU integration | self-contained vs shared PG (REQ-04 inherits self-contained default) | Phase 5 (Bridge) | 2026-04-26 |
| **storage identity (ADR-PG)** | **CodeNexus = "curl-and-run tool" (single-binary, SQLite) vs "local-ecosystem service" (PG-dependent, JOIN to memU). User-confirmed framing 2026-04-28: this is identity question, not technical question. Adapter pattern (PG opt-in, SQLite default) is mid-path option.** | **Phase 5 (Bridge) — write formal ADR-PG when memU integration forces the question** | **2026-04-28** |
| GitHub repo visibility | public-from-Phase-1 vs wait-for-MVP | Phase 1 wrap-up | 2026-04-26 |

## Session Continuity

Last session: 2026-04-28T06:47:37.913Z
  (1) Plan 1 (loader + cosine equivalence): commits f327d3a + 117746a + 0ff4a6a + fc9dfc6 + 3c0a323. In-process embedder via fastembed-rs 5.13 / candle-transformers 0.10; cosine equivalence on 30-query set mean=0.9994 / p10=0.9993 (gate ≥0.97/≥0.95 PASS). compute_version_hash source-of-truth bin: deterministic hex `f2b47aa16b17`.
  (2) Plan 2 (cross-corpus eval + closure): commits 0054804 + 30dcb56 + a13bf08 + b1fa94b + 19983fc + this STATE.md commit. poc.db reindex 2116 symbols (0% drift) + REQ-10 B1-B7=67.9% (literal 60% gate PASS, byte-identical to ollama) + fsc.db FULL 2307 in 8m22s (Phase 3.5b 132/2307 burst-hang resolved) + F1-F10 hand-eval generous=72% (Rule 7 gate PASS) + ARCH §9.8/§9.10 rewrite + PROJECT.md P0 backlog CLOSED.
Stopped at: Phase 4 context gathered
Resume files: this STATE.md + `.planning/phases/03.6-.../03.6-SUMMARY.md` + `.planning/PROJECT.md` Phase 4+ Backlog (P2 production-grade resilience entry still active).
Next-session entry: user says "继续" or "Phase 4" → **`/gsd-add-phase` to formalize Phase 4 Parity as milestone-scoped phase** (multi-language tree-sitter + multi-repo registry + git overlay + CodeFlow port). Or:

  - Sub-task 1: source qwen3-embedding-0.6b in candle-compatible format. Cheap path locked in PROJECT.md backlog: GGUF via `llama.cpp/convert_hf_to_gguf.py` → `candle-transformers` `quantized::llama` loader. Validate dim=1024 + instruction-prefix bit-equivalence on 30-query regression set (else §9.8 version-hash mismatch).
  - Sub-task 2: write candle model loader replacing `embed_once` HTTP body with in-process tensor inference; retry wrapper becomes "defensive only" since no network.
  - Sub-task 3: re-index obsidian-llm-wiki (cosine-distance equivalence on 30-query set), then re-index FSC to full 2307 symbols + re-run F1-F10 hand-eval per EVAL_DESIGN_NOTES Rule 7 (generous denominator, locked before run).
  Estimated: code ≈ 1d, decision/spec landing ≈ 0.5d fixed cost (per PROJECT.md backlog two-axis estimate). Total ≈ 1.5d minimum; +1d spike if GGUF route fails equivalence check.
Parallel-track entry: user says "Phase 3 Gate" or "LLM-judge" → run okaoi-vs-官方 grader cross-validation on 30-call sample (e.g. seed-42 axis-3 hits) using `r7b_llm_judge_axis3.py --smoke` against both providers, compare per-hit grades, decide whether okaoi is interchangeable for inner-loop iteration. Prerequisite for any §9.4 gate-flipping run.

## Linear cross-reference

Linear project ID `5c8f1e26-c63d-4372-bcd9-4d94d04788a3` — renamed to **CodeNexus** on 2026-04-26. 44 issues now (XAR-224 → XAR-266 + XAR-270 new IPC spike) across 8 milestones.

**Synced state (2026-04-26 apply)**:

- Decisions milestone XAR-224 → 231: all 8 closed to Done with per-issue decision verdicts in description
- XAR-238 (rmcp spike): Canceled, replacement note points to mcp-go
- XAR-270 (new): Phase 2 (SPEC Phase 0) spike — A2A endpoint + Go-Rust IPC over A2A protocol, in Phase 0 Spike milestone

GSD `.planning/` canonical for phase-internal state; Linear canonical for milestone-level tracking. Sync script kept at `D:/projects/codenexus/scripts/linear_sync_decisions.py` for audit (single-shot, won't re-run).
