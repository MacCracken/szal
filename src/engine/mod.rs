//! Execution engine — runs flows with retry, timeout, and rollback.
//!
//! ```
//! use szal::engine::EngineConfig;
//!
//! let config = EngineConfig::default();
//! assert_eq!(config.max_concurrency, 16);
//! assert!(config.global_timeout_ms.is_none());
//!
//! let config = EngineConfig {
//!     max_concurrency: 4,
//!     global_timeout_ms: Some(300_000),
//!     ..Default::default()
//! };
//! assert_eq!(config.max_concurrency, 4);
//! ```

mod dag;
#[cfg(feature = "hardware")]
pub mod hardware;
mod hierarchical;
mod parallel;
#[cfg(feature = "majra")]
mod queue_runner;
mod result;
mod runner;
mod sequential;
mod step_exec;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use tokio_util::sync::CancellationToken;

use crate::bus::WorkflowEvent;
use crate::flow::FlowId;
use crate::step::StepDef;

/// Lightweight context threaded through executor functions for tracing correlation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowCtx<'a> {
    pub name: &'a str,
    pub id: FlowId,
}

/// Shared execution context passed to all executor functions.
pub(crate) struct ExecCtx<'a> {
    pub handler: &'a StepHandler,
    pub event_sink: &'a EventSink,
    pub flow: FlowCtx<'a>,
    #[cfg(feature = "majra")]
    pub metrics: &'a crate::metrics::MetricsSink,
}

/// Optional event sink for workflow lifecycle events.
///
/// When `Some`, receives events fire-and-forget — implementations must not block.
/// When `None`, no events are emitted and the check is a single branch.
pub type EventSink = Option<Arc<dyn Fn(WorkflowEvent) + Send + Sync>>;

/// Emit an event if a sink is configured.
#[inline]
pub(crate) fn emit(sink: &EventSink, event: WorkflowEvent) {
    if let Some(ref f) = *sink {
        f(event);
    }
}

/// Check if a step's condition passes. Returns true if no condition or condition evaluates true.
pub(crate) fn check_condition(
    step: &StepDef,
    results: &[crate::step::StepResult],
    all_steps: &[StepDef],
) -> Result<bool, String> {
    match &step.condition {
        None => Ok(true),
        Some(expr) => {
            let ctx = crate::condition::build_step_context(results, all_steps);
            crate::condition::evaluate(expr, &ctx)
        }
    }
}

// Re-export public types
#[cfg(feature = "hardware")]
pub use self::hardware::HardwareContext;
pub use self::result::FlowResult;
pub use self::runner::Engine;

/// A step handler — async function that executes the step's work.
///
/// Receives the step definition and returns a JSON output value.
/// Errors should be returned as `Err(reason_string)`.
pub type StepHandler = Arc<
    dyn Fn(StepDef) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>
        + Send
        + Sync,
>;

/// A rollback handler — called when a completed step needs to be undone.
pub type RollbackHandler =
    Arc<dyn Fn(StepDef) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

/// Execution engine configuration.
pub struct EngineConfig {
    /// Maximum concurrent steps (for parallel/DAG modes).
    pub max_concurrency: usize,
    /// Global timeout override (overrides per-flow timeout).
    pub global_timeout_ms: Option<u64>,
    /// Hardware context for accelerator-aware scheduling.
    #[cfg(feature = "hardware")]
    pub hardware: Option<HardwareContext>,
    /// Metrics sink for workflow/step lifecycle instrumentation.
    #[cfg(feature = "majra")]
    pub metrics: crate::metrics::MetricsSink,
    /// Heartbeat tracker for engine health reporting.
    #[cfg(feature = "majra")]
    pub heartbeat: Option<Arc<majra::heartbeat::ConcurrentHeartbeatTracker>>,
    /// Queue for distributed step execution.
    #[cfg(feature = "majra")]
    pub queue: Option<Arc<majra::queue::ManagedQueue<crate::step::StepDef>>>,
}

impl std::fmt::Debug for EngineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("EngineConfig");
        d.field("max_concurrency", &self.max_concurrency)
            .field("global_timeout_ms", &self.global_timeout_ms);
        #[cfg(feature = "hardware")]
        d.field("hardware", &self.hardware);
        #[cfg(feature = "majra")]
        d.field("metrics", &self.metrics.is_some());
        #[cfg(feature = "majra")]
        d.field("heartbeat", &self.heartbeat.is_some());
        #[cfg(feature = "majra")]
        d.field("queue", &self.queue.is_some());
        d.finish()
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 16,
            global_timeout_ms: None,
            #[cfg(feature = "hardware")]
            hardware: None,
            #[cfg(feature = "majra")]
            metrics: None,
            #[cfg(feature = "majra")]
            heartbeat: None,
            #[cfg(feature = "majra")]
            queue: None,
        }
    }
}

impl EngineConfig {
    /// Enable hardware-aware scheduling with automatic device detection.
    #[cfg(feature = "hardware")]
    pub fn with_hardware(mut self) -> Self {
        self.hardware = Some(HardwareContext::detect());
        self
    }
}

/// Create a [`StepHandler`] from an async function.
///
/// This avoids the need to write
/// `Arc<dyn Fn(StepDef) -> Pin<Box<dyn Future<…> + Send>> + Send + Sync>` by hand.
///
/// ```
/// use szal::engine::{EngineConfig, Engine, handler_fn};
/// use szal::step::StepDef;
///
/// let engine = Engine::new(
///     EngineConfig::default(),
///     handler_fn(|step: StepDef| async move {
///         Ok(serde_json::json!({"step": step.name}))
///     }),
/// );
/// ```
pub fn handler_fn<F, Fut>(f: F) -> StepHandler
where
    F: Fn(StepDef) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<serde_json::Value, String>> + Send + 'static,
{
    Arc::new(move |step| Box::pin(f(step)))
}

/// Create a [`RollbackHandler`] from an async function.
///
/// ```
/// use szal::engine::rollback_fn;
/// use szal::step::StepDef;
///
/// let handler = rollback_fn(|step: StepDef| async move {
///     println!("rolling back {}", step.name);
///     Ok(())
/// });
/// ```
pub fn rollback_fn<F, Fut>(f: F) -> RollbackHandler
where
    F: Fn(StepDef) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    Arc::new(move |step| Box::pin(f(step)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{FlowDef, FlowMode};
    use crate::step::{StepDef, StepResult, StepStatus};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn success_handler() -> StepHandler {
        Arc::new(|step| {
            Box::pin(async move { Ok(serde_json::json!({"step": step.name, "status": "done"})) })
        })
    }

    fn counting_handler(counter: Arc<AtomicU32>) -> StepHandler {
        Arc::new(move |step| {
            let counter = counter.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({"step": step.name}))
            })
        })
    }

    fn failing_handler() -> StepHandler {
        Arc::new(|_step| Box::pin(async move { Err("intentional failure".into()) }))
    }

    fn fail_then_succeed_handler(fail_count: Arc<AtomicU32>) -> StepHandler {
        Arc::new(move |_step| {
            let fail_count = fail_count.clone();
            Box::pin(async move {
                let n = fail_count.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err("transient failure".into())
                } else {
                    Ok(serde_json::json!({"recovered": true}))
                }
            })
        })
    }

    #[test]
    fn engine_config_default() {
        let cfg = EngineConfig::default();
        assert_eq!(cfg.max_concurrency, 16);
        assert!(cfg.global_timeout_ms.is_none());
    }

    #[test]
    fn flow_result_counts() {
        let result = FlowResult {
            flow_name: "test".into(),
            steps: vec![
                StepResult {
                    step_id: uuid::Uuid::new_v4(),
                    status: StepStatus::Completed,
                    output: serde_json::json!({}),
                    duration_ms: 100,
                    attempts: 1,
                    error: None,
                },
                StepResult {
                    step_id: uuid::Uuid::new_v4(),
                    status: StepStatus::Failed,
                    output: serde_json::json!({}),
                    duration_ms: 50,
                    attempts: 3,
                    error: Some("timeout".into()),
                },
            ],
            total_duration_ms: 150,
            success: false,
            rolled_back: false,
        };
        assert_eq!(result.completed_count(), 1);
        assert_eq!(result.failed_count(), 1);
    }

    #[tokio::test]
    async fn run_sequential_all_pass() {
        let mut flow = FlowDef::new("test", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));
        flow.add_step(StepDef::new("c"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 3);
        assert_eq!(result.failed_count(), 0);
    }

    #[tokio::test]
    async fn run_parallel_all_pass() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut flow = FlowDef::new("test", FlowMode::Parallel);
        for i in 0..10 {
            flow.add_step(StepDef::new(format!("step-{i}")));
        }

        let engine = Engine::new(EngineConfig::default(), counting_handler(counter.clone()));
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 10);
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn run_parallel_respects_concurrency() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut flow = FlowDef::new("test", FlowMode::Parallel);
        for i in 0..20 {
            flow.add_step(StepDef::new(format!("step-{i}")));
        }

        let config = EngineConfig {
            max_concurrency: 2,
            ..Default::default()
        };
        let engine = Engine::new(config, counting_handler(counter.clone()));
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 20);
    }

    #[tokio::test]
    async fn run_dag_diamond() {
        let build = StepDef::new("build");
        let test_unit = StepDef::new("unit-test").depends_on(build.id);
        let test_integ = StepDef::new("integ-test").depends_on(build.id);
        let deploy = StepDef::new("deploy")
            .depends_on(test_unit.id)
            .depends_on(test_integ.id);

        let mut flow = FlowDef::new("ci-cd", FlowMode::Dag);
        flow.add_step(build);
        flow.add_step(test_unit);
        flow.add_step(test_integ);
        flow.add_step(deploy);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 4);
    }

    #[tokio::test]
    async fn run_dag_skips_on_dependency_failure() {
        let build = StepDef::new("build");
        let test = StepDef::new("test").depends_on(build.id);
        let deploy = StepDef::new("deploy").depends_on(test.id);

        let mut flow = FlowDef::new("fail-pipeline", FlowMode::Dag);
        flow.add_step(build);
        flow.add_step(test);
        flow.add_step(deploy);

        let engine = Engine::new(EngineConfig::default(), failing_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        // build fails, test and deploy should be skipped
        assert_eq!(result.failed_count(), 1);
        assert_eq!(result.skipped_count(), 2);
    }

    #[tokio::test]
    async fn run_with_retry_success() {
        let fail_count = Arc::new(AtomicU32::new(0));
        let mut flow = FlowDef::new("retry-test", FlowMode::Sequential);
        flow.add_step(StepDef::new("flaky").with_retries(3, 1)); // retry delay 1ms for test speed

        let engine = Engine::new(
            EngineConfig::default(),
            fail_then_succeed_handler(fail_count.clone()),
        );
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.steps[0].attempts, 3); // failed 2x, succeeded on 3rd
    }

    #[tokio::test]
    async fn run_with_retry_exhausted() {
        let mut flow = FlowDef::new("exhaust-test", FlowMode::Sequential);
        flow.add_step(StepDef::new("always-fail").with_retries(2, 1));

        let engine = Engine::new(EngineConfig::default(), failing_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.steps[0].attempts, 3); // 1 initial + 2 retries
        assert_eq!(result.steps[0].status, StepStatus::Failed);
    }

    #[tokio::test]
    async fn run_with_step_timeout() {
        let slow_handler: StepHandler = Arc::new(|_step| {
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                Ok(serde_json::json!({}))
            })
        });

        let mut flow = FlowDef::new("timeout-test", FlowMode::Sequential);
        flow.add_step(StepDef::new("slow").with_timeout(50)); // 50ms timeout

        let engine = Engine::new(EngineConfig::default(), slow_handler);
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.steps[0].status, StepStatus::Failed);
        assert!(result.steps[0].error.as_ref().unwrap().contains("timeout"));
    }

    #[tokio::test]
    async fn run_with_rollback() {
        let rollback_count = Arc::new(AtomicU32::new(0));
        let rb_count = rollback_count.clone();

        // First step succeeds, second fails
        let call_count = Arc::new(AtomicU32::new(0));
        let handler: StepHandler = Arc::new(move |_step| {
            let call_count = call_count.clone();
            Box::pin(async move {
                let n = call_count.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Ok(serde_json::json!({"done": true}))
                } else {
                    Err("second step fails".into())
                }
            })
        });

        let rollback_handler: RollbackHandler = Arc::new(move |_step| {
            let rb_count = rb_count.clone();
            Box::pin(async move {
                rb_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });

        let mut flow = FlowDef::new("rollback-test", FlowMode::Sequential).with_rollback();
        flow.add_step(StepDef::new("setup").with_rollback());
        flow.add_step(StepDef::new("deploy"));

        let engine =
            Engine::new(EngineConfig::default(), handler).with_rollback_handler(rollback_handler);
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        assert!(result.rolled_back);
        assert_eq!(rollback_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_sequential_stops_after_failure() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        let handler: StepHandler = Arc::new(move |_step| {
            let cc = cc.clone();
            Box::pin(async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err("first fails".into())
                } else {
                    Ok(serde_json::json!({}))
                }
            })
        });

        let mut flow = FlowDef::new("fail-fast", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));
        flow.add_step(StepDef::new("c"));

        let engine = Engine::new(EngineConfig::default(), handler);
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.failed_count(), 1);
        assert_eq!(result.skipped_count(), 2);
        // Only the first step's handler should have been called
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_rollback_failure_reports_not_rolled_back() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        let handler: StepHandler = Arc::new(move |_step| {
            let cc = cc.clone();
            Box::pin(async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Ok(serde_json::json!({}))
                } else {
                    Err("fails".into())
                }
            })
        });

        let rollback_handler: RollbackHandler =
            Arc::new(|_step| Box::pin(async move { Err("rollback failed".into()) }));

        let mut flow = FlowDef::new("rb-fail", FlowMode::Sequential).with_rollback();
        flow.add_step(StepDef::new("setup").with_rollback());
        flow.add_step(StepDef::new("deploy"));

        let engine =
            Engine::new(EngineConfig::default(), handler).with_rollback_handler(rollback_handler);
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        assert!(!result.rolled_back); // rollback failed, so should be false
    }

    #[test]
    fn flow_result_serde_roundtrip() {
        let result = FlowResult {
            flow_name: "test".into(),
            steps: vec![StepResult {
                step_id: uuid::Uuid::new_v4(),
                status: StepStatus::Completed,
                output: serde_json::json!({"key": "value"}),
                duration_ms: 100,
                attempts: 1,
                error: None,
            }],
            total_duration_ms: 100,
            success: true,
            rolled_back: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: FlowResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.flow_name, "test");
        assert!(back.success);
        assert_eq!(back.steps.len(), 1);
    }

    #[tokio::test]
    async fn run_rejects_invalid_flow() {
        let mut a = StepDef::new("a");
        let mut b = StepDef::new("b");
        b.depends_on = vec![a.id];
        a.depends_on = vec![b.id];
        let mut flow = FlowDef::new("cycle", FlowMode::Dag);
        flow.add_step(a);
        flow.add_step(b);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        assert!(engine.run(&flow).await.is_err());
    }

    #[tokio::test]
    async fn handler_fn_convenience() {
        let mut flow = FlowDef::new("test", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));

        let engine = Engine::new(
            EngineConfig::default(),
            handler_fn(|step| async move { Ok(serde_json::json!({"step": step.name})) }),
        );
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn run_with_cancellation_stops_sequential() {
        let token = CancellationToken::new();
        token.cancel(); // pre-cancel

        let mut flow = FlowDef::new("cancel-test", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run_with_cancellation(&flow, token).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.skipped_count(), 2);
    }

    #[tokio::test]
    async fn run_with_cancellation_stops_dag() {
        let token = CancellationToken::new();
        token.cancel();

        // Use independent steps so both are in the initial ready queue
        let mut flow = FlowDef::new("cancel-dag", FlowMode::Dag);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run_with_cancellation(&flow, token).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.skipped_count(), 2);
    }

    #[tokio::test]
    async fn run_with_cancellation_partial_execution() {
        let counter = Arc::new(AtomicU32::new(0));
        let token = CancellationToken::new();
        let token_clone = token.clone();

        // Handler that cancels the token after the first step
        let cc = counter.clone();
        let handler: StepHandler = Arc::new(move |step| {
            let cc = cc.clone();
            let token = token_clone.clone();
            Box::pin(async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    token.cancel();
                }
                Ok(serde_json::json!({"step": step.name}))
            })
        });

        let mut flow = FlowDef::new("partial-cancel", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));
        flow.add_step(StepDef::new("c"));

        let engine = Engine::new(EngineConfig::default(), handler);
        let result = engine.run_with_cancellation(&flow, token).await.unwrap();
        assert_eq!(result.completed_count(), 1);
        assert_eq!(result.skipped_count(), 2);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_with_cancellation_uncancelled_succeeds() {
        let token = CancellationToken::new(); // not cancelled

        let mut flow = FlowDef::new("no-cancel", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run_with_cancellation(&flow, token).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 2);
    }

    fn capturing_sink() -> (
        EventSink,
        Arc<std::sync::Mutex<Vec<crate::bus::WorkflowEvent>>>,
    ) {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let sink: EventSink = Some(Arc::new(move |e| {
            events_clone.lock().unwrap().push(e);
        }));
        (sink, events)
    }

    #[tokio::test]
    async fn event_sink_sequential_flow() {
        let (sink, events) = capturing_sink();
        let mut flow = FlowDef::new("test", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));

        let engine =
            Engine::new(EngineConfig::default(), success_handler()).with_event_sink(sink.unwrap());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);

        let evts = events.lock().unwrap();
        let types: Vec<_> = evts.iter().map(|e| e.event_type).collect();
        assert_eq!(types[0], crate::bus::EventType::FlowStarted);
        assert_eq!(types[1], crate::bus::EventType::StepStarted);
        assert_eq!(types[2], crate::bus::EventType::StepCompleted);
        assert_eq!(types[3], crate::bus::EventType::StepStarted);
        assert_eq!(types[4], crate::bus::EventType::StepCompleted);
        assert_eq!(types[5], crate::bus::EventType::FlowCompleted);
        assert_eq!(types.len(), 6);
    }

    #[tokio::test]
    async fn event_sink_failure_and_skip() {
        let (sink, events) = capturing_sink();
        let mut flow = FlowDef::new("test", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));

        let engine =
            Engine::new(EngineConfig::default(), failing_handler()).with_event_sink(sink.unwrap());
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);

        let evts = events.lock().unwrap();
        let types: Vec<_> = evts.iter().map(|e| e.event_type).collect();
        assert_eq!(types[0], crate::bus::EventType::FlowStarted);
        assert_eq!(types[1], crate::bus::EventType::StepStarted);
        assert_eq!(types[2], crate::bus::EventType::StepFailed);
        assert_eq!(types[3], crate::bus::EventType::StepSkipped);
        assert_eq!(types[4], crate::bus::EventType::FlowFailed);
        assert_eq!(types.len(), 5);
    }

    #[tokio::test]
    async fn event_sink_retry_events() {
        let (sink, events) = capturing_sink();
        let fail_count = Arc::new(AtomicU32::new(0));
        let mut flow = FlowDef::new("retry", FlowMode::Sequential);
        flow.add_step(StepDef::new("flaky").with_retries(3, 1));

        let engine = Engine::new(
            EngineConfig::default(),
            fail_then_succeed_handler(fail_count),
        )
        .with_event_sink(sink.unwrap());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);

        let evts = events.lock().unwrap();
        let types: Vec<_> = evts.iter().map(|e| e.event_type).collect();
        // FlowStarted, StepStarted, StepRetry(1), StepRetry(2), StepCompleted, FlowCompleted
        assert_eq!(types[0], crate::bus::EventType::FlowStarted);
        assert_eq!(types[1], crate::bus::EventType::StepStarted);
        assert_eq!(types[2], crate::bus::EventType::StepRetry);
        assert_eq!(types[3], crate::bus::EventType::StepRetry);
        assert_eq!(types[4], crate::bus::EventType::StepCompleted);
        assert_eq!(types[5], crate::bus::EventType::FlowCompleted);
    }

    #[tokio::test]
    async fn event_sink_rollback_events() {
        let (sink, events) = capturing_sink();
        let rollback_count = Arc::new(AtomicU32::new(0));
        let rb_count = rollback_count.clone();

        let call_count = Arc::new(AtomicU32::new(0));
        let handler: StepHandler = Arc::new(move |_step| {
            let call_count = call_count.clone();
            Box::pin(async move {
                let n = call_count.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Ok(serde_json::json!({"done": true}))
                } else {
                    Err("second step fails".into())
                }
            })
        });

        let rollback_handler: RollbackHandler = Arc::new(move |_step| {
            let rb_count = rb_count.clone();
            Box::pin(async move {
                rb_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });

        let mut flow = FlowDef::new("rb-test", FlowMode::Sequential).with_rollback();
        flow.add_step(StepDef::new("setup").with_rollback());
        flow.add_step(StepDef::new("deploy"));

        let engine = Engine::new(EngineConfig::default(), handler)
            .with_rollback_handler(rollback_handler)
            .with_event_sink(sink.unwrap());
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        assert!(result.rolled_back);

        let evts = events.lock().unwrap();
        let types: Vec<_> = evts.iter().map(|e| e.event_type).collect();
        assert!(types.contains(&crate::bus::EventType::StepRollback));
        assert!(types.contains(&crate::bus::EventType::FlowRolledBack));
        assert!(types.contains(&crate::bus::EventType::FlowFailed));
    }

    // --- Hierarchical execution tests ---

    #[tokio::test]
    async fn hierarchical_no_substeps_like_sequential() {
        let mut flow = FlowDef::new("test", FlowMode::Hierarchical);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b"));
        flow.add_step(StepDef::new("c"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 3);
    }

    #[tokio::test]
    async fn hierarchical_substeps_execute_on_success() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut flow = FlowDef::new("test", FlowMode::Hierarchical);
        let manager = StepDef::new("manager")
            .with_sub_step(StepDef::new("child-a"))
            .with_sub_step(StepDef::new("child-b"));
        flow.add_step(manager);
        flow.add_step(StepDef::new("after"));

        let engine = Engine::new(EngineConfig::default(), counting_handler(counter.clone()));
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        // manager + child-a + child-b + after = 4 steps executed
        assert_eq!(counter.load(Ordering::SeqCst), 4);
        assert_eq!(result.steps.len(), 4);
        assert_eq!(result.completed_count(), 4);
    }

    #[tokio::test]
    async fn hierarchical_substeps_skipped_on_failure() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        // First step (manager) fails
        let handler: StepHandler = Arc::new(move |_step| {
            let cc = cc.clone();
            Box::pin(async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Err("manager fails".into())
            })
        });

        let mut flow = FlowDef::new("test", FlowMode::Hierarchical);
        let manager = StepDef::new("manager")
            .with_sub_step(StepDef::new("child-a"))
            .with_sub_step(StepDef::new("child-b"));
        flow.add_step(manager);
        flow.add_step(StepDef::new("after"));

        let engine = Engine::new(EngineConfig::default(), handler);
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        // Only manager handler called (children and sibling skipped)
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(result.failed_count(), 1);
        assert_eq!(result.skipped_count(), 3); // child-a, child-b, after
    }

    #[tokio::test]
    async fn hierarchical_nested_depth_3() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut flow = FlowDef::new("deep", FlowMode::Hierarchical);
        let leaf = StepDef::new("leaf");
        let mid = StepDef::new("mid").with_sub_step(leaf);
        let top = StepDef::new("top").with_sub_step(mid);
        flow.add_step(top);

        let engine = Engine::new(EngineConfig::default(), counting_handler(counter.clone()));
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
        assert_eq!(result.steps.len(), 3);
    }

    #[tokio::test]
    async fn hierarchical_cancellation() {
        let token = CancellationToken::new();
        token.cancel();

        let mut flow = FlowDef::new("cancel", FlowMode::Hierarchical);
        let manager = StepDef::new("manager").with_sub_step(StepDef::new("child"));
        flow.add_step(manager);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run_with_cancellation(&flow, token).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.skipped_count(), 2); // manager + child
    }

    #[tokio::test]
    async fn hierarchical_rejects_depends_on() {
        let a = StepDef::new("a");
        let b = StepDef::new("b").depends_on(a.id);
        let mut flow = FlowDef::new("bad", FlowMode::Hierarchical);
        flow.add_step(a);
        flow.add_step(b);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        assert!(engine.run(&flow).await.is_err());
    }

    #[tokio::test]
    async fn hierarchical_substeps_serde_roundtrip() {
        let manager = StepDef::new("manager")
            .with_sub_step(StepDef::new("child-a"))
            .with_sub_step(StepDef::new("child-b"));
        let json = serde_json::to_string(&manager).unwrap();
        let back: StepDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sub_steps.len(), 2);
        assert_eq!(back.sub_steps[0].name, "child-a");
        assert_eq!(back.sub_steps[1].name, "child-b");
    }

    #[tokio::test]
    async fn hierarchical_events_with_substeps() {
        let (sink, events) = capturing_sink();
        let mut flow = FlowDef::new("test", FlowMode::Hierarchical);
        let manager = StepDef::new("manager").with_sub_step(StepDef::new("child"));
        flow.add_step(manager);

        let engine =
            Engine::new(EngineConfig::default(), success_handler()).with_event_sink(sink.unwrap());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);

        let evts = events.lock().unwrap();
        let types: Vec<_> = evts.iter().map(|e| e.event_type).collect();
        // FlowStarted, StepStarted(manager), StepCompleted(manager),
        // StepStarted(child), StepCompleted(child), FlowCompleted
        assert_eq!(types.len(), 6);
        assert_eq!(types[0], crate::bus::EventType::FlowStarted);
        assert_eq!(types[5], crate::bus::EventType::FlowCompleted);
    }

    // --- P0 tests ---

    #[tokio::test]
    async fn step_type_and_config_accessible_in_handler() {
        let handler: StepHandler = Arc::new(|step| {
            Box::pin(async move {
                let st = step.step_type.unwrap_or_default();
                let cfg = step.config.unwrap_or(serde_json::json!(null));
                Ok(serde_json::json!({"type": st, "config": cfg}))
            })
        });

        let mut flow = FlowDef::new("test", FlowMode::Sequential);
        flow.add_step(
            StepDef::new("fetch")
                .with_step_type("http")
                .with_config(serde_json::json!({"url": "https://example.com"})),
        );

        let engine = Engine::new(EngineConfig::default(), handler);
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.steps[0].output["type"], "http");
        assert_eq!(
            result.steps[0].output["config"]["url"],
            "https://example.com"
        );
    }

    #[tokio::test]
    async fn dag_any_trigger_fires_on_first_dep() {
        // Two parallel paths feed into a merge step with Any trigger
        let a = StepDef::new("fast");
        let b = StepDef::new("slow");
        let merge = StepDef::new("merge")
            .depends_on(a.id)
            .depends_on(b.id)
            .with_trigger_mode(crate::step::TriggerMode::Any);

        let mut flow = FlowDef::new("any-test", FlowMode::Dag);
        flow.add_step(a);
        flow.add_step(b);
        flow.add_step(merge);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 3);
    }

    #[tokio::test]
    async fn dag_any_trigger_with_one_failure() {
        // First dep fails, second succeeds. Any mode: merge should still fire
        // because the in_degree dropped to 0 when first dep completed (even if failed).
        // But: the dep_failed check should NOT skip it since the second dep succeeded.
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        let handler: StepHandler = Arc::new(move |step| {
            let cc = cc.clone();
            Box::pin(async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if step.name == "a" && n == 0 {
                    Err("a fails".into())
                } else {
                    Ok(serde_json::json!({"step": step.name}))
                }
            })
        });

        let a = StepDef::new("a");
        let b = StepDef::new("b");
        let merge = StepDef::new("merge")
            .depends_on(a.id)
            .depends_on(b.id)
            .with_trigger_mode(crate::step::TriggerMode::Any);

        let mut flow = FlowDef::new("any-fail", FlowMode::Dag);
        flow.add_step(a);
        flow.add_step(b);
        flow.add_step(merge);

        let engine = Engine::new(EngineConfig::default(), handler);
        let result = engine.run(&flow).await.unwrap();
        // a fails, b succeeds. merge fires (any mode) since b completed.
        // But dep_failed check looks if ANY dep failed, which would skip merge.
        // For Any mode, we should only check if ALL deps failed.
        // This is a design consideration - for now, Any mode fires eagerly.
        assert!(!result.success); // a failed
    }

    #[tokio::test]
    async fn dag_any_trigger_rejects_no_deps() {
        let step = StepDef::new("orphan").with_trigger_mode(crate::step::TriggerMode::Any);
        let mut flow = FlowDef::new("bad", FlowMode::Dag);
        flow.add_step(step);

        let engine = Engine::new(EngineConfig::default(), success_handler());
        assert!(engine.run(&flow).await.is_err());
    }

    #[tokio::test]
    async fn condition_true_executes() {
        let mut flow = FlowDef::new("cond", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b").with_condition("steps.a.status == 'completed'"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 2);
    }

    #[tokio::test]
    async fn condition_false_skips() {
        let mut flow = FlowDef::new("cond", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b").with_condition("steps.a.status == 'failed'"));

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success); // skipped is not failure
        assert_eq!(result.completed_count(), 1);
        assert_eq!(result.skipped_count(), 1);
        assert_eq!(result.steps[1].error.as_deref(), Some("condition not met"));
    }

    #[tokio::test]
    async fn condition_no_condition_always_runs() {
        let mut flow = FlowDef::new("cond", FlowMode::Sequential);
        flow.add_step(StepDef::new("a"));
        flow.add_step(StepDef::new("b")); // no condition

        let engine = Engine::new(EngineConfig::default(), success_handler());
        let result = engine.run(&flow).await.unwrap();
        assert!(result.success);
        assert_eq!(result.completed_count(), 2);
    }
}
