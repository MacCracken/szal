use std::future::Future;
use std::pin::Pin;

use crate::bus::WorkflowEvent;
use crate::step::{StepDef, StepResult, StepStatus};
use tokio_util::sync::CancellationToken;

use super::step_exec::execute_step_with_handler;
use super::{EventSink, StepHandler, emit};

pub(crate) async fn run_hierarchical(
    steps: &[StepDef],
    handler: &StepHandler,
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&CancellationToken>,
    event_sink: &EventSink,
) -> Vec<StepResult> {
    tracing::debug!(steps = steps.len(), "running hierarchical execution");
    let mut results = Vec::new();
    execute_tree(
        steps,
        handler,
        timeout_ms,
        start,
        token,
        event_sink,
        &mut results,
    )
    .await;
    results
}

fn execute_tree<'a>(
    steps: &'a [StepDef],
    handler: &'a StepHandler,
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&'a CancellationToken>,
    event_sink: &'a EventSink,
    results: &'a mut Vec<StepResult>,
) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        let mut failed = false;
        for step in steps {
            let cancelled = token.is_some_and(|t| t.is_cancelled());
            if cancelled || failed {
                let reason = if cancelled {
                    "cancelled"
                } else {
                    "prior step failed"
                };
                skip_step_and_children(step, reason, event_sink, results);
                continue;
            }
            if start.elapsed().as_millis() as u64 > timeout_ms {
                skip_step_and_children(step, "flow timeout exceeded", event_sink, results);
                continue;
            }

            let result = execute_step_with_handler(step, handler, event_sink).await;
            let succeeded = result.status == StepStatus::Completed;
            results.push(result);

            if succeeded && !step.sub_steps.is_empty() {
                execute_tree(
                    &step.sub_steps,
                    handler,
                    timeout_ms,
                    start,
                    token,
                    event_sink,
                    results,
                )
                .await;
            } else if !succeeded {
                // Skip sub-steps on failure
                skip_children(step, "parent step failed", event_sink, results);
                failed = true;
            }
        }
    })
}

fn skip_step_and_children(
    step: &StepDef,
    reason: &str,
    event_sink: &EventSink,
    results: &mut Vec<StepResult>,
) {
    emit(
        event_sink,
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
    skip_children(step, reason, event_sink, results);
}

fn skip_children(
    step: &StepDef,
    reason: &str,
    event_sink: &EventSink,
    results: &mut Vec<StepResult>,
) {
    for sub in &step.sub_steps {
        skip_step_and_children(sub, reason, event_sink, results);
    }
}
