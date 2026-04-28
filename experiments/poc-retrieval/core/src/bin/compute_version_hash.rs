// Source-of-truth version-hash compute binary for ARCH §9.8.
//
// Rationale: bash `echo -n "...\nQuery: "` on POSIX / git-bash emits a
// LITERAL backslash-n (2 bytes), NOT a real LF (1 byte 0x0A). The Rust
// const QUERY_INSTRUCT in embedder.rs is a Rust string literal where
// `\n` is parsed at compile time into a real LF. If §9.8 hash were
// computed via bash echo, the committed hash would never match runtime
// behavior, breaking drift detection silently from day one.
//
// This binary imports QUERY_INSTRUCT from embedder.rs (single source of
// truth, via the lib facade in src/lib.rs) and prints
// sha256(model_id|dim|QUERY_INSTRUCT)[:12] hex.
//
// Plan 2 Task 4.2 invokes this binary directly:
//   ./target/release/compute_version_hash
// and pipes stdout into the ARCH §9.8 history table. Determinism is
// validated in Plan 1 task 2.5 acceptance gate (two consecutive runs
// produce byte-identical 12-char hex output).

use codenexus_core::embedder::QUERY_INSTRUCT;
use sha2::{Digest, Sha256};

const MODEL_ID: &str = "Qwen/Qwen3-Embedding-0.6B";
const DIM: u32 = 1024;

fn main() {
    let payload = format!("{}|{}|{}", MODEL_ID, DIM, QUERY_INSTRUCT);
    let mut hasher = Sha256::new();
    hasher.update(payload.as_bytes());
    let digest = hasher.finalize();
    // 6 bytes * 2 hex chars = 12-char prefix per ARCH §9.8 convention.
    let hex: String = digest.iter().take(6).map(|b| format!("{:02x}", b)).collect();
    println!("{}", hex);
}
