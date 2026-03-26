//! Workflow metrics — extends majra's `MajraMetrics` with workflow lifecycle hooks.
//!
//! The [`SzalMetrics`] trait adds workflow-run and workflow-step metrics
//! on top of majra's infrastructure metrics (queue, pubsub, heartbeat).

#[cfg(feature = "majra")]
use std::sync::Arc;

/// Workflow metrics trait — extends majra's `MajraMetrics` with workflow lifecycle hooks.
///
/// All methods have default no-op implementations, so consumers only override
/// the metrics they care about.
#[cfg(feature = "majra")]
pub trait SzalMetrics: Send + Sync {
    /// A workflow run was started.
    fn workflow_run_started(&self, _workflow_id: &str) {}

    /// A workflow run completed successfully.
    fn workflow_run_completed(&self, _workflow_id: &str, _duration_ms: u64) {}

    /// A workflow run failed.
    fn workflow_run_failed(&self, _workflow_id: &str, _duration_ms: u64) {}

    /// A workflow step began executing.
    fn workflow_step_started(&self, _workflow_id: &str, _step_id: &str) {}

    /// A workflow step reached a terminal status.
    fn workflow_step_finished(
        &self,
        _workflow_id: &str,
        _step_id: &str,
        _status: &str,
        _duration_ms: u64,
    ) {
    }
}

/// No-op metrics sink.
#[cfg(feature = "majra")]
pub struct NoopSzalMetrics;

#[cfg(feature = "majra")]
impl SzalMetrics for NoopSzalMetrics {}

/// Optional metrics sink threaded through the engine.
///
/// When `Some`, lifecycle methods are called fire-and-forget.
/// When `None`, no metrics are emitted.
#[cfg(feature = "majra")]
pub type MetricsSink = Option<Arc<dyn SzalMetrics>>;

#[cfg(feature = "majra")]
#[inline]
pub(crate) fn metric_run_started(sink: &MetricsSink, workflow_id: &str) {
    if let Some(ref m) = *sink {
        m.workflow_run_started(workflow_id);
    }
}

#[cfg(feature = "majra")]
#[inline]
pub(crate) fn metric_run_completed(sink: &MetricsSink, workflow_id: &str, duration_ms: u64) {
    if let Some(ref m) = *sink {
        m.workflow_run_completed(workflow_id, duration_ms);
    }
}

#[cfg(feature = "majra")]
#[inline]
pub(crate) fn metric_run_failed(sink: &MetricsSink, workflow_id: &str, duration_ms: u64) {
    if let Some(ref m) = *sink {
        m.workflow_run_failed(workflow_id, duration_ms);
    }
}

#[cfg(feature = "majra")]
#[inline]
pub(crate) fn metric_step_started(sink: &MetricsSink, workflow_id: &str, step_id: &str) {
    if let Some(ref m) = *sink {
        m.workflow_step_started(workflow_id, step_id);
    }
}

#[cfg(feature = "majra")]
#[inline]
pub(crate) fn metric_step_finished(
    sink: &MetricsSink,
    workflow_id: &str,
    step_id: &str,
    status: &str,
    duration_ms: u64,
) {
    if let Some(ref m) = *sink {
        m.workflow_step_finished(workflow_id, step_id, status, duration_ms);
    }
}

#[cfg(all(test, feature = "majra"))]
mod tests {
    use super::*;

    #[test]
    fn metric_sink_none_is_noop() {
        let sink: MetricsSink = None;
        metric_run_started(&sink, "test");
        metric_run_completed(&sink, "test", 100);
        metric_run_failed(&sink, "test", 50);
        metric_step_started(&sink, "test", "step-1");
        metric_step_finished(&sink, "test", "step-1", "completed", 30);
    }

    #[test]
    fn metric_sink_with_noop() {
        let sink: MetricsSink = Some(Arc::new(NoopSzalMetrics));
        metric_run_started(&sink, "test");
        metric_run_completed(&sink, "test", 100);
        metric_step_started(&sink, "test", "step-1");
        metric_step_finished(&sink, "test", "step-1", "failed", 30);
    }
}
