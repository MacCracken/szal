use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::step::{StepDef, StepId, StepResult, StepStatus};

use super::StepHandler;
use super::step_exec::execute_step_with_handler;

pub(crate) async fn run_dag(
    steps: &[StepDef],
    handler: &StepHandler,
    max_concurrency: usize,
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&CancellationToken>,
) -> Vec<StepResult> {
    tracing::debug!(steps = steps.len(), "running DAG execution");
    let sem = Arc::new(Semaphore::new(max_concurrency.max(1)));
    let mut results: Vec<StepResult> = Vec::with_capacity(steps.len());
    let mut failed: HashSet<StepId> = HashSet::new();

    // Build in-degree map
    let step_map: HashMap<StepId, &StepDef> = steps.iter().map(|s| (s.id, s)).collect();
    let mut in_degree: HashMap<StepId, usize> = HashMap::new();
    let mut dependents: HashMap<StepId, Vec<StepId>> = HashMap::new();

    for step in steps {
        in_degree.insert(step.id, step.depends_on.len());
        for &dep in &step.depends_on {
            dependents.entry(dep).or_default().push(step.id);
        }
    }

    // Start with steps that have no dependencies
    let mut ready: VecDeque<StepId> = steps
        .iter()
        .filter(|s| s.depends_on.is_empty())
        .map(|s| s.id)
        .collect();

    while !ready.is_empty() {
        let cancelled = token.is_some_and(|t| t.is_cancelled());
        if cancelled || start.elapsed().as_millis() as u64 > timeout_ms {
            let reason = if cancelled {
                "cancelled"
            } else {
                "flow timeout exceeded"
            };
            for &id in ready.iter() {
                if let Some(step) = step_map.get(&id) {
                    results.push(StepResult {
                        step_id: step.id,
                        status: StepStatus::Skipped,
                        output: serde_json::json!(null),
                        duration_ms: 0,
                        attempts: 0,
                        error: Some(reason.into()),
                    });
                }
            }
            break;
        }

        // Execute all ready steps concurrently
        let mut handles = Vec::new();
        let mut dag_step_ids = Vec::new();
        let batch_len = ready.len();

        for _ in 0..batch_len {
            let Some(id) = ready.pop_front() else { break };
            if let Some(&step) = step_map.get(&id) {
                // Skip if a dependency failed
                let dep_failed = step.depends_on.iter().any(|d| failed.contains(d));
                if dep_failed {
                    tracing::debug!(step = %step.name, "skipping step due to dependency failure");
                    results.push(StepResult {
                        step_id: step.id,
                        status: StepStatus::Skipped,
                        output: serde_json::json!(null),
                        duration_ms: 0,
                        attempts: 0,
                        error: Some("dependency failed".into()),
                    });
                    failed.insert(step.id);
                    unlock_dependents(step.id, &dependents, &mut in_degree, &mut ready);
                    continue;
                }

                let sem = sem.clone();
                let handler = handler.clone();
                let step = step.clone();
                dag_step_ids.push(step.id);
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
        }

        for (handle, original_id) in handles.into_iter().zip(dag_step_ids) {
            match handle.await {
                Ok(result) => {
                    if result.status != StepStatus::Completed {
                        failed.insert(original_id);
                    }
                    unlock_dependents(original_id, &dependents, &mut in_degree, &mut ready);
                    results.push(result);
                }
                Err(e) => {
                    tracing::error!(step_id = %original_id, error = %e, "spawned task panicked");
                    failed.insert(original_id);
                    unlock_dependents(original_id, &dependents, &mut in_degree, &mut ready);
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
    }

    results
}

fn unlock_dependents(
    step_id: StepId,
    dependents: &HashMap<StepId, Vec<StepId>>,
    in_degree: &mut HashMap<StepId, usize>,
    ready: &mut VecDeque<StepId>,
) {
    if let Some(deps) = dependents.get(&step_id) {
        for &dep_id in deps {
            if let Some(deg) = in_degree.get_mut(&dep_id) {
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    ready.push_back(dep_id);
                }
            }
        }
    }
}
