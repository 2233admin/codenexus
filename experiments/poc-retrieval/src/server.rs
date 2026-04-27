//! axum HTTP server exposing the A2A v0.2 endpoint (REQ-06).
//!
//! Routes:
//!   POST /tasks/send  — accepts OperationRequest, spawns worker, returns Task{Submitted}
//!   GET  /tasks/{id}  — returns Task or 404
//!   GET  /healthz     — process liveness, returns "ok"
//!
//! All DB-touching ops are wrapped in `tokio::task::spawn_blocking` because
//! rusqlite is sync. Store is opened per-call (cheap ~1ms) — no shared
//! Connection across async handlers.

use std::collections::{HashMap, HashSet};
use std::path::Path as FsPath;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::a2a::{
    CallerView, HitView, OperationRequest, OperationResponse, SymbolView, Task, TaskSendBody,
};
use crate::embedder;
use crate::graph_build;
use crate::graph_ppr;
use crate::parser;
use crate::reranker;
use crate::search;
use crate::storage::Store;
use crate::task_state::TaskStore;

pub fn router(state: Arc<TaskStore>) -> Router {
    Router::new()
        .route("/tasks/send", post(task_send))
        .route("/tasks/:id", get(task_get))
        .route("/healthz", get(healthz))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn task_send(
    State(store): State<Arc<TaskStore>>,
    Json(body): Json<TaskSendBody>,
) -> Result<Json<Task>, StatusCode> {
    let task = store.submit(body.operation.clone());
    let task_id = task.id;
    let store_for_worker = store.clone();
    let op = body.operation;

    tokio::spawn(async move {
        store_for_worker.mark_working(&task_id);
        let db_path = store_for_worker.db_path().to_string();
        let result = tokio::task::spawn_blocking(move || dispatch(db_path, op)).await;
        match result {
            Ok(Ok(resp)) => store_for_worker.complete(&task_id, resp),
            Ok(Err(e)) => store_for_worker.fail(&task_id, format!("op error: {:#}", e)),
            Err(e) => store_for_worker.fail(&task_id, format!("worker join error: {}", e)),
        }
    });

    Ok(Json(task))
}

async fn task_get(
    State(store): State<Arc<TaskStore>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Task>, StatusCode> {
    store.get(&id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// Synchronous dispatcher — runs inside spawn_blocking. Errors bubble up
/// to task_send's match arm which marks task Failed with the error string.
fn dispatch(db_path: String, op: OperationRequest) -> anyhow::Result<OperationResponse> {
    match op {
        OperationRequest::Query {
            text,
            top,
            alpha,
            rerank,
        } => {
            let store = Store::open(&db_path).context("open db")?;
            let embedder = embedder::Embedder::new();
            let rr = if rerank {
                Some(reranker::Reranker::new().context("rerank init (JINA_API_KEY?)")?)
            } else {
                None
            };
            let hits = search::search(&store, &embedder, rr.as_ref(), &text, top, alpha)?;
            let hit_views: Vec<HitView> = hits
                .iter()
                .map(|h| HitView {
                    path: h.symbol.path.clone(),
                    name: h.symbol.name.clone(),
                    kind: h.symbol.kind.clone(),
                    start_line: h.symbol.start_line,
                    end_line: h.symbol.end_line,
                    score: h.rerank_score.unwrap_or(h.rrf_score),
                })
                .collect();
            Ok(OperationResponse::Query { hits: hit_views })
        }

        OperationRequest::GetSymbol { id } => {
            let store = Store::open(&db_path).context("open db")?;
            let view = store
                .symbol_by_id(id)?
                .map(|(path, name, kind)| SymbolView { id, path, name, kind });
            Ok(OperationResponse::GetSymbol { symbol: view })
        }

        OperationRequest::ListCallers { name, top } => {
            let store = Store::open(&db_path).context("open db")?;
            let entry_ids = store.find_symbols_by_name(&name)?;
            if entry_ids.is_empty() {
                return Ok(OperationResponse::ListCallers { callers: vec![] });
            }
            // Reverse Calls edges: PPR from target finds incoming callers.
            // ARCHITECTURE.md §9.7 confidence_min default = 0.5; hardcoded here
            // since list_callers is the only consumer and the magic number has
            // a single canonical home (the spec). Phase 4: lift to a const if a
            // second consumer appears.
            let edges_with_conf = store.edges_of_kinds(&["Calls"], 0.5)?;
            let edges: Vec<(i64, i64)> = edges_with_conf.iter().map(|(u, v, _)| (*v, *u)).collect();
            // Edge-confidence map keyed by (caller_id, target_id), reversed
            // direction matches the `edges` vec above. Fold-take-max so when
            // multiple Calls edges exist between the same pair we surface the
            // strongest evidence — never silently overwrite. Phase 4 Leiden
            // community detection can read this directly as edge weight.
            let edge_conf: HashMap<(i64, i64), f64> = edges_with_conf.iter().fold(
                HashMap::new(),
                |mut map, (u, v, c)| {
                    let entry = map.entry((*v, *u)).or_insert(0.0);
                    if *c > *entry {
                        *entry = *c;
                    }
                    map
                },
            );
            let ranked = graph_ppr::ppr_from_edge_list(&edges, &entry_ids, 0.85, 30);
            let entry_set: HashSet<i64> = entry_ids.iter().copied().collect();
            let mut seen: HashSet<(String, String)> = HashSet::new();
            let mut callers: Vec<CallerView> = Vec::new();
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
                    // Highest confidence over all entry_ids targets — caller
                    // may PPR-rank via multiple targets simultaneously.
                    let confidence = entry_ids
                        .iter()
                        .filter_map(|t| edge_conf.get(&(*id, *t)).copied())
                        .fold(0.0_f64, f64::max);
                    callers.push(CallerView {
                        path,
                        name,
                        kind,
                        ppr_score: *score,
                        confidence,
                    });
                    if callers.len() >= top {
                        break;
                    }
                }
            }
            Ok(OperationResponse::ListCallers { callers })
        }

        OperationRequest::IndexRepo { repo } => {
            // Phase 3 MVP: destructive reindex matching CLI Cmd::Index behaviour.
            // Phase 4 should incrementally emit task progress events.
            let store = Store::open(&db_path).context("open db")?;
            store.clear()?;
            let embedder = embedder::Embedder::new();
            let repo_path = FsPath::new(&repo);
            let symbols = parser::parse_repo(repo_path).context("parse repo")?;
            let total = symbols.len();
            let mut indexed = 0usize;
            for s in symbols.iter() {
                let dn = search::decompose(&s.name);
                let ds = search::decompose(&s.snippet);
                let blob = format!("{} {}", dn, ds);
                let text = format!("{} {} {}", s.kind, s.name, s.snippet);
                let emb = match embedder.embed(&text, embedder::Role::Passage) {
                    Ok(v) => v,
                    Err(_) => continue, // best-effort skip on embed failure
                };
                store.insert(s, &blob, &emb)?;
                indexed += 1;
            }
            // Build graph in same op so subsequent list_callers work immediately.
            let mut builder = graph_build::EdgeBuilder::new(&store, PathBuf::from(&repo))
                .context("graph builder init")?;
            let stats = builder.build_all().context("build graph")?;
            let edges_built = stats.calls + stats.imports + stats.implements + stats.extends;
            Ok(OperationResponse::IndexRepo {
                symbols_indexed: indexed.min(total),
                edges_built,
            })
        }
    }
}
