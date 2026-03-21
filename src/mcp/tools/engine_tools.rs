//! MCP tools for engine configuration and flow result inspection.

use crate::engine::EngineConfig;
use crate::mcp::{Tool, tool_def, result_ok, result_ok_json, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;



pub struct EngineCreate;

impl Tool for EngineCreate {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_engine_create",
            "Create an engine configuration with concurrency and timeout settings",
            json!({
                "max_concurrency": { "type": "integer" },
                "global_timeout_ms": { "type": "integer" }
            }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let mut config = EngineConfig::default();
            if let Some(c) = args.get("max_concurrency").and_then(|v| v.as_u64()) {
                config.max_concurrency = c as usize;
            }
            if let Some(t) = args.get("global_timeout_ms").and_then(|v| v.as_u64()) {
                config.global_timeout_ms = Some(t);
            }
            result_ok_json(&json!({
                "max_concurrency": config.max_concurrency,
                "global_timeout_ms": config.global_timeout_ms,
            }))
        })
    }
}

pub struct ResultInspect;

impl Tool for ResultInspect {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_result_inspect",
            "Inspect a flow execution result — step counts, duration, success/failure",
            json!({ "result_json": { "type": "string" } }),
            vec!["result_json".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("result_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: result_json"),
            };
            let val: serde_json::Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(e) => return result_error(format!("invalid JSON: {e}")),
            };
            let flow_name = val.get("flow_name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let success = val.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let rolled_back = val.get("rolled_back").and_then(|v| v.as_bool()).unwrap_or(false);
            let total_ms = val.get("total_duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            let steps = val.get("steps").and_then(|v| v.as_array());
            let step_count = steps.map(|s| s.len()).unwrap_or(0);
            let completed = steps.map(|s| s.iter().filter(|st| st.get("status").and_then(|v| v.as_str()) == Some("Completed")).count()).unwrap_or(0);
            let failed = steps.map(|s| s.iter().filter(|st| st.get("status").and_then(|v| v.as_str()) == Some("Failed")).count()).unwrap_or(0);
            result_ok(&serde_json::to_string_pretty(&json!({
                "flow_name": flow_name, "success": success, "rolled_back": rolled_back,
                "total_duration_ms": total_ms, "step_count": step_count, "completed": completed, "failed": failed,
            })).unwrap_or_default())
        })
    }
}

pub struct StepStatusList;

impl Tool for StepStatusList {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_step_status_list", "List all possible step execution statuses", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            result_ok(&serde_json::to_string_pretty(&json!([
                { "status": "pending", "description": "Step has not started" },
                { "status": "running", "description": "Step is currently executing" },
                { "status": "completed", "description": "Step finished successfully" },
                { "status": "failed", "description": "Step execution failed" },
                { "status": "skipped", "description": "Step was skipped" },
                { "status": "rolled_back", "description": "Step was rolled back after failure" },
            ])).unwrap_or_default())
        })
    }
}

pub struct ErrorList;

impl Tool for ErrorList {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_error_list", "List all workflow error types with descriptions", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            result_ok(&serde_json::to_string_pretty(&json!([
                { "error": "StepFailed", "description": "A step failed with a specific reason" },
                { "error": "StepTimeout", "description": "A step exceeded its timeout" },
                { "error": "InvalidFlow", "description": "Flow configuration is invalid" },
                { "error": "RetryExhausted", "description": "All retry attempts failed" },
                { "error": "RollbackFailed", "description": "Rollback operation failed" },
                { "error": "CycleDetected", "description": "DAG contains a cycle" },
            ])).unwrap_or_default())
        })
    }
}

pub struct ServerInfo;

impl Tool for ServerInfo {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_server_info", "Show szal server info — version, capabilities", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            result_ok_json(&json!({
                "name": "szal",
                "version": env!("CARGO_PKG_VERSION"),
                "description": env!("CARGO_PKG_DESCRIPTION"),
                "mcp_backend": "bote",
                "capabilities": {
                    "execution_modes": ["sequential", "parallel", "dag", "hierarchical"],
                    "features": ["retry", "rollback", "timeout", "dag_cycle_detection", "state_machine"],
                },
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn engine_create_default() {
        let result = EngineCreate.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"max_concurrency\": 16"));
    }

    #[tokio::test]
    async fn engine_create_custom() {
        let result = EngineCreate.call(json!({"max_concurrency": 4, "global_timeout_ms": 60000})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"max_concurrency\": 4"));
    }

    #[tokio::test]
    async fn step_status_list() {
        let result = StepStatusList.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("pending"));
    }

    #[tokio::test]
    async fn error_list() {
        let result = ErrorList.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("CycleDetected"));
    }

    #[tokio::test]
    async fn server_info() {
        let result = ServerInfo.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("szal"));
        assert!(text.contains("bote"));
    }
}
