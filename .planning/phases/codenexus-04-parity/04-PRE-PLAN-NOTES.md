# Phase 4 First Slice -- Pre-Plan Notes (NON-LOCKING)

**Purpose:** Informational signals for planner. NOT requirements (those live in `04-SPEC.md`). `04-SPEC.md` is the contract; this file is a planning shortcut. Planner can ignore everything here if `cargo doc` reveals a better path -- SPEC still binds.

**Created:** 2026-04-28
**Companion to:** `04-SPEC.md` (5 R locked, ambiguity 0.156)

---

## Hint 1 -- HF revision-fetch path evaluation (R1)

### Context

`04-SPEC.md` R1 locks "revision MUST be pinned". Implementation path is intentionally NOT locked because fastembed-rs 5.13 `Qwen3TextEmbedding::from_hf(repo, device, dtype, max_len)` 4-arg signature does not expose a revision parameter (verified at `experiments/poc-retrieval/src/embedder.rs:72` and via cargo cache spelunking documented in Phase 03.6 `03.6-01-SUMMARY.md:153-188`).

### Two libraries to evaluate during plan-phase

Both should be a 5-minute `cargo doc` look before committing to a path:

#### Option A -- `hf-hub` crate (mature, community-widely-used)

- Already a **transitive dep** of fastembed-rs (no new top-level dep needed if we go this route).
- API surface: `ApiBuilder` to build a client → `Repo::with_revision("<sha>")` to bind to a specific commit → `repo.get(filename) -> PathBuf` to download (or hit cache for) a single file.
- Cache layout: `~/.cache/huggingface/hub/models--<repo>/snapshots/<sha>/`. R1.c reload test relies on this layout being predictable.
- Used by fastembed internally, so behavior is well-trodden.

#### Option B -- `huggingface_hub_rust` crate (NEW, 2026-04-09 official HF release)

- API surface closer to Python `huggingface_hub`, may expose `snapshot_download(repo, revision=...) -> PathBuf` directly returning the snapshot directory in one call.
- Worth checking license (need Apache 2.0 / MIT / BSD per project Out-of-Scope GPL/AGPL ban).
- Worth checking maintainer responsiveness (HF official is a positive signal but does not guarantee it).
- Risk: newer crate, less battle-tested at version 0.x.

### Decision criteria

1. If `huggingface_hub_rust` exposes a clean `snapshot_download(repo, revision) -> PathBuf` AND license is Apache 2.0 / MIT / BSD AND the crate has a non-pre-release version → **prefer it** (Python-convention parity is future-proof; one call returns snapshot dir, no manual file enumeration).
2. Else fall back to `hf-hub` `Repo::with_revision` + enumerated `repo.get(<filename>)` calls for each of: `config.json`, `model.safetensors`, `tokenizer.json`, `tokenizer_config.json`, `special_tokens_map.json` (and any other files Qwen3-Embedding-0.6B requires for fastembed loading -- verify via fastembed source which file list it reads).
3. Either way: pass the resulting local snapshot directory path to `Qwen3TextEmbedding::from_hf` (which accepts repo-id-or-local-path per the same fastembed source verified during 03.6).

### Anti-pattern

**Do NOT fork fastembed-rs to add revision support.** Wrapper layer (snapshot fetch with revision → from_hf with local path) is the cleaner separation. Forking creates a maintenance burden and a divergence from upstream that future fastembed updates have to chase.

---

## Hint 2 -- Progress indicator path (R2 (c) deferred)

If the R1 plan picks `hf-hub` or `huggingface_hub_rust` custom fetch (i.e. NOT the fastembed-internal path which hides progress), the fetch loop in either crate exposes callback hooks (`hf-hub` has `download_with_progress` or equivalent; `huggingface_hub_rust` may have a callback parameter on `snapshot_download`). In that case, implementer can deliver progress as a side benefit:

```
[embedder] downloading Qwen/Qwen3-Embedding-0.6B@<sha-prefix> from huggingface.co
[embedder] [12% / 144 MB / ETA 47s]
[embedder] [38% / 456 MB / ETA 28s]
[embedder] [done / 1.2 GB / 38s]
```

If this progress lands, **update SPEC R2 acceptance to add (c)**:
- (c) During first-run download (clean cache), `eprintln!` emits >= 2 progress lines containing percentage / MB / ETA before the "done" line.

If the R1 plan picks fastembed-internal path (no callback exposure), R2 (c) stays deferred. SPEC `Out of scope` mentions "Progress indicator (R2 (c)) -- DEFERRED unless implementation path naturally exposes it" -- this is the trigger condition for promotion.

---

## Hint 3 -- R4/R5 mechanical patch implementation (Q5=B locked)

### R4 (server.rs A2A Index handler)

The `--max-consecutive-fail` counter pattern in `main.rs` (Phase 3.5b commit `8f4da66`, see `experiments/poc-retrieval/src/main.rs:156` area) is approximately:

```rust
let mut consecutive_fails: usize = 0;
for (i, sym) in symbols.iter().enumerate() {
    match embedder.embed(&sym.text(), Role::Passage) {
        Ok(v) => {
            consecutive_fails = 0;
            // store v
        }
        Err(e) => {
            consecutive_fails += 1;
            if consecutive_fails >= max_consecutive_fail {
                anyhow::bail!("aborted at i={}: {} consecutive embedder fails (last error: {})",
                              i, consecutive_fails, e);
            }
            eprintln!("[{}/{}] embed failed (consecutive={}/{}), continuing", i+1, total, consecutive_fails, max_consecutive_fail);
        }
    }
}
```

For `server.rs:198` A2A Index handler, the equivalent pattern goes inside the loop body that iterates symbols. The bail / abort mechanism translates to "set A2A task state to `failed` with structured error" instead of `anyhow::bail!`. The threshold value source: NOT a CLI flag (server has no CLI for handler params), but EITHER (a) a config-file value read at server startup, OR (b) a per-request override field in the A2A Index task envelope (the cleaner path -- A2A clients can override per-request without restarting server). Recommend (b) but planner picks.

### R5 (search.rs Query path)

The Query path retry budget cap can be implemented two ways (planner picks):

#### Option (i) -- new `embed_query` method (cleaner for caller, more code)

```rust
impl Embedder {
    /// Query path retry budget: 2 attempts, 250ms total. Single failure
    /// surfaces as Err in <1s, NOT 7.75s like Index path.
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        const QUERY_MAX_ATTEMPTS: u32 = 2;
        const QUERY_DELAY_MS: u64 = 250;
        let mut last_err = None;
        for attempt in 0..QUERY_MAX_ATTEMPTS {
            match self.embed_once(text, Role::Query) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    last_err = Some(e);
                    if attempt + 1 < QUERY_MAX_ATTEMPTS {
                        std::thread::sleep(std::time::Duration::from_millis(QUERY_DELAY_MS));
                    }
                }
            }
        }
        Err(last_err.unwrap())
    }
}
```

`search.rs:31` then changes from `embedder.embed(query, Role::Query)` to `embedder.embed_query(query)`.

#### Option (ii) -- parameterized `embed_with_policy` (more flexible, fewer methods)

```rust
pub struct RetryPolicy { pub max_attempts: u32, pub base_delay_ms: u64, pub exponential: bool }

impl Embedder {
    pub fn embed_with_policy(&self, text: &str, role: Role, policy: RetryPolicy) -> Result<Vec<f32>> { ... }
}

// search.rs:31
embedder.embed_with_policy(query, Role::Query,
    RetryPolicy { max_attempts: 2, base_delay_ms: 250, exponential: false })?
```

Option (i) is simpler and clearly conveys intent at call sites. Option (ii) is more flexible if future Query/Index split grows. **Recommend (i)** for this slice (Q5=B locked "mechanical patch only", flexibility is a future-phase concern).

---

## Hint 4 -- E2E smoke harness (Q6=B locked, supports R1+R2+R3 acceptance)

E2E smoke is verifier territory, but planner should know the harness exists / will be built so that test infrastructure does not become a blocking dep.

### Required harness pieces

1. **Small pre-indexed test corpus**: ~5-file TS repo (already exists in poc.db / fixtures somewhere -- planner verifies); 2 NL queries against it (one expected to succeed, one negative). 100 KB max.
2. **Cache-clear shell snippet**: `rm -rf ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/`. Verifier-runnable, no permissions issue (user's own cache).
3. **Network-block snippet**: cross-platform tricky. Linux/macOS: `iptables` or `pf` rule. Windows: hosts file edit (`echo "0.0.0.0 huggingface.co" >> C:\Windows\System32\drivers\etc\hosts` requires admin). Easier path: kill Clash if Clash is the only route to huggingface.co (Curry's 上海 hosts setup), OR use `HTTPS_PROXY=http://0.0.0.0:1` env var to force a connect failure. Recommend `HTTPS_PROXY` route -- no admin, no system file edit, undo is `unset`.

### Suggested smoke-script location

`experiments/poc-retrieval/eval/e2e_first_run_smoke.sh` (parallel to existing `eval/req10_alpha06.json`). Idempotent: cleans up env / cache between phases.

---

## Hint 5 -- Eval no-regression check (constraint enforcement)

After R1 lands the pinned SHA, BEFORE merging, planner / executor MUST rerun:

```bash
cd experiments/poc-retrieval
cargo run --release -- eval --queries eval/queries.json --db poc.db --alpha 0.6 --out eval/req10_post_pin.json
```

And compare `req10_post_pin.json` mean precision_at_5 vs Phase 03.6 baseline `req10_alpha06.json` 67.9%. **Acceptable delta: ±2pp** (per SPEC Constraints section). Outside that, R1 has accidentally pinned a wrong SHA (e.g. a newer revision with different weights) -- bail and re-pin to the pre-03.6 SHA.

The pre-03.6 SHA (the one whose model produced 03.6's 67.9%) can be recovered from:
- `~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/` (assuming cache was not cleared since 03.6) -- the snapshot subdir name IS the SHA
- OR HuggingFace Hub web UI history → look up commits as of 2026-04-28 (the 03.6 closure date)

---

## Files this slice will touch (planner planning surface)

```
experiments/poc-retrieval/src/embedder.rs       (R1 const, R2 messaging, R5 embed_query)
experiments/poc-retrieval/src/server.rs         (R4 counter)
experiments/poc-retrieval/src/search.rs         (R5 call site change)
experiments/poc-retrieval/Cargo.toml            (R1 if huggingface_hub_rust adopted)
experiments/poc-retrieval/eval/e2e_first_run_smoke.sh  (Q6 E2E harness, NEW)
docs/embedder-offline-bootstrap.md              (R3, NEW)
docs/ARCHITECTURE.md                            (R1 §9.8 history row)
README.md                                       (R3 Quick start link)
```

8 files (1 new in src tree, 1 new doc, 1 new harness, 1 new ARCH row, 1 README edit, 3 src edits, 1 Cargo.toml edit). Plan-phase will likely group into 2-3 plans (e.g. plan 1 = R1 + R2 + R3 first-run UX cluster; plan 2 = R4 + R5 P2 mechanical patches; plan 3 = E2E smoke harness + verification + commit closure). Planner picks final grouping.

---

## What this file is NOT

- **NOT a plan** (`/gsd-plan-phase 4` produces PLAN.md files; this is hints for that planner)
- **NOT a contract** (only `04-SPEC.md` binds; planner / executor / verifier ignore this file if SPEC and code disagree with it)
- **NOT a substitute for `cargo doc`** (planner MUST verify Hint 1 by actually reading hf-hub and huggingface_hub_rust docs)
- **NOT versioned by §9.8 protocol** (only ARCH §9.8 history rows track version-hash-affecting changes; this file is informational, no protocol)
