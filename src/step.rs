//! Workflow steps — the atomic unit of work.
//!
//! ```
//! use szal::step::StepDef;
//!
//! let build = StepDef::new("build").with_timeout(60_000);
//! let test = StepDef::new("test")
//!     .depends_on(build.id)
//!     .with_retries(2, 1_000);
//! let deploy = StepDef::new("deploy")
//!     .depends_on(test.id)
//!     .with_rollback();
//!
//! assert_eq!(deploy.depends_on.len(), 1);
//! assert!(deploy.rollbackable);
//! ```

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type StepId = Uuid;

/// Trigger mode for DAG dependency resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum TriggerMode {
    /// Wait for all dependencies to complete (default).
    #[default]
    All,
    /// Ready when any single dependency completes.
    Any,
}

/// Step execution status.
///
/// ```
/// use szal::step::StepStatus;
///
/// let status = StepStatus::Completed;
/// assert_eq!(status.to_string(), "completed");
/// ```
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
///
/// Use the builder pattern to configure:
///
/// ```
/// use szal::step::StepDef;
///
/// let step = StepDef::new("deploy")
///     .with_timeout(60_000)
///     .with_retries(3, 5_000)
///     .with_rollback();
///
/// assert_eq!(step.name, "deploy");
/// assert_eq!(step.timeout_ms, 60_000);
/// assert_eq!(step.max_retries, 3);
/// assert!(step.rollbackable);
/// ```
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
    /// Step type for handler dispatch (e.g. "http", "bash", "webhook").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_type: Option<String>,
    /// Arbitrary configuration passed to the step handler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    /// Condition expression evaluated before execution.
    /// When set and evaluates to false, the step is skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Steps that must complete before this one (DAG edges).
    pub depends_on: Vec<StepId>,
    /// Trigger mode for DAG dependencies.
    #[serde(default)]
    pub trigger_mode: TriggerMode,
    /// Sub-steps for hierarchical execution.
    /// When this step completes successfully, sub-steps are executed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_steps: Vec<StepDef>,
    /// Hardware accelerator requirement for this step.
    #[cfg(feature = "hardware")]
    #[serde(default)]
    pub hardware: ai_hwaccel::AcceleratorRequirement,
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
            step_type: None,
            config: None,
            condition: None,
            depends_on: Vec::new(),
            trigger_mode: TriggerMode::All,
            sub_steps: Vec::new(),
            #[cfg(feature = "hardware")]
            hardware: ai_hwaccel::AcceleratorRequirement::None,
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

    pub fn with_step_type(mut self, step_type: impl Into<String>) -> Self {
        self.step_type = Some(step_type.into());
        self
    }

    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_condition(mut self, expr: impl Into<String>) -> Self {
        self.condition = Some(expr.into());
        self
    }

    pub fn with_trigger_mode(mut self, mode: TriggerMode) -> Self {
        self.trigger_mode = mode;
        self
    }

    pub fn with_sub_step(mut self, step: StepDef) -> Self {
        self.sub_steps.push(step);
        self
    }

    /// Set the hardware accelerator requirement for this step.
    #[cfg(feature = "hardware")]
    pub fn with_hardware(mut self, req: ai_hwaccel::AcceleratorRequirement) -> Self {
        self.hardware = req;
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

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn serde_roundtrip_any(
            name in "[a-z][a-z0-9_-]{0,30}",
            timeout in 1u64..600_000,
            retries in 0u32..10,
            delay in 0u64..60_000,
        ) {
            let step = StepDef::new(name.clone())
                .with_timeout(timeout)
                .with_retries(retries, delay);
            let json = serde_json::to_string(&step).unwrap();
            let back: StepDef = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(&back.name, &name);
            prop_assert_eq!(back.timeout_ms, timeout);
            prop_assert_eq!(back.max_retries, retries);
            prop_assert_eq!(back.retry_delay_ms, delay);
        }

        #[test]
        fn builder_preserves_id(
            name in "[a-z][a-z0-9_-]{0,30}",
        ) {
            let step = StepDef::new(name);
            let id = step.id;
            let step = step.with_timeout(5000).with_retries(2, 1000).with_rollback();
            prop_assert_eq!(step.id, id);
        }
    }
}
