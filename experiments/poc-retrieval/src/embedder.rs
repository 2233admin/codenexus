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
// Locked decisions:
//   - F32 weights (Phase 4 will explore F16). Locked decision #4.
//   - QUERY_INSTRUCT byte-identical incl trailing space. Locked decision #5.
//   - CPU-only target (cuda/metal feature flags Phase 4+). Locked decision #6.

use anyhow::{Context, Result};
use candle_core::{DType, Device};
use fastembed::Qwen3TextEmbedding;
use std::sync::OnceLock;

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

/// Tokenizer max sequence length. Symbols rarely exceed 1k tokens
/// (RESEARCH.md §"Security Domain"); 8192 leaves headroom for long
/// docstrings + signature combos without truncation drift.
const MAX_LEN: usize = 8192;

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

    fn ensure_loaded(&self) -> Result<&Qwen3TextEmbedding> {
        if let Some(m) = self.inner.get() {
            return Ok(m);
        }
        let device = Device::Cpu;
        eprintln!(
            "[embedder] loading {} (first-run downloads ~1.2 GB to HF cache; this can take 30-60s on first call) ...",
            MODEL_REPO
        );
        let model = Qwen3TextEmbedding::from_hf(MODEL_REPO, &device, DType::F32, MAX_LEN)
            .map_err(|e| anyhow::anyhow!("Qwen3TextEmbedding::from_hf {}: {}", MODEL_REPO, e))
            .context("model download failed -- check internet to huggingface.co")?;
        let _ = self.inner.set(model);
        Ok(self.inner.get().unwrap())
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

    fn embed_once(&self, text: &str, role: Role) -> Result<Vec<f32>> {
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
}
