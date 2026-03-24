use crate::step::{StepDef, StepResult, StepStatus};

use super::StepHandler;

pub(crate) async fn execute_step_with_handler(step: &StepDef, handler: &StepHandler) -> StepResult {
    let max_attempts = step.max_retries + 1;
    let mut last_error = None;
    let total_start = std::time::Instant::now();

    tracing::debug!(step = %step.name, attempt = 1, "starting step execution");

    for attempt in 1..=max_attempts {
        let step_start = std::time::Instant::now();

        let fut = (handler)(step.clone());
        let result = if step.timeout_ms < u64::MAX {
            match tokio::time::timeout(std::time::Duration::from_millis(step.timeout_ms), fut).await
            {
                Ok(r) => r,
                Err(_) => {
                    tracing::warn!(step = %step.name, timeout_ms = step.timeout_ms, "step timed out");
                    Err(format!("timeout after {}ms", step.timeout_ms))
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
                    duration_ms,
                    attempts = attempt,
                    "step completed successfully"
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
                        attempt,
                        error = %e,
                        "step failed, retrying"
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
        attempts = max_attempts,
        "step failed after all retries exhausted"
    );

    StepResult {
        step_id: step.id,
        status: StepStatus::Failed,
        output: serde_json::json!(null),
        duration_ms: total_start.elapsed().as_millis() as u64,
        attempts: max_attempts,
        error: last_error,
    }
}
