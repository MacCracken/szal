use crate::SzalError;
use crate::flow::{FlowDef, FlowMode};
use crate::step::{StepDef, StepResult, StepStatus};
use tokio_util::sync::CancellationToken;

#[cfg(feature = "majra")]
use super::queue_runner;
use super::result::FlowResult;
use super::{EngineConfig, EventSink, ExecCtx, FlowCtx, RollbackHandler, StepHandler, emit};
use super::{dag, hierarchical, parallel, sequential};

/// The workflow execution engine.
pub struct Engine {
    config: EngineConfig,
    handler: StepHandler,
    rollback_handler: Option<RollbackHandler>,
    event_sink: EventSink,
}

impl Engine {
    /// Create an engine with a step handler.
    #[must_use]
    pub fn new(config: EngineConfig, handler: StepHandler) -> Self {
        Self {
            config,
            handler,
            rollback_handler: None,
            event_sink: None,
        }
    }

    /// Set a rollback handler for steps that support rollback.
    #[must_use]
    pub fn with_rollback_handler(mut self, handler: RollbackHandler) -> Self {
        self.rollback_handler = Some(handler);
        self
    }

    /// Attach workflow storage for dynamic subworkflow resolution.
    #[must_use]
    pub fn with_storage(
        mut self,
        storage: std::sync::Arc<dyn crate::storage::WorkflowStorage>,
    ) -> Self {
        self.config.storage = Some(storage);
        self
    }

    /// Attach a custom event sink for workflow lifecycle events.
    #[must_use]
    pub fn with_event_sink(
        mut self,
        sink: std::sync::Arc<dyn Fn(crate::bus::WorkflowEvent) + Send + Sync>,
    ) -> Self {
        self.event_sink = Some(sink);
        self
    }

    /// Attach an [`EventBus`](crate::bus::EventBus) as the event sink.
    #[cfg(feature = "majra")]
    #[must_use]
    pub fn with_event_bus(self, bus: std::sync::Arc<crate::bus::EventBus>) -> Self {
        self.with_event_sink(std::sync::Arc::new(move |e| bus.publish(&e)))
    }

    /// Attach a metrics sink for workflow/step lifecycle instrumentation.
    #[cfg(feature = "majra")]
    #[must_use]
    pub fn with_metrics(
        mut self,
        metrics: std::sync::Arc<dyn crate::metrics::SzalMetrics>,
    ) -> Self {
        self.config.metrics = Some(metrics);
        self
    }

    /// Attach a heartbeat tracker for engine health reporting.
    #[cfg(feature = "majra")]
    #[must_use]
    pub fn with_heartbeat(
        mut self,
        tracker: std::sync::Arc<majra::heartbeat::ConcurrentHeartbeatTracker>,
    ) -> Self {
        self.config.heartbeat = Some(tracker);
        self
    }

    /// Attach a managed queue for distributed step execution.
    #[cfg(feature = "majra")]
    #[must_use]
    pub fn with_queue(
        mut self,
        queue: std::sync::Arc<majra::queue::ManagedQueue<crate::step::StepDef>>,
    ) -> Self {
        self.config.queue = Some(queue);
        self
    }

    /// Execute a flow and return the result.
    #[tracing::instrument(skip(self, flow), fields(flow = %flow.name, mode = %flow.mode))]
    pub async fn run(&self, flow: &FlowDef) -> crate::Result<FlowResult> {
        flow.validate()?;

        #[cfg(feature = "hardware")]
        if let Some(ref hw) = self.config.hardware {
            hw.check_requirements(&flow.steps)?;
        }

        tracing::info!(flow = %flow.name, steps = flow.steps.len(), "starting flow execution");
        emit(
            &self.event_sink,
            crate::bus::WorkflowEvent::flow_started(&flow.name),
        );
        #[cfg(feature = "majra")]
        crate::metrics::metric_run_started(&self.config.metrics, &flow.name);
        #[cfg(feature = "majra")]
        let _heartbeat_guard = self.start_heartbeat(flow);

        let timeout = self
            .config
            .global_timeout_ms
            .or(flow.timeout_ms)
            .unwrap_or(u64::MAX);

        let start = std::time::Instant::now();
        let exec = ExecCtx {
            handler: &self.handler,
            event_sink: &self.event_sink,
            flow: FlowCtx {
                name: &flow.name,
                id: flow.id,
            },
            #[cfg(feature = "majra")]
            metrics: &self.config.metrics,
        };

        // Queue-backed execution path: enqueue + dequeue instead of direct execution
        #[cfg(feature = "majra")]
        if let Some(ref queue) = self.config.queue {
            let step_results = queue_runner::run_queued(&flow.steps, queue, &exec).await;
            let total_duration_ms = start.elapsed().as_millis() as u64;
            let has_failures = step_results.iter().any(|r| r.status == StepStatus::Failed);
            let mut rolled_back = false;
            if has_failures && flow.rollback_on_failure {
                rolled_back = self.rollback_completed_steps(flow, &step_results).await;
            }
            if has_failures {
                if rolled_back {
                    emit(
                        &self.event_sink,
                        crate::bus::WorkflowEvent::flow_rolled_back(&flow.name),
                    );
                }
                emit(
                    &self.event_sink,
                    crate::bus::WorkflowEvent::flow_failed(&flow.name, "failed"),
                );
                crate::metrics::metric_run_failed(
                    &self.config.metrics,
                    &flow.name,
                    total_duration_ms,
                );
            } else {
                emit(
                    &self.event_sink,
                    crate::bus::WorkflowEvent::flow_completed(&flow.name, total_duration_ms),
                );
                crate::metrics::metric_run_completed(
                    &self.config.metrics,
                    &flow.name,
                    total_duration_ms,
                );
            }
            return Ok(FlowResult {
                flow_name: flow.name.clone(),
                steps: step_results,
                total_duration_ms,
                success: !has_failures,
                rolled_back,
            });
        }

        let step_results = match flow.mode {
            FlowMode::Sequential => {
                sequential::run_sequential(&flow.steps, timeout, start, None, &exec).await
            }
            FlowMode::Parallel => {
                parallel::run_parallel(
                    &flow.steps,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    None,
                    &exec,
                )
                .await
            }
            FlowMode::Dag => {
                dag::run_dag(
                    &flow.steps,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    None,
                    &exec,
                )
                .await
            }
            FlowMode::Hierarchical => {
                hierarchical::run_hierarchical(&flow.steps, timeout, start, None, &exec).await
            }
        };

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let has_failures = step_results.iter().any(|r| r.status == StepStatus::Failed);
        let mut rolled_back = false;

        // Rollback on failure if configured
        if has_failures && flow.rollback_on_failure {
            rolled_back = self.rollback_completed_steps(flow, &step_results).await;
        }

        let result_status = if has_failures {
            if rolled_back { "rolled_back" } else { "failed" }
        } else {
            "success"
        };
        tracing::info!(
            flow = %flow.name,
            duration_ms = total_duration_ms,
            steps = step_results.len(),
            result = result_status,
            "flow execution completed"
        );

        if has_failures {
            if rolled_back {
                emit(
                    &self.event_sink,
                    crate::bus::WorkflowEvent::flow_rolled_back(&flow.name),
                );
            }
            emit(
                &self.event_sink,
                crate::bus::WorkflowEvent::flow_failed(&flow.name, result_status),
            );
            #[cfg(feature = "majra")]
            crate::metrics::metric_run_failed(&self.config.metrics, &flow.name, total_duration_ms);
        } else {
            emit(
                &self.event_sink,
                crate::bus::WorkflowEvent::flow_completed(&flow.name, total_duration_ms),
            );
            #[cfg(feature = "majra")]
            crate::metrics::metric_run_completed(
                &self.config.metrics,
                &flow.name,
                total_duration_ms,
            );
        }

        Ok(FlowResult {
            flow_name: flow.name.clone(),
            steps: step_results,
            total_duration_ms,
            success: !has_failures,
            rolled_back,
        })
    }

    /// Execute a flow with cancellation support.
    ///
    /// Behaves identically to [`run`](Self::run) but checks the provided
    /// [`CancellationToken`] between steps. When the token is cancelled,
    /// remaining steps are marked [`StepStatus::Skipped`].
    #[tracing::instrument(skip(self, flow, token), fields(flow = %flow.name, mode = %flow.mode))]
    pub async fn run_with_cancellation(
        &self,
        flow: &FlowDef,
        token: CancellationToken,
    ) -> crate::Result<FlowResult> {
        flow.validate()?;

        #[cfg(feature = "hardware")]
        if let Some(ref hw) = self.config.hardware {
            hw.check_requirements(&flow.steps)?;
        }

        tracing::info!(flow = %flow.name, steps = flow.steps.len(), "starting flow execution (cancellable)");
        emit(
            &self.event_sink,
            crate::bus::WorkflowEvent::flow_started(&flow.name),
        );
        #[cfg(feature = "majra")]
        crate::metrics::metric_run_started(&self.config.metrics, &flow.name);
        #[cfg(feature = "majra")]
        let _heartbeat_guard = self.start_heartbeat(flow);

        let timeout = self
            .config
            .global_timeout_ms
            .or(flow.timeout_ms)
            .unwrap_or(u64::MAX);

        let start = std::time::Instant::now();
        let exec = ExecCtx {
            handler: &self.handler,
            event_sink: &self.event_sink,
            flow: FlowCtx {
                name: &flow.name,
                id: flow.id,
            },
            #[cfg(feature = "majra")]
            metrics: &self.config.metrics,
        };

        let step_results = match flow.mode {
            FlowMode::Sequential => {
                sequential::run_sequential(&flow.steps, timeout, start, Some(&token), &exec).await
            }
            FlowMode::Parallel => {
                parallel::run_parallel(
                    &flow.steps,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    Some(&token),
                    &exec,
                )
                .await
            }
            FlowMode::Dag => {
                dag::run_dag(
                    &flow.steps,
                    self.config.max_concurrency,
                    timeout,
                    start,
                    Some(&token),
                    &exec,
                )
                .await
            }
            FlowMode::Hierarchical => {
                hierarchical::run_hierarchical(&flow.steps, timeout, start, Some(&token), &exec)
                    .await
            }
        };

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let has_failures = step_results.iter().any(|r| r.status == StepStatus::Failed);
        let was_cancelled = token.is_cancelled()
            && step_results.iter().any(|r| {
                r.status == StepStatus::Skipped && r.error.as_deref() == Some("cancelled")
            });
        let mut rolled_back = false;

        if has_failures && flow.rollback_on_failure {
            rolled_back = self.rollback_completed_steps(flow, &step_results).await;
        }

        let success = !has_failures && !was_cancelled;
        let result_status = if was_cancelled {
            "cancelled"
        } else if has_failures {
            if rolled_back { "rolled_back" } else { "failed" }
        } else {
            "success"
        };
        tracing::info!(
            flow = %flow.name,
            duration_ms = total_duration_ms,
            steps = step_results.len(),
            result = result_status,
            "flow execution completed"
        );

        if !success {
            if rolled_back {
                emit(
                    &self.event_sink,
                    crate::bus::WorkflowEvent::flow_rolled_back(&flow.name),
                );
            }
            emit(
                &self.event_sink,
                crate::bus::WorkflowEvent::flow_failed(&flow.name, result_status),
            );
            #[cfg(feature = "majra")]
            crate::metrics::metric_run_failed(&self.config.metrics, &flow.name, total_duration_ms);
        } else {
            emit(
                &self.event_sink,
                crate::bus::WorkflowEvent::flow_completed(&flow.name, total_duration_ms),
            );
            #[cfg(feature = "majra")]
            crate::metrics::metric_run_completed(
                &self.config.metrics,
                &flow.name,
                total_duration_ms,
            );
        }

        Ok(FlowResult {
            flow_name: flow.name.clone(),
            steps: step_results,
            total_duration_ms,
            success,
            rolled_back,
        })
    }

    async fn rollback_completed_steps(&self, flow: &FlowDef, step_results: &[StepResult]) -> bool {
        let Some(ref rollback_handler) = self.rollback_handler else {
            return false;
        };

        let completed_steps: Vec<&StepDef> = flow
            .steps
            .iter()
            .filter(|s| {
                s.rollbackable
                    && step_results
                        .iter()
                        .any(|r| r.step_id == s.id && r.status == StepStatus::Completed)
            })
            .collect();

        tracing::info!(flow = %flow.name, steps = completed_steps.len(), "starting rollback");
        let mut all_rolled_back = true;
        for step in completed_steps.into_iter().rev() {
            emit(
                &self.event_sink,
                crate::bus::WorkflowEvent::step_rollback(&step.name, &step.id.to_string()),
            );
            if let Err(reason) = (rollback_handler)(step.clone()).await {
                let err = SzalError::RollbackFailed {
                    step: step.name.clone(),
                    reason,
                };
                tracing::warn!(step = %step.name, error = %err, "rollback step failed");
                all_rolled_back = false;
            }
        }
        tracing::info!(flow = %flow.name, success = all_rolled_back, "rollback completed");
        all_rolled_back
    }

    /// Start heartbeat reporting for a flow execution.
    /// Returns a guard that deregisters and aborts the heartbeat task on drop.
    #[cfg(feature = "majra")]
    fn start_heartbeat(&self, flow: &FlowDef) -> Option<HeartbeatGuard> {
        let tracker = self.config.heartbeat.as_ref()?;
        let engine_id = flow.id.to_string();
        tracker.register(
            &engine_id,
            serde_json::json!({
                "flow": flow.name,
                "mode": flow.mode.to_string(),
                "steps": flow.steps.len(),
            }),
        );
        tracing::debug!(engine_id = %engine_id, flow = %flow.name, "heartbeat registered");

        let t = tracker.clone();
        let id = engine_id.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                let _ = t.heartbeat(&id);
            }
        });

        Some(HeartbeatGuard {
            tracker: tracker.clone(),
            engine_id,
            handle,
        })
    }
}

/// RAII guard that deregisters heartbeat and aborts the background task on drop.
#[cfg(feature = "majra")]
struct HeartbeatGuard {
    tracker: std::sync::Arc<majra::heartbeat::ConcurrentHeartbeatTracker>,
    engine_id: String,
    handle: tokio::task::JoinHandle<()>,
}

#[cfg(feature = "majra")]
impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.handle.abort();
        self.tracker.deregister(&self.engine_id);
        tracing::debug!(engine_id = %self.engine_id, "heartbeat deregistered");
    }
}
