use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::step::{StepDef, StepResult, StepStatus};

use super::StepHandler;
use super::step_exec::execute_step_with_handler;

pub(crate) async fn run_parallel(
    steps: &[StepDef],
    handler: &StepHandler,
    max_concurrency: usize,
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&CancellationToken>,
) -> Vec<StepResult> {
    tracing::debug!(steps = steps.len(), "running parallel execution");
    let sem = Arc::new(Semaphore::new(max_concurrency.max(1)));
    let mut handles = Vec::with_capacity(steps.len());

    let mut step_ids = Vec::with_capacity(steps.len());
    for step in steps {
        step_ids.push(step.id);
        let sem = sem.clone();
        let handler = handler.clone();
        let step = step.clone();
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
            execute_step_with_handler(&step, &handler).await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for (handle, original_id) in handles.into_iter().zip(step_ids) {
        let cancelled = token.is_some_and(|t| t.is_cancelled());
        if cancelled || start.elapsed().as_millis() as u64 > timeout_ms {
            handle.abort();
            results.push(StepResult {
                step_id: original_id,
                status: StepStatus::Skipped,
                output: serde_json::json!(null),
                duration_ms: 0,
                attempts: 0,
                error: Some(if cancelled {
                    "cancelled".into()
                } else {
                    "flow timeout exceeded".into()
                }),
            });
            continue;
        }
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                tracing::error!(step_id = %original_id, error = %e, "spawned task panicked");
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
    results
}
