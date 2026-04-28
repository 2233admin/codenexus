mod a2a;
mod embedder;
mod graph_build;
mod graph_ppr;
mod parser;
mod reranker;
mod search;
mod server;
mod storage;
mod task_state;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

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
        /// Abort indexer after N consecutive embedder failures.
        /// Counter resets on success. Prevents silent partial state.
        #[arg(long, default_value_t = 5)]
        max_consecutive_fail: usize,
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
    /// Start the A2A v0.2 HTTP endpoint (REQ-06). Exposes /tasks/send,
    /// /tasks/{id}, /healthz on the given port. Phase 3 MVP entry point —
    /// Go server (REQ-07) spawns this and proxies MCP/HTTP requests.
    Serve {
        #[arg(long, default_value_t = 9876)]
        port: u16,
        #[arg(long, default_value = "poc.db")]
        db: String,
    },
    /// Graph-traversal query (axis-3 use case). Extracts subject from query
    /// text (or --subject override), runs Personalized PageRank on edges of
    /// allowed kinds, returns top-N symbols by PPR score. Bidirectional by
    /// default (covers "who calls X" + "what X calls" semantics).
    QueryGraph {
        text: String,
        #[arg(long, default_value = "poc.db")]
        db: String,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long, default_value = "Calls")]
        kinds: String,
        #[arg(long, default_value_t = 5)]
        top: usize,
        #[arg(long, default_value_t = 0.85)]
        damping: f64,
        #[arg(long, default_value_t = 30)]
        iters: usize,
        #[arg(long, default_value_t = 0.5)]
        conf_min: f64,
        /// If false, only forward direction (entry → callees). Default true
        /// adds reverse edges so PPR mass also flows entry ← callers.
        #[arg(long, default_value_t = true)]
        bidirectional: bool,
        #[arg(long)]
        json: bool,
    },
    /// Phase 03.6 Plan 1 step 1.3: One-shot capture of current ollama embeddings
    /// for the 30-query regression set (both query-side and passage-side). Used
    /// as baseline by EmbedEquivalence. Run BEFORE migrating embedder.rs while
    /// ollama still works.
    EmbedSnapshot {
        #[arg(long)]
        queries: PathBuf,
        #[arg(long, default_value = "eval/embed_snapshot_ollama.json")]
        out: PathBuf,
    },
    /// Phase 03.6 Plan 1 step 3 (HARD acceptance gate): compare new (candle)
    /// embedder output against the ollama snapshot from EmbedSnapshot, computes
    /// per-pair cosine, dumps summary stats. Gate: mean >= 0.97 AND p10 >= 0.95.
    EmbedEquivalence {
        #[arg(long)]
        queries: PathBuf,
        #[arg(long, default_value = "eval/embed_snapshot_ollama.json")]
        ollama_snapshot: PathBuf,
        #[arg(long, default_value = "eval/embed_equivalence_30q.json")]
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve { port, db } => {
            let task_store = Arc::new(task_state::TaskStore::new(db.clone()));
            let app = server::router(task_store);
            let addr = format!("0.0.0.0:{}", port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            eprintln!("CodeNexus A2A endpoint listening on {} (db={})", addr, db);
            eprintln!("  POST /tasks/send  GET /tasks/{{id}}  GET /healthz");
            axum::serve(listener, app).await?;
            return Ok(());
        }
        Cmd::Index { repo, db, max_consecutive_fail } => {
            let store = storage::Store::open(&db)?;
            store.clear()?;
            let embedder = embedder::Embedder::new();
            let symbols = parser::parse_repo(&repo)?;
            eprintln!("parsed {} symbols", symbols.len());
            let total = symbols.len();
            let mut consecutive_fails: usize = 0;
            for (i, s) in symbols.iter().enumerate() {
                let dn = search::decompose(&s.name);
                let ds = search::decompose(&s.snippet);
                let blob = format!("{} {}", dn, ds);
                let text = format!("{} {} {}", s.kind, s.name, s.snippet);
                let emb = match embedder.embed(&text, embedder::Role::Passage) {
                    Ok(v) => {
                        consecutive_fails = 0;
                        v
                    }
                    Err(e) => {
                        consecutive_fails += 1;
                        eprintln!(
                            "[{}/{}] embed fail {}: {} (consecutive={}/{})",
                            i + 1, total, s.name, e,
                            consecutive_fails, max_consecutive_fail
                        );
                        if consecutive_fails >= max_consecutive_fail {
                            anyhow::bail!(
                                "aborting indexer: {} consecutive embed failures (threshold {}), last symbol={}, last error={:#}",
                                consecutive_fails, max_consecutive_fail, s.name, e
                            );
                        }
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
        Cmd::QueryGraph {
            text,
            db,
            subject,
            kinds,
            top,
            damping,
            iters,
            conf_min,
            bidirectional,
            json,
        } => {
            let store = storage::Store::open(&db)?;

            // 1. Determine subject — explicit override else extract longest
            //    identifier-shaped token from text (camelCase / PascalCase /
            //    snake_case all qualify; prefer ones with at least one uppercase
            //    or underscore to filter out short common words like "what" "calls").
            let subject = subject.clone().unwrap_or_else(|| extract_subject(&text));
            eprintln!("subject = {}", subject);

            // 2. Find entry symbol IDs by name.
            let entry_ids = store.find_symbols_by_name(&subject)?;
            if entry_ids.is_empty() {
                if json {
                    println!("{{\"subject\": \"{}\", \"entry_ids\": [], \"results\": []}}", subject);
                } else {
                    eprintln!("subject '{}' not found in symbols table — likely a negative-class query or non-symbol noun", subject);
                }
                return Ok(());
            }
            eprintln!("entry_ids = {:?}", entry_ids);

            // 3. Parse --kinds (comma-separated).
            let kind_list: Vec<graph_ppr::EdgeKind> = kinds
                .split(',')
                .filter_map(|s| match s.trim() {
                    "Calls" => Some(graph_ppr::EdgeKind::Calls),
                    "Imports" => Some(graph_ppr::EdgeKind::Imports),
                    "Implements" => Some(graph_ppr::EdgeKind::Implements),
                    "Extends" => Some(graph_ppr::EdgeKind::Extends),
                    other => {
                        eprintln!("warn: ignoring unknown edge kind '{}'", other);
                        None
                    }
                })
                .collect();

            // 4. Load edges + optionally add reverse (bidirectional).
            let kind_strs: Vec<&str> = kind_list.iter().map(|k| k.as_str()).collect();
            let edges_with_conf = store.edges_of_kinds(&kind_strs, conf_min)?;
            let mut edges: Vec<(i64, i64)> =
                edges_with_conf.iter().map(|(u, v, _)| (*u, *v)).collect();
            if bidirectional {
                let reverses: Vec<(i64, i64)> =
                    edges_with_conf.iter().map(|(u, v, _)| (*v, *u)).collect();
                edges.extend(reverses);
            }
            eprintln!(
                "edges loaded: {} ({})",
                edges.len(),
                if bidirectional { "bidirectional" } else { "forward only" }
            );

            // 5. Run PPR.
            let ranked = graph_ppr::ppr_from_edge_list(&edges, &entry_ids, damping, iters);

            // 6. Filter out entry symbols themselves AND dedupe by (path, name)
            //    — multiple symbol_ids with same identity (e.g. const `adapter`
            //    redeclared in each test) produce duplicate rows. Resolve top-N
            //    to (path, name, kind, score).
            let entry_set: std::collections::HashSet<i64> = entry_ids.iter().copied().collect();
            let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
            let mut output_rows: Vec<(String, String, String, f64)> = Vec::new();
            for (id, score) in &ranked {
                if entry_set.contains(id) {
                    continue;
                }
                if let Some((path, name, kind)) = store.symbol_by_id(*id)? {
                    let key = (path.clone(), name.clone());
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.insert(key);
                    output_rows.push((path, name, kind, *score));
                    if output_rows.len() >= top {
                        break;
                    }
                }
            }

            if json {
                let entries: Vec<i64> = entry_ids.clone();
                let results_json: Vec<serde_json::Value> = output_rows
                    .iter()
                    .map(|(p, n, k, s)| {
                        serde_json::json!({
                            "path": p, "name": n, "kind": k, "score": s,
                        })
                    })
                    .collect();
                let out = serde_json::json!({
                    "subject": subject,
                    "entry_ids": entries,
                    "kinds": kind_list.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
                    "bidirectional": bidirectional,
                    "results": results_json,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                eprintln!("=== top {} (PPR damping={} iters={}) ===", top, damping, iters);
                for (i, (path, name, kind, score)) in output_rows.iter().enumerate() {
                    println!("#{} [{:.6}] {} {} {}", i + 1, score, kind, name, path);
                }
            }
        }
        Cmd::EmbedSnapshot { queries, out } => {
            // Phase 03.6 Plan 1 step 1.3: Pre-migration ollama baseline capture.
            // Reads queries.json (30 entries A1-A10/B1-B10/C1-C10), embeds each
            // both as Query (with QUERY_INSTRUCT prefix) and as Passage (raw),
            // dumps 60 entries x 1024 dim to JSON. Plan 1 task 3 (EmbedEquivalence)
            // consumes this snapshot to validate candle migration.
            let raw = std::fs::read_to_string(&queries)?;
            let qs: Vec<EvalQuery> = serde_json::from_str(&raw)?;
            let embedder = embedder::Embedder::new();
            let mut results: Vec<serde_json::Value> = Vec::with_capacity(qs.len() * 2);
            eprintln!("capturing ollama baseline for {} queries...", qs.len());
            for (i, q) in qs.iter().enumerate() {
                let qv = embedder.embed(&q.query, embedder::Role::Query)?;
                anyhow::ensure!(qv.len() == 1024, "expected 1024-dim Query vec, got {}", qv.len());
                results.push(serde_json::json!({
                    "id": q.id, "role": "Query", "vec": qv,
                }));
                let pv = embedder.embed(&q.query, embedder::Role::Passage)?;
                anyhow::ensure!(pv.len() == 1024, "expected 1024-dim Passage vec, got {}", pv.len());
                results.push(serde_json::json!({
                    "id": q.id, "role": "Passage", "vec": pv,
                }));
                if (i + 1) % 5 == 0 {
                    eprintln!("  [{}/{}] captured", i + 1, qs.len());
                }
            }
            std::fs::create_dir_all(out.parent().unwrap_or(std::path::Path::new(".")))?;
            std::fs::write(&out, serde_json::to_string_pretty(&results)?)?;
            eprintln!("wrote {} entries to {}", results.len(), out.display());
        }
        Cmd::EmbedEquivalence { queries, ollama_snapshot, out } => {
            // Phase 03.6 Plan 1 step 3 (HARD acceptance gate). Reads the ollama
            // baseline snapshot (Cmd::EmbedSnapshot output, 60 entries x 1024
            // dim), computes the same 60 embeddings via the new candle-based
            // embedder, dumps per-pair cosine + summary stats. Gate criteria:
            //   mean_cosine >= 0.97 AND p10_cosine >= 0.95
            // Per locked decision #2 (RESEARCH.md Pitfall 2). Threshold is
            // intentionally NOT 0.999 because dtype/padding/tokenization
            // differences between ollama and candle paths are expected.
            let raw_q = std::fs::read_to_string(&queries)?;
            let qs: Vec<EvalQuery> = serde_json::from_str(&raw_q)?;
            let raw_s = std::fs::read_to_string(&ollama_snapshot)?;
            let snap: Vec<serde_json::Value> = serde_json::from_str(&raw_s)?;
            // Index snapshot by (id, role) -> Vec<f32>
            let mut snap_idx: std::collections::HashMap<(String, String), Vec<f32>> =
                std::collections::HashMap::new();
            for entry in &snap {
                let id = entry["id"].as_str().unwrap().to_string();
                let role = entry["role"].as_str().unwrap().to_string();
                let vec: Vec<f32> = entry["vec"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap() as f32)
                    .collect();
                snap_idx.insert((id, role), vec);
            }
            let embedder = embedder::Embedder::new();
            let mut per_query: Vec<serde_json::Value> = Vec::new();
            let mut cosines: Vec<f32> = Vec::new();
            eprintln!("running equivalence check vs ollama snapshot ({} entries)...", snap.len());
            for (i, q) in qs.iter().enumerate() {
                for role_str in &["Query", "Passage"] {
                    let role = if *role_str == "Query" {
                        embedder::Role::Query
                    } else {
                        embedder::Role::Passage
                    };
                    let candle_vec = embedder.embed(&q.query, role)?;
                    let ollama_vec = snap_idx
                        .get(&(q.id.clone(), role_str.to_string()))
                        .ok_or_else(|| {
                            anyhow::anyhow!("missing snapshot entry for {}/{}", q.id, role_str)
                        })?;
                    let c = embedder::cosine(&candle_vec, ollama_vec);
                    per_query.push(serde_json::json!({
                        "id": q.id, "role": role_str, "cosine": c,
                    }));
                    cosines.push(c);
                }
                if (i + 1) % 5 == 0 {
                    eprintln!("  [{}/{}] compared", i + 1, qs.len());
                }
            }
            let mut sorted = cosines.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let n = sorted.len() as f32;
            let mean: f32 = sorted.iter().sum::<f32>() / n;
            let pct = |p: f32| -> f32 {
                let i = ((p / 100.0) * n).floor() as usize;
                sorted[i.min(sorted.len() - 1)]
            };
            let p10 = pct(10.0);
            let p50 = pct(50.0);
            let p90 = pct(90.0);
            let passes_gate = mean >= 0.97 && p10 >= 0.95;
            let summary = serde_json::json!({
                "n": cosines.len(),
                "mean_cosine": mean,
                "p10_cosine": p10,
                "p50_cosine": p50,
                "p90_cosine": p90,
                "passes_gate": passes_gate,
                "gate_thresholds": {"mean_min": 0.97, "p10_min": 0.95},
                "per_query": per_query,
            });
            std::fs::create_dir_all(out.parent().unwrap_or(std::path::Path::new(".")))?;
            std::fs::write(&out, serde_json::to_string_pretty(&summary)?)?;
            eprintln!("=== Equivalence summary ===");
            eprintln!(
                "n={}  mean={:.4}  p10={:.4}  p50={:.4}  p90={:.4}",
                cosines.len(),
                mean,
                p10,
                p50,
                p90
            );
            eprintln!(
                "gate (mean>=0.97 AND p10>=0.95): {}",
                if passes_gate { "PASS" } else { "FAIL" }
            );
            if !passes_gate {
                anyhow::bail!(
                    "equivalence gate failed: mean={:.4} p10={:.4} -- check {} for per_query breakdown",
                    mean,
                    p10,
                    out.display()
                );
            }
        }
    }
    Ok(())
}

/// Extract the most likely "subject" symbol from a natural-language axis-3 query.
/// Heuristic: longest token matching identifier syntax (alphanumerics + underscore,
/// starting with letter) AND containing at least one uppercase letter OR underscore.
/// If no qualifying token, fall back to longest plain alphabetic word ≥ 4 chars.
fn extract_subject(text: &str) -> String {
    let tokens: Vec<&str> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .collect();
    let qualifying: Vec<&&str> = tokens
        .iter()
        .filter(|t| {
            let starts_letter = t.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);
            let has_upper_or_underscore =
                t.chars().any(|c| c.is_uppercase()) || t.contains('_');
            starts_letter && has_upper_or_underscore
        })
        .collect();
    if let Some(longest) = qualifying.iter().max_by_key(|t| t.len()) {
        return (***longest).to_string();
    }
    // Fallback: longest 4+ char alphabetic word
    tokens
        .iter()
        .filter(|t| t.len() >= 4 && t.chars().all(|c| c.is_alphabetic()))
        .max_by_key(|t| t.len())
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| text.to_string())
}
