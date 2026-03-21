//! MCP tools for workflow state machine operations.

use crate::mcp::tool::{Tool, ToolDef, ToolResult};
use crate::state::WorkflowState;
use serde_json::json;
use std::future::Future;
use std::pin::Pin;

/// Check properties of a workflow state.
pub struct StateCheck;

impl Tool for StateCheck {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_state_check".into(),
            description: "Check if a workflow state is terminal and list its valid transitions".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "state": {
                        "type": "string",
                        "enum": ["created", "running", "paused", "completed", "failed", "rolling_back", "rolled_back", "cancelled"],
                        "description": "Workflow state to check"
                    }
                },
                "required": ["state"]
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let state_str = match args.get("state").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: state"),
            };

            let state = match parse_state(state_str) {
                Some(s) => s,
                None => return ToolResult::error(format!("invalid state: {state_str}")),
            };

            let all_states = all_workflow_states();
            let valid_targets: Vec<&str> = all_states
                .iter()
                .filter(|(_, s)| state.valid_transition(s))
                .map(|(name, _)| *name)
                .collect();

            let info = json!({
                "state": state_str,
                "is_terminal": state.is_terminal(),
                "valid_transitions": valid_targets,
            });

            ToolResult::success(serde_json::to_string_pretty(&info).unwrap_or_default())
        })
    }
}

/// Check if a specific state transition is valid.
pub struct StateTransition;

impl Tool for StateTransition {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_state_transition".into(),
            description: "Check if a state transition from one state to another is valid".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from": {
                        "type": "string",
                        "enum": ["created", "running", "paused", "completed", "failed", "rolling_back", "rolled_back", "cancelled"],
                        "description": "Current state"
                    },
                    "to": {
                        "type": "string",
                        "enum": ["created", "running", "paused", "completed", "failed", "rolling_back", "rolled_back", "cancelled"],
                        "description": "Target state"
                    }
                },
                "required": ["from", "to"]
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let from_str = match args.get("from").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: from"),
            };
            let to_str = match args.get("to").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: to"),
            };

            let from = match parse_state(from_str) {
                Some(s) => s,
                None => return ToolResult::error(format!("invalid state: {from_str}")),
            };
            let to = match parse_state(to_str) {
                Some(s) => s,
                None => return ToolResult::error(format!("invalid state: {to_str}")),
            };

            let valid = from.valid_transition(&to);
            let info = json!({
                "from": from_str,
                "to": to_str,
                "valid": valid,
            });

            ToolResult::success(serde_json::to_string_pretty(&info).unwrap_or_default())
        })
    }
}

/// Show the complete state machine lifecycle.
pub struct StateLifecycle;

impl Tool for StateLifecycle {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_state_lifecycle".into(),
            description: "Show the complete workflow state machine — all states, transitions, and terminal states".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async {
            let all = all_workflow_states();
            let states: Vec<serde_json::Value> = all
                .iter()
                .map(|(name, state)| {
                    let targets: Vec<&str> = all
                        .iter()
                        .filter(|(_, s)| state.valid_transition(s))
                        .map(|(n, _)| *n)
                        .collect();
                    json!({
                        "state": name,
                        "is_terminal": state.is_terminal(),
                        "transitions_to": targets,
                    })
                })
                .collect();

            ToolResult::success(serde_json::to_string_pretty(&states).unwrap_or_default())
        })
    }
}

fn parse_state(s: &str) -> Option<WorkflowState> {
    match s {
        "created" => Some(WorkflowState::Created),
        "running" => Some(WorkflowState::Running),
        "paused" => Some(WorkflowState::Paused),
        "completed" => Some(WorkflowState::Completed),
        "failed" => Some(WorkflowState::Failed),
        "rolling_back" => Some(WorkflowState::RollingBack),
        "rolled_back" => Some(WorkflowState::RolledBack),
        "cancelled" => Some(WorkflowState::Cancelled),
        _ => None,
    }
}

fn all_workflow_states() -> Vec<(&'static str, WorkflowState)> {
    vec![
        ("created", WorkflowState::Created),
        ("running", WorkflowState::Running),
        ("paused", WorkflowState::Paused),
        ("completed", WorkflowState::Completed),
        ("failed", WorkflowState::Failed),
        ("rolling_back", WorkflowState::RollingBack),
        ("rolled_back", WorkflowState::RolledBack),
        ("cancelled", WorkflowState::Cancelled),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn state_check_running() {
        let tool = StateCheck;
        let result = tool.call(json!({"state": "running"})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("\"is_terminal\": false"));
        assert!(text.contains("completed"));
        assert!(text.contains("failed"));
    }

    #[tokio::test]
    async fn state_check_completed() {
        let tool = StateCheck;
        let result = tool.call(json!({"state": "completed"})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("\"is_terminal\": true"));
        assert!(text.contains("\"valid_transitions\": []"));
    }

    #[tokio::test]
    async fn state_check_invalid() {
        let tool = StateCheck;
        let result = tool.call(json!({"state": "nope"})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn state_transition_valid() {
        let tool = StateTransition;
        let result = tool.call(json!({"from": "created", "to": "running"})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("\"valid\": true"));
    }

    #[tokio::test]
    async fn state_transition_invalid() {
        let tool = StateTransition;
        let result = tool.call(json!({"from": "completed", "to": "running"})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("\"valid\": false"));
    }

    #[tokio::test]
    async fn state_lifecycle() {
        let tool = StateLifecycle;
        let result = tool.call(json!({})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        let states: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(states.len(), 8);
    }
}
