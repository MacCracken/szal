//! Streaming step progress over SSE / WebSocket transports.
//!
//! The engine already emits [`StepProgress`] events through a
//! [`ProgressSink`](crate::engine::ProgressSink). This module makes those events
//! easy to fan out to remote clients without pulling a web-server dependency into
//! the engine:
//!
//! - [`ProgressHub`] is a [`tokio::sync::broadcast`] hub. Attach its
//!   [`sink`](ProgressHub::sink) to the engine and [`subscribe`](ProgressHub::subscribe)
//!   from each connected client task.
//! - [`progress_to_sse`] / [`sse_frame`] encode events as
//!   [Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html)
//!   frames, ready to write to an HTTP `text/event-stream` body.
//!
//! Transport (axum, hyper, tungstenite, …) stays in the consumer. The hub is
//! protocol-agnostic: a WebSocket server forwards the raw [`StepProgress`], while
//! an SSE endpoint writes [`progress_to_sse`] output.
//!
//! ## Example
//!
//! ```
//! use szal::engine::{EngineConfig, Engine, handler_fn_with_progress};
//! use szal::stream::{ProgressHub, progress_to_sse};
//! use szal::flow::{FlowDef, FlowMode};
//! use szal::step::StepDef;
//!
//! # async fn run() {
//! let hub = ProgressHub::new(256);
//! let mut rx = hub.subscribe();
//!
//! let sink = hub.sink().unwrap();
//! let handler = handler_fn_with_progress(sink.clone(), |step, progress| async move {
//!     progress.report(serde_json::json!({"percent": 100}));
//!     Ok(serde_json::json!({"step": step.name}))
//! });
//!
//! let engine = Engine::new(
//!     EngineConfig { progress_sink: Some(sink), ..Default::default() },
//!     handler,
//! );
//!
//! let mut flow = FlowDef::new("upload", FlowMode::Sequential);
//! flow.add_step(StepDef::new("part-1"));
//! engine.run(&flow).await.unwrap();
//!
//! // A connected SSE client task drains the receiver and writes frames.
//! let event = rx.recv().await.unwrap();
//! let frame = progress_to_sse(&event);
//! assert!(frame.starts_with("event: step_progress\n"));
//! # }
//! ```

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::engine::{ProgressSink, StepProgress};

/// The SSE `event:` name used by [`progress_to_sse`].
pub const SSE_EVENT_NAME: &str = "step_progress";

/// A broadcast hub that fans out [`StepProgress`] events to many subscribers.
///
/// Backed by [`tokio::sync::broadcast`]: cheap to clone, lock-free on the send
/// path, and lossy under back-pressure (slow subscribers observe
/// [`RecvError::Lagged`](broadcast::error::RecvError::Lagged) rather than
/// stalling producers). Pick `capacity` to bound the per-subscriber backlog.
pub struct ProgressHub {
    tx: broadcast::Sender<StepProgress>,
}

impl ProgressHub {
    /// Create a hub buffering up to `capacity` events per subscriber.
    ///
    /// `capacity` is clamped to at least 1.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity.max(1));
        Self { tx }
    }

    /// A [`ProgressSink`] that publishes every reported event to the hub.
    ///
    /// Plug into [`EngineConfig::progress_sink`](crate::engine::EngineConfig::progress_sink).
    /// Sends are fire-and-forget: when no subscribers are listening the event is
    /// dropped, never blocking the engine.
    #[must_use]
    pub fn sink(&self) -> ProgressSink {
        let tx = self.tx.clone();
        Some(Arc::new(move |progress: StepProgress| {
            // A send error only means there are currently no receivers — fine.
            let _ = tx.send(progress);
        }))
    }

    /// Subscribe a new receiver. Each receiver sees every event published after
    /// it subscribed.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<StepProgress> {
        self.tx.subscribe()
    }

    /// Number of currently active subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl std::fmt::Debug for ProgressHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressHub")
            .field("subscribers", &self.tx.receiver_count())
            .finish()
    }
}

/// Encode a [`StepProgress`] as an SSE frame.
///
/// The frame uses the [`SSE_EVENT_NAME`] event type, the step id as the SSE
/// `id:`, and the JSON-serialized event as the `data:` payload. The returned
/// string ends with the blank line that terminates an SSE event, so frames can
/// be concatenated directly into a `text/event-stream` body.
///
/// ```
/// use szal::engine::StepProgress;
/// use szal::stream::progress_to_sse;
/// use serde_json::json;
///
/// let p = StepProgress {
///     step_name: "upload".into(),
///     step_id: "abc".into(),
///     data: json!({"percent": 50}),
/// };
/// let frame = progress_to_sse(&p);
/// assert!(frame.contains("event: step_progress\n"));
/// assert!(frame.contains("id: abc\n"));
/// assert!(frame.ends_with("\n\n"));
/// ```
#[must_use]
pub fn progress_to_sse(progress: &StepProgress) -> String {
    // StepProgress always serializes (no non-string map keys), but stay total.
    let data = serde_json::to_string(progress).unwrap_or_else(|_| "{}".to_owned());
    sse_frame(Some(SSE_EVENT_NAME), Some(&progress.step_id), &data)
}

/// Build a single SSE frame from optional `event`/`id` fields and a `data` body.
///
/// Multi-line `data` is split across multiple `data:` lines per the SSE spec.
/// The frame is terminated by a blank line.
///
/// ```
/// use szal::stream::sse_frame;
///
/// let frame = sse_frame(Some("tick"), Some("7"), "line1\nline2");
/// assert_eq!(frame, "event: tick\nid: 7\ndata: line1\ndata: line2\n\n");
/// ```
#[must_use]
pub fn sse_frame(event: Option<&str>, id: Option<&str>, data: &str) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(data.len() + 32);
    if let Some(event) = event {
        let _ = writeln!(out, "event: {event}");
    }
    if let Some(id) = id {
        let _ = writeln!(out, "id: {id}");
    }
    for line in data.split('\n') {
        let _ = writeln!(out, "data: {line}");
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn progress(name: &str, data: serde_json::Value) -> StepProgress {
        StepProgress {
            step_name: name.to_owned(),
            step_id: format!("{name}-id"),
            data,
        }
    }

    #[test]
    fn sse_frame_basic() {
        let frame = sse_frame(Some("step_progress"), Some("s1"), "{\"x\":1}");
        assert_eq!(frame, "event: step_progress\nid: s1\ndata: {\"x\":1}\n\n");
    }

    #[test]
    fn sse_frame_multiline_data() {
        let frame = sse_frame(None, None, "a\nb\nc");
        assert_eq!(frame, "data: a\ndata: b\ndata: c\n\n");
    }

    #[test]
    fn sse_frame_omits_absent_fields() {
        let frame = sse_frame(None, Some("9"), "hi");
        assert_eq!(frame, "id: 9\ndata: hi\n\n");
    }

    #[test]
    fn progress_to_sse_shape() {
        let p = progress("upload", json!({"percent": 50}));
        let frame = progress_to_sse(&p);
        assert!(frame.starts_with("event: step_progress\n"));
        assert!(frame.contains("id: upload-id\n"));
        assert!(frame.contains("\"percent\":50"));
        assert!(frame.ends_with("\n\n"));
    }

    #[tokio::test]
    async fn hub_delivers_to_subscriber() {
        let hub = ProgressHub::new(16);
        let mut rx = hub.subscribe();
        let sink = hub.sink().unwrap();

        sink(progress("a", json!({"percent": 10})));
        sink(progress("b", json!({"percent": 20})));

        let first = rx.recv().await.unwrap();
        let second = rx.recv().await.unwrap();
        assert_eq!(first.step_name, "a");
        assert_eq!(second.data["percent"], 20);
    }

    #[tokio::test]
    async fn hub_fans_out_to_multiple_subscribers() {
        let hub = ProgressHub::new(16);
        let mut rx1 = hub.subscribe();
        let mut rx2 = hub.subscribe();
        assert_eq!(hub.subscriber_count(), 2);

        hub.sink().unwrap()(progress("x", json!(null)));

        assert_eq!(rx1.recv().await.unwrap().step_name, "x");
        assert_eq!(rx2.recv().await.unwrap().step_name, "x");
    }

    #[tokio::test]
    async fn hub_send_without_subscribers_is_ok() {
        let hub = ProgressHub::new(4);
        // No subscribers: send is a no-op, must not panic.
        hub.sink().unwrap()(progress("orphan", json!(null)));
        assert_eq!(hub.subscriber_count(), 0);
    }

    #[test]
    fn hub_capacity_clamped() {
        // capacity 0 would panic in broadcast::channel; ensure we clamp to >= 1.
        let hub = ProgressHub::new(0);
        let _rx = hub.subscribe();
        assert_eq!(hub.subscriber_count(), 1);
    }
}
