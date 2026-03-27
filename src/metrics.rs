//! Workflow metrics — re-exports majra's [`MajraMetrics`] trait for workflow lifecycle hooks.
//!
//! Majra's `MajraMetrics` provides workflow-run and workflow-step metrics alongside
//! infrastructure metrics (queue, pubsub, heartbeat, rate limiter). Consumers implement
//! `MajraMetrics` to wire szal's engine into their metrics backend.

#[cfg(feature = "majra")]
use std::sync::Arc;

#[cfg(feature = "majra")]
pub use majra::metrics::MajraMetrics;

#[cfg(feature = "majra")]
pub use majra::metrics::NoopMetrics;

/// Optional metrics sink threaded through the engine.
///
/// When `Some`, lifecycle methods are called fire-and-forget.
/// When `None`, no metrics are emitted.
#[cfg(feature = "majra")]
pub type MetricsSink = Option<Arc<dyn MajraMetrics>>;

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
        let sink: MetricsSink = Some(Arc::new(NoopMetrics));
        metric_run_started(&sink, "test");
        metric_run_completed(&sink, "test", 100);
        metric_step_started(&sink, "test", "step-1");
        metric_step_finished(&sink, "test", "step-1", "failed", 30);
    }
}
