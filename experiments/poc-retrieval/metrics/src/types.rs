// SPDX-License-Identifier: MIT
//
// Lifted from sentrux-core/src/core/types.rs (ImportEdge, EntryPoint) +
// sentrux-core/src/metrics/types.rs (is_mod_declaration_edge stub).

//! Minimal type contract used by `arch` graph algorithms.
//!
//! These shapes mirror sentrux's `core::types` so the algorithm code can
//! lift verbatim. Phase 04.5-02b will add a Snapshot adapter that
//! converts codenexus's symbol-id-keyed edges (storage.rs) into
//! file-path-keyed `ImportEdge` for these algorithms to consume.

/// An import dependency edge between two source files.
///
/// File-path keyed (not symbol-id keyed) because the graph algorithms
/// here operate on the file-level dependency DAG, not the symbol-call
/// graph. The Snapshot adapter (04.5-02b) projects symbol-edges to
/// file-edges before invoking these algorithms.
#[derive(Debug, Clone)]
pub struct ImportEdge {
    pub from_file: String,
    pub to_file: String,
}

/// An entry point into the codebase (e.g. `main`, an HTTP handler).
///
/// Used by `compute_attack_surface` to seed the BFS that determines
/// reachable code from public APIs. Only the `file` field is consumed
/// by the algorithms in this slice; `func`/`lang`/`confidence` are
/// forward-compatible scaffolding for 04.5-02b's evo/dsm modules.
#[derive(Debug, Clone)]
pub struct EntryPoint {
    pub file: String,
    pub func: String,
    pub lang: String,
    pub confidence: String,
}

/// Stub: classify an edge as a "mod declaration" edge (Rust `pub mod foo;`
/// from a parent `mod.rs` to its sibling `foo.rs`).
///
/// Sentrux uses this to filter out structural-containment edges from
/// blast-radius computation (a sub-module change does not propagate
/// through the parent's `pub mod` declaration).
///
/// Codenexus targets TypeScript and Python, neither of which has the
/// Rust `mod.rs` pattern, so the safe default for now is "no edge is a
/// mod declaration" — every edge is treated as a functional dependency.
/// Phase 04.5-02b will revisit if Rust support enters scope.
pub(crate) fn is_mod_declaration_edge(_edge: &ImportEdge) -> bool {
    false
}
