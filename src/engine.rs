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
pub use tokio_util::sync::CancellationToken;

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
pub type RollbackHandler =
    Arc<dyn Fn(StepDef) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

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
    #[tracing::instrument(skip(self, flow), fields(flow = %flow.name, mode = %flow.mode))]
    pub async fn run(&self, flow: &FlowDef) -> crate::Result<FlowResult> {
        flow.validate()?;

        tracing::info!(flow = %flow.name, steps = flow.steps.len(), "starting flow execution");

        let timeout = self
            .config
            .global_timeout_ms
            .or(flow.timeout_ms)
            .unwrap_or(u64::MAX);

        let start = std::time::Instant::now();

        let step_results = match flow.mode {
            FlowMode::Sequential | FlowMode::Hierarchical => {
                self.run_sequential(&flow.steps, timeout, start, None).await
            }
            FlowMode::Parallel => self.run_parallel(&flow.steps, timeout, start, None).await,
            FlowMode::Dag => self.run_dag(&flow.steps, timeout, start, None).await,
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
        token: Option<&CancellationToken>,
    ) -> Vec<StepResult> {
        tracing::debug!(steps = steps.len(), "running sequential execution");
        let mut results = Vec::with_capacity(steps.len());
        let mut failed = false;
        for step in steps {
            let cancelled = token.is_some_and(|t| t.is_cancelled());
            if cancelled || failed {
                results.push(StepResult {
                    step_id: step.id,
                    status: StepStatus::Skipped,
                    output: serde_json::json!(null),
                    duration_ms: 0,
                    attempts: 0,
                    error: Some(if cancelled {
                        "cancelled".into()
                    } else {
                        "prior step failed".into()
                    }),
                });
                continue;
            }
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
            let result = self.execute_step(step).await;
            if result.status == StepStatus::Failed {
                failed = true;
            }
            results.push(result);
        }
        results
    }

    async fn run_parallel(
        &self,
        steps: &[StepDef],
        timeout_ms: u64,
        start: std::time::Instant,
        token: Option<&CancellationToken>,
    ) -> Vec<StepResult> {
        tracing::debug!(steps = steps.len(), "running parallel execution");
        let sem = Arc::new(Semaphore::new(self.config.max_concurrency.max(1)));
        let mut handles = Vec::with_capacity(steps.len());

        let mut step_ids = Vec::with_capacity(steps.len());
        for step in steps {
            step_ids.push(step.id);
            let sem = sem.clone();
            let handler = self.handler.clone();
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

    async fn run_dag(
        &self,
        steps: &[StepDef],
        timeout_ms: u64,
        start: std::time::Instant,
        token: Option<&CancellationToken>,
    ) -> Vec<StepResult> {
        tracing::debug!(steps = steps.len(), "running DAG execution");
        let sem = Arc::new(Semaphore::new(self.config.max_concurrency.max(1)));
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
                let id = ready.pop_front().unwrap();
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
                        // Unlock dependents
                        if let Some(deps) = dependents.get(&original_id) {
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
                        tracing::error!(step_id = %original_id, error = %e, "spawned task panicked");
                        failed.insert(original_id);
                        // Unlock dependents even on panic
                        if let Some(deps) = dependents.get(&original_id) {
                            for &dep_id in deps {
                                if let Some(deg) = in_degree.get_mut(&dep_id) {
                                    *deg = deg.saturating_sub(1);
                                    if *deg == 0 {
                                        ready.push_back(dep_id);
                                    }
                                }
                            }
                        }
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

    async fn execute_step(&self, step: &StepDef) -> StepResult {
        execute_step_with_handler(step, &self.handler).await
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
            if (rollback_handler)(step.clone()).await.is_err() {
                tracing::warn!(step = %step.name, "rollback step failed");
                all_rolled_back = false;
            }
        }
        tracing::info!(flow = %flow.name, success = all_rolled_back, "rollback completed");
        all_rolled_back
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

        tracing::info!(flow = %flow.name, steps = flow.steps.len(), "starting flow execution (cancellable)");

        let timeout = self
            .config
            .global_timeout_ms
            .or(flow.timeout_ms)
            .unwrap_or(u64::MAX);

        let start = std::time::Instant::now();

        let step_results = match flow.mode {
            FlowMode::Sequential | FlowMode::Hierarchical => {
                self.run_sequential(&flow.steps, timeout, start, Some(&token))
                    .await
            }
            FlowMode::Parallel => {
                self.run_parallel(&flow.steps, timeout, start, Some(&token))
                    .await
            }
            FlowMode::Dag => {
                self.run_dag(&flow.steps, timeout, start, Some(&token))
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

        Ok(FlowResult {
            flow_name: flow.name.clone(),
            steps: step_results,
            total_duration_ms,
            success,
            rolled_back,
        })
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

async fn execute_step_with_handler(step: &StepDef, handler: &StepHandler) -> StepResult {
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
