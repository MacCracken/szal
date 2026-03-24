use crate::step::{StepDef, StepResult, StepStatus};
use tokio_util::sync::CancellationToken;

use super::StepHandler;
use super::step_exec::execute_step_with_handler;

pub(crate) async fn run_sequential(
    steps: &[StepDef],
    handler: &StepHandler,
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&CancellationToken>,
) -> Vec<StepResult> {
    tracing::debug!(steps = steps.len(), "running sequential execution");
    let mut results = Vec::with_capacity(steps.len());
    let mut failed = false;
    for step in steps {
        let cancelled = token.is_some_and(|t| t.is_cancelled());
        if cancelled || failed {
            results.push(StepResult {
                step_id: step.id,
                status: StepStatus::Skipped,
                output: serde_json::json!(null),
                duration_ms: 0,
                attempts: 0,
                error: Some(if cancelled {
                    "cancelled".into()
                } else {
                    "prior step failed".into()
                }),
            });
            continue;
        }
        if start.elapsed().as_millis() as u64 > timeout_ms {
            results.push(StepResult {
                step_id: step.id,
                status: StepStatus::Skipped,
                output: serde_json::json!(null),
                duration_ms: 0,
                attempts: 0,
                error: Some("flow timeout exceeded".into()),
            });
            continue;
        }
        let result = execute_step_with_handler(step, handler).await;
        if result.status == StepStatus::Failed {
            failed = true;
        }
        results.push(result);
    }
    results
}
