//! Distributed DAG execution across a fleet of engine instances.
//!
//! Where [`run_dag`](super::dag::run_dag) executes a DAG with local tasks, this
//! coordinator distributes ready steps across the nodes of a
//! [`majra::fleet::FleetQueue`]. Each node owns a [`ManagedQueue`](majra::queue::ManagedQueue);
//! a worker per node dequeues, executes via the shared step handler, and reports
//! results back to the coordinator, which unlocks dependents and submits the next
//! wave. Idle nodes steal work from overloaded peers via
//! [`FleetQueue::rebalance`](majra::fleet::FleetQueue::rebalance).
//!
//! Nodes model independent engine instances: in-process here, but the fleet's
//! queue can be backed by a distributed transport so workers run on separate
//! hosts without changing this coordination logic.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use majra::fleet::FleetQueue;
use majra::queue::{Priority, ResourcePool};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::step::{StepDef, StepId, StepResult, StepStatus};

use super::step_exec::execute_step_with_handler;
use super::{ExecCtx, FlowCtx, emit};
use crate::bus::WorkflowEvent;

/// Coordinate DAG execution across the nodes of `fleet`.
///
/// Returns one [`StepResult`] per step. Steps whose dependencies failed, or whose
/// condition evaluated false, are recorded as [`Skipped`](StepStatus::Skipped)
/// without being submitted.
#[tracing::instrument(skip_all, fields(steps = steps.len(), nodes = fleet.node_count()))]
pub(crate) async fn run_distributed_dag(
    steps: &[StepDef],
    fleet: &Arc<FleetQueue<StepDef>>,
    timeout_ms: u64,
    start: std::time::Instant,
    token: Option<&CancellationToken>,
    ctx: &ExecCtx<'_>,
) -> Vec<StepResult> {
    let total = steps.len();
    let step_map: HashMap<StepId, &StepDef> = steps.iter().map(|s| (s.id, s)).collect();

    // In-degree + dependents, honoring TriggerMode (mirrors run_dag).
    let mut in_degree: HashMap<StepId, usize> = HashMap::new();
    let mut dependents: HashMap<StepId, Vec<StepId>> = HashMap::new();
    for step in steps {
        let deg = match step.trigger_mode {
            crate::step::TriggerMode::All => step.depends_on.len(),
            crate::step::TriggerMode::Any if !step.depends_on.is_empty() => 1,
            _ => step.depends_on.len(),
        };
        in_degree.insert(step.id, deg);
        for &dep in &step.depends_on {
            dependents.entry(dep).or_default().push(step.id);
        }
    }

    let mut ready: VecDeque<StepId> = steps
        .iter()
        .filter(|s| s.depends_on.is_empty())
        .map(|s| s.id)
        .collect();

    let mut results: Vec<StepResult> = Vec::with_capacity(total);
    let mut failed: HashSet<StepId> = HashSet::new();

    // Spawn one worker per registered node. Workers drain their node queue until
    // `done` is cancelled, reporting each result back over `tx`.
    let (tx, mut rx) = mpsc::unbounded_channel::<StepResult>();
    let done = CancellationToken::new();
    let flow_name: Arc<str> = ctx.flow.name.into();
    let flow_id = ctx.flow.id;
    let node_ids: Vec<String> = fleet.node_loads().into_iter().map(|(id, _)| id).collect();

    if node_ids.is_empty() {
        tracing::error!(flow = %ctx.flow.name, "distributed DAG requested but fleet has no nodes");
    }

    let mut workers = Vec::with_capacity(node_ids.len());
    for node_id in &node_ids {
        let Some(queue) = fleet.node_queue(node_id) else {
            continue;
        };
        let tx = tx.clone();
        let done = done.clone();
        let handler = ctx.handler.clone();
        let sink = ctx.event_sink.clone();
        let fname = Arc::clone(&flow_name);
        #[cfg(feature = "majra")]
        let metrics = ctx.metrics.clone();
        let stm = ctx.step_type_metrics.clone();
        let psink = ctx.progress_sink.clone();
        workers.push(tokio::spawn(async move {
            let pool = ResourcePool {
                gpu_count: 0,
                vram_mb: 0,
            };
            loop {
                if done.is_cancelled() {
                    break;
                }
                let item = tokio::select! {
                    biased;
                    () = done.cancelled() => break,
                    item = queue.dequeue(&pool) => item,
                };
                let Some(item) = item else {
                    // Queue momentarily empty; wait for work or shutdown.
                    tokio::select! {
                        () = done.cancelled() => break,
                        () = tokio::time::sleep(std::time::Duration::from_millis(2)) => continue,
                    }
                };
                let task_id = item.id;
                let flow = FlowCtx {
                    name: &fname,
                    id: flow_id,
                };
                let result = execute_step_with_handler(
                    &item.payload,
                    &handler,
                    &sink,
                    flow,
                    #[cfg(feature = "majra")]
                    &metrics,
                    &stm,
                    &psink,
                )
                .await;
                if result.status == StepStatus::Completed {
                    let _ = queue.complete(task_id);
                } else {
                    let _ = queue.fail(task_id);
                }
                if tx.send(result).is_err() {
                    break; // coordinator gone
                }
            }
        }));
    }
    drop(tx); // only workers retain senders

    let mut in_flight = 0usize;
    loop {
        // Submit or skip every currently-ready step.
        while let Some(id) = ready.pop_front() {
            let Some(&step) = step_map.get(&id) else {
                continue;
            };

            if step.depends_on.iter().any(|d| failed.contains(d)) {
                emit(
                    ctx.event_sink,
                    WorkflowEvent::step_skipped(&step.name, &id.to_string(), "dependency failed"),
                );
                results.push(skipped(id, "dependency failed"));
                failed.insert(id);
                super::dag::unlock_dependents(id, &dependents, &mut in_degree, &mut ready);
                continue;
            }

            if step.condition.is_some() {
                match super::check_condition(step, &results, steps, ctx.condition_cache) {
                    Ok(false) => {
                        emit(
                            ctx.event_sink,
                            WorkflowEvent::step_skipped(
                                &step.name,
                                &id.to_string(),
                                "condition not met",
                            ),
                        );
                        results.push(skipped(id, "condition not met"));
                        super::dag::unlock_dependents(id, &dependents, &mut in_degree, &mut ready);
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(step = %step.name, error = %e, "condition evaluation failed");
                    }
                    Ok(true) => {}
                }
            }

            match fleet.submit(Priority::Normal, step.clone(), None).await {
                Some((node, task_id)) => {
                    in_flight += 1;
                    tracing::debug!(step = %step.name, %node, %task_id, "submitted step to fleet");
                }
                None => {
                    tracing::error!(step = %step.name, "no fleet node could accept step");
                    results.push(StepResult {
                        step_id: id,
                        status: StepStatus::Failed,
                        output: serde_json::json!(null),
                        duration_ms: 0,
                        attempts: 0,
                        error: Some("no fleet node available".into()),
                    });
                    failed.insert(id);
                    super::dag::unlock_dependents(id, &dependents, &mut in_degree, &mut ready);
                }
            }
        }

        if results.len() >= total || in_flight == 0 {
            break;
        }

        let cancelled = token.is_some_and(|t| t.is_cancelled());
        let timed_out = start.elapsed().as_millis() as u64 > timeout_ms;
        if cancelled || timed_out {
            break;
        }

        // Wait for the next completion, a timeout, or cancellation.
        let recv = tokio::select! {
            biased;
            res = rx.recv() => res,
            () = async { if let Some(t) = token { t.cancelled().await } else { std::future::pending().await } } => None,
            () = tokio::time::sleep(remaining(timeout_ms, start)) => None,
        };

        match recv {
            Some(result) => {
                in_flight -= 1;
                let sid = result.step_id;
                if result.status != StepStatus::Completed {
                    failed.insert(sid);
                }
                results.push(result);
                super::dag::unlock_dependents(sid, &dependents, &mut in_degree, &mut ready);
                // Rebalance so idle nodes steal from any backlog.
                fleet.rebalance().await;
            }
            None => break, // timeout / cancellation / all workers gone
        }
    }

    // Skip anything not yet executed (timeout, cancellation, or starved nodes).
    if results.len() < total {
        let seen: HashSet<StepId> = results.iter().map(|r| r.step_id).collect();
        let reason = if token.is_some_and(|t| t.is_cancelled()) {
            "cancelled"
        } else if start.elapsed().as_millis() as u64 > timeout_ms {
            "flow timeout exceeded"
        } else {
            "not scheduled"
        };
        for step in steps {
            if !seen.contains(&step.id) {
                emit(
                    ctx.event_sink,
                    WorkflowEvent::step_skipped(&step.name, &step.id.to_string(), reason),
                );
                results.push(skipped(step.id, reason));
            }
        }
    }

    done.cancel();
    for worker in workers {
        let _ = worker.await;
    }

    results
}

#[inline]
fn skipped(step_id: StepId, reason: &str) -> StepResult {
    StepResult {
        step_id,
        status: StepStatus::Skipped,
        output: serde_json::json!(null),
        duration_ms: 0,
        attempts: 0,
        error: Some(reason.to_owned()),
    }
}

/// Remaining time before the flow timeout, clamped to a small floor so the
/// select arm always makes progress.
#[inline]
fn remaining(timeout_ms: u64, start: std::time::Instant) -> std::time::Duration {
    let elapsed = start.elapsed().as_millis() as u64;
    std::time::Duration::from_millis(timeout_ms.saturating_sub(elapsed).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, EngineConfig};
    use crate::flow::{FlowDef, FlowMode};
    use majra::fleet::{FleetQueue, FleetQueueConfig};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn fleet_with_nodes(n: usize) -> Arc<FleetQueue<StepDef>> {
        let fleet = Arc::new(FleetQueue::new(FleetQueueConfig::default()));
        for i in 0..n {
            fleet.register_node(
                format!("node-{i}"),
                ResourcePool {
                    gpu_count: 0,
                    vram_mb: 0,
                },
            );
        }
        fleet
    }

    fn success_handler() -> crate::engine::StepHandler {
        Arc::new(|step: StepDef| {
            Box::pin(async move { Ok(serde_json::json!({"step": step.name})) })
        })
    }

    #[tokio::test]
    async fn distributed_diamond_across_two_nodes() {
        let build = StepDef::new("build");
        let unit = StepDef::new("unit").depends_on(build.id);
        let integ = StepDef::new("integ").depends_on(build.id);
        let deploy = StepDef::new("deploy")
            .depends_on(unit.id)
            .depends_on(integ.id);

        let mut flow = FlowDef::new("ci", FlowMode::Dag);
        flow.add_step(build);
        flow.add_step(unit);
        flow.add_step(integ);
        flow.add_step(deploy);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine
            .run_distributed(&flow, fleet_with_nodes(2))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 4);
    }

    #[tokio::test]
    async fn distributed_runs_every_step_once() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let handler: crate::engine::StepHandler = Arc::new(move |step: StepDef| {
            let c = c.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({"step": step.name}))
            })
        });

        // Fan-out: one root, many independent leaves — exercises multi-node spread.
        let root = StepDef::new("root");
        let root_id = root.id;
        let mut flow = FlowDef::new("fanout", FlowMode::Dag);
        flow.add_step(root);
        for i in 0..12 {
            flow.add_step(StepDef::new(format!("leaf-{i}")).depends_on(root_id));
        }

        let engine = Engine::new(EngineConfig::default(), handler);
        let result = engine
            .run_distributed(&flow, fleet_with_nodes(3))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 13);
        assert_eq!(counter.load(Ordering::SeqCst), 13);
    }

    #[tokio::test]
    async fn distributed_skips_on_dependency_failure() {
        let handler: crate::engine::StepHandler =
            Arc::new(|_step: StepDef| Box::pin(async move { Err("boom".into()) }));

        let build = StepDef::new("build");
        let test = StepDef::new("test").depends_on(build.id);
        let deploy = StepDef::new("deploy").depends_on(test.id);

        let mut flow = FlowDef::new("fail", FlowMode::Dag);
        flow.add_step(build);
        flow.add_step(test);
        flow.add_step(deploy);

        let engine = Engine::new(EngineConfig::default(), handler);
        let result = engine
            .run_distributed(&flow, fleet_with_nodes(2))
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(result.failed_count(), 1);
        assert_eq!(result.skipped_count(), 2);
    }

    #[tokio::test]
    async fn distributed_skips_on_condition_false() {
        let mut flow = FlowDef::new("cond", FlowMode::Dag);
        let a = StepDef::new("a");
        let b = StepDef::new("b")
            .depends_on(a.id)
            .with_condition("steps.a.status == 'failed'");
        flow.add_step(a);
        flow.add_step(b);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine
            .run_distributed(&flow, fleet_with_nodes(1))
            .await
            .unwrap();
        assert!(result.success); // skip is not failure
        assert_eq!(result.completed_count(), 1);
        assert_eq!(result.skipped_count(), 1);
    }

    #[tokio::test]
    async fn distributed_rejects_non_dag_mode() {
        let mut flow = FlowDef::new("seq", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        assert!(
            engine
                .run_distributed(&flow, fleet_with_nodes(1))
                .await
                .is_err()
        );
    }
}
