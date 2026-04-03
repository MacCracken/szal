use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::bus::WorkflowEvent;
use crate::step::{StepDef, StepResult, StepStatus};

use super::step_exec::execute_step_with_handler;
use super::{ExecCtx, FlowCtx, emit};

pub(crate) async fn run_parallel(
    steps: &[StepDef],
    max_concurrency: usize,
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&CancellationToken>,
    ctx: &ExecCtx<'_>,
) -> Vec<StepResult> {
    tracing::debug!(steps = steps.len(), flow_id = %ctx.flow.id, flow = %ctx.flow.name, "running parallel execution");
    let sem = Arc::new(Semaphore::new(max_concurrency.max(1)));
    let mut handles = Vec::with_capacity(steps.len());

    let mut step_ids = Vec::with_capacity(steps.len());
    let mut step_names: Vec<String> = Vec::with_capacity(steps.len());
    let flow_name: Arc<str> = ctx.flow.name.into();
    let flow_id = ctx.flow.id;
    let mut pre_skipped: Vec<StepResult> = Vec::new();
    for step in steps {
        // Condition evaluation (before spawning — no sibling results available)
        if let Some(ref _cond) = step.condition {
            match crate::engine::check_condition(step, &pre_skipped, steps) {
                Ok(false) => {
                    emit(
                        ctx.event_sink,
                        WorkflowEvent::step_skipped(
                            &step.name,
                            &step.id.to_string(),
                            "condition not met",
                        ),
                    );
                    pre_skipped.push(StepResult {
                        step_id: step.id,
                        status: StepStatus::Skipped,
                        output: serde_json::json!(null),
                        duration_ms: 0,
                        attempts: 0,
                        error: Some("condition not met".into()),
                    });
                    continue;
                }
                Err(e) => {
                    tracing::warn!(step = %step.name, error = %e, "condition evaluation failed");
                }
                Ok(true) => {}
            }
        }
        step_ids.push(step.id);
        step_names.push(step.name.clone());
        let sem = sem.clone();
        let handler = ctx.handler.clone();
        let step = step.clone();
        let sink = ctx.event_sink.clone();
        let fname = Arc::clone(&flow_name);
        #[cfg(feature = "majra")]
        let metrics = ctx.metrics.clone();
        let stm = ctx.step_type_metrics.clone();
        let psink = ctx.progress_sink.clone();
        handles.push(tokio::spawn(async move {
            let _permit = match sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    return StepResult {
                        step_id: step.id,
                        status: StepStatus::Failed,
                        output: serde_json::json!(null),
                        duration_ms: 0,
                        attempts: 0,
                        error: Some("semaphore closed".into()),
                    };
                }
            };
            let flow = FlowCtx {
                name: &fname,
                id: flow_id,
            };
            execute_step_with_handler(
                &step,
                &handler,
                &sink,
                flow,
                #[cfg(feature = "majra")]
                &metrics,
                &stm,
                &psink,
            )
            .await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for ((handle, original_id), name) in handles.into_iter().zip(step_ids).zip(step_names) {
        let cancelled = token.is_some_and(|t| t.is_cancelled());
        if cancelled || start.elapsed().as_millis() as u64 > timeout_ms {
            handle.abort();
            let reason = if cancelled {
                "cancelled"
            } else {
                "flow timeout exceeded"
            };
            emit(
                ctx.event_sink,
                WorkflowEvent::step_skipped(&name, &original_id.to_string(), reason),
            );
            results.push(StepResult {
                step_id: original_id,
                status: StepStatus::Skipped,
                output: serde_json::json!(null),
                duration_ms: 0,
                attempts: 0,
                error: Some(reason.into()),
            });
            continue;
        }
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                tracing::error!(step_id = %original_id, flow_id = %ctx.flow.id, flow = %ctx.flow.name, error = %e, "spawned task panicked");
                results.push(StepResult {
                    step_id: original_id,
                    status: StepStatus::Failed,
                    output: serde_json::json!(null),
                    duration_ms: 0,
                    attempts: 0,
                    error: Some(format!("task panicked: {e}")),
                });
            }
        }
    }
    // Prepend condition-skipped results
    pre_skipped.extend(results);
    pre_skipped
}
