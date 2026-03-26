use crate::SzalError;
use crate::bus::WorkflowEvent;
use crate::step::{StepDef, StepResult, StepStatus};

use super::{EventSink, FlowCtx, StepHandler, emit};

pub(crate) async fn execute_step_with_handler(
    step: &StepDef,
    handler: &StepHandler,
    event_sink: &EventSink,
    flow: FlowCtx<'_>,
    #[cfg(feature = "majra")] metrics: &crate::metrics::MetricsSink,
) -> StepResult {
    let max_attempts = step.max_retries + 1;
    let mut last_error = None;
    let total_start = std::time::Instant::now();
    let step_id_str = step.id.to_string();

    tracing::debug!(step = %step.name, flow_id = %flow.id, flow = %flow.name, attempt = 1, "starting step execution");
    emit(
        event_sink,
        WorkflowEvent::step_started(&step.name, &step_id_str),
    );
    #[cfg(feature = "majra")]
    crate::metrics::metric_step_started(metrics, flow.name, &step.name);

    for attempt in 1..=max_attempts {
        let step_start = std::time::Instant::now();

        let fut = (handler)(step.clone());
        let result = if step.timeout_ms < u64::MAX {
            match tokio::time::timeout(std::time::Duration::from_millis(step.timeout_ms), fut).await
            {
                Ok(r) => r,
                Err(_) => {
                    tracing::warn!(step = %step.name, flow_id = %flow.id, flow = %flow.name, timeout_ms = step.timeout_ms, "step timed out");
                    let err = SzalError::StepTimeout {
                        step: step.name.clone(),
                        timeout_ms: step.timeout_ms,
                    };
                    emit(
                        event_sink,
                        WorkflowEvent::step_failed(
                            &step.name,
                            &step_id_str,
                            &err.to_string(),
                            attempt,
                        )
                        .with_duration(step_start.elapsed().as_millis() as u64),
                    );
                    Err(err.to_string())
                }
            }
        } else {
            fut.await
        };

        let duration_ms = step_start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                tracing::debug!(
                    step = %step.name,
                    flow_id = %flow.id,
                    flow = %flow.name,
                    duration_ms,
                    attempts = attempt,
                    "step completed successfully"
                );
                emit(
                    event_sink,
                    WorkflowEvent::step_completed(&step.name, &step_id_str, duration_ms, attempt),
                );
                #[cfg(feature = "majra")]
                crate::metrics::metric_step_finished(
                    metrics,
                    flow.name,
                    &step.name,
                    "completed",
                    duration_ms,
                );
                return StepResult {
                    step_id: step.id,
                    status: StepStatus::Completed,
                    output,
                    duration_ms,
                    attempts: attempt,
                    error: None,
                };
            }
            Err(e) => {
                if attempt < max_attempts {
                    tracing::warn!(
                        step = %step.name,
                        flow_id = %flow.id,
                        flow = %flow.name,
                        attempt,
                        error = %e,
                        "step failed, retrying"
                    );
                    emit(
                        event_sink,
                        WorkflowEvent::step_retry(&step.name, &step_id_str, attempt),
                    );
                }
                last_error = Some(e);
                if attempt < max_attempts {
                    tokio::time::sleep(std::time::Duration::from_millis(step.retry_delay_ms)).await;
                }
            }
        }
    }

    tracing::error!(
        step = %step.name,
        flow_id = %flow.id,
        flow = %flow.name,
        attempts = max_attempts,
        "step failed after all retries exhausted"
    );

    let error = if max_attempts > 1 {
        Some(
            SzalError::RetryExhausted {
                step: step.name.clone(),
                attempts: max_attempts,
            }
            .to_string(),
        )
    } else {
        last_error.clone()
    };

    let total_duration_ms = total_start.elapsed().as_millis() as u64;

    emit(
        event_sink,
        WorkflowEvent::step_failed(
            &step.name,
            &step_id_str,
            error.as_deref().unwrap_or("unknown"),
            max_attempts,
        ),
    );
    #[cfg(feature = "majra")]
    crate::metrics::metric_step_finished(
        metrics,
        flow.name,
        &step.name,
        "failed",
        total_duration_ms,
    );

    StepResult {
        step_id: step.id,
        status: StepStatus::Failed,
        output: serde_json::json!(null),
        duration_ms: total_duration_ms,
        attempts: max_attempts,
        error,
    }
}
