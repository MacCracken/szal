//! Workflow event bus — powered by majra pub/sub.
//!
//! Publishes workflow lifecycle events to topics that external systems
//! can subscribe to for monitoring, logging, and orchestration.
//!
//! ## Topic hierarchy
//!
//! ```text
//! szal/flow/{flow_name}/started
//! szal/flow/{flow_name}/completed
//! szal/flow/{flow_name}/failed
//! szal/step/{step_name}/started
//! szal/step/{step_name}/completed
//! szal/step/{step_name}/failed
//! szal/step/{step_name}/retry
//! szal/step/{step_name}/rollback
//! ```
//!
//! Subscribe with wildcards: `szal/flow/#` for all flow events,
//! `szal/step/*/failed` for all step failures.

#[cfg(feature = "majra")]
use majra::pubsub::PubSub;
use serde::{Deserialize, Serialize};

/// A workflow lifecycle event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub event_type: EventType,
    pub flow_name: Option<String>,
    pub step_name: Option<String>,
    pub step_id: Option<String>,
    pub attempt: Option<u32>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Event types emitted during workflow execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    FlowStarted,
    FlowCompleted,
    FlowFailed,
    FlowRolledBack,
    StepStarted,
    StepCompleted,
    StepFailed,
    StepRetry,
    StepRollback,
    StepSkipped,
    StepTimeout,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FlowStarted => write!(f, "flow_started"),
            Self::FlowCompleted => write!(f, "flow_completed"),
            Self::FlowFailed => write!(f, "flow_failed"),
            Self::FlowRolledBack => write!(f, "flow_rolled_back"),
            Self::StepStarted => write!(f, "step_started"),
            Self::StepCompleted => write!(f, "step_completed"),
            Self::StepFailed => write!(f, "step_failed"),
            Self::StepRetry => write!(f, "step_retry"),
            Self::StepRollback => write!(f, "step_rollback"),
            Self::StepSkipped => write!(f, "step_skipped"),
            Self::StepTimeout => write!(f, "step_timeout"),
        }
    }
}

impl WorkflowEvent {
    fn new(event_type: EventType) -> Self {
        Self {
            event_type,
            flow_name: None,
            step_name: None,
            step_id: None,
            attempt: None,
            duration_ms: None,
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_flow(mut self, name: &str) -> Self {
        self.flow_name = Some(name.into());
        self
    }

    pub fn with_step(mut self, name: &str, id: &str) -> Self {
        self.step_name = Some(name.into());
        self.step_id = Some(id.into());
        self
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    pub fn with_attempt(mut self, attempt: u32) -> Self {
        self.attempt = Some(attempt);
        self
    }

    pub fn with_error(mut self, error: &str) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn flow_started(flow_name: &str) -> Self {
        Self::new(EventType::FlowStarted).with_flow(flow_name)
    }

    pub fn flow_completed(flow_name: &str, duration_ms: u64) -> Self {
        Self::new(EventType::FlowCompleted)
            .with_flow(flow_name)
            .with_duration(duration_ms)
    }

    pub fn flow_failed(flow_name: &str, error: &str) -> Self {
        Self::new(EventType::FlowFailed)
            .with_flow(flow_name)
            .with_error(error)
    }

    pub fn flow_rolled_back(flow_name: &str) -> Self {
        Self::new(EventType::FlowRolledBack).with_flow(flow_name)
    }

    pub fn step_started(step_name: &str, step_id: &str) -> Self {
        Self::new(EventType::StepStarted).with_step(step_name, step_id)
    }

    pub fn step_completed(step_name: &str, step_id: &str, duration_ms: u64, attempt: u32) -> Self {
        Self::new(EventType::StepCompleted)
            .with_step(step_name, step_id)
            .with_duration(duration_ms)
            .with_attempt(attempt)
    }

    pub fn step_failed(step_name: &str, step_id: &str, error: &str, attempt: u32) -> Self {
        Self::new(EventType::StepFailed)
            .with_step(step_name, step_id)
            .with_error(error)
            .with_attempt(attempt)
    }

    pub fn step_retry(step_name: &str, step_id: &str, attempt: u32) -> Self {
        Self::new(EventType::StepRetry)
            .with_step(step_name, step_id)
            .with_attempt(attempt)
    }

    /// Build the topic string for this event.
    pub fn topic(&self) -> String {
        match self.event_type {
            EventType::FlowStarted
            | EventType::FlowCompleted
            | EventType::FlowFailed
            | EventType::FlowRolledBack => {
                let name = self.flow_name.as_deref().unwrap_or("unknown");
                format!("szal/flow/{name}/{}", self.event_type)
            }
            _ => {
                let name = self.step_name.as_deref().unwrap_or("unknown");
                format!("szal/step/{name}/{}", self.event_type)
            }
        }
    }
}

/// Workflow event bus backed by majra pub/sub.
#[cfg(feature = "majra")]
pub struct EventBus {
    pubsub: PubSub,
}

#[cfg(feature = "majra")]
impl EventBus {
    /// Create a new event bus.
    pub fn new() -> Self {
        Self {
            pubsub: PubSub::new(),
        }
    }

    /// Publish a workflow event.
    pub fn publish(&self, event: &WorkflowEvent) {
        let topic = event.topic();
        let payload = match serde_json::to_value(event) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize workflow event");
                return;
            }
        };
        self.pubsub.publish(&topic, payload);
    }

    /// Subscribe to workflow events matching a pattern.
    ///
    /// Examples:
    /// - `"szal/flow/#"` — all flow events
    /// - `"szal/step/*/step_failed"` — all step failures
    /// - `"szal/#"` — everything
    pub fn subscribe(
        &self,
        pattern: &str,
    ) -> tokio::sync::broadcast::Receiver<majra::pubsub::TopicMessage> {
        self.pubsub.subscribe(pattern)
    }
}

#[cfg(feature = "majra")]
impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_topic_flow() {
        let e = WorkflowEvent::flow_started("deploy");
        assert_eq!(e.topic(), "szal/flow/deploy/flow_started");
    }

    #[test]
    fn event_topic_step() {
        let e = WorkflowEvent::step_completed("build", "abc-123", 500, 1);
        assert_eq!(e.topic(), "szal/step/build/step_completed");
    }

    #[test]
    fn event_serde_roundtrip() {
        let e = WorkflowEvent::step_failed("deploy", "id-1", "timeout", 3);
        let json = serde_json::to_string(&e).unwrap();
        let back: WorkflowEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_type, EventType::StepFailed);
        assert_eq!(back.attempt, Some(3));
    }

    #[test]
    fn event_type_display() {
        assert_eq!(EventType::FlowStarted.to_string(), "flow_started");
        assert_eq!(EventType::StepRetry.to_string(), "step_retry");
    }

    #[test]
    fn builder_with_flow() {
        let e = WorkflowEvent::flow_started("deploy");
        assert_eq!(e.flow_name, Some("deploy".into()));
    }

    #[test]
    fn builder_with_step() {
        let e = WorkflowEvent::step_started("build", "id-1");
        assert_eq!(e.step_name, Some("build".into()));
        assert_eq!(e.step_id, Some("id-1".into()));
    }

    #[test]
    fn builder_with_duration() {
        let e = WorkflowEvent::step_completed("s", "id", 500, 1);
        assert_eq!(e.duration_ms, Some(500));
    }

    #[test]
    fn builder_with_attempt() {
        let e = WorkflowEvent::step_retry("s", "id", 3);
        assert_eq!(e.attempt, Some(3));
    }

    #[test]
    fn builder_with_error() {
        let e = WorkflowEvent::flow_failed("f", "oops");
        assert_eq!(e.error, Some("oops".into()));
    }

    #[test]
    fn builder_chaining() {
        let e = WorkflowEvent::step_failed("s", "id", "e", 2);
        assert_eq!(e.event_type, EventType::StepFailed);
        assert_eq!(e.step_name, Some("s".into()));
        assert_eq!(e.step_id, Some("id".into()));
        assert_eq!(e.error, Some("e".into()));
        assert_eq!(e.attempt, Some(2));
    }

    #[test]
    fn flow_rolled_back_topic() {
        let e = WorkflowEvent::flow_rolled_back("deploy");
        assert_eq!(e.topic(), "szal/flow/deploy/flow_rolled_back");
    }

    #[cfg(feature = "majra")]
    #[tokio::test]
    async fn event_bus_publish_subscribe() {
        let bus = EventBus::new();
        let mut sub = bus.subscribe("szal/flow/#");

        bus.publish(&WorkflowEvent::flow_started("test"));

        let msg = sub.recv().await.unwrap();
        assert_eq!(msg.topic, "szal/flow/test/flow_started");
    }
}
