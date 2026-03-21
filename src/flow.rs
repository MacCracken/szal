//! Flow definitions — how steps are wired together.
//!
//! ```
//! use szal::flow::{FlowDef, FlowMode};
//! use szal::step::StepDef;
//!
//! let build = StepDef::new("build");
//! let test = StepDef::new("test").depends_on(build.id);
//! let deploy = StepDef::new("deploy").depends_on(test.id);
//!
//! let mut flow = FlowDef::new("pipeline", FlowMode::Dag);
//! flow.add_step(build);
//! flow.add_step(test);
//! flow.add_step(deploy);
//! flow.validate().unwrap();
//! ```

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::step::StepDef;

pub type FlowId = Uuid;

/// Execution mode for a flow.
///
/// ```
/// use szal::flow::FlowMode;
///
/// assert_eq!(FlowMode::Dag.to_string(), "dag");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowMode {
    /// Steps run one after another.
    Sequential,
    /// Steps run concurrently (no dependencies).
    Parallel,
    /// Steps run based on dependency graph (Kahn's algorithm).
    Dag,
    /// Manager step delegates to sub-steps dynamically.
    Hierarchical,
}

impl std::fmt::Display for FlowMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequential => write!(f, "sequential"),
            Self::Parallel => write!(f, "parallel"),
            Self::Dag => write!(f, "dag"),
            Self::Hierarchical => write!(f, "hierarchical"),
        }
    }
}

/// A workflow flow — a collection of steps with an execution mode.
///
/// ```
/// use szal::flow::{FlowDef, FlowMode};
///
/// let flow = FlowDef::new("deploy", FlowMode::Parallel)
///     .with_rollback()
///     .with_timeout(120_000);
///
/// assert!(flow.rollback_on_failure);
/// assert_eq!(flow.timeout_ms, Some(120_000));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDef {
    pub id: FlowId,
    pub name: String,
    pub description: String,
    pub mode: FlowMode,
    pub steps: Vec<StepDef>,
    /// Whether to rollback completed steps on failure.
    pub rollback_on_failure: bool,
    /// Maximum total flow duration in milliseconds.
    pub timeout_ms: Option<u64>,
}

impl FlowDef {
    pub fn new(name: impl Into<String>, mode: FlowMode) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: String::new(),
            mode,
            steps: Vec::new(),
            rollback_on_failure: false,
            timeout_ms: None,
        }
    }

    pub fn add_step(&mut self, step: StepDef) {
        self.steps.push(step);
    }

    pub fn with_rollback(mut self) -> Self {
        self.rollback_on_failure = true;
        self
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Validate the flow (check for cycles in DAG mode, etc.)
    pub fn validate(&self) -> crate::Result<()> {
        if self.mode == FlowMode::Dag {
            self.check_cycles()?;
        }
        Ok(())
    }

    fn check_cycles(&self) -> crate::Result<()> {
        use std::collections::{HashMap, HashSet};

        let id_set: HashSet<_> = self.steps.iter().map(|s| s.id).collect();
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let deps: HashMap<_, _> = self.steps.iter().map(|s| (s.id, &s.depends_on)).collect();

        fn dfs(
            node: uuid::Uuid,
            deps: &HashMap<uuid::Uuid, &Vec<uuid::Uuid>>,
            visited: &mut HashSet<uuid::Uuid>,
            in_stack: &mut HashSet<uuid::Uuid>,
        ) -> bool {
            visited.insert(node);
            in_stack.insert(node);
            if let Some(neighbors) = deps.get(&node) {
                for &n in *neighbors {
                    if !visited.contains(&n) {
                        if dfs(n, deps, visited, in_stack) {
                            return true;
                        }
                    } else if in_stack.contains(&n) {
                        return true;
                    }
                }
            }
            in_stack.remove(&node);
            false
        }

        for step in &self.steps {
            if !visited.contains(&step.id) && dfs(step.id, &deps, &mut visited, &mut in_stack) {
                return Err(crate::SzalError::CycleDetected(self.name.clone()));
            }
        }

        // Check for references to non-existent steps
        for step in &self.steps {
            for dep in &step.depends_on {
                if !id_set.contains(dep) {
                    return Err(crate::SzalError::InvalidFlow(format!(
                        "step '{}' depends on non-existent step",
                        step.name
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_sequential() {
        let mut flow = FlowDef::new("deploy", FlowMode::Sequential);
        flow.add_step(StepDef::new("build"));
        flow.add_step(StepDef::new("test"));
        flow.add_step(StepDef::new("deploy"));
        assert_eq!(flow.steps.len(), 3);
        assert_eq!(flow.mode, FlowMode::Sequential);
    }

    #[test]
    fn flow_dag_valid() {
        let build = StepDef::new("build");
        let test = StepDef::new("test").depends_on(build.id);
        let deploy = StepDef::new("deploy").depends_on(test.id);

        let mut flow = FlowDef::new("pipeline", FlowMode::Dag);
        flow.add_step(build);
        flow.add_step(test);
        flow.add_step(deploy);
        assert!(flow.validate().is_ok());
    }

    #[test]
    fn flow_dag_cycle_detected() {
        let mut a = StepDef::new("a");
        let mut b = StepDef::new("b");
        b.depends_on = vec![a.id];
        a.depends_on = vec![b.id]; // cycle!

        let mut flow = FlowDef::new("broken", FlowMode::Dag);
        flow.add_step(a);
        flow.add_step(b);
        assert!(flow.validate().is_err());
    }

    #[test]
    fn flow_mode_display() {
        assert_eq!(FlowMode::Sequential.to_string(), "sequential");
        assert_eq!(FlowMode::Dag.to_string(), "dag");
    }

    #[test]
    fn flow_builder() {
        let flow = FlowDef::new("test", FlowMode::Parallel)
            .with_rollback()
            .with_timeout(120_000);
        assert!(flow.rollback_on_failure);
        assert_eq!(flow.timeout_ms, Some(120_000));
    }

    #[test]
    fn flow_serde_roundtrip() {
        let build = StepDef::new("build");
        let test = StepDef::new("test").depends_on(build.id);
        let mut flow = FlowDef::new("pipeline", FlowMode::Dag).with_rollback();
        flow.add_step(build);
        flow.add_step(test);
        let json = serde_json::to_string(&flow).unwrap();
        let back: FlowDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "pipeline");
        assert_eq!(back.steps.len(), 2);
        assert!(back.rollback_on_failure);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn linear_dag_never_cycles(n in 2usize..50) {
            let mut flow = FlowDef::new("linear", FlowMode::Dag);
            let mut prev_id = None;
            for i in 0..n {
                let mut step = StepDef::new(format!("s{i}"));
                if let Some(pid) = prev_id {
                    step = step.depends_on(pid);
                }
                prev_id = Some(step.id);
                flow.add_step(step);
            }
            prop_assert!(flow.validate().is_ok());
        }

        #[test]
        fn fanout_dag_never_cycles(leaves in 2usize..50) {
            let root = StepDef::new("root");
            let root_id = root.id;
            let mut flow = FlowDef::new("fanout", FlowMode::Dag);
            flow.add_step(root);
            for i in 0..leaves {
                flow.add_step(StepDef::new(format!("leaf{i}")).depends_on(root_id));
            }
            prop_assert!(flow.validate().is_ok());
        }

        #[test]
        fn sequential_always_valid(n in 1usize..50) {
            let mut flow = FlowDef::new("seq", FlowMode::Sequential);
            for i in 0..n {
                flow.add_step(StepDef::new(format!("s{i}")));
            }
            prop_assert!(flow.validate().is_ok());
        }
    }
}
