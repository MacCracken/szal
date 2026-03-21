//! Workflow steps — the atomic unit of work.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type StepId = Uuid;

/// Step execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    RolledBack,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
            Self::RolledBack => write!(f, "rolled_back"),
        }
    }
}

/// Definition of a workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDef {
    pub id: StepId,
    pub name: String,
    pub description: String,
    /// Maximum execution time in milliseconds.
    pub timeout_ms: u64,
    /// Number of retry attempts on failure.
    pub max_retries: u32,
    /// Delay between retries in milliseconds.
    pub retry_delay_ms: u64,
    /// Whether this step can be rolled back.
    pub rollbackable: bool,
    /// Steps that must complete before this one (DAG edges).
    pub depends_on: Vec<StepId>,
}

impl StepDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: String::new(),
            timeout_ms: 30_000,
            max_retries: 0,
            retry_delay_ms: 1_000,
            rollbackable: false,
            depends_on: Vec::new(),
        }
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    pub fn with_retries(mut self, max: u32, delay_ms: u64) -> Self {
        self.max_retries = max;
        self.retry_delay_ms = delay_ms;
        self
    }

    pub fn with_rollback(mut self) -> Self {
        self.rollbackable = true;
        self
    }

    pub fn depends_on(mut self, step_id: StepId) -> Self {
        self.depends_on.push(step_id);
        self
    }
}

/// Result of executing a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: StepId,
    pub status: StepStatus,
    pub output: serde_json::Value,
    pub duration_ms: u64,
    pub attempts: u32,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_builder() {
        let step = StepDef::new("deploy")
            .with_timeout(60_000)
            .with_retries(3, 5_000)
            .with_rollback();
        assert_eq!(step.name, "deploy");
        assert_eq!(step.timeout_ms, 60_000);
        assert_eq!(step.max_retries, 3);
        assert!(step.rollbackable);
    }

    #[test]
    fn step_dependencies() {
        let a = StepDef::new("build");
        let b = StepDef::new("test").depends_on(a.id);
        assert_eq!(b.depends_on.len(), 1);
        assert_eq!(b.depends_on[0], a.id);
    }

    #[test]
    fn status_display() {
        assert_eq!(StepStatus::Completed.to_string(), "completed");
        assert_eq!(StepStatus::RolledBack.to_string(), "rolled_back");
    }

    #[test]
    fn serde_roundtrip() {
        let step = StepDef::new("test");
        let json = serde_json::to_string(&step).unwrap();
        let back: StepDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
    }
}
