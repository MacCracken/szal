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
}
