// Phase 03.6 in-process embedder via candle (fastembed-rs wrapper).
//
// Replaces the prior ollama HTTP path (commit bf01780 era) which had a
// burst-failure mode at ~130 sequential calls (Phase 3.5b RCA: 60s send-timeout
// x 5 retries = 5min/symbol, indexer fail-loud bailed at 132/2307 fsc.db).
//
// Stack rationale (RESEARCH.md §"Summary"):
//   - fastembed-rs 5.13 (Apache-2.0) wraps candle-transformers Qwen3Model
//     and provides built-in left-padding + last-token pool + L2 normalize,
//     matching the official Qwen3-Embedding reference implementation.
//   - Direct candle path (RESEARCH.md §"Pattern 1") was the fallback if
//     fastembed config knobs were inadequate; verified 2026-04-27 that
//     fastembed exposes the full caller-prefix flow (no instruction-prefix
//     hardcoding -- caller passes any string including QUERY_INSTRUCT).
//
// Phase 4 first-slice changes (04-01 v2):
//   - R1 redesigned: bypasses Qwen3TextEmbedding::from_hf entirely.
//     fastembed-5.13.3/src/models/qwen3.rs:1014 wraps input as
//     api.model(repo_id) and re-fetches config.json from default `main`
//     regardless of what path is passed -- making the SHA pin decorative.
//     v2 uses Qwen3TextEmbedding::new(model, tokenizer) with LOCAL files
//     only (G-05 path (a) from Plan 04-01 v2 plan_time_verification).
//   - R2.c promoted: DownloadProgress impl using hf-hub's
//     download_with_progress<P: Progress> API (sync.rs:795). v1 incorrectly
//     claimed no callback hook exists. Emits eprintln at 25% milestones.
//   - M1: per-file snapshot root validation (starts_with assertion).
//   - M5: OnceLock::get_or_try_init eliminates check-then-load-then-set race.
//
// Locked decisions:
//   - F32 weights (Phase 4 will explore F16). Locked decision #4.
//   - QUERY_INSTRUCT byte-identical incl trailing space. Locked decision #5.
//   - CPU-only target (cuda/metal feature flags Phase 4+). Locked decision #6.

use anyhow::{Context, Result};
use candle_core::{DType, Device};
use candle_nn::VarBuilder;
use fastembed::{Qwen3Config, Qwen3Model, Qwen3TextEmbedding};
use hf_hub::api::Progress;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokenizers::{PaddingDirection, PaddingParams, PaddingStrategy, TruncationParams};

/// Source of truth for the §9.8 version-hash compute. Exported `pub` so
/// `src/bin/compute_version_hash.rs` imports it without copy-paste drift.
///
/// PRESERVED VERBATIM from prior ollama embedder, byte-for-byte unchanged
/// per Pitfall 4 / locked decision #5. The trailing space after "Query:" is
/// part of the empirical config that produced poc.db's 67.9% B1-B7 baseline.
/// Dropping the space invalidates poc.db AND breaks the cosine equivalence
/// gate (Plan 1 Task 3).
pub const QUERY_INSTRUCT: &str =
    "Instruct: Given a natural language code search query, retrieve the most relevant code symbol from a TypeScript codebase\nQuery: ";

/// Model repo id on Hugging Face Hub. First-run downloads ~1.2 GB to
/// `~/.cache/huggingface/`. Subsequent loads are mmap-only.
const MODEL_REPO: &str = "Qwen/Qwen3-Embedding-0.6B";

/// HF Hub revision (commit SHA) pinned per ARCHITECTURE.md §9.8 row dated
/// 2026-04-28. This is the SHA whose model produced poc.db's 67.9% B1-B7
/// baseline (recovered from local cache `refs/main` 2026-04-26 14:48 timestamp
/// matching Phase 03.6 closure). Bumping this constant requires a new
/// §9.8 history row + post-pin REQ-10 +/-2pp delta verification.
///
/// **The pin is functional, not decorative.** v2 (Phase 4 first slice) calls
/// `Qwen3TextEmbedding::new(model, tokenizer)` with a `Qwen3Model` built from
/// LOCAL safetensors at `snapshot_dir()/model.safetensors`. We bypass
/// `Qwen3TextEmbedding::from_hf` entirely, because fastembed-5.13.3's
/// `from_hf` re-fetches `config.json` from default `main` regardless of
/// what path is passed (qwen3.rs:1014). See Plan 04-01 v2 G-05 record for
/// the full source-code evidence and rationale.
const QWEN3_REVISION: &str = "97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3";

/// Tokenizer max sequence length. Symbols rarely exceed 1k tokens
/// (RESEARCH.md §"Security Domain"); 8192 leaves headroom for long
/// docstrings + signature combos without truncation drift.
const MAX_LEN: usize = 8192;

/// hf-hub Progress callback emitting `eprintln!` lines at 25% milestones.
/// Used by snapshot_dir() for the largest file (model.safetensors ~600 MB).
/// R2.c (promoted in v2 per REVIEWS.md HIGH#5; D-06 corrected): hf-hub 0.5.0
/// `download_with_progress<P: Progress>` IS the caller-side callback hook
/// (sync.rs:795). v1 incorrectly claimed no callback exists.
struct DownloadProgress {
    filename: String,
    current: usize,
    total: usize,
    last_milestone_pct: u32,
}

impl DownloadProgress {
    fn new(filename: &str) -> Self {
        Self {
            filename: filename.to_string(),
            current: 0,
            total: 0,
            last_milestone_pct: 0,
        }
    }
}

impl Progress for DownloadProgress {
    fn init(&mut self, size: usize, _filename: &str) {
        self.total = size;
        self.current = 0;
        self.last_milestone_pct = 0;
        eprintln!(
            "[embedder] downloading {} ({} MB)",
            self.filename,
            self.total / 1_048_576
        );
    }

    fn update(&mut self, size: usize) {
        self.current = self.current.saturating_add(size);
        if self.total == 0 {
            return;
        }
        let pct = ((self.current as f64 / self.total as f64) * 100.0) as u32;
        // Emit at every 25% milestone -- guarantees >=2 lines for files >=25%
        // (model.safetensors at 600 MB ALWAYS hits 25/50/75/100).
        if pct >= self.last_milestone_pct + 25 && pct < 100 {
            self.last_milestone_pct = pct;
            eprintln!(
                "[embedder] downloading model: {}% ({} / {} MB)",
                pct,
                self.current / 1_048_576,
                self.total / 1_048_576
            );
        }
    }

    fn finish(&mut self) {
        eprintln!(
            "[embedder] {} download complete ({} MB)",
            self.filename,
            self.current / 1_048_576
        );
    }
}

#[derive(Copy, Clone)]
pub enum Role {
    Query,
    Passage,
}

pub struct Embedder {
    inner: OnceLock<Qwen3TextEmbedding>,
}

impl Embedder {
    pub fn new() -> Self {
        Self {
            inner: OnceLock::new(),
        }
    }

    /// Fetch all 9 files of the pinned snapshot via hf-hub::Repo::with_revision.
    /// Uses download_with_progress for the largest file (model.safetensors) so
    /// R2.c progress milestones are emitted (>=2 percentage lines per E2E).
    ///
    /// **M1 (REVIEWS.md):** every fetched path validated to live under the
    /// same `snapshots/<QWEN3_REVISION>/` root. Catches cache-corruption /
    /// hf-hub layout drift early.
    fn snapshot_dir() -> Result<PathBuf> {
        use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};
        let api = ApiBuilder::new()
            .build()
            .map_err(|e| anyhow::anyhow!("hf-hub ApiBuilder: {}", e))?;
        let repo = api.repo(Repo::with_revision(
            MODEL_REPO.to_string(),
            RepoType::Model,
            QWEN3_REVISION.to_string(),
        ));
        // Enumerated file list verified against cached snapshot
        // ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/<sha>/
        // 3 of 9 are functionally required (config.json, tokenizer.json,
        // model.safetensors); other 6 fetched for cache-completeness.
        const FILES: &[&str] = &[
            "config.json",
            "config_sentence_transformers.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "vocab.json",
            "merges.txt",
            "modules.json",
            "1_Pooling/config.json",
            // model.safetensors LAST -- uses progress callback below
        ];
        let mut fetched: Vec<PathBuf> = Vec::with_capacity(FILES.len() + 1);
        for f in FILES {
            let p = repo
                .get(f)
                .map_err(|e| anyhow::anyhow!("hf-hub fetch {}: {}", f, e))?;
            fetched.push(p);
        }
        // model.safetensors: cache-first via repo.get, fallback to
        // download_with_progress on cache miss.
        //
        // Phase 4 follow-up (04-04 RCA correction): the prior unconditional
        // download_with_progress(...) call walls deterministically at
        // ~49% / 567 MB on this Windows host even when the complete 1.2 GB
        // blob already exists in cache (verified 2026-04-28 across 6 runs:
        // 4 in 04-03 harness + 2 standalone eval probes). repo.get is
        // cache-aware and returns the snapshot symlink without re-fetching
        // when cache is intact.
        //
        // Trade-off accepted: fresh-install cold-cache path still uses
        // download_with_progress for model.safetensors (the dominant 1.2 GB
        // blob -- R2.c milestone UX preserved on this branch). The 8 smaller
        // files (config/tokenizer/vocab/etc) always go through repo.get above
        // and download silently when cache-missing -- no progress UX for them
        // even on cold cache, since their combined size is < 16 MB and
        // completes in seconds without per-file feedback. Cache-hit path for
        // model.safetensors is silent. The grep-level R2.c gate (commit
        // fc4df3a) is unaffected since DownloadProgress impl is still
        // referenced by the fallback branch. M1 path validation below still
        // asserts the returned path lives under the pinned SHA snapshot
        // root, catching any cache-layout drift.
        //
        // Lifts from DEFERRED → runtime-PASS: EVAL_NO_REGRESSION (byte-
        // identical 30/30 vs Phase 03.6 baseline) + R1.d offline-mode
        // (HF_HUB_OFFLINE=1 + complete cache -> 6.85s clean run, no network).
        // R1.c reload (delete snapshot + redownload yields same SHA) remains
        // DEFERRED -- still requires the fresh-download path which is owned
        // by the First-run UX P1 cluster (PROJECT.md line 71), not this slice.
        let safetensors_path = match repo.get("model.safetensors") {
            Ok(p) => {
                // Guard against zero-byte blob symlinks (rare, but covers
                // half-extracted cache states).
                if std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false) {
                    p
                } else {
                    // unwrap_or(false) folds 3 cases into 1 fallback: zero-byte
                    // file, broken symlink, and IO/permission errors. All 3
                    // are recoverable by re-downloading; the log line names
                    // the dominant case but covers all three.
                    eprintln!(
                        "[embedder] cache hit but blob unreadable or empty, falling back to download_with_progress"
                    );
                    repo.download_with_progress(
                        "model.safetensors",
                        DownloadProgress::new("model.safetensors"),
                    )
                    .map_err(|e| anyhow::anyhow!("hf-hub fetch model.safetensors: {}", e))?
                }
            }
            Err(e_get) => {
                eprintln!(
                    "[embedder] cache-first lookup failed ({}), falling back to download_with_progress",
                    e_get
                );
                repo.download_with_progress(
                    "model.safetensors",
                    DownloadProgress::new("model.safetensors"),
                )
                .map_err(|e| anyhow::anyhow!("hf-hub fetch model.safetensors: {}", e))?
            }
        };
        fetched.push(safetensors_path);

        // Compute snapshot root from the FIRST top-level file (config.json):
        // hf-hub layout is .../snapshots/<sha>/<filename> for top-level,
        // .../snapshots/<sha>/<subdir>/<filename> for nested. config.json is
        // top-level so its parent IS the snapshot root.
        let snapshot_root = fetched[0]
            .parent()
            .ok_or_else(|| anyhow::anyhow!("snapshot path missing parent: {:?}", fetched[0]))?
            .to_path_buf();

        // M1: validate snapshot root ends with QWEN3_REVISION
        let root_str = snapshot_root.to_string_lossy();
        anyhow::ensure!(
            root_str.contains(QWEN3_REVISION),
            "snapshot root {:?} does not contain pinned SHA {} -- hf-hub may be using non-pinned cache",
            snapshot_root,
            QWEN3_REVISION
        );
        // M1: validate every fetched path is under the same snapshot root
        for path in &fetched {
            anyhow::ensure!(
                path.starts_with(&snapshot_root),
                "fetched path {:?} not under snapshot root {:?} -- cache layout drift",
                path,
                snapshot_root
            );
        }
        Ok(snapshot_root)
    }

    /// Build Qwen3TextEmbedding from local pinned snapshot -- bypasses
    /// fastembed's `from_hf` (which re-fetches config.json from default `main`
    /// per qwen3.rs:1014, making any SHA pin decorative). v2 path (a) per
    /// REVIEWS.md HIGH#1: replicates the body of fastembed's own from_hf
    /// (qwen3.rs:1010-1077) but uses LOCAL files only.
    fn load_pinned_model() -> Result<Qwen3TextEmbedding> {
        let snapshot = Self::snapshot_dir()?;
        let device = Device::Cpu;

        // 1. Parse config.json into Qwen3Config (re-exported public Deserialize).
        let cfg_bytes = std::fs::read(snapshot.join("config.json"))
            .with_context(|| format!("read {}", snapshot.join("config.json").display()))?;
        let cfg: Qwen3Config = serde_json::from_slice(&cfg_bytes)
            .context("parse config.json as Qwen3Config")?;

        // 2. Build VarBuilder from local model.safetensors (mmap).
        //    SAFETY: mmap requires the file to remain valid for the duration
        //    of VarBuilder usage. We hold `weight_files` in scope through
        //    Qwen3Model::new (which copies tensors into the model).
        let weight_files = vec![snapshot.join("model.safetensors")];
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&weight_files, DType::F32, &device)
        }
        .context("VarBuilder::from_mmaped_safetensors")?;

        // 3. Build Qwen3Model. weight_prefix is None for plain Qwen3-Embedding
        //    (NOT VL -- see qwen3.rs:67 in fastembed-5.13.3 where the config
        //    deserialize-as-Config branch returns (cfg, None)).
        let model = Qwen3Model::new(cfg, vb).context("Qwen3Model::new")?;

        // 4. Load tokenizer from local file + apply LEFT padding + truncation.
        //    Replicates fastembed/src/models/qwen3.rs:1057-1077 verbatim.
        let mut tokenizer =
            tokenizers::Tokenizer::from_file(snapshot.join("tokenizer.json"))
                .map_err(|e| anyhow::anyhow!("Tokenizer::from_file: {}", e))?;
        let _ = tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            direction: PaddingDirection::Left,
            ..Default::default()
        }));
        let _ = tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_LEN,
                ..Default::default()
            }))
            .map_err(|e| anyhow::anyhow!("Tokenizer::with_truncation: {}", e))?;

        // 5. Wrap in Qwen3TextEmbedding via public ::new constructor.
        Ok(Qwen3TextEmbedding::new(model, tokenizer))
    }

    fn ensure_loaded(&self) -> Result<&Qwen3TextEmbedding> {
        // M5 (REVIEWS.md): use get() fast-path first, then attempt load+set.
        // OnceLock::set is atomic: if two threads race here, only one wins
        // the set (the other's set silently returns Err(model) and is dropped,
        // which is safe since Qwen3TextEmbedding holds no external handles).
        // get_or_try_init would be cleaner but it requires the once_cell_try
        // nightly feature (rust-lang/rust#109737); use this stable equivalent.
        // Note: in the rare concurrent-first-call scenario, the losing thread
        // loads the model redundantly but discards it -- idempotent and safe.
        if let Some(m) = self.inner.get() {
            return Ok(m);
        }
        // R2.a (SPEC): start prompt mentions "first-run download",
        // "huggingface.co" URL, and ETA wording ("30-60s on broadband").
        // Single-line grep target for `first-run download|huggingface\.co`.
        eprintln!(
            "[embedder] first-run download from huggingface.co/{}@{} (~1.2 GB, 30-60s on broadband)",
            MODEL_REPO,
            &QWEN3_REVISION[..12]
        );
        eprintln!(
            "[embedder] subsequent loads are mmap-only from ~/.cache/huggingface/"
        );
        // R2.b (SPEC): on failure, emit a recovery-link line BEFORE
        // bubbling the error up. Both eprintln (for stderr UX) and
        // .with_context (for error string) reference the recovery doc.
        let model = Self::load_pinned_model().map_err(|e| {
            eprintln!(
                "[embedder] download failed. See docs/embedder-offline-bootstrap.md for offline / Clash-down recovery (HF_HOME pre-seeding, HF_HUB_OFFLINE mode)."
            );
            e.context(
                "model load failed -- check internet to huggingface.co; see docs/embedder-offline-bootstrap.md",
            )
        })?;
        // Discard the model if another thread already set it (OnceLock::set
        // returns Err(value) if the cell is already initialized).
        let _ = self.inner.set(model);
        Ok(self.inner.get().expect("OnceLock just set or already set by concurrent thread"))
    }

    /// Public retry-wrapped embed. Signature byte-identical to prior ollama
    /// path so callers (main.rs / search.rs / server.rs) need no changes.
    /// Retry preserved as defensive-only per ARCH §9.9 D-W9 row 1: in-process
    /// inference doesn't have ollama's HTTP-burst failure mode, so the 5-attempt
    /// retry should rarely fire in practice.
    pub fn embed(&self, text: &str, role: Role) -> Result<Vec<f32>> {
        const MAX_ATTEMPTS: u32 = 5;
        const BASE_DELAY_MS: u64 = 250;
        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 0..MAX_ATTEMPTS {
            match self.embed_once(text, role) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    last_err = Some(e);
                    if attempt + 1 < MAX_ATTEMPTS {
                        let delay = BASE_DELAY_MS << attempt;
                        std::thread::sleep(std::time::Duration::from_millis(delay));
                    }
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Query path retry budget: 2 attempts, 250ms total. Single failure
    /// surfaces as Err in <1s, NOT 7.75s like the shared Index wrapper.
    ///
    /// Per ARCH §9.9 D-W9: caller-policy split — Query callers (interactive
    /// UX) need fast-fail, Index callers (long indexing loops) need the
    /// 5-attempt wrapper. Same `embed_once` primitive, two retry policies.
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        const QUERY_MAX_ATTEMPTS: u32 = 2;
        const QUERY_DELAY_MS: u64 = 250;
        let mut last_err: Option<anyhow::Error> = None;
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

    fn embed_once(&self, text: &str, role: Role) -> Result<Vec<f32>> {
        // R4.b / R5.b fault injection (Plan 04-02 v2 per REVIEWS.md HIGH#4).
        // Env-gated synthetic failure for E2E synthetic-failure tests in
        // Plan 04-03. Operator MUST `unset CODENEXUS_EMBED_FAIL` before
        // production deployment (test-only feature; threat-modeled in
        // 04-02-PLAN.md T-04-15).
        //
        // Modes:
        //   always   -> every call returns synthetic Err
        //   once     -> first call returns Err; subsequent succeed
        //   after_N  -> first N calls succeed, then every call returns Err
        if let Ok(mode) = std::env::var("CODENEXUS_EMBED_FAIL") {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static FAULT_COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = FAULT_COUNTER.fetch_add(1, Ordering::Relaxed);
            let should_fail = match mode.as_str() {
                "always" => true,
                "once" => n == 0,
                s if s.starts_with("after_") => {
                    if let Ok(threshold) = s.trim_start_matches("after_").parse::<usize>() {
                        n >= threshold
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if should_fail {
                return Err(anyhow::anyhow!(
                    "CODENEXUS_EMBED_FAIL={}: synthetic embed failure (n={})",
                    mode,
                    n
                ));
            }
        }
        let model = self.ensure_loaded()?;
        let prompt: String = match role {
            Role::Query => format!("{}{}", QUERY_INSTRUCT, text),
            Role::Passage => text.to_string(),
        };
        // fastembed batch API: takes &[S: AsRef<str>], returns Vec<Vec<f32>>
        // already L2-normalized + last-token-pooled (left padding under the
        // hood matches the official Qwen3-Embedding reference impl).
        let out = model
            .embed(&[prompt.as_str()])
            .map_err(|e| anyhow::anyhow!("fastembed embed: {}", e))?;
        let v = out
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("fastembed returned empty result"))?;
        anyhow::ensure!(v.len() == 1024, "expected dim=1024, got {}", v.len());
        Ok(v)
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = (na.sqrt() * nb.sqrt()).max(1e-12);
    dot / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: lazy load triggers on first embed call; subsequent cached.
    #[test]
    fn loads_model() {
        let e = Embedder::new();
        let v = e.embed("hello", Role::Query).expect("first embed should succeed");
        assert!(!v.is_empty());
    }

    /// Test 2: dim must be 1024 (NOT 768 / 384 / etc).
    #[test]
    fn dim_is_1024() {
        let e = Embedder::new();
        let v = e.embed("hello world", Role::Query).expect("embed");
        assert_eq!(v.len(), 1024, "expected 1024-dim, got {}", v.len());
    }

    /// Test 3: identical input -> cosine ~ 1.0 (allow tolerance for fp accumulation).
    #[test]
    fn deterministic() {
        let e = Embedder::new();
        let v1 = e.embed("test text", Role::Query).expect("embed1");
        let v2 = e.embed("test text", Role::Query).expect("embed2");
        let c = cosine(&v1, &v2);
        assert!(c > 0.99999, "expected ~1.0 cosine for identical input, got {}", c);
    }

    /// Test 4: signature `pub fn embed(&self, &str, Role) -> Result<Vec<f32>>`
    /// must remain byte-identical (caller-side compile sanity). Plus retry
    /// wrapper preserved per ARCH §9.9 D-W9 row 1 (defensive-only).
    #[test]
    fn retry_preserved_signature() {
        // Compile-time signature pin -- if signature changes, this fails to compile.
        let _: fn(&Embedder, &str, Role) -> Result<Vec<f32>> = Embedder::embed;
        let e = Embedder::new();
        let v = e.embed("retry sanity", Role::Passage).expect("embed");
        assert_eq!(v.len(), 1024);
    }

    /// Test 5 (R5): embed_query method exists, returns 1024-dim, distinct
    /// from embed() (which uses 5-attempt wrapper). Compile-time signature
    /// pin guards against accidental signature drift.
    #[test]
    fn embed_query_works() {
        let _: fn(&Embedder, &str) -> Result<Vec<f32>> = Embedder::embed_query;
        let e = Embedder::new();
        let v = e.embed_query("query test").expect("embed_query");
        assert_eq!(v.len(), 1024);
    }

    /// Test 6 (R5.b probe): with CODENEXUS_EMBED_FAIL=always, embed_query
    /// returns Err in <1s wall clock (single 250ms sleep + processing).
    /// Full E2E timing test runs in Plan 04-03 against the release binary.
    /// Note: this test sets and unsets the env var; if multiple tests run
    /// in parallel the env var may leak between threads. Run with
    /// `cargo test embed_query_fault_injection -- --test-threads=1` if so.
    #[test]
    fn embed_query_fault_injection() {
        let prev = std::env::var("CODENEXUS_EMBED_FAIL").ok();
        std::env::set_var("CODENEXUS_EMBED_FAIL", "always");
        let e = Embedder::new();
        let start = std::time::Instant::now();
        let result = e.embed_query("fault test");
        let elapsed = start.elapsed();
        // restore env even on assertion failure
        match prev {
            Some(v) => std::env::set_var("CODENEXUS_EMBED_FAIL", v),
            None => std::env::remove_var("CODENEXUS_EMBED_FAIL"),
        }
        assert!(result.is_err(), "expected Err with CODENEXUS_EMBED_FAIL=always");
        assert!(
            elapsed < std::time::Duration::from_millis(900),
            "embed_query budget exceeded: {:?} (expected <900ms = 250ms sleep + headroom)",
            elapsed
        );
    }
}
