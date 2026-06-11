use std::sync::Arc;

use crate::step::{StepDef, StepResult, StepStatus};

use super::{ExecCtx, step_exec::execute_step_with_handler};

/// Execute steps via a [`majra::queue::ManagedQueue`].
///
/// Each step is enqueued with [`Priority::Normal`](majra::queue::Priority::Normal) and no
/// resource requirements. A worker loop then dequeues items one at a time,
/// delegates to [`execute_step_with_handler`], and marks them complete or failed
/// in the queue.
#[tracing::instrument(skip_all, fields(steps = steps.len()))]
pub(crate) async fn run_queued(
    steps: &[StepDef],
    queue: &Arc<majra::queue::ManagedQueue<StepDef>>,
    ctx: &ExecCtx<'_>,
) -> Vec<StepResult> {
    let total = steps.len();
    tracing::info!(count = total, "enqueuing steps into managed queue");

    // 1. Enqueue all steps.
    for step in steps {
        let task_id = queue
            .enqueue(majra::queue::Priority::Normal, step.clone(), None)
            .await;
        tracing::debug!(step = %step.name, %task_id, "step enqueued");
    }

    // 2. Resource pool — no GPU requirements for now.
    let pool = majra::queue::ResourcePool {
        gpu_count: 0,
        vram_mb: 0,
    };

    // 3. Worker loop: dequeue, execute, mark result.
    let mut results = Vec::with_capacity(total);

    loop {
        // Try to dequeue the next item.
        let item = match queue.dequeue(&pool).await {
            Some(item) => item,
            None => {
                // Nothing dequeued — check if there are still queued items
                // (another worker may be running concurrently, or items are
                // resource-gated). If the queue is truly empty, we are done.
                if queue.queued_count().await == 0 && queue.running_count() == 0 {
                    tracing::debug!("queue drained, worker loop exiting");
                    break;
                }
                // Yield briefly to avoid busy-spinning while waiting for
                // concurrency slots to free up.
                tokio::task::yield_now().await;
                continue;
            }
        };

        let task_id = item.id;
        let step = &item.payload;
        tracing::info!(step = %step.name, %task_id, "dequeued step for execution");

        // 4. Execute via the shared step handler.
        let result = execute_step_with_handler(
            step,
            ctx.handler,
            ctx.event_sink,
            ctx.flow,
            ctx.metrics,
            ctx.step_type_metrics,
            ctx.progress_sink,
        )
        .await;

        // 5/6. Mark complete or failed in the queue.
        match result.status {
            StepStatus::Completed => {
                if let Err(e) = queue.complete(task_id) {
                    tracing::warn!(%task_id, error = %e, "failed to mark queue item complete");
                }
                tracing::debug!(%task_id, "queue item marked complete");
            }
            _ => {
                if let Err(e) = queue.fail(task_id) {
                    tracing::warn!(%task_id, error = %e, "failed to mark queue item failed");
                }
                tracing::debug!(
                    %task_id,
                    status = %result.status,
                    error = result.error.as_deref().unwrap_or("none"),
                    "queue item marked failed",
                );
            }
        }

        // 7. Collect the result.
        results.push(result);

        // 8. Exit when we have processed all steps.
        if results.len() >= total {
            tracing::info!(
                processed = results.len(),
                "all steps processed, exiting worker loop"
            );
            break;
        }
    }

    results
}
