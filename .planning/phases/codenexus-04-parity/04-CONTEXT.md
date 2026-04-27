# Phase 4 (First Slice): First-Run UX Cluster + P2 Resilience Same-Crate -- Context

**Gathered:** 2026-04-28
**Status:** Ready for planning
**Companion:** `04-SPEC.md` (binds), `04-PRE-PLAN-NOTES.md` (informational), `INTERVIEW_LOG.md` (spec interview history)

<domain>
## Phase Boundary

Replace Phase 03.6's silent first-run failure mode with explicit first-run UX (R1 HF revision pin + R2 download messaging + R3 offline doc) AND opportunistically apply Phase 3.5b's `consecutive_fails` resilience pattern from `main.rs Index` to two same-crate call sites (R4 server.rs:198 A2A Index handler, R5 search.rs:31 Query path). Co-location rule: only modify `experiments/poc-retrieval/src/{embedder,server,search}.rs` + `docs/embedder-offline-bootstrap.md` + `docs/ARCHITECTURE.md` (§9.8 row) + `README.md` (Quick start link) + `experiments/poc-retrieval/eval/e2e_first_run_smoke.sh` (NEW).

**Eval no-regression invariant:** post-pin REQ-10 B1-B7 mean precision_at_5 within +/- 2pp of Phase 03.6 67.9% baseline. Pinning revision MUST be the SHA whose model produced 03.6's 67.9% (NOT a newer SHA).

</domain>

<spec_lock>
## Requirements (locked via 04-SPEC.md)

**5 requirements are locked.** See `04-SPEC.md` for full requirements (R1-R5), boundaries (7 explicit anti-scope items), and acceptance criteria (13 pass/fail checkboxes + E2E smoke).

Downstream agents (researcher, planner, executor, verifier) MUST read `04-SPEC.md` before planning or implementing. Requirements are NOT duplicated here.

**In scope (from SPEC):**
- HF revision pin (R1) including SHA const + ARCH §9.8 row + reload test
- First-run download UX (R2) augmenting existing `embedder.rs:67-71/73-74`
- Offline / Clash-down recovery doc (R3) including 4 documented paths + README.md link
- `server.rs:198` A2A Index handler counter (R4) -- same-crate mechanical patch
- `search.rs:31` Query path retry budget cap (R5) -- same-crate mechanical patch
- E2E smoke harness for first-run UX (Q6=B locked)

**Out of scope (from SPEC):**
- Multi-language tree-sitter (Phase 4 group 2, separate SPEC)
- Multi-repo registry (Phase 4 group 3, separate SPEC)
- Git overlay via gix (Phase 4 group 4, separate SPEC)
- Pattern detection / security scanners ported from CodeFlow MIT (Phase 4 group 5, separate SPEC)
- `tracing` framework migration -- ASSUMPTION CORRECTION #1, use `eprintln!`
- `EmbedError` enum (Transient / Permanent / Timeout) and 33 caller-site arm-match -- Q5=B locked
- Progress indicator (R2 (c)) -- DEFERRED unless implementation path naturally exposes callback (see D-06 below)
- Fork of fastembed-rs to add revision support
- Any change to `core/` (Rust placeholder), `server/` (Go), `ui/` -- co-location boundary

</spec_lock>

<decisions>
## Implementation Decisions

### R1: HF revision-fetch path (D-01, D-06)

- **D-01:** Use `huggingface_hub_rust` crate (HF official 2026-04 release) for revision-pinned model fetch. `snapshot_download(repo, revision) -> PathBuf` is the target API; resulting local path is then passed to `Qwen3TextEmbedding::from_hf` (which accepts repo-id-or-local-path per Phase 03.6 cargo cache spelunking documented in `03.6-01-SUMMARY.md:153-188`).
- **D-06:** If `huggingface_hub_rust::snapshot_download` exposes a progress callback parameter, executor MUST deliver progress indicator as a side benefit and update SPEC R2 (c) acceptance from DEFERRED to required (per `04-SPEC.md` R2 trigger condition). If no callback exposure, R2 (c) stays DEFERRED -- not a regression.
- **NOT chosen:** `hf-hub` (transitive dep of fastembed-rs) -- would require manual file enumeration over 5 files (config.json, model.safetensors, tokenizer.json, tokenizer_config.json, special_tokens_map.json). User pre-committed to huggingface_hub_rust for cleaner Python-parity API; planner MUST verify (see Plan-Time Gates G-01..G-03 below).
- **Anti-pattern locked:** Do NOT fork fastembed-rs to add revision support -- wrapper layer (snapshot fetch with revision -> from_hf with local path) is the cleaner separation per PRE-PLAN-NOTES.

### R5: Query path retry refactor (D-02)

- **D-02:** Add `embed_query()` method on `Embedder` impl with internal const `QUERY_MAX_ATTEMPTS=2` and `QUERY_DELAY_MS=250` (NO exponential backoff). `search.rs:31` call site changes from `embedder.embed(query, Role::Query)` to `embedder.embed_query(query)`. The shared 5-attempt retry wrapper in `embedder.rs:84-101` is left untouched for Index callers.
- **NOT chosen:** `embed_with_policy(text, role, policy: RetryPolicy)` parameterized -- more flexible but introduces hidden state machine via policy enum; conflicts with Q5=B "mechanical patch only" lock. Future Index/Query split tuning can introduce policy struct in a later phase.

### R4: server.rs A2A Index threshold source (D-05)

- **D-05:** Hybrid threshold source. Priority order: A2A request envelope per-call override > config.toml startup value > hardcoded const = 5 fallback.
  - **Envelope:** A2A Index task params extends with optional `max_consecutive_fail: Option<usize>`. `None` means "use lower-priority source" (NOT "disable counter"). Server reads from envelope first.
  - **config.toml:** Optional `[embedder].max_consecutive_fail = N` at server config section. If absent, falls through to hardcoded.
  - **Hardcoded const:** `MAX_CONSECUTIVE_FAIL_DEFAULT: usize = 5` in server.rs handler scope (mirrors main.rs CLI flag default from Phase 3.5b commit `8f4da66`).
- **A2A schema extension caveat:** Planner MUST verify A2A v0.2 spec allows vendor-prefix params extension OR free-form `params` map BEFORE adding the field (see Plan-Time Gate G-04 below). If A2A v0.2 forbids extension, fall back to (config.toml + hardcoded) only -- NO envelope override.

### E2E smoke harness (D-03, D-03b, D-04)

- **D-03:** Hybrid harness shape. Bash script (`experiments/poc-retrieval/eval/e2e_first_run_smoke.sh`) drives env setup + cache mutation; Rust release binary runs the actual scenario. Bash handles `rm -rf ~/.cache/huggingface/hub/...`, `export HTTPS_PROXY=...`, exit-code checking; Rust handles parsing + indexing + querying.
- **D-03b:** Rust side uses `./bin/codenexus query` (the release artifact from `make build`), NOT `cargo run --release -- query`. This tests the `make build` + Go `//go:embed` packaging chain, which is what users actually receive. cargo run is dev-iteration only, can mask packaging bugs.
- **D-04:** Network-block mechanism = `HTTPS_PROXY=http://0.0.0.0:1` env override. One line export, one line unset, no admin / no hosts file edit / no Clash kill. Cross-environment consistent (CI / 5090 windows / Curry's Shanghai Clash machine all behave identically).
- **NOT chosen:** Pure bash with curl probes (insufficient -- doesn't test embedder loader code path); Rust `#[test]` integration (`tests/e2e_first_run.rs` -- cache mutation pollutes sibling tests, env override needs `serial_test` crate, P2 complexity mismatch first-slice mechanical); hosts file edit (admin required, CI cannot reproduce); kill Clash (only Curry's Shanghai box, not portable).

### Plan-Time Verification Gates (mandatory, must pass before plan body)

These gates are **binding on failure**: if any gate fails verification, planner falls back to the documented alternative WITHOUT re-asking user.

- **G-01:** `huggingface_hub_rust` crate license MUST be in {Apache 2.0, MIT, BSD}. If GPL/AGPL/SSPL, fall back to `hf-hub` per PROJECT.md "Out of Scope: GPL/AGPL deps banned" constraint. Verify via `cargo metadata --format-version 1 | jq '.packages[] | select(.name=="huggingface_hub_rust") | .license'` after adding.
- **G-02:** `huggingface_hub_rust` crate version MUST be non-pre-release and >= 0.1.0 (NOT 0.0.x). Verify via `cargo search huggingface_hub_rust` or `crates.io/crates/huggingface_hub_rust` page. If pre-release / 0.0.x, fall back to `hf-hub`.
- **G-03:** `huggingface_hub_rust` MUST expose `snapshot_download(repo, revision) -> PathBuf` (or named-arg equivalent that returns a snapshot directory). Verify via `cargo doc --open` or source inspection. If API is materially different (e.g. only individual file fetch), evaluate whether wrapping is < 30 lines; if not, fall back to `hf-hub`.
- **G-04:** A2A v0.2 spec MUST allow vendor params extension on Index task envelopes. Verify against `https://google.github.io/A2A/` Task envelope spec section. If forbidden, drop D-05 envelope-override layer; threshold source becomes (config.toml + hardcoded) only -- update D-05 sub-bullet in CONTEXT.md before plan write.

### Claude's Discretion

- **Plan grouping:** SPEC un-locks plan count. Default planner allocation: 3 plans (Plan 1 = R1+R2+R3 first-run UX cluster + ARCH §9.8 row; Plan 2 = R4+R5 P2 mechanical patches; Plan 3 = E2E smoke harness + verification + commit closure). User said "SPEC 未锁, planner 默认给 3" -- so 3 plans is the discretionary default, planner can collapse to 2 if R4 or R5 ends up trivially small.
- **R4 envelope field naming:** if A2A v0.2 requires vendor prefix, use `x-codenexus-max-consecutive-fail` as the field name. If free-form `params` map allowed, use `max_consecutive_fail` (no prefix).
- **R2 messaging exact wording:** SPEC R2.a requires "first-run download" + `huggingface.co` URL + ETA wording ("30-60s" or "broadband" or equivalent). Exact phrasing is planner/executor's call -- the verifier grep is the contract.

### Folded Todos

(None -- no GSD todos matched Phase 4 first slice scope at discuss time.)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### SPEC + planning trail (Phase 4 first slice)

- `.planning/phases/codenexus-04-parity/04-SPEC.md` -- LOCKED requirements (5 R), boundaries (7 anti-scope), acceptance (13 checkbox + E2E smoke). MUST read first.
- `.planning/phases/codenexus-04-parity/04-PRE-PLAN-NOTES.md` -- informational hints for planner (Hint 1 R1 fetch lib evaluation, Hint 3 R4/R5 implementation patterns, Hint 4 E2E harness). NOT binding -- if cargo doc reveals different reality, SPEC binds.
- `.planning/phases/codenexus-04-parity/INTERVIEW_LOG.md` -- 8-round spec interview history (Q1-Q6 lock decisions).

### Phase 03.6 baseline (R1 SHA + REQ-10 67.9% reference)

- `.planning/phases/03.6-candle-in-process-embedder-migration-qwen3-embedding-0-6b-gg/03.6-SUMMARY.md` -- 03.6 closure summary; cosine equivalence + REQ-10 67.9% B1-B7 baseline source.
- `.planning/phases/03.6-candle-in-process-embedder-migration-qwen3-embedding-0-6b-gg/03.6-01-SUMMARY.md` §153-188 -- cargo cache spelunking that documented `Qwen3TextEmbedding::from_hf` 4-arg signature (no revision param).
- `experiments/poc-retrieval/eval/req10_alpha06.json` -- 03.6 baseline harness; post-pin no-regression run compares to this.

### ARCHITECTURE.md sections (R1 protocol + 03.6 negative rationale)

- `docs/ARCHITECTURE.md` §9.8 -- version-hash protocol; R1 MUST add new history row with pinned SHA per "version-hash-affecting changes" discipline.
- `docs/ARCHITECTURE.md` §9.10 -- candle migration negative rationale block (GGUF lm_head problem); not directly affecting first slice but contextualizes why fastembed safetensors path is canonical.

### Project-level constraints

- `.planning/PROJECT.md` Phase 4+ Backlog (lines 71-94) -- this slice supersedes lines 71-94 first-run UX cluster + P2 resilience entries. Will be marked `[CLOSED via Phase 4 first slice]` after slice closure.
- `.planning/PROJECT.md` Out of Scope -- "GPL/AGPL deps banned" constraint sources Plan-Time Gate G-01.
- `.planning/PROJECT.md` Constraints -- "Single-fat-binary distribution invariant" -- safetensors blob MUST NOT be vendored into Go-embedded binary (ARCH §3 line 89).
- `.planning/REQUIREMENTS.md` REQ-10 -- 60% literal gate; post-pin must stay within +/- 2pp of 67.9%.
- `.planning/ROADMAP.md` Phase 4 -- success criteria 1-7 (this first slice does NOT close all 7; only opens Parity work; multi-language tree-sitter / multi-repo registry / git overlay / CodeFlow port get separate SPECs).

### Existing source code (R1-R5 modification surfaces)

- `experiments/poc-retrieval/src/embedder.rs:39` -- `const MODEL_REPO` definition (R1 target: add adjacent `const QWEN3_REVISION`).
- `experiments/poc-retrieval/src/embedder.rs:67-71` -- existing first-run start-prompt `eprintln!` (R2 target: augment with URL + ETA).
- `experiments/poc-retrieval/src/embedder.rs:72` -- `Qwen3TextEmbedding::from_hf(MODEL_REPO, ...)` call site (R1 target: replace MODEL_REPO arg with locally-fetched snapshot dir from D-01).
- `experiments/poc-retrieval/src/embedder.rs:73-74` -- existing failure `.context()` line (R2 target: link to docs/embedder-offline-bootstrap.md).
- `experiments/poc-retrieval/src/embedder.rs:84-101` -- shared 5-attempt retry wrapper (`MAX_ATTEMPTS=5`, `BASE_DELAY_MS=250`, exponential). R5 ADDS `embed_query()` method, does NOT modify this wrapper.
- `experiments/poc-retrieval/src/main.rs:156` (vicinity) -- existing `consecutive_fails` counter pattern from Phase 3.5b commit `8f4da66`. R4 reference implementation.
- `experiments/poc-retrieval/src/server.rs:198` -- A2A Index handler (R4 target: copy counter pattern from main.rs).
- `experiments/poc-retrieval/src/search.rs:31` -- Query call site (R5 target: switch to embed_query).
- `experiments/poc-retrieval/Cargo.toml` -- R1 may add `huggingface_hub_rust = "..."` dep (subject to Plan-Time Gates G-01..G-03).
- `README.md` Quick start section (lines 53-58) -- R3 target: add link to docs/embedder-offline-bootstrap.md.

### External spec references (Plan-Time Gate G-04)

- A2A v0.2 specification: `https://google.github.io/A2A/` -- planner MUST verify Task envelope params extension allowance for D-05 envelope override path.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`main.rs:156` consecutive_fails counter pattern** (Phase 3.5b commit `8f4da66`): exact pattern R4 server.rs Index handler copies. Loop body increments on Err, resets on Ok, bails when threshold reached. Bail mechanism in main.rs is `anyhow::bail!`; in server.rs translates to "set A2A task state to `failed` with structured error message containing consecutive count".
- **`embedder.rs:67-71` start-prompt eprintln!**: existing first-run message mentions `~1.2 GB` + HF cache path. R2 augments with `huggingface.co` URL + ETA wording ("30-60s on broadband"); does NOT replace from zero.
- **`embedder.rs:73-74` failure context line**: existing `.context("model download failed -- check internet to huggingface.co")`. R2 augments with second line linking to `docs/embedder-offline-bootstrap.md`.
- **`embedder.rs:84-101` shared retry wrapper** (`MAX_ATTEMPTS=5`, exponential backoff = ~7.75s budget): R5's `embed_query()` does NOT use this wrapper; it has a private 2-attempt 250ms loop. Index callers continue using the shared wrapper unchanged.
- **`Qwen3TextEmbedding::from_hf` accepts repo-id-or-local-path**: verified during Phase 03.6 cargo cache spelunking. R1 implementation passes locally-fetched snapshot dir (from huggingface_hub_rust::snapshot_download) instead of repo ID string.
- **fastembed-rs is the embedder layer** (5.13, wraps candle-transformers 0.10): R1 does NOT replace fastembed; R1 wraps fastembed's load path with a revision-pinned fetch.

### Established Patterns

- **`eprintln!` for user messaging** (33 uses in `experiments/poc-retrieval/src/`, 0 uses of `tracing`). R2 augmentation uses `eprintln!`. Tracing migration is explicitly out of scope (ASSUMPTION CORRECTION #1).
- **Result objects + anyhow** for error propagation. `.context()` chains used at failure paths. R2 failure messaging extends existing `.context()` chain, does NOT introduce new error types (Q5=B locked).
- **A2A v0.2 task envelope** for cross-process communication. R4 envelope-override path extends Index task envelope params; depends on Plan-Time Gate G-04 verification.
- **Phase 03.6 §9.8 protocol**: every model_id / dim / prefix / revision change appends a history row to ARCH §9.8. R1 follows this protocol -- new row with pinned SHA.
- **eval/<name>.json harness pattern**: `eval/req10_alpha06.json` is the existing baseline; new harness `eval/e2e_first_run_smoke.sh` lives parallel.

### Integration Points

- **Cargo.toml dep addition**: R1 adds `huggingface_hub_rust` (subject to Gates G-01..G-03). No other deps added in this slice.
- **A2A Index task envelope schema**: R4 envelope-override extends `params` (subject to Gate G-04). If forbidden, scope shrinks to (config.toml + hardcoded) -- NOT a hard block.
- **Build chain**: D-03 E2E harness uses `make build` -> `./bin/codenexus query` flow. Verifies `//go:embed` packaging chain end-to-end on every E2E run.
- **`docs/` directory**: gets one new file (`embedder-offline-bootstrap.md`) and one row addition to ARCHITECTURE.md §9.8. No other doc directory changes.
- **README.md**: gets one link addition to Quick Start section. No other README changes.

</code_context>

<specifics>
## Specific Ideas

- **User pre-committed `huggingface_hub_rust` over hf-hub** despite cargo-doc-then-decide being the workflow-recommended option. Signal: user prefers Python-convention parity API surface and HF-official maintenance. Compatible with PROJECT.md "新>成熟" preference (`feedback_tool_selection_filter.md`). Plan-Time Gates G-01..G-03 act as binding-on-failure verification, NOT as user re-question; if any gate fails, planner falls back without asking.
- **User picked Hybrid for both E2E harness AND R4 threshold source** (2 of 4 areas). Signal: comfortable with first-slice carrying production texture (config.toml integration, A2A schema extension) as long as architectural decisions stay locked (no EmbedError enum, no tracing). This calibrates planner to prefer "full plumbing rather than minimal patch" within Q5=B mechanical-only spirit.
- **User explicitly chose `./bin/codenexus query` for E2E** (D-03b). Reasoning surfaced during discussion: testing `make build` + `//go:embed` packaging chain catches release-artifact bugs that `cargo run` would mask. Planner MUST honor this -- E2E harness invokes release binary, NOT cargo run.
- **R4 server.rs and main.rs both have counter pattern but DIFFERENT bail semantics**: main.rs CLI = `anyhow::bail!` exits process; server.rs A2A handler = sets task state to `failed` with structured error in response, server keeps running to serve next request. Same counter logic, different terminal action. This is intentional per ARCH constraint that server is long-running multi-client.

</specifics>

<deferred>
## Deferred Ideas

(None -- discussion stayed within first-slice scope. No scope-creep was attempted; user did not propose multi-language tree-sitter / multi-repo registry / git overlay / CodeFlow port topics during discuss-phase.)

The four explicit Phase 4 deferred groups (multi-language tree-sitter, multi-repo registry, git overlay, CodeFlow MIT port) remain on track for separate SPECs after this first slice closes -- this CONTEXT.md does NOT capture decisions for them.

</deferred>

---

*Phase: codenexus-04-parity (first slice)*
*Context gathered: 2026-04-28*
*Next step: `/gsd-plan-phase 4` will read this CONTEXT.md + 04-SPEC.md + 04-PRE-PLAN-NOTES.md to break the slice into 2-3 executable plans (default 3: UX cluster / P2 patches / E2E harness; planner can collapse to 2 if R4 or R5 ends up small).*
