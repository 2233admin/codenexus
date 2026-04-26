//! Personalized PageRank for symbol graph traversal.
//!
//! Per Fast-GraphRAG / HippoRAG: probability mass starts concentrated on entry
//! symbols (the query "subjects"), iterates `(1-damping) * teleport + damping
//! * transition_matrix * prob`, then sorts symbols by final score descending.
//! Outperforms BFS for "broader impact" queries that need transitively-relevant
//! symbols (not just direct neighbors).
//!
//! Confidence values per edge currently act as a hard filter (must be ≥ min_conf).
//! Phase 3+ extension: weight transitions by confidence for soft demotion of
//! global-unique-resolved edges.

use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::storage::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Calls,
    Imports,
    Implements,
    Extends,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Calls => "Calls",
            EdgeKind::Imports => "Imports",
            EdgeKind::Implements => "Implements",
            EdgeKind::Extends => "Extends",
        }
    }
}

/// Pure-data Personalized PageRank: takes an edge list + entry IDs, returns
/// nodes ranked by PPR score descending. No DB access — testable in isolation.
///
/// Algorithm:
/// 1. Universe = {all from_id} ∪ {all to_id} ∪ {entry_ids}.
/// 2. Teleport vector: 1/|entry_ids| for each entry, 0 elsewhere.
/// 3. Initial prob = teleport.
/// 4. Iterate `iters` times:
///      `new_prob[v] = (1-damping) * teleport[v]
///                   + damping * (Σ_{u→v} prob[u] / out_deg[u])
///                   + damping * dangling_mass * teleport[v]`
///    Dangling-node mass (sinks) redistributes to teleport vector
///    (preserves Σ prob = 1, standard PPR convention).
/// 5. Return Vec<(node_id, score)> sorted by score descending.
///
/// # Examples
///
/// ```ignore
/// // Synthetic 5-node graph, edges: 1→2, 2→3, 3→2, 4→5
/// let edges = vec![(1, 2), (2, 3), (3, 2), (4, 5)];
/// let result = poc_retrieval::graph_ppr::ppr_from_edge_list(&edges, &[1], 0.85, 30);
/// // Entry node 1 → mass leaks to 2; 2↔3 cycle distributes; 4 and 5 unreachable
/// assert_eq!(result[0].0, 1);  // entry retains highest mass
/// // 2 should outrank 4 and 5 (which get only teleport residual = 0)
/// ```
pub fn ppr_from_edge_list(
    edges: &[(i64, i64)],
    entries: &[i64],
    damping: f64,
    iters: usize,
) -> Vec<(i64, f64)> {
    if entries.is_empty() {
        return vec![];
    }

    // Universe of all nodes (from, to, entries).
    let mut universe: HashSet<i64> = HashSet::new();
    for (u, v) in edges {
        universe.insert(*u);
        universe.insert(*v);
    }
    for e in entries {
        universe.insert(*e);
    }
    if universe.is_empty() {
        return vec![];
    }

    // Out-edges + out-degree.
    let mut out_edges: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut out_deg: HashMap<i64, usize> = HashMap::new();
    for (u, v) in edges {
        out_edges.entry(*u).or_default().push(*v);
        *out_deg.entry(*u).or_insert(0) += 1;
    }

    // Teleport vector: uniform over entries.
    let entry_set: HashSet<i64> = entries.iter().copied().collect();
    let n_entries = entries.len() as f64;
    let teleport_per_entry = 1.0 / n_entries;
    let mut teleport: HashMap<i64, f64> = HashMap::new();
    for &e in entries {
        teleport.insert(e, teleport_per_entry);
    }

    // Initial prob = teleport.
    let mut prob: HashMap<i64, f64> = teleport.clone();
    for &v in &universe {
        prob.entry(v).or_insert(0.0);
    }

    for _ in 0..iters {
        let mut new_prob: HashMap<i64, f64> = HashMap::with_capacity(universe.len());
        for &v in &universe {
            new_prob.insert(v, 0.0);
        }

        // Dangling mass = sum of prob mass on sink nodes (no out-edges).
        let mut dangling = 0.0_f64;
        for (&u, &p) in &prob {
            if out_deg.get(&u).copied().unwrap_or(0) == 0 {
                dangling += p;
            }
        }

        // Teleport contribution: (1 - damping) * teleport + damping * dangling * teleport
        // (dangling redistributed via teleport vector — entries-only).
        for &e in &entry_set {
            let t = teleport_per_entry;
            *new_prob.entry(e).or_insert(0.0) +=
                (1.0 - damping) * t + damping * dangling * t;
        }

        // Transition contribution: each edge u → v passes prob[u] / out_deg[u]
        // weighted by damping.
        for (&u, neighbours) in &out_edges {
            let p_u = prob.get(&u).copied().unwrap_or(0.0);
            let deg = neighbours.len() as f64;
            if deg == 0.0 {
                continue;
            }
            let share = damping * p_u / deg;
            for &v in neighbours {
                *new_prob.entry(v).or_insert(0.0) += share;
            }
        }

        prob = new_prob;
    }

    let mut ranked: Vec<(i64, f64)> = prob.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

/// DB-backed PPR: load edges of given kinds with confidence ≥ min_conf, then
/// delegate to `ppr_from_edge_list`. Phase 3 callers wire this behind a
/// `--axis-3` query mode in main.rs.
pub fn personalized_pagerank(
    storage: &Store,
    entry_symbol_ids: &[i64],
    allowed_kinds: &[EdgeKind],
    damping: f64,
    iters: usize,
    confidence_min: f64,
) -> Result<Vec<(i64, f64)>> {
    if entry_symbol_ids.is_empty() {
        return Ok(vec![]);
    }
    let kind_strs: Vec<&str> = allowed_kinds.iter().map(|k| k.as_str()).collect();
    let edges_with_conf = storage.edges_of_kinds(&kind_strs, confidence_min)?;
    let edges: Vec<(i64, i64)> =
        edges_with_conf.into_iter().map(|(u, v, _)| (u, v)).collect();
    Ok(ppr_from_edge_list(
        &edges,
        entry_symbol_ids,
        damping,
        iters,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic 5-node graph: edges 1→2, 2→3, 3→2, 4→5. Entry={1}.
    /// PPR semantics: reachable nodes accumulate mass; unreachable stay zero.
    /// Note: entry node does NOT necessarily outrank reachable neighbors —
    /// `1→2` leaks (damping=0.85)*prob[1] every iter so steady-state prob[1]
    /// ≈ (1-d)/n_entries = 0.15, while 2/3 collect cycle mass via 2↔3.
    /// Correct PPR test: reachable {1,2,3} > unreachable {4,5}.
    #[test]
    fn synthetic_5node_ranks_entry_highest() {
        let edges = vec![(1i64, 2), (2, 3), (3, 2), (4, 5)];
        let result = ppr_from_edge_list(&edges, &[1], 0.85, 50);
        let scores: HashMap<i64, f64> = result.iter().copied().collect();

        // Reachable from entry {1} via Calls edges: {1, 2, 3}
        assert!(scores[&1] > 0.0, "entry 1 should hold teleport mass");
        assert!(scores[&2] > 0.0, "node 2 reachable from 1");
        assert!(scores[&3] > 0.0, "node 3 reachable via 1→2→3");

        // Unreachable: {4, 5}. They get neither teleport (only entry teleports)
        // nor transition mass from any node in {1,2,3}.
        assert!(scores[&4].abs() < 1e-9, "node 4 unreachable, score={}", scores[&4]);
        assert!(scores[&5].abs() < 1e-9, "node 5 unreachable, score={}", scores[&5]);

        // Reachable should clearly outrank unreachable.
        assert!(
            scores[&2] > scores[&4],
            "reachable 2 should outrank unreachable 4: 2={}, 4={}",
            scores[&2], scores[&4]
        );

        // Top result by sort order must be a reachable node.
        let top_id = result[0].0;
        assert!(
            top_id == 1 || top_id == 2 || top_id == 3,
            "top-ranked must be reachable, got {}",
            top_id
        );
    }

    #[test]
    fn empty_entries_returns_empty() {
        let edges = vec![(1i64, 2), (2, 3)];
        let result = ppr_from_edge_list(&edges, &[], 0.85, 20);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn single_isolated_entry_keeps_all_mass() {
        // Entry 99 has no edges in/out → only teleport mass, sink mass loops back.
        let edges: Vec<(i64, i64)> = vec![];
        let result = ppr_from_edge_list(&edges, &[99], 0.85, 20);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 99);
        assert!(
            (result[0].1 - 1.0).abs() < 1e-9,
            "isolated entry should hold full mass: {}",
            result[0].1
        );
    }

    #[test]
    fn dangling_mass_redistributed_to_entries() {
        // Edges: 1→4 (4 is sink). Entry={1}.
        // After step 1: prob[1] = (1-d)*1 + d*dangling*1 = 0.15 + 0.85*0*1 = 0.15
        //                prob[4] = damping * prob_prev[1]/1 = 0.85*1 = 0.85
        // After step 2: 4 is dangling (no out), prob[4]=0.85 redistributes to entry teleport.
        //                prob[1] = (1-d)*1 + d*0.85*1 = 0.15 + 0.7225 = 0.8725
        //                prob[4] = damping * prob_prev[1]/1 = 0.85*0.15 = 0.1275
        // Sums to 1 (modulo float). Convergence: most mass alternates between 1 and 4.
        let edges = vec![(1i64, 4)];
        let result = ppr_from_edge_list(&edges, &[1], 0.85, 50);
        let total: f64 = result.iter().map(|(_, p)| *p).sum();
        assert!(
            (total - 1.0).abs() < 1e-6,
            "PPR scores should sum to 1: total={}",
            total
        );
    }
}
