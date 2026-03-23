//! MCP tools for workflow state machine operations.

use crate::mcp::{Tool, result_error, result_ok, tool_def};
use crate::state::WorkflowState;
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

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

pub struct StateCheck;

impl Tool for StateCheck {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_state_check",
            "Check if a workflow state is terminal and list its valid transitions",
            json!({ "state": { "type": "string", "enum": ["created","running","paused","completed","failed","rolling_back","rolled_back","cancelled"] } }),
            vec!["state".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let state_str = match args.get("state").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: state"),
            };
            let state = match parse_state(state_str) {
                Some(s) => s,
                None => return result_error(format!("invalid state: {state_str}")),
            };
            let all = all_workflow_states();
            let valid_targets: Vec<&str> = all
                .iter()
                .filter(|(_, s)| state.valid_transition(s))
                .map(|(n, _)| *n)
                .collect();
            result_ok(
                &serde_json::to_string_pretty(&json!({
                    "state": state_str,
                    "is_terminal": state.is_terminal(),
                    "valid_transitions": valid_targets,
                }))
                .unwrap_or_default(),
            )
        })
    }
}

pub struct StateTransition;

impl Tool for StateTransition {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_state_transition",
            "Check if a state transition from one state to another is valid",
            json!({
                "from": { "type": "string" },
                "to": { "type": "string" }
            }),
            vec!["from".into(), "to".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let from = match args
                .get("from")
                .and_then(|v| v.as_str())
                .and_then(parse_state)
            {
                Some(s) => s,
                None => return result_error("missing or invalid 'from' state"),
            };
            let to = match args
                .get("to")
                .and_then(|v| v.as_str())
                .and_then(parse_state)
            {
                Some(s) => s,
                None => return result_error("missing or invalid 'to' state"),
            };
            result_ok(
                &serde_json::to_string_pretty(&json!({
                    "from": args["from"],
                    "to": args["to"],
                    "valid": from.valid_transition(&to),
                }))
                .unwrap_or_default(),
            )
        })
    }
}

pub struct StateLifecycle;

impl Tool for StateLifecycle {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_state_lifecycle",
            "Show the complete workflow state machine — all states, transitions, and terminal states",
            json!({}),
            vec![],
        )
    }

    fn call(
        &self,
        _args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            let all = all_workflow_states();
            let states: Vec<serde_json::Value> = all.iter().map(|(name, state)| {
                let targets: Vec<&str> = all.iter().filter(|(_, s)| state.valid_transition(s)).map(|(n, _)| *n).collect();
                json!({"state": name, "is_terminal": state.is_terminal(), "transitions_to": targets})
            }).collect();
            result_ok(&serde_json::to_string_pretty(&states).unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn state_check_running() {
        let result = StateCheck.call(json!({"state": "running"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"is_terminal\": false"));
    }

    #[tokio::test]
    async fn state_check_completed() {
        let result = StateCheck.call(json!({"state": "completed"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"is_terminal\": true"));
    }

    #[tokio::test]
    async fn state_check_invalid() {
        let result = StateCheck.call(json!({"state": "nope"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn state_transition_valid() {
        let result = StateTransition
            .call(json!({"from": "created", "to": "running"}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"valid\": true"));
    }

    #[tokio::test]
    async fn state_transition_invalid() {
        let result = StateTransition
            .call(json!({"from": "completed", "to": "running"}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"valid\": false"));
    }

    #[tokio::test]
    async fn state_lifecycle() {
        let result = StateLifecycle.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let states: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(states.len(), 8);
    }
}
