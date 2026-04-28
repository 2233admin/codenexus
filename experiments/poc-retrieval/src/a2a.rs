//! Google A2A v0.2 protocol envelope types for the CodeNexus Rust core
//! HTTP endpoint (REQ-06). Operation surface is the 4 envelopes documented
//! in ARCHITECTURE.md §3.5: index_repo, query, get_symbol, list_callers.
//!
//! State machine: Submitted -> Working -> {Completed | Failed}
//! Long-running ops (index_repo) return Submitted immediately and clients
//! poll /tasks/{id} until Completed/Failed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Submitted,
    Working,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub state: TaskState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub operation: OperationRequest,
    /// Populated when state == Completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<OperationResponse>,
    /// Populated when state == Failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Task {
    pub fn new(op: OperationRequest) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            state: TaskState::Submitted,
            created_at: now,
            updated_at: now,
            operation: op,
            result: None,
            error: None,
        }
    }
}

/// 4 operation envelopes per ARCHITECTURE.md §3.5. Externally tagged
/// `{"operation": {"query": {...}}}` for clean A2A client compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationRequest {
    IndexRepo {
        repo: String,
        /// R4 (Phase 4 first slice, Plan 04-02 v2): per-call override for
        /// the consecutive-embed-failure abort threshold in the server.rs
        /// IndexRepo handler. None means "use the hardcoded const = 5".
        ///
        /// **Schema versioning, not A2A metadata pass-through (M6 fix):**
        /// This is an extension to CodeNexus's own typed `OperationRequest`
        /// enum, with `#[serde(default)]` for back-compat -- existing A2A
        /// clients that don't know about this field still produce valid
        /// JSON (deserializes to `None`). The G-04 spec read confirms A2A
        /// 1.0 free-form `metadata` would ALSO have allowed it (positive
        /// knowledge for future per-task metadata extension), but our
        /// chosen path is operation-schema versioning.
        #[serde(default)]
        max_consecutive_fail: Option<usize>,
    },
    Query {
        text: String,
        #[serde(default = "default_top")]
        top: usize,
        #[serde(default = "default_alpha")]
        alpha: f32,
        #[serde(default)]
        rerank: bool,
    },
    GetSymbol {
        id: i64,
    },
    ListCallers {
        name: String,
        #[serde(default = "default_top")]
        top: usize,
    },
}

fn default_top() -> usize {
    5
}
fn default_alpha() -> f32 {
    0.6
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationResponse {
    IndexRepo {
        symbols_indexed: usize,
        edges_built: usize,
    },
    Query {
        hits: Vec<HitView>,
    },
    GetSymbol {
        symbol: Option<SymbolView>,
    },
    ListCallers {
        callers: Vec<CallerView>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitView {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolView {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallerView {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub ppr_score: f64,
    /// Highest edge confidence observed on any Calls edge from this caller to
    /// the queried target (per ARCHITECTURE.md §9.7; default filter ≥ 0.5).
    /// When multiple edges between the same pair exist (different EdgeKind or
    /// resolver passes), we surface the maximum so consumers see the strongest
    /// evidence for "X calls Y". Phase 4 Leiden can reuse this directly as
    /// edge weight — adding it now is the cheapest window in the project's
    /// lifetime (no Go-side deserializer in flight).
    pub confidence: f64,
}

/// Wrapper struct for POST /tasks/send body. Keeps `operation` keyed at
/// the top level so future fields (auth, trace-id, etc.) can land beside it.
#[derive(Debug, Deserialize)]
pub struct TaskSendBody {
    pub operation: OperationRequest,
}
