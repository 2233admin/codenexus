//! In-memory task store for the A2A endpoint (REQ-06). Single-process
//! MVP — no persistence, restart loses in-flight tasks. Phase 4+ may add
//! disk-backed task ledger so graceful restart resumes long index_repo ops.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::Utc;
use uuid::Uuid;

use crate::a2a::{OperationRequest, OperationResponse, Task, TaskState};

pub struct TaskStore {
    db_path: String,
    tasks: RwLock<HashMap<Uuid, Task>>,
}

impl TaskStore {
    pub fn new(db_path: String) -> Self {
        Self {
            db_path,
            tasks: RwLock::new(HashMap::new()),
        }
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    /// Submit a fresh task. Returns its assigned UUID. Caller is expected to
    /// kick off the worker (tokio::spawn) and call `mark_working` /
    /// `complete` / `fail` as the operation progresses.
    pub fn submit(&self, op: OperationRequest) -> Task {
        let task = Task::new(op);
        let mut guard = self.tasks.write().expect("task store poisoned");
        guard.insert(task.id, task.clone());
        task
    }

    pub fn get(&self, id: &Uuid) -> Option<Task> {
        self.tasks
            .read()
            .expect("task store poisoned")
            .get(id)
            .cloned()
    }

    pub fn mark_working(&self, id: &Uuid) {
        let mut guard = self.tasks.write().expect("task store poisoned");
        if let Some(t) = guard.get_mut(id) {
            t.state = TaskState::Working;
            t.updated_at = Utc::now();
        }
    }

    pub fn complete(&self, id: &Uuid, result: OperationResponse) {
        let mut guard = self.tasks.write().expect("task store poisoned");
        if let Some(t) = guard.get_mut(id) {
            t.state = TaskState::Completed;
            t.result = Some(result);
            t.updated_at = Utc::now();
        }
    }

    pub fn fail(&self, id: &Uuid, err: String) {
        let mut guard = self.tasks.write().expect("task store poisoned");
        if let Some(t) = guard.get_mut(id) {
            t.state = TaskState::Failed;
            t.error = Some(err);
            t.updated_at = Utc::now();
        }
    }
}
