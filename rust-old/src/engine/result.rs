use crate::step::{StepResult, StepStatus};

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlowResult {
    pub flow_name: String,
    pub steps: Vec<StepResult>,
    pub total_duration_ms: u64,
    pub success: bool,
    pub rolled_back: bool,
}

impl FlowResult {
    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count()
    }

    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Failed)
            .count()
    }

    #[must_use]
    pub fn skipped_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Skipped)
            .count()
    }
}
