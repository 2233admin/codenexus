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

/// R4 (Phase 4 first slice): default consecutive-embed-failure abort
/// threshold for the A2A IndexRepo handler. Mirrors `main.rs` CLI flag
/// `--max-consecutive-fail` default from Phase 3.5b commit `8f4da66`.
/// Override per-call via `OperationRequest::IndexRepo.max_consecutive_fail`.
/// Plan 04-02 v2 G-04: operation-schema versioning via `#[serde(default)]`,
/// not A2A metadata pass-through (M6 corrected framing).
const MAX_CONSECUTIVE_FAIL_DEFAULT: usize = 5;

/// R4 (Plan 04-02 v2, M2 fix): upper bound on per-call envelope override.
/// 100 = pathological-repo upper bound; even a fully-broken embedder hits
/// 100 consecutive failures within ~13 minutes (5 attempts x ~7.75s x 100 /
/// 60s = ~12.9 min) -- fast enough to be useful as a sanity bound. Tighter
/// than 1000 (which defeats the purpose of a safety bound while a fully-
/// broken embedder still consumes ~130 minutes wall-clock).
const MAX_RAISED_THRESHOLD: usize = 100;

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

        OperationRequest::IndexRepo { repo, max_consecutive_fail } => {
            // Phase 3 MVP: destructive reindex matching CLI Cmd::Index behaviour.
            // Phase 4 should incrementally emit task progress events.
            //
            // Phase 4 plan 04-06 (2026-04-28): `store.clear()` is deferred from
            // here to the loop body, gated by `cleared = false` until the first
            // successful embed. Reason: pre-04-06 behaviour cleared symbols at
            // handler entry, so any failure mode that bailed before the first
            // insert (e.g. R4.b probe with CODENEXUS_EMBED_FAIL=always)
            // destroyed pre-existing data with no transaction wrapping. Discovered
            // 2026-04-28 during Plan 04-05 T2 design (eval-based R1.c probe broke
            // because R4.b probe had emptied poc.db). Fix preserves existing data
            // when all embeds fail; new data still replaces old once at least one
            // embed succeeds. Same semantic as main.rs CLI Index's
            // consecutive_fails pattern.
            let store = Store::open(&db_path).context("open db")?;
            let embedder = embedder::Embedder::new();
            let repo_path = FsPath::new(&repo);
            let symbols = parser::parse_repo(repo_path).context("parse repo")?;
            let total = symbols.len();
            // R4 (D-05): envelope override > hardcoded default. Bound check
            // `1..=MAX_RAISED_THRESHOLD` (1..=100 per M2 / T-04-06 envelope
            // injection guard). `Some(0)` rejected -- zero-tolerance threshold
            // would abort on first failure regardless of recovery, which is
            // not a knob users want.
            let max_consecutive_fail = match max_consecutive_fail {
                Some(n) if (1..=MAX_RAISED_THRESHOLD).contains(&n) => n,
                Some(n) => {
                    return Err(anyhow::anyhow!(
                        "max_consecutive_fail out of bounds: {} (allowed 1..={})",
                        n, MAX_RAISED_THRESHOLD
                    ));
                }
                None => MAX_CONSECUTIVE_FAIL_DEFAULT,
            };
            let mut indexed = 0usize;
            let mut consecutive_fails: usize = 0;
            // Plan 04-06 fix: deferred clear -- only wipe existing rows once the
            // first embed succeeds, so synthetic-fail / network-down / any
            // bail-before-first-insert path leaves pre-existing data intact.
            let mut cleared = false;
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
                            "[a2a-index {}/{}] embed fail {}: {} (consecutive={}/{})",
                            i + 1, total, s.name, e,
                            consecutive_fails, max_consecutive_fail
                        );
                        if consecutive_fails >= max_consecutive_fail {
                            // R4 bail: return Err -- server.rs:64-66 maps this to
                            // `store.fail(&task_id, "op error: ...")` which sets
                            // A2A task state to `failed` with structured message
                            // containing the consecutive count. Server keeps
                            // running to serve next request (vs main.rs CLI
                            // which `anyhow::bail!` exits the process).
                            return Err(anyhow::anyhow!(
                                "aborting a2a indexer: {} consecutive embed failures (threshold {}), last symbol={}, indexed={}/{}, last error={:#}",
                                consecutive_fails, max_consecutive_fail, s.name, indexed, total, e
                            ));
                        }
                        continue;
                    }
                };
                // First successful embed: clear pre-existing rows now (deferred
                // from handler entry per Plan 04-06). Brief non-atomic window
                // between clear and insert is acceptable since this is one-client
                // -per-A2A-request and embedder confirmed working.
                if !cleared {
                    store.clear()?;
                    cleared = true;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Symbol;

    /// Plan 04-06 deferred-clear invariant: when IndexRepo bails before any
    /// embed succeeds (here: empty repo dir -> 0 symbols -> embed loop body
    /// never runs), pre-existing rows MUST survive. Pre-04-06 had unconditional
    /// `store.clear()` at handler entry which wiped all rows on every bail
    /// path (R4.b probe with CODENEXUS_EMBED_FAIL=always destroyed poc.db).
    /// Post-04-06: clear() is gated behind `cleared = false` and fires only
    /// after the first successful embed -- empty repo means no embed runs,
    /// so the marker survives.
    ///
    /// Companion to r4b_probe.sh (synthetic A2A path); this is the unit-test
    /// regression guard so a future server.rs edit cannot silently revert
    /// the deferred-clear pattern.
    #[test]
    fn index_repo_empty_repo_preserves_existing_data() {
        let uid = uuid::Uuid::new_v4();
        let tmp = std::env::temp_dir();
        let repo_dir = tmp.join(format!("codenexus_test_repo_{}", uid));
        let db_dir = tmp.join(format!("codenexus_test_db_{}", uid));
        std::fs::create_dir_all(&repo_dir).expect("mk repo dir");
        std::fs::create_dir_all(&db_dir).expect("mk db dir");
        let db_path = db_dir.join("test.db").to_str().unwrap().to_string();

        // Pre-existing marker symbol (must survive empty-repo IndexRepo).
        {
            let store = Store::open(&db_path).expect("open db");
            let marker = Symbol {
                kind: "test_marker".into(),
                name: "MARKER_DEFERRED_CLEAR".into(),
                path: "test/marker.ts".into(),
                start_line: 1,
                end_line: 1,
                snippet: "// 04-06 invariant guard".into(),
            };
            let dummy_emb = vec![0.0f32; 1024];
            store
                .insert(&marker, "MARKER_DEFERRED_CLEAR", &dummy_emb)
                .expect("insert marker");
        }

        // Act: empty repo -> parser returns 0 symbols -> embed loop never runs
        // -> `cleared` stays false -> store.clear() never fires.
        let req = OperationRequest::IndexRepo {
            repo: repo_dir.to_str().unwrap().to_string(),
            max_consecutive_fail: None,
        };
        let _resp = dispatch(db_path.clone(), req).expect("dispatch IndexRepo");

        // Assert: marker survived. Pre-04-06 this assertion would fail.
        let store = Store::open(&db_path).expect("reopen db");
        let ids = store
            .find_symbols_by_name("MARKER_DEFERRED_CLEAR")
            .expect("find marker");
        assert!(
            !ids.is_empty(),
            "MARKER must survive empty-repo IndexRepo (deferred-clear invariant from Plan 04-06)"
        );

        let _ = std::fs::remove_dir_all(&repo_dir);
        let _ = std::fs::remove_dir_all(&db_dir);
    }
}
