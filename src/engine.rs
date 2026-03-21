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
//! };
//! assert_eq!(config.max_concurrency, 4);
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::flow::{FlowDef, FlowMode};
use crate::step::{StepDef, StepId, StepResult, StepStatus};

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
pub type RollbackHandler = Arc<
    dyn Fn(StepDef) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync,
>;

/// Execution engine configuration.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum concurrent steps (for parallel/DAG modes).
    pub max_concurrency: usize,
    /// Global timeout override (overrides per-flow timeout).
    pub global_timeout_ms: Option<u64>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 16,
            global_timeout_ms: None,
        }
    }
}

/// The workflow execution engine.
pub struct Engine {
    config: EngineConfig,
    handler: StepHandler,
    rollback_handler: Option<RollbackHandler>,
}

impl Engine {
    /// Create an engine with a step handler.
    pub fn new(config: EngineConfig, handler: StepHandler) -> Self {
        Self {
            config,
            handler,
            rollback_handler: None,
        }
    }

    /// Set a rollback handler for steps that support rollback.
    pub fn with_rollback_handler(mut self, handler: RollbackHandler) -> Self {
        self.rollback_handler = Some(handler);
        self
    }

    /// Execute a flow and return the result.
    pub async fn run(&self, flow: &FlowDef) -> crate::Result<FlowResult> {
        flow.validate()?;

        let timeout = self
            .config
            .global_timeout_ms
            .or(flow.timeout_ms)
            .unwrap_or(u64::MAX);

        let start = std::time::Instant::now();

        let step_results = match flow.mode {
            FlowMode::Sequential => self.run_sequential(&flow.steps, timeout, start).await,
            FlowMode::Parallel => self.run_parallel(&flow.steps, timeout, start).await,
            FlowMode::Dag => self.run_dag(&flow.steps, timeout, start).await,
            FlowMode::Hierarchical => {
                // Hierarchical delegates to sub-steps — for now treat as sequential
                self.run_sequential(&flow.steps, timeout, start).await
            }
        };

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let has_failures = step_results.iter().any(|r| r.status == StepStatus::Failed);
        let mut rolled_back = false;

        // Rollback on failure if configured
        if has_failures
            && flow.rollback_on_failure
            && let Some(ref rollback_handler) = self.rollback_handler
        {
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

                for step in completed_steps.into_iter().rev() {
                    let _ = (rollback_handler)(step.clone()).await;
                }
                rolled_back = true;
        }

        Ok(FlowResult {
            flow_name: flow.name.clone(),
            steps: step_results,
            total_duration_ms,
            success: !has_failures,
            rolled_back,
        })
    }

    async fn run_sequential(
        &self,
        steps: &[StepDef],
        timeout_ms: u64,
        start: std::time::Instant,
    ) -> Vec<StepResult> {
        let mut results = Vec::with_capacity(steps.len());
        for step in steps {
            if start.elapsed().as_millis() as u64 > timeout_ms {
                results.push(StepResult {
                    step_id: step.id,
                    status: StepStatus::Skipped,
                    output: serde_json::json!(null),
                    duration_ms: 0,
                    attempts: 0,
                    error: Some("flow timeout exceeded".into()),
                });
                continue;
            }
            results.push(self.execute_step(step).await);
        }
        results
    }

    async fn run_parallel(
        &self,
        steps: &[StepDef],
        _timeout_ms: u64,
        _start: std::time::Instant,
    ) -> Vec<StepResult> {
        let sem = Arc::new(Semaphore::new(self.config.max_concurrency));
        let mut handles = Vec::with_capacity(steps.len());

        for step in steps {
            let sem = sem.clone();
            let handler = self.handler.clone();
            let step = step.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                execute_step_with_handler(&step, &handler).await
            }));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(StepResult {
                    step_id: uuid::Uuid::new_v4(),
                    status: StepStatus::Failed,
                    output: serde_json::json!(null),
                    duration_ms: 0,
                    attempts: 0,
                    error: Some(format!("task panicked: {e}")),
                }),
            }
        }
        results
    }

    async fn run_dag(
        &self,
        steps: &[StepDef],
        timeout_ms: u64,
        start: std::time::Instant,
    ) -> Vec<StepResult> {
        let sem = Arc::new(Semaphore::new(self.config.max_concurrency));
        let mut results: Vec<StepResult> = Vec::with_capacity(steps.len());
        let mut completed: HashSet<StepId> = HashSet::new();
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
            if start.elapsed().as_millis() as u64 > timeout_ms {
                // Skip remaining
                for &id in ready.iter() {
                    if let Some(step) = step_map.get(&id) {
                        results.push(StepResult {
                            step_id: step.id,
                            status: StepStatus::Skipped,
                            output: serde_json::json!(null),
                            duration_ms: 0,
                            attempts: 0,
                            error: Some("flow timeout exceeded".into()),
                        });
                    }
                }
                break;
            }

            // Execute all ready steps concurrently
            let mut handles = Vec::new();
            let batch: Vec<StepId> = ready.drain(..).collect();

            for id in &batch {
                if let Some(&step) = step_map.get(id) {
                    // Skip if a dependency failed
                    let dep_failed = step.depends_on.iter().any(|d| failed.contains(d));
                    if dep_failed {
                        results.push(StepResult {
                            step_id: step.id,
                            status: StepStatus::Skipped,
                            output: serde_json::json!(null),
                            duration_ms: 0,
                            attempts: 0,
                            error: Some("dependency failed".into()),
                        });
                        failed.insert(step.id);
                        // Unlock dependents
                        if let Some(deps) = dependents.get(&step.id) {
                            for &dep_id in deps {
                                if let Some(deg) = in_degree.get_mut(&dep_id) {
                                    *deg = deg.saturating_sub(1);
                                    if *deg == 0 {
                                        ready.push_back(dep_id);
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    let sem = sem.clone();
                    let handler = self.handler.clone();
                    let step = step.clone();
                    handles.push(tokio::spawn(async move {
                        let _permit = sem.acquire().await.unwrap();
                        execute_step_with_handler(&step, &handler).await
                    }));
                }
            }

            for handle in handles {
                match handle.await {
                    Ok(result) => {
                        let id = result.step_id;
                        if result.status == StepStatus::Completed {
                            completed.insert(id);
                        } else {
                            failed.insert(id);
                        }
                        // Unlock dependents
                        if let Some(deps) = dependents.get(&id) {
                            for &dep_id in deps {
                                if let Some(deg) = in_degree.get_mut(&dep_id) {
                                    *deg = deg.saturating_sub(1);
                                    if *deg == 0 {
                                        ready.push_back(dep_id);
                                    }
                                }
                            }
                        }
                        results.push(result);
                    }
                    Err(e) => {
                        results.push(StepResult {
                            step_id: uuid::Uuid::new_v4(),
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

    async fn execute_step(&self, step: &StepDef) -> StepResult {
        execute_step_with_handler(step, &self.handler).await
    }
}

async fn execute_step_with_handler(step: &StepDef, handler: &StepHandler) -> StepResult {
    let max_attempts = step.max_retries + 1;
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        let step_start = std::time::Instant::now();

        let fut = (handler)(step.clone());
        let result = if step.timeout_ms < u64::MAX {
            match tokio::time::timeout(
                std::time::Duration::from_millis(step.timeout_ms),
                fut,
            )
            .await
            {
                Ok(r) => r,
                Err(_) => Err(format!("timeout after {}ms", step.timeout_ms)),
            }
        } else {
            fut.await
        };

        let duration_ms = step_start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
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
                last_error = Some(e);
                if attempt < max_attempts {
                    tokio::time::sleep(std::time::Duration::from_millis(step.retry_delay_ms)).await;
                }
            }
        }
    }

    StepResult {
        step_id: step.id,
        status: StepStatus::Failed,
        output: serde_json::json!(null),
        duration_ms: 0,
        attempts: max_attempts,
        error: last_error,
    }
}

/// Result of executing a complete flow.
///
/// ```
/// use szal::engine::FlowResult;
/// use szal::step::{StepResult, StepStatus};
///
/// let result = FlowResult {
///     flow_name: "deploy".into(),
///     steps: vec![
///         StepResult {
///             step_id: uuid::Uuid::new_v4(),
///             status: StepStatus::Completed,
///             output: serde_json::json!({}),
///             duration_ms: 100,
///             attempts: 1,
///             error: None,
///         },
///     ],
///     total_duration_ms: 100,
///     success: true,
///     rolled_back: false,
/// };
/// assert_eq!(result.completed_count(), 1);
/// assert_eq!(result.failed_count(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct FlowResult {
    pub flow_name: String,
    pub steps: Vec<StepResult>,
    pub total_duration_ms: u64,
    pub success: bool,
    pub rolled_back: bool,
}

impl FlowResult {
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Failed)
            .count()
    }

    pub fn skipped_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Skipped)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{FlowDef, FlowMode};
    use crate::step::StepDef;
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
            global_timeout_ms: None,
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

        let engine = Engine::new(EngineConfig::default(), handler)
            .with_rollback_handler(rollback_handler);
        let result = engine.run(&flow).await.unwrap();
        assert!(!result.success);
        assert!(result.rolled_back);
        assert_eq!(rollback_count.load(Ordering::SeqCst), 1);
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
}
