// SPDX-License-Identifier: MIT
//
// Lifted (with adaptation) from sentrux-core (MIT, 2026 Sentrux):
//   https://github.com/2233admin/sentrux
//
// See workspace-root NOTICE for full attribution.

//! CodeNexus structural metrics.
//!
//! Phase 04.5-02a (this commit) lifts only `arch` graph algorithms.
//! Phase 04.5-02b will add `evo`, `dsm`, `rules` plus a Snapshot adapter
//! against `codenexus_core::storage::Store` and an A2A `query_metrics` op.

pub mod arch;
pub mod types;

pub use types::{EntryPoint, ImportEdge};
