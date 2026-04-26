use anyhow::Result;
use std::collections::HashMap;

use crate::embedder::{cosine, Embedder, Role};
use crate::parser::Symbol;
use crate::storage::Store;

#[derive(Debug, serde::Serialize)]
pub struct Hit {
    pub id: i64,
    pub bm25_score: f32,
    pub vector_score: f32,
    pub rrf_score: f32,
    pub symbol: Symbol,
}

pub fn search(store: &Store, embedder: &Embedder, query: &str, k: usize) -> Result<Vec<Hit>> {
    let bm25 = store.bm25(&fts_escape(query), 50).unwrap_or_default();

    let qv = embedder.embed(query, Role::Query)?;
    let mut vec_scored: Vec<(i64, f32)> = store
        .all_embeddings()?
        .into_iter()
        .map(|(id, v)| (id, cosine(&qv, &v)))
        .collect();
    vec_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let vec_top: Vec<(i64, f32)> = vec_scored.into_iter().take(50).collect();

    let bm25_rank: HashMap<i64, usize> = bm25.iter().enumerate().map(|(i, (id, _))| (*id, i + 1)).collect();
    let vec_rank: HashMap<i64, usize> = vec_top.iter().enumerate().map(|(i, (id, _))| (*id, i + 1)).collect();
    let bm25_score: HashMap<i64, f32> = bm25.iter().copied().collect();
    let vec_score: HashMap<i64, f32> = vec_top.iter().copied().collect();

    let c = 60.0f32;
    let mut all_ids: Vec<i64> = bm25_rank.keys().chain(vec_rank.keys()).copied().collect();
    all_ids.sort();
    all_ids.dedup();
    let mut fused: Vec<(i64, f32)> = all_ids
        .into_iter()
        .map(|id| {
            let r1 = bm25_rank.get(&id).map(|r| 1.0 / (c + *r as f32)).unwrap_or(0.0);
            let r2 = vec_rank.get(&id).map(|r| 1.0 / (c + *r as f32)).unwrap_or(0.0);
            (id, r1 + r2)
        })
        .collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut hits = Vec::new();
    for (id, rrf) in fused.into_iter().take(k) {
        let sym = store.fetch(id)?;
        hits.push(Hit {
            id,
            bm25_score: *bm25_score.get(&id).unwrap_or(&0.0),
            vector_score: *vec_score.get(&id).unwrap_or(&0.0),
            rrf_score: rrf,
            symbol: sym,
        });
    }
    Ok(hits)
}

fn fts_escape(q: &str) -> String {
    q.split_whitespace()
        .map(|t| {
            let cleaned: String = t.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
            if cleaned.is_empty() {
                String::new()
            } else {
                format!("\"{}\"", cleaned)
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" OR ")
}
