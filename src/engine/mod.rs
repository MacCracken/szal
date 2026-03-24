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

mod runner;
mod dag;
#[cfg(feature = "hardware")]
pub mod hardware;
mod parallel;
mod result;
mod sequential;
mod step_exec;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use tokio_util::sync::CancellationToken;

use crate::step::StepDef;

// Re-export public types
pub use self::runner::Engine;
#[cfg(feature = "hardware")]
pub use self::hardware::HardwareContext;
pub use self::result::FlowResult;

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
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum concurrent steps (for parallel/DAG modes).
    pub max_concurrency: usize,
    /// Global timeout override (overrides per-flow timeout).
    pub global_timeout_ms: Option<u64>,
    /// Hardware context for accelerator-aware scheduling.
    #[cfg(feature = "hardware")]
    pub hardware: Option<HardwareContext>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 16,
            global_timeout_ms: None,
            #[cfg(feature = "hardware")]
            hardware: None,
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
}
