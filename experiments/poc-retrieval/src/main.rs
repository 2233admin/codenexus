mod embedder;
mod graph_build;
mod graph_ppr;
mod parser;
mod reranker;
mod search;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "CodeNexus retrieval POC")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Index {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "poc.db")]
        db: String,
    },
    Query {
        text: String,
        #[arg(long, default_value = "poc.db")]
        db: String,
        #[arg(long, default_value_t = 5)]
        top: usize,
        #[arg(long, default_value_t = 0.5)]
        alpha: f32,
        #[arg(long)]
        rerank: bool,
        #[arg(long)]
        json: bool,
    },
    Eval {
        #[arg(long)]
        queries: PathBuf,
        #[arg(long, default_value = "poc.db")]
        db: String,
        #[arg(long, default_value_t = 0.5)]
        alpha: f32,
        #[arg(long)]
        rerank: bool,
        #[arg(long, default_value = "eval/results.json")]
        out: PathBuf,
    },
    BuildGraph {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "poc.db")]
        db: String,
    },
    DumpEdges {
        #[arg(long, default_value = "poc.db")]
        db: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(serde::Deserialize, serde::Serialize)]
struct EvalQuery {
    id: String,
    axis: u8,
    query: String,
    expected_paths: Vec<String>,
    #[serde(default)]
    negative: bool,
}

#[derive(serde::Serialize)]
struct EvalResult {
    id: String,
    axis: u8,
    query: String,
    negative: bool,
    top5: Vec<String>,
    precision_at_5: f32,
    notes: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Index { repo, db } => {
            let store = storage::Store::open(&db)?;
            store.clear()?;
            let embedder = embedder::Embedder::new();
            let symbols = parser::parse_repo(&repo)?;
            eprintln!("parsed {} symbols", symbols.len());
            let total = symbols.len();
            for (i, s) in symbols.iter().enumerate() {
                let dn = search::decompose(&s.name);
                let ds = search::decompose(&s.snippet);
                let blob = format!("{} {}", dn, ds);
                let text = format!("{} {} {}", s.kind, s.name, s.snippet);
                let emb = match embedder.embed(&text, embedder::Role::Passage) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[{}/{}] embed fail {}: {}", i + 1, total, s.name, e);
                        continue;
                    }
                };
                store.insert(s, &blob, &emb)?;
                if (i + 1) % 200 == 0 {
                    eprintln!("[{}/{}] indexed", i + 1, total);
                }
            }
            eprintln!("done.");
        }
        Cmd::Query { text, db, top, alpha, rerank, json } => {
            let store = storage::Store::open(&db)?;
            let embedder = embedder::Embedder::new();
            let rr = if rerank { Some(reranker::Reranker::new()?) } else { None };
            let hits = search::search(&store, &embedder, rr.as_ref(), &text, top, alpha)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else {
                eprintln!("alpha={} (vec_w={:.2}, bm25_w={:.2})", alpha, alpha, 1.0 - alpha);
                for (i, h) in hits.iter().enumerate() {
                    println!(
                        "#{} [{:.4}] {} {} {}:{}-{}",
                        i + 1,
                        h.rrf_score,
                        h.symbol.kind,
                        h.symbol.name,
                        h.symbol.path,
                        h.symbol.start_line,
                        h.symbol.end_line
                    );
                }
            }
        }
        Cmd::Eval { queries, db, alpha, rerank, out } => {
            let raw = std::fs::read_to_string(&queries)?;
            let qs: Vec<EvalQuery> = serde_json::from_str(&raw)?;
            let store = storage::Store::open(&db)?;
            let embedder = embedder::Embedder::new();
            let rr = if rerank { Some(reranker::Reranker::new()?) } else { None };
            let mut results = Vec::new();
            eprintln!("running eval at alpha={} rerank={}", alpha, rerank);
            for q in qs {
                let hits = search::search(&store, &embedder, rr.as_ref(), &q.query, 5, alpha)?;
                if rerank {
                    // Jina free tier RPM limit; 2s spacing keeps us safely under.
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                }
                let top5: Vec<String> = hits
                    .iter()
                    .map(|h| format!("{}:{}", h.symbol.path, h.symbol.name))
                    .collect();
                let matches = |h: &search::Hit| -> bool {
                    let p = h.symbol.path.to_lowercase().replace('\\', "/");
                    let n = h.symbol.name.to_lowercase();
                    q.expected_paths.iter().any(|ep| {
                        let e = ep.to_lowercase().replace('\\', "/");
                        p.contains(&e) || n == e || n.contains(&e)
                    })
                };
                let p = if q.negative {
                    // Negative threshold uses rerank score when active (range ~0..1, threshold 0.15
                    // empirically separates B5-style noise ~0.01-0.05 from positive matches ~0.35+).
                    // Without rerank: scaled to alpha-weighted RRF max 1/(c+1)≈0.0164, threshold 0.012.
                    let top1_score = hits
                        .first()
                        .map(|h| h.rerank_score.unwrap_or(h.rrf_score))
                        .unwrap_or(0.0);
                    // Rerank threshold 0.30 tolerates semantic-near false-positives
                    // (e.g. parseFrontmatter for parseYAMLFrontmatter query) while still
                    // catching true confident-wrongs (resetEpoch for rate-limiting=0.47).
                    let threshold = if rerank { 0.30 } else { 0.012 };
                    if hits.is_empty() || top1_score < threshold {
                        1.0
                    } else {
                        -0.25
                    }
                } else if hits.first().map(matches).unwrap_or(false) {
                    1.0
                } else if hits.iter().take(3).any(matches) {
                    0.5
                } else {
                    0.0
                };
                let top1_rrf = hits.first().map(|h| h.rrf_score).unwrap_or(0.0);
                let top1_rerank = hits.first().and_then(|h| h.rerank_score);
                let notes = match top1_rerank {
                    Some(s) => format!("top1_rrf={:.4} top1_rerank={:.4}", top1_rrf, s),
                    None => format!("top1_rrf={:.4}", top1_rrf),
                };
                results.push(EvalResult {
                    id: q.id,
                    axis: q.axis,
                    query: q.query,
                    negative: q.negative,
                    top5,
                    precision_at_5: p,
                    notes,
                });
            }
            let by_axis: std::collections::BTreeMap<u8, (f32, usize)> =
                results.iter().fold(Default::default(), |mut acc, r| {
                    let e = acc.entry(r.axis).or_insert((0.0, 0));
                    e.0 += r.precision_at_5;
                    e.1 += 1;
                    acc
                });
            eprintln!("\n=== axis precision (alpha={}) ===", alpha);
            for (a, (sum, n)) in &by_axis {
                eprintln!("axis {}: {:.1}% (n={})", a, sum / *n as f32 * 100.0, n);
            }
            let total: f32 = results.iter().map(|r| r.precision_at_5).sum();
            eprintln!("overall: {:.1}% (n={})", total / results.len() as f32 * 100.0, results.len());
            std::fs::create_dir_all(out.parent().unwrap_or(std::path::Path::new(".")))?;
            std::fs::write(&out, serde_json::to_string_pretty(&results)?)?;
            eprintln!("wrote {}", out.display());
        }
        Cmd::BuildGraph { repo, db } => {
            let store = storage::Store::open(&db)?;
            let mut builder = graph_build::EdgeBuilder::new(&store, repo)?;
            let stats = builder.build_all()?;
            eprintln!(
                "Calls: {}, Imports: {}, Implements: {}, Extends: {}, unresolved: {}",
                stats.calls, stats.imports, stats.implements, stats.extends, stats.unresolved
            );
            let totals = store.count_edges_by_kind()?;
            eprintln!("=== final edge counts ===");
            for (k, n) in totals {
                eprintln!("  {}: {}", k, n);
            }
        }
        Cmd::DumpEdges { db, kind, limit } => {
            let store = storage::Store::open(&db)?;
            let rows = store.dump_edges_join(kind.as_deref(), limit)?;
            for (fp, fname, k, tp, tname) in rows {
                println!("{}\t{}\t{}\t{}\t{}", fp, fname, k, tp, tname);
            }
        }
    }
    Ok(())
}
