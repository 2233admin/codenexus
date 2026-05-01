// SPDX-License-Identifier: MIT
//
// Lifted (with import-path adaptation) from sentrux-core/src/metrics/arch/graph.rs
// (MIT, 2026 Sentrux). Algorithm code is byte-equivalent to upstream commit at
// recon time (audit dir: AppData/Local/Temp/sentrux-audit-LdXuzt, 2026-04-28).
//
// Adaptations from upstream:
//   - `use crate::core::types::{EntryPoint, ImportEdge}` -> `use crate::types::{EntryPoint, ImportEdge}`
//   - `crate::metrics::types::is_mod_declaration_edge`   -> `crate::types::is_mod_declaration_edge`
//   - File renamed `arch/graph.rs` -> `arch.rs` (no sub-module split this slice)
//   - Test helper `edge()` reimplemented locally (was `crate::metrics::test_helpers::edge`)
//   - Tests requiring `crate::core::snapshot::Snapshot` deferred to 04.5-02b
//
// See workspace-root NOTICE for full attribution.

//! Graph algorithms for architecture metrics.
//!
//! Contains SCC computation (Kosaraju), levelization (Lakos 1996),
//! upward dependency violation detection, blast radius (transitive reach),
//! and attack surface analysis.
//!
//! All algorithms are O(V+E) with iterative DFS (no stack overflow risk).

use crate::types::{EntryPoint, ImportEdge};
use std::collections::{HashMap, HashSet, VecDeque};

// ── Shared SCC computation (Kosaraju) ──
//
// Used by both compute_levels and find_upward_violations to avoid redundant
// O(V+E) SCC computation. Previously each function had its own Kosaraju copy.

/// Result of SCC computation: node→SCC ID mapping and per-SCC member count.
/// Public within crate so compute_arch can pass the result to both
/// compute_levels_with_sccs and find_upward_violations_with_sccs,
/// avoiding redundant O(V+E) computation.
pub(crate) struct SccResult<'a> {
    pub(crate) scc_id: HashMap<&'a str, usize>,
    pub(crate) scc_sizes: Vec<usize>,
    pub(crate) scc_count: usize,
}

/// Compute SCCs via Kosaraju's algorithm. O(V+E), iterative (no stack overflow).
pub(crate) fn compute_sccs<'a>(edges: &'a [ImportEdge]) -> SccResult<'a> {
    let mut nodes: HashSet<&str> = HashSet::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        nodes.insert(edge.from_file.as_str());
        nodes.insert(edge.to_file.as_str());
        adj.entry(edge.from_file.as_str())
            .or_default()
            .push(edge.to_file.as_str());
    }

    // Pass 1: DFS finish-order on forward graph
    let finish_order = kosaraju_finish_order(&nodes, &adj);

    // Build reverse adjacency
    let mut rev_adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        rev_adj
            .entry(edge.to_file.as_str())
            .or_default()
            .push(edge.from_file.as_str());
    }

    // Pass 2: assign SCC IDs on reverse graph
    kosaraju_assign_sccs(&finish_order, &rev_adj)
}

/// Process one step of iterative DFS for Kosaraju finish-order computation.
fn kosaraju_dfs_step<'a>(
    n: &'a str,
    finished: bool,
    visited: &mut HashSet<&'a str>,
    finish_order: &mut Vec<&'a str>,
    stack: &mut Vec<(&'a str, bool)>,
    adj: &HashMap<&'a str, Vec<&'a str>>,
) {
    if finished {
        finish_order.push(n);
        return;
    }
    if !visited.insert(n) {
        return;
    }
    stack.push((n, true));
    if let Some(neighbors) = adj.get(n) {
        for &neighbor in neighbors {
            if !visited.contains(neighbor) {
                stack.push((neighbor, false));
            }
        }
    }
}

/// Kosaraju pass 1: iterative DFS to compute finish order on the forward graph.
fn kosaraju_finish_order<'a>(
    nodes: &HashSet<&'a str>,
    adj: &HashMap<&'a str, Vec<&'a str>>,
) -> Vec<&'a str> {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut finish_order: Vec<&str> = Vec::with_capacity(nodes.len());
    for &node in nodes {
        if visited.contains(node) {
            continue;
        }
        let mut stack = vec![(node, false)];
        while let Some((n, finished)) = stack.pop() {
            kosaraju_dfs_step(n, finished, &mut visited, &mut finish_order, &mut stack, adj);
        }
    }
    finish_order
}

/// DFS from a single start node on the reverse graph, assigning all reachable
/// unvisited nodes to the given SCC ID. Returns the SCC size.
fn assign_scc_component<'a>(
    start: &'a str,
    id: usize,
    rev_adj: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    scc_id: &mut HashMap<&'a str, usize>,
) -> usize {
    let mut size = 0;
    let mut stack = vec![start];
    while let Some(n) = stack.pop() {
        if !visited.insert(n) {
            continue;
        }
        scc_id.insert(n, id);
        size += 1;
        if let Some(neighbors) = rev_adj.get(n) {
            for &neighbor in neighbors {
                if !visited.contains(neighbor) {
                    stack.push(neighbor);
                }
            }
        }
    }
    size
}

/// Kosaraju pass 2: DFS on reverse graph in reverse finish order to assign SCC IDs.
fn kosaraju_assign_sccs<'a>(
    finish_order: &[&'a str],
    rev_adj: &HashMap<&'a str, Vec<&'a str>>,
) -> SccResult<'a> {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut scc_id: HashMap<&'a str, usize> = HashMap::new();
    let mut scc_sizes: Vec<usize> = Vec::new();
    for &node in finish_order.iter().rev() {
        if visited.contains(node) {
            continue;
        }
        let id = scc_sizes.len();
        let size = assign_scc_component(node, id, rev_adj, &mut visited, &mut scc_id);
        scc_sizes.push(size);
    }

    let scc_count = scc_sizes.len();
    SccResult { scc_id, scc_sizes, scc_count }
}

// ── Levelization (Lakos 1996) ──
//
// Assign each file a "level" based on its position in the dependency DAG.
// Level 0 = leaf nodes (depend on nothing in the graph).
// Level N = depends on files up to level N-1.
// Cycles get the same level (conservative: max of cycle members).

/// Compute levels via Kahn's topological sort on the SCC DAG.
/// Level 0 = leaf nodes (no outgoing imports). Level N = imports up to level N-1.
/// Cycles are handled by collapsing SCCs — all nodes in a cycle get the same
/// level (max of their dependencies + 1). O(V+E), no re-enqueue.
pub fn compute_levels(edges: &[ImportEdge]) -> (HashMap<String, u32>, u32) {
    if edges.is_empty() {
        return (HashMap::new(), 0);
    }
    let sccs = compute_sccs(edges);
    compute_levels_with_sccs(edges, &sccs)
}

/// Same as `compute_levels` but accepts pre-computed SCCs to avoid redundant
/// O(V+E) Kosaraju when the caller also needs SCCs for other purposes.
pub(crate) fn compute_levels_with_sccs(edges: &[ImportEdge], sccs: &SccResult<'_>) -> (HashMap<String, u32>, u32) {
    if edges.is_empty() {
        return (HashMap::new(), 0);
    }

    // ── Build DAG of SCCs ──
    let mut scc_out: Vec<HashSet<usize>> = vec![HashSet::new(); sccs.scc_count];
    let mut scc_out_degree: Vec<u32> = vec![0; sccs.scc_count];
    let mut scc_rev: Vec<Vec<usize>> = vec![Vec::new(); sccs.scc_count];
    for edge in edges {
        let from_scc = sccs.scc_id[edge.from_file.as_str()];
        let to_scc = sccs.scc_id[edge.to_file.as_str()];
        if from_scc != to_scc && scc_out[from_scc].insert(to_scc) {
            scc_out_degree[from_scc] += 1;
            scc_rev[to_scc].push(from_scc);
        }
    }

    // ── Kahn's topological sort on the SCC DAG ──
    // Level 0 = SCC with no outgoing edges (leaf dependencies)
    let mut scc_levels: Vec<u32> = vec![0; sccs.scc_count];
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut remaining_out: Vec<u32> = scc_out_degree.clone();

    for (s, out) in remaining_out.iter().enumerate() {
        if *out == 0 {
            queue.push_back(s);
        }
    }

    let mut max_level: u32 = 0;
    while let Some(s) = queue.pop_front() {
        let s_level = scc_levels[s];
        for &parent in &scc_rev[s] {
            let new_level = s_level + 1;
            if new_level > scc_levels[parent] {
                scc_levels[parent] = new_level;
                if new_level > max_level {
                    max_level = new_level;
                }
            }
            remaining_out[parent] -= 1;
            if remaining_out[parent] == 0 {
                queue.push_back(parent);
            }
        }
    }

    // Map SCC levels back to individual nodes
    let owned: HashMap<String, u32> = sccs.scc_id
        .iter()
        .map(|(&node, &sid)| (node.to_string(), scc_levels[sid]))
        .collect();
    (owned, max_level)
}

/// An import edge that goes from a lower level to a higher level (violation).
#[derive(Debug, Clone)]
pub struct UpwardViolation {
    /// File containing the violating import statement
    pub from_file: String,
    /// Level of the importing file in the DAG
    pub from_level: u32,
    /// File being imported in the wrong direction
    pub to_file: String,
    /// Level of the imported file in the DAG
    pub to_level: u32,
}

/// Detect upward dependency violations: edges where the source has a LOWER level
/// than the target, plus intra-SCC edges (cycles prevent clean layering).
pub fn find_upward_violations(
    edges: &[ImportEdge],
    levels: &HashMap<String, u32>,
) -> Vec<UpwardViolation> {
    let sccs = compute_sccs(edges);
    find_upward_violations_with_sccs(edges, levels, &sccs)
}

/// Same as `find_upward_violations` but accepts pre-computed SCCs to avoid
/// redundant O(V+E) Kosaraju when `compute_arch` already computed SCCs.
pub(crate) fn find_upward_violations_with_sccs(
    edges: &[ImportEdge],
    levels: &HashMap<String, u32>,
    sccs: &SccResult<'_>,
) -> Vec<UpwardViolation> {

    let mut violations = Vec::new();
    for edge in edges {
        let from_level = levels.get(&edge.from_file).copied().unwrap_or(0);
        let to_level = levels.get(&edge.to_file).copied().unwrap_or(0);

        // Cross-SCC upward violation (rare given levels are computed from edges,
        // but possible with edge filtering or external level overrides)
        if from_level < to_level {
            violations.push(UpwardViolation {
                from_file: edge.from_file.clone(),
                from_level,
                to_file: edge.to_file.clone(),
                to_level,
            });
        } else {
            // Intra-SCC edge: both files are in the same cycle (same level).
            // These ARE architectural violations — the cycle prevents clean layering.
            // Only count edges within SCCs of size > 1 (actual cycles, not self-loops).
            let from_scc = sccs.scc_id.get(edge.from_file.as_str()).copied().unwrap_or(0);
            let to_scc = sccs.scc_id.get(edge.to_file.as_str()).copied().unwrap_or(0);
            if from_scc == to_scc
                && sccs.scc_sizes.get(from_scc).copied().unwrap_or(0) > 1
                && edge.from_file != edge.to_file
            {
                violations.push(UpwardViolation {
                    from_file: edge.from_file.clone(),
                    from_level,
                    to_file: edge.to_file.clone(),
                    to_level,
                });
            }
        }
    }
    violations.sort_unstable_by(|a, b| {
        let a_diff = a.to_level.abs_diff(a.from_level);
        let b_diff = b.to_level.abs_diff(b.from_level);
        b_diff.cmp(&a_diff)
    });
    violations
}

// ── Blast Radius ──

/// For each file in the import graph, compute how many files are transitively
/// reachable by REVERSE edges (i.e., if this file changes, how many files
/// that directly or indirectly depend on it could be affected).
///
/// Uses index-based BFS with a reusable `Vec<bool>` instead of per-node
/// `HashSet<&str>` allocation. For V nodes: old approach allocated V HashSets
/// growing up to V entries = O(V^2) memory. New approach: one Vec<bool> of
/// size V, cleared between runs = O(V) memory.
///
/// For very large graphs (>5000 nodes), samples uniformly across the node set
/// to avoid O(V^2) time while preserving the max_blast_radius accuracy.
pub fn compute_blast_radius(edges: &[ImportEdge]) -> HashMap<String, u32> {
    if edges.is_empty() {
        return HashMap::new();
    }

    // Filter out mod-declaration edges — these are structural containment, not
    // functional dependencies. See `crate::types::is_mod_declaration_edge` doc
    // for codenexus-specific behaviour (TS/Python: always returns false).
    let dep_edges: Vec<&ImportEdge> = edges.iter()
        .filter(|e| !crate::types::is_mod_declaration_edge(e))
        .collect();

    // Build index-based adjacency for O(1) lookup instead of HashMap<&str, Vec<&str>>.
    // Include ALL files as nodes (even those only in mod-declaration edges) so that
    // every file gets a blast radius entry, but only functional edges create propagation.
    let mut node_set: HashSet<&str> = HashSet::new();
    for edge in edges {
        node_set.insert(edge.from_file.as_str());
        node_set.insert(edge.to_file.as_str());
    }
    let mut node_list: Vec<&str> = node_set.into_iter().collect();
    // Sort for deterministic ordering — HashSet iteration is nondeterministic,
    // which caused different sampling subsets on each run (violating idempotency).
    node_list.sort_unstable();
    let node_idx: HashMap<&str, usize> = node_list.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let n = node_list.len();

    // Reverse adjacency: if A imports B, changing B affects A → edge B→A
    // Uses functional edges only (mod-declarations filtered out above).
    let mut rev_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for edge in &dep_edges {
        if let (Some(&from_idx), Some(&to_idx)) = (node_idx.get(edge.to_file.as_str()), node_idx.get(edge.from_file.as_str())) {
            rev_adj[from_idx].push(to_idx);
        }
    }

    let bfs_indices = blast_radius_sample_indices(n, &rev_adj);
    let mut result = blast_radius_bfs(&node_list, &rev_adj, &bfs_indices);

    // For sampled graphs: nodes not in bfs_indices get blast_radius = 0 (conservative).
    if n > 5000 {
        for node in &node_list {
            result.entry(node.to_string()).or_insert(0);
        }
    }

    result
}

/// Select which node indices to BFS from. Full set when V <= 5000, uniform
/// sample otherwise (with max-degree node guaranteed).
fn blast_radius_sample_indices(n: usize, rev_adj: &[Vec<usize>]) -> Vec<usize> {
    const MAX_BFS_NODES: usize = 5000;
    if n <= MAX_BFS_NODES {
        (0..n).collect()
    } else {
        let step = n as f64 / MAX_BFS_NODES as f64;
        let mut indices: Vec<usize> = (0..MAX_BFS_NODES)
            .map(|i| ((i as f64) * step) as usize)
            .collect();
        // Guarantee the node with max reverse-out-degree is sampled.
        if let Some((max_deg_idx, _)) = rev_adj.iter().enumerate().max_by_key(|(_, adj)| adj.len()) {
            if !indices.contains(&max_deg_idx) {
                if let Some(last) = indices.last_mut() {
                    *last = max_deg_idx;
                }
            }
        }
        indices
    }
}

/// Run BFS from each start index on the reverse adjacency, returning per-node
/// transitive reach counts.
fn blast_radius_bfs(
    node_list: &[&str],
    rev_adj: &[Vec<usize>],
    bfs_indices: &[usize],
) -> HashMap<String, u32> {
    let n = node_list.len();
    let mut result: HashMap<String, u32> = HashMap::with_capacity(n);
    let mut visited = vec![false; n];
    let mut queue: VecDeque<usize> = VecDeque::new();

    for &start in bfs_indices {
        visited.fill(false);
        visited[start] = true;
        queue.clear();
        queue.push_back(start);
        let mut reach = 0u32;

        while let Some(node) = queue.pop_front() {
            for &dep in &rev_adj[node] {
                if !visited[dep] {
                    visited[dep] = true;
                    reach += 1;
                    queue.push_back(dep);
                }
            }
        }
        result.insert(node_list[start].to_string(), reach);
    }

    result
}

// ── Attack Surface ──

/// Compute how many files are transitively reachable from entry points.
/// This represents the "attack surface" — code reachable from public APIs.
pub fn compute_attack_surface(
    edges: &[ImportEdge],
    entry_points: &[EntryPoint],
) -> (u32, u32) {
    let (adj, nodes) = build_forward_adjacency(edges);
    let total = nodes.len() as u32;

    if edges.is_empty() || entry_points.is_empty() {
        return (0, total);
    }

    let reachable = bfs_from_entry_points(&adj, &nodes, entry_points);
    (reachable, total)
}

/// Build forward adjacency list and node set from import edges.
fn build_forward_adjacency(edges: &[ImportEdge]) -> (HashMap<&str, Vec<&str>>, HashSet<&str>) {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut nodes: HashSet<&str> = HashSet::new();
    for edge in edges {
        nodes.insert(edge.from_file.as_str());
        nodes.insert(edge.to_file.as_str());
        adj.entry(edge.from_file.as_str())
            .or_default()
            .push(edge.to_file.as_str());
    }
    (adj, nodes)
}

/// BFS from all entry points on the forward adjacency, returning reachable count.
fn bfs_from_entry_points<'a>(
    adj: &HashMap<&'a str, Vec<&'a str>>,
    nodes: &HashSet<&'a str>,
    entry_points: &'a [EntryPoint],
) -> u32 {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    for ep in entry_points {
        if nodes.contains(ep.file.as_str()) && visited.insert(ep.file.as_str()) {
            queue.push_back(ep.file.as_str());
        }
    }

    while let Some(node) = queue.pop_front() {
        if let Some(targets) = adj.get(node) {
            for &target in targets {
                if visited.insert(target) {
                    queue.push_back(target);
                }
            }
        }
    }

    visited.len() as u32
}

// ────────────────────────────────────────────────────────────────────────────
// Tests (lifted from sentrux-core/src/metrics/arch/tests.rs, adapted)
//
// Snapshot-dependent tests (ArchAnalyzer trait coverage) are deferred to
// 04.5-02b when the Snapshot adapter against codenexus storage lands.
// ────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Local replacement for `crate::metrics::test_helpers::edge` upstream.
    fn edge(from: &str, to: &str) -> ImportEdge {
        ImportEdge {
            from_file: from.to_string(),
            to_file: to.to_string(),
        }
    }

    // ── Levelization ──

    #[test]
    fn levels_linear_chain() {
        let edges = vec![edge("a.rs", "b.rs"), edge("b.rs", "c.rs")];
        let (levels, max) = compute_levels(&edges);
        assert_eq!(levels["c.rs"], 0);
        assert_eq!(levels["b.rs"], 1);
        assert_eq!(levels["a.rs"], 2);
        assert_eq!(max, 2);
    }

    #[test]
    fn levels_diamond() {
        let edges = vec![
            edge("a.rs", "b.rs"),
            edge("a.rs", "c.rs"),
            edge("b.rs", "d.rs"),
            edge("c.rs", "d.rs"),
        ];
        let (levels, max) = compute_levels(&edges);
        assert_eq!(levels["d.rs"], 0);
        assert_eq!(levels["b.rs"], 1);
        assert_eq!(levels["c.rs"], 1);
        assert_eq!(levels["a.rs"], 2);
        assert_eq!(max, 2);
    }

    #[test]
    fn levels_empty() {
        let (levels, max) = compute_levels(&[]);
        assert!(levels.is_empty());
        assert_eq!(max, 0);
    }

    // ── Upward violations ──

    #[test]
    fn no_violations_in_clean_chain() {
        let edges = vec![edge("a.rs", "b.rs"), edge("b.rs", "c.rs")];
        let (levels, _) = compute_levels(&edges);
        let violations = find_upward_violations(&edges, &levels);
        assert!(violations.is_empty(), "clean chain has no upward violations");
    }

    // ── Blast radius ──

    #[test]
    fn blast_radius_linear() {
        let edges = vec![edge("a.rs", "b.rs"), edge("b.rs", "c.rs")];
        let radius = compute_blast_radius(&edges);
        assert_eq!(radius["c.rs"], 2, "c affects a and b");
        assert_eq!(radius["b.rs"], 1, "b affects only a");
        assert_eq!(radius["a.rs"], 0, "a affects nobody");
    }

    #[test]
    fn blast_radius_star() {
        let edges = vec![
            edge("a.rs", "x.rs"),
            edge("b.rs", "x.rs"),
            edge("c.rs", "x.rs"),
        ];
        let radius = compute_blast_radius(&edges);
        assert_eq!(radius["x.rs"], 3);
        assert_eq!(radius["a.rs"], 0);
    }

    #[test]
    fn blast_radius_empty() {
        let radius = compute_blast_radius(&[]);
        assert!(radius.is_empty());
    }

    // ── Attack surface (probe-only test, sentrux upstream tests use Snapshot) ──

    #[test]
    fn attack_surface_reaches_from_entry() {
        let edges = vec![
            edge("main.rs", "lib.rs"),
            edge("lib.rs", "util.rs"),
            edge("orphan.rs", "stub.rs"),
        ];
        let entries = vec![EntryPoint {
            file: "main.rs".to_string(),
            func: "main".to_string(),
            lang: "rust".to_string(),
            confidence: "high".to_string(),
        }];
        let (reachable, total) = compute_attack_surface(&edges, &entries);
        assert_eq!(total, 5, "5 distinct files in graph");
        assert_eq!(reachable, 3, "main + lib + util reachable; orphan + stub not");
    }
}
