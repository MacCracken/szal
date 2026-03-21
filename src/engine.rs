//! Execution engine — runs flows with retry, timeout, and rollback.
//!
//! ```
//! use szal::engine::EngineConfig;
//!
//! let config = EngineConfig::default();
//! assert_eq!(config.max_concurrency, 16);
//! assert!(config.global_timeout_ms.is_none());
//!
//! let config = EngineConfig {
//!     max_concurrency: 4,
//!     global_timeout_ms: Some(300_000),
//! };
//! assert_eq!(config.max_concurrency, 4);
//! ```

use crate::step::{StepResult, StepStatus};

/// Execution engine configuration.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum concurrent steps (for parallel/DAG modes).
    pub max_concurrency: usize,
    /// Global timeout override (overrides per-flow timeout).
    pub global_timeout_ms: Option<u64>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 16,
            global_timeout_ms: None,
        }
    }
}

/// Result of executing a complete flow.
///
/// ```
/// use szal::engine::FlowResult;
/// use szal::step::{StepResult, StepStatus};
///
/// let result = FlowResult {
///     flow_name: "deploy".into(),
///     steps: vec![
///         StepResult {
///             step_id: uuid::Uuid::new_v4(),
///             status: StepStatus::Completed,
///             output: serde_json::json!({}),
///             duration_ms: 100,
///             attempts: 1,
///             error: None,
///         },
///     ],
///     total_duration_ms: 100,
///     success: true,
///     rolled_back: false,
/// };
/// assert_eq!(result.completed_count(), 1);
/// assert_eq!(result.failed_count(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct FlowResult {
    pub flow_name: String,
    pub steps: Vec<StepResult>,
    pub total_duration_ms: u64,
    pub success: bool,
    pub rolled_back: bool,
}

impl FlowResult {
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Failed)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_config_default() {
        let cfg = EngineConfig::default();
        assert_eq!(cfg.max_concurrency, 16);
        assert!(cfg.global_timeout_ms.is_none());
    }

    #[test]
    fn flow_result_counts() {
        let result = FlowResult {
            flow_name: "test".into(),
            steps: vec![
                StepResult {
                    step_id: uuid::Uuid::new_v4(),
                    status: StepStatus::Completed,
                    output: serde_json::json!({}),
                    duration_ms: 100,
                    attempts: 1,
                    error: None,
                },
                StepResult {
                    step_id: uuid::Uuid::new_v4(),
                    status: StepStatus::Failed,
                    output: serde_json::json!({}),
                    duration_ms: 50,
                    attempts: 3,
                    error: Some("timeout".into()),
                },
            ],
            total_duration_ms: 150,
            success: false,
            rolled_back: false,
        };
        assert_eq!(result.completed_count(), 1);
        assert_eq!(result.failed_count(), 1);
    }
}
