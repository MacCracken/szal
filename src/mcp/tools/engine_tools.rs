//! MCP tools for engine configuration and flow result inspection.

use crate::engine::EngineConfig;
use crate::mcp::tool::{Tool, ToolDef, ToolResult};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;

/// Create an engine configuration.
pub struct EngineCreate;

impl Tool for EngineCreate {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_engine_create".into(),
            description: "Create an engine configuration with concurrency and timeout settings".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "max_concurrency": { "type": "integer", "description": "Max concurrent steps (default: 16)" },
                    "global_timeout_ms": { "type": "integer", "description": "Global timeout in ms (overrides per-flow)" }
                }
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let mut config = EngineConfig::default();
            if let Some(c) = args.get("max_concurrency").and_then(|v| v.as_u64()) {
                config.max_concurrency = c as usize;
            }
            if let Some(t) = args.get("global_timeout_ms").and_then(|v| v.as_u64()) {
                config.global_timeout_ms = Some(t);
            }
            let info = json!({
                "max_concurrency": config.max_concurrency,
                "global_timeout_ms": config.global_timeout_ms,
            });
            ToolResult::success(serde_json::to_string_pretty(&info).unwrap_or_default())
        })
    }
}

/// Inspect a flow execution result.
pub struct ResultInspect;

impl Tool for ResultInspect {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_result_inspect".into(),
            description: "Inspect a flow execution result — step counts, duration, success/failure".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "result_json": { "type": "string", "description": "Flow result as JSON string" }
                },
                "required": ["result_json"]
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("result_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: result_json"),
            };

            // Parse as generic JSON since FlowResult doesn't derive Deserialize
            let val: serde_json::Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(e) => return ToolResult::error(format!("invalid JSON: {e}")),
            };

            let flow_name = val.get("flow_name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let success = val.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let rolled_back = val.get("rolled_back").and_then(|v| v.as_bool()).unwrap_or(false);
            let total_ms = val.get("total_duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            let steps = val.get("steps").and_then(|v| v.as_array());

            let step_count = steps.map(|s| s.len()).unwrap_or(0);
            let completed = steps.map(|s| s.iter().filter(|st| st.get("status").and_then(|v| v.as_str()) == Some("Completed")).count()).unwrap_or(0);
            let failed = steps.map(|s| s.iter().filter(|st| st.get("status").and_then(|v| v.as_str()) == Some("Failed")).count()).unwrap_or(0);

            let info = json!({
                "flow_name": flow_name,
                "success": success,
                "rolled_back": rolled_back,
                "total_duration_ms": total_ms,
                "step_count": step_count,
                "completed": completed,
                "failed": failed,
            });

            ToolResult::success(serde_json::to_string_pretty(&info).unwrap_or_default())
        })
    }
}

/// List available step statuses.
pub struct StepStatusList;

impl Tool for StepStatusList {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_step_status_list".into(),
            description: "List all possible step execution statuses".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async {
            let statuses = json!([
                { "status": "pending", "description": "Step has not started" },
                { "status": "running", "description": "Step is currently executing" },
                { "status": "completed", "description": "Step finished successfully" },
                { "status": "failed", "description": "Step execution failed" },
                { "status": "skipped", "description": "Step was skipped (dependency failed or condition not met)" },
                { "status": "rolled_back", "description": "Step was rolled back after failure" },
            ]);
            ToolResult::success(serde_json::to_string_pretty(&statuses).unwrap_or_default())
        })
    }
}

/// Show error types that can occur during workflow execution.
pub struct ErrorList;

impl Tool for ErrorList {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_error_list".into(),
            description: "List all workflow error types with descriptions".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async {
            let errors = json!([
                { "error": "StepFailed", "description": "A step failed with a specific reason" },
                { "error": "StepTimeout", "description": "A step exceeded its timeout" },
                { "error": "InvalidFlow", "description": "Flow configuration is invalid" },
                { "error": "RetryExhausted", "description": "All retry attempts failed" },
                { "error": "RollbackFailed", "description": "Rollback operation failed" },
                { "error": "CycleDetected", "description": "DAG contains a cycle" },
            ]);
            ToolResult::success(serde_json::to_string_pretty(&errors).unwrap_or_default())
        })
    }
}

/// Describe szal capabilities and version.
pub struct ServerInfo;

impl Tool for ServerInfo {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_server_info".into(),
            description: "Show szal server info — version, capabilities, registered tool count".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async {
            let info = json!({
                "name": "szal",
                "version": env!("CARGO_PKG_VERSION"),
                "description": env!("CARGO_PKG_DESCRIPTION"),
                "capabilities": {
                    "execution_modes": ["sequential", "parallel", "dag", "hierarchical"],
                    "features": ["retry", "rollback", "timeout", "dag_cycle_detection", "state_machine"],
                    "transports": ["stdio", "http", "sse"],
                },
            });
            ToolResult::success(serde_json::to_string_pretty(&info).unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn engine_create_default() {
        let tool = EngineCreate;
        let result = tool.call(json!({})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("\"max_concurrency\": 16"));
    }

    #[tokio::test]
    async fn engine_create_custom() {
        let tool = EngineCreate;
        let result = tool.call(json!({"max_concurrency": 4, "global_timeout_ms": 60000})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("\"max_concurrency\": 4"));
        assert!(text.contains("60000"));
    }

    #[tokio::test]
    async fn step_status_list() {
        let tool = StepStatusList;
        let result = tool.call(json!({})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("pending"));
        assert!(text.contains("rolled_back"));
    }

    #[tokio::test]
    async fn error_list() {
        let tool = ErrorList;
        let result = tool.call(json!({})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("CycleDetected"));
        assert!(text.contains("RetryExhausted"));
    }

    #[tokio::test]
    async fn server_info() {
        let tool = ServerInfo;
        let result = tool.call(json!({})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("szal"));
        assert!(text.contains("dag"));
    }
}
