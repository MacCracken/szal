use crate::bus::WorkflowEvent;
use crate::step::{StepDef, StepResult, StepStatus};
use tokio_util::sync::CancellationToken;

use super::step_exec::execute_step_with_handler;
use super::{ExecCtx, emit};

pub(crate) async fn run_sequential(
    steps: &[StepDef],
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&CancellationToken>,
    ctx: &ExecCtx<'_>,
) -> Vec<StepResult> {
    tracing::debug!(steps = steps.len(), flow_id = %ctx.flow.id, flow = %ctx.flow.name, "running sequential execution");
    let mut results = Vec::with_capacity(steps.len());
    let mut failed = false;
    for step in steps {
        let cancelled = token.is_some_and(|t| t.is_cancelled());
        if cancelled || failed {
            let reason = if cancelled {
                "cancelled"
            } else {
                "prior step failed"
            };
            emit(
                ctx.event_sink,
                WorkflowEvent::step_skipped(&step.name, &step.id.to_string(), reason),
            );
            results.push(StepResult {
                step_id: step.id,
                status: StepStatus::Skipped,
                output: serde_json::json!(null),
                duration_ms: 0,
                attempts: 0,
                error: Some(reason.into()),
            });
            continue;
        }
        if start.elapsed().as_millis() as u64 > timeout_ms {
            let reason = "flow timeout exceeded";
            emit(
                ctx.event_sink,
                WorkflowEvent::step_skipped(&step.name, &step.id.to_string(), reason),
            );
            results.push(StepResult {
                step_id: step.id,
                status: StepStatus::Skipped,
                output: serde_json::json!(null),
                duration_ms: 0,
                attempts: 0,
                error: Some(reason.into()),
            });
            continue;
        }
        let result = execute_step_with_handler(step, ctx.handler, ctx.event_sink, ctx.flow).await;
        if result.status == StepStatus::Failed {
            failed = true;
        }
        results.push(result);
    }
    results
}
