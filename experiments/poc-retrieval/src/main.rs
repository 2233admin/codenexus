mod embedder;
mod parser;
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
        #[arg(long)]
        json: bool,
    },
    Eval {
        #[arg(long)]
        queries: PathBuf,
        #[arg(long, default_value = "poc.db")]
        db: String,
        #[arg(long, default_value = "eval/results.json")]
        out: PathBuf,
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
                let text = format!("{} {} {}", s.kind, s.name, s.snippet);
                let emb = match embedder.embed(&text, embedder::Role::Passage) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[{}/{}] embed fail {}: {}", i + 1, total, s.name, e);
                        continue;
                    }
                };
                store.insert(s, &emb)?;
                if (i + 1) % 50 == 0 {
                    eprintln!("[{}/{}] indexed", i + 1, total);
                }
            }
            eprintln!("done.");
        }
        Cmd::Query { text, db, top, json } => {
            let store = storage::Store::open(&db)?;
            let embedder = embedder::Embedder::new();
            let hits = search::search(&store, &embedder, &text, top)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else {
                for (i, h) in hits.iter().enumerate() {
                    println!(
                        "#{} [{:.3}] {} {} {}:{}-{}",
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
        Cmd::Eval { queries, db, out } => {
            let raw = std::fs::read_to_string(&queries)?;
            let qs: Vec<EvalQuery> = serde_json::from_str(&raw)?;
            let store = storage::Store::open(&db)?;
            let embedder = embedder::Embedder::new();
            let mut results = Vec::new();
            for q in qs {
                let hits = search::search(&store, &embedder, &q.query, 5)?;
                let top5: Vec<String> = hits
                    .iter()
                    .map(|h| format!("{}:{}", h.symbol.path, h.symbol.name))
                    .collect();
                let matches = |h: &search::Hit| -> bool {
                    let p = h.symbol.path.to_lowercase();
                    let n = h.symbol.name.to_lowercase();
                    q.expected_paths.iter().any(|ep| {
                        let e = ep.to_lowercase();
                        p.contains(&e) || n == e || n.contains(&e)
                    })
                };
                let p = if q.negative {
                    if hits.is_empty() || hits[0].rrf_score < 0.025 {
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
                results.push(EvalResult {
                    id: q.id,
                    axis: q.axis,
                    query: q.query,
                    negative: q.negative,
                    top5,
                    precision_at_5: p,
                    notes: format!("top1_rrf={:.4}", top1_rrf),
                });
            }
            let by_axis: std::collections::BTreeMap<u8, (f32, usize)> =
                results.iter().fold(Default::default(), |mut acc, r| {
                    let e = acc.entry(r.axis).or_insert((0.0, 0));
                    e.0 += r.precision_at_5;
                    e.1 += 1;
                    acc
                });
            eprintln!("\n=== axis precision ===");
            for (a, (sum, n)) in &by_axis {
                eprintln!("axis {}: {:.1}% (n={})", a, sum / *n as f32 * 100.0, n);
            }
            let total: f32 = results.iter().map(|r| r.precision_at_5).sum();
            eprintln!("overall: {:.1}% (n={})", total / results.len() as f32 * 100.0, results.len());
            std::fs::create_dir_all(out.parent().unwrap_or(std::path::Path::new(".")))?;
            std::fs::write(&out, serde_json::to_string_pretty(&results)?)?;
            eprintln!("wrote {}", out.display());
        }
    }
    Ok(())
}
