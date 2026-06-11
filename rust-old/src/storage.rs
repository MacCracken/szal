//! Workflow storage trait for dynamic subworkflow resolution.
//!
//! Consumers implement [`WorkflowStorage`] to enable runtime workflow lookup
//! by name or ID. Step handlers capture `Arc<dyn WorkflowStorage>` to fetch
//! and execute sub-workflows dynamically.
//!
//! ## Example
//!
//! ```
//! use szal::storage::WorkflowStorage;
//! use szal::flow::FlowDef;
//! use std::collections::HashMap;
//! use std::sync::RwLock;
//!
//! struct InMemoryStorage {
//!     flows: RwLock<HashMap<String, FlowDef>>,
//! }
//!
//! impl WorkflowStorage for InMemoryStorage {
//!     fn get_by_name(&self, name: &str) -> Option<FlowDef> {
//!         self.flows.read().unwrap().get(name).cloned()
//!     }
//!
//!     fn get_by_id(&self, id: &str) -> Option<FlowDef> {
//!         let uuid: uuid::Uuid = id.parse().ok()?;
//!         self.flows.read().unwrap().values().find(|f| f.id == uuid).cloned()
//!     }
//!
//!     fn list(&self) -> Vec<String> {
//!         self.flows.read().unwrap().keys().cloned().collect()
//!     }
//! }
//! ```

use crate::flow::FlowDef;

/// Trait for runtime workflow resolution.
///
/// Consumers implement this to allow step handlers to dynamically look up
/// and execute sub-workflows by name or ID.
pub trait WorkflowStorage: Send + Sync {
    /// Look up a workflow definition by name.
    fn get_by_name(&self, name: &str) -> Option<FlowDef>;

    /// Look up a workflow definition by ID (UUID string).
    fn get_by_id(&self, id: &str) -> Option<FlowDef>;

    /// List all available workflow names.
    fn list(&self) -> Vec<String>;
}

/// In-memory workflow storage backed by a `HashMap`.
///
/// Suitable for testing and small deployments.
pub struct InMemoryStorage {
    flows: std::sync::RwLock<std::collections::HashMap<String, FlowDef>>,
}

impl InMemoryStorage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            flows: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Store a workflow definition, keyed by its name.
    pub fn insert(&self, flow: FlowDef) {
        self.flows
            .write()
            .expect("storage lock poisoned")
            .insert(flow.name.clone(), flow);
    }

    /// Remove a workflow by name.
    pub fn remove(&self, name: &str) -> Option<FlowDef> {
        self.flows
            .write()
            .expect("storage lock poisoned")
            .remove(name)
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowStorage for InMemoryStorage {
    fn get_by_name(&self, name: &str) -> Option<FlowDef> {
        self.flows
            .read()
            .expect("storage lock poisoned")
            .get(name)
            .cloned()
    }

    fn get_by_id(&self, id: &str) -> Option<FlowDef> {
        let uuid: uuid::Uuid = id.parse().ok()?;
        self.flows
            .read()
            .expect("storage lock poisoned")
            .values()
            .find(|f| f.id == uuid)
            .cloned()
    }

    fn list(&self) -> Vec<String> {
        self.flows
            .read()
            .expect("storage lock poisoned")
            .keys()
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Execution state persistence
// ---------------------------------------------------------------------------

use crate::engine::FlowResult;
use crate::state::WorkflowState;

/// A snapshot of a running or completed flow execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionRecord {
    /// Unique execution ID.
    pub execution_id: String,
    /// Name of the flow being executed.
    pub flow_name: String,
    /// Current workflow state.
    pub state: WorkflowState,
    /// Step results accumulated so far.
    pub result: Option<FlowResult>,
    /// When the execution started (UTC ISO-8601).
    pub started_at: String,
    /// When the execution finished (UTC ISO-8601), if terminal.
    pub finished_at: Option<String>,
}

/// Trait for persisting workflow execution state.
///
/// Consumers implement this to enable durable execution tracking — for
/// dashboards, auditing, and crash recovery.
pub trait ExecutionStore: Send + Sync {
    /// Save or update an execution record.
    fn save(&self, record: ExecutionRecord);

    /// Load an execution record by ID.
    fn get(&self, execution_id: &str) -> Option<ExecutionRecord>;

    /// List execution IDs, optionally filtered by flow name.
    fn list(&self, flow_name: Option<&str>) -> Vec<String>;

    /// Remove an execution record.
    fn remove(&self, execution_id: &str) -> Option<ExecutionRecord>;
}

/// In-memory execution store for testing.
pub struct InMemoryExecutionStore {
    records: std::sync::RwLock<std::collections::HashMap<String, ExecutionRecord>>,
}

impl InMemoryExecutionStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryExecutionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionStore for InMemoryExecutionStore {
    fn save(&self, record: ExecutionRecord) {
        self.records
            .write()
            .expect("execution store lock poisoned")
            .insert(record.execution_id.clone(), record);
    }

    fn get(&self, execution_id: &str) -> Option<ExecutionRecord> {
        self.records
            .read()
            .expect("execution store lock poisoned")
            .get(execution_id)
            .cloned()
    }

    fn list(&self, flow_name: Option<&str>) -> Vec<String> {
        self.records
            .read()
            .expect("execution store lock poisoned")
            .values()
            .filter(|r| flow_name.is_none_or(|n| r.flow_name == n))
            .map(|r| r.execution_id.clone())
            .collect()
    }

    fn remove(&self, execution_id: &str) -> Option<ExecutionRecord> {
        self.records
            .write()
            .expect("execution store lock poisoned")
            .remove(execution_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::FlowMode;
    use crate::step::StepDef;

    #[test]
    fn in_memory_insert_and_get() {
        let storage = InMemoryStorage::new();
        let mut flow = FlowDef::new("deploy", FlowMode::Sequential);
        flow.add_step(StepDef::new("build"));

        let flow_id = flow.id.to_string();
        storage.insert(flow);

        let by_name = storage.get_by_name("deploy");
        assert!(by_name.is_some());
        assert_eq!(by_name.unwrap().name, "deploy");

        let by_id = storage.get_by_id(&flow_id);
        assert!(by_id.is_some());

        assert!(storage.get_by_name("missing").is_none());
    }

    #[test]
    fn in_memory_list() {
        let storage = InMemoryStorage::new();
        storage.insert(FlowDef::new("a", FlowMode::Sequential));
        storage.insert(FlowDef::new("b", FlowMode::Parallel));

        let names = storage.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
    }

    #[test]
    fn in_memory_remove() {
        let storage = InMemoryStorage::new();
        storage.insert(FlowDef::new("x", FlowMode::Sequential));
        assert!(storage.remove("x").is_some());
        assert!(storage.get_by_name("x").is_none());
    }

    // -- ExecutionStore tests --

    #[test]
    fn execution_store_save_and_get() {
        let store = InMemoryExecutionStore::new();
        store.save(ExecutionRecord {
            execution_id: "exec-1".into(),
            flow_name: "deploy".into(),
            state: WorkflowState::Running,
            result: None,
            started_at: "2026-04-03T00:00:00Z".into(),
            finished_at: None,
        });

        let rec = store.get("exec-1");
        assert!(rec.is_some());
        let rec = rec.unwrap();
        assert_eq!(rec.flow_name, "deploy");
        assert_eq!(rec.state, WorkflowState::Running);
        assert!(rec.result.is_none());
    }

    #[test]
    fn execution_store_list_filters() {
        let store = InMemoryExecutionStore::new();
        store.save(ExecutionRecord {
            execution_id: "e1".into(),
            flow_name: "deploy".into(),
            state: WorkflowState::Completed,
            result: None,
            started_at: String::new(),
            finished_at: None,
        });
        store.save(ExecutionRecord {
            execution_id: "e2".into(),
            flow_name: "test".into(),
            state: WorkflowState::Failed,
            result: None,
            started_at: String::new(),
            finished_at: None,
        });
        store.save(ExecutionRecord {
            execution_id: "e3".into(),
            flow_name: "deploy".into(),
            state: WorkflowState::RolledBack,
            result: None,
            started_at: String::new(),
            finished_at: None,
        });

        assert_eq!(store.list(None).len(), 3);
        assert_eq!(store.list(Some("deploy")).len(), 2);
        assert_eq!(store.list(Some("test")).len(), 1);
        assert_eq!(store.list(Some("missing")).len(), 0);
    }

    #[test]
    fn execution_store_remove() {
        let store = InMemoryExecutionStore::new();
        store.save(ExecutionRecord {
            execution_id: "e1".into(),
            flow_name: "x".into(),
            state: WorkflowState::Created,
            result: None,
            started_at: String::new(),
            finished_at: None,
        });
        assert!(store.remove("e1").is_some());
        assert!(store.get("e1").is_none());
    }
}
