use crate::flow::{FlowDef, FlowMode};
use crate::step::{StepDef, StepResult, StepStatus};
use tokio_util::sync::CancellationToken;

use super::result::FlowResult;
use super::{EngineConfig, RollbackHandler, StepHandler};
use super::{dag, parallel, sequential};

/// The workflow execution engine.
pub struct Engine {
    config: EngineConfig,
    handler: StepHandler,
    rollback_handler: Option<RollbackHandler>,
}

impl Engine {
    /// Create an engine with a step handler.
    pub fn new(config: EngineConfig, handler: StepHandler) -> Self {
        Self {
            config,
            handler,
            rollback_handler: None,
        }
    }

    /// Set a rollback handler for steps that support rollback.
    pub fn with_rollback_handler(mut self, handler: RollbackHandler) -> Self {
        self.rollback_handler = Some(handler);
        self
    }

    /// Execute a flow and return the result.
    #[tracing::instrument(skip(self, flow), fields(flow = %flow.name, mode = %flow.mode))]
    pub async fn run(&self, flow: &FlowDef) -> crate::Result<FlowResult> {
        flow.validate()?;

        #[cfg(feature = "hardware")]
        if let Some(ref hw) = self.config.hardware {
            hw.check_requirements(&flow.steps)?;
        }

        tracing::info!(flow = %flow.name, steps = flow.steps.len(), "starting flow execution");

        let timeout = self
            .config
            .global_timeout_ms
            .or(flow.timeout_ms)
            .unwrap_or(u64::MAX);

        let start = std::time::Instant::now();

        let step_results = match flow.mode {
            FlowMode::Sequential | FlowMode::Hierarchical => {
                sequential::run_sequential(&flow.steps, &self.handler, timeout, start, None).await
            }
            FlowMode::Parallel => {
                parallel::run_parallel(
                    &flow.steps,
                    &self.handler,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    None,
                )
                .await
            }
            FlowMode::Dag => {
                dag::run_dag(
                    &flow.steps,
                    &self.handler,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    None,
                )
                .await
            }
        };

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let has_failures = step_results.iter().any(|r| r.status == StepStatus::Failed);
        let mut rolled_back = false;

        // Rollback on failure if configured
        if has_failures && flow.rollback_on_failure {
            rolled_back = self.rollback_completed_steps(flow, &step_results).await;
        }

        let result_status = if has_failures {
            if rolled_back { "rolled_back" } else { "failed" }
        } else {
            "success"
        };
        tracing::info!(
            flow = %flow.name,
            duration_ms = total_duration_ms,
            steps = step_results.len(),
            result = result_status,
            "flow execution completed"
        );

        Ok(FlowResult {
            flow_name: flow.name.clone(),
            steps: step_results,
            total_duration_ms,
            success: !has_failures,
            rolled_back,
        })
    }

    /// Execute a flow with cancellation support.
    ///
    /// Behaves identically to [`run`](Self::run) but checks the provided
    /// [`CancellationToken`] between steps. When the token is cancelled,
    /// remaining steps are marked [`StepStatus::Skipped`].
    #[tracing::instrument(skip(self, flow, token), fields(flow = %flow.name, mode = %flow.mode))]
    pub async fn run_with_cancellation(
        &self,
        flow: &FlowDef,
        token: CancellationToken,
    ) -> crate::Result<FlowResult> {
        flow.validate()?;

        #[cfg(feature = "hardware")]
        if let Some(ref hw) = self.config.hardware {
            hw.check_requirements(&flow.steps)?;
        }

        tracing::info!(flow = %flow.name, steps = flow.steps.len(), "starting flow execution (cancellable)");

        let timeout = self
            .config
            .global_timeout_ms
            .or(flow.timeout_ms)
            .unwrap_or(u64::MAX);

        let start = std::time::Instant::now();

        let step_results = match flow.mode {
            FlowMode::Sequential | FlowMode::Hierarchical => {
                sequential::run_sequential(&flow.steps, &self.handler, timeout, start, Some(&token))
                    .await
            }
            FlowMode::Parallel => {
                parallel::run_parallel(
                    &flow.steps,
                    &self.handler,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    Some(&token),
                )
                .await
            }
            FlowMode::Dag => {
                dag::run_dag(
                    &flow.steps,
                    &self.handler,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    Some(&token),
                )
                .await
            }
        };

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let has_failures = step_results.iter().any(|r| r.status == StepStatus::Failed);
        let was_cancelled = token.is_cancelled()
            && step_results.iter().any(|r| {
                r.status == StepStatus::Skipped && r.error.as_deref() == Some("cancelled")
            });
        let mut rolled_back = false;

        if has_failures && flow.rollback_on_failure {
            rolled_back = self.rollback_completed_steps(flow, &step_results).await;
        }

        let success = !has_failures && !was_cancelled;
        let result_status = if was_cancelled {
            "cancelled"
        } else if has_failures {
            if rolled_back { "rolled_back" } else { "failed" }
        } else {
            "success"
        };
        tracing::info!(
            flow = %flow.name,
            duration_ms = total_duration_ms,
            steps = step_results.len(),
            result = result_status,
            "flow execution completed"
        );

        Ok(FlowResult {
            flow_name: flow.name.clone(),
            steps: step_results,
            total_duration_ms,
            success,
            rolled_back,
        })
    }

    async fn rollback_completed_steps(&self, flow: &FlowDef, step_results: &[StepResult]) -> bool {
        let Some(ref rollback_handler) = self.rollback_handler else {
            return false;
        };

        let completed_steps: Vec<&StepDef> = flow
            .steps
            .iter()
            .filter(|s| {
                s.rollbackable
                    && step_results
                        .iter()
                        .any(|r| r.step_id == s.id && r.status == StepStatus::Completed)
            })
            .collect();

        tracing::info!(flow = %flow.name, steps = completed_steps.len(), "starting rollback");
        let mut all_rolled_back = true;
        for step in completed_steps.into_iter().rev() {
            if (rollback_handler)(step.clone()).await.is_err() {
                tracing::warn!(step = %step.name, "rollback step failed");
                all_rolled_back = false;
            }
        }
        tracing::info!(flow = %flow.name, success = all_rolled_back, "rollback completed");
        all_rolled_back
    }
}
