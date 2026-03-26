//! MCP tools for step creation and inspection.

use crate::mcp::{McpErrorCode, Tool, result_error_typed, result_ok, result_ok_json, tool_def};
use crate::step::StepDef;
use bote::ToolDef;
use serde_json::json;
use std::pin::Pin;

/// Create a workflow step with optional configuration.
pub struct StepCreate;

impl Tool for StepCreate {
    fn definition(&self) -> ToolDef {
        tool_def(
            "szal_step_create",
            "Create a workflow step definition with timeout, retry, and rollback config",
            json!({
                "name": { "type": "string", "description": "Step name" },
                "description": { "type": "string", "description": "Step description" },
                "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds (default: 30000)" },
                "max_retries": { "type": "integer", "description": "Max retry attempts (default: 0)" },
                "retry_delay_ms": { "type": "integer", "description": "Delay between retries in ms (default: 1000)" },
                "rollbackable": { "type": "boolean", "description": "Whether step supports rollback (default: false)" },
                "depends_on": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "UUIDs of steps this depends on"
                }
            }),
            vec!["name".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: name",
                    );
                }
            };

            let mut step = StepDef::new(name);

            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                step.description = desc.to_string();
            }
            if let Some(t) = args.get("timeout_ms").and_then(|v| v.as_u64()) {
                step = step.with_timeout(t);
            }
            if let Some(r) = args.get("max_retries").and_then(|v| v.as_u64()) {
                let delay = args
                    .get("retry_delay_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1_000);
                step = step.with_retries(r as u32, delay);
            }
            if args
                .get("rollbackable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                step = step.with_rollback();
            }
            if let Some(deps) = args.get("depends_on").and_then(|v| v.as_array()) {
                for dep in deps {
                    if let Some(id_str) = dep.as_str() {
                        match uuid::Uuid::parse_str(id_str) {
                            Ok(id) => step = step.depends_on(id),
                            Err(_) => {
                                return result_error_typed(
                                    McpErrorCode::Validation,
                                    format!("invalid UUID: {id_str}"),
                                );
                            }
                        }
                    }
                }
            }

            result_ok_json(&serde_json::to_value(&step).unwrap_or_default())
        })
    }
}

/// Validate a step definition from JSON.
pub struct StepValidate;

impl Tool for StepValidate {
    fn definition(&self) -> ToolDef {
        tool_def(
            "szal_step_validate",
            "Validate a step definition JSON, checking all fields are well-formed",
            json!({
                "step_json": { "type": "string", "description": "Step definition as JSON string" }
            }),
            vec!["step_json".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("step_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: step_json",
                    );
                }
            };

            match serde_json::from_str::<StepDef>(json_str) {
                Ok(step) => {
                    let mut issues = Vec::new();
                    if step.name.is_empty() {
                        issues.push("name is empty");
                    }
                    if step.timeout_ms == 0 {
                        issues.push("timeout_ms is zero");
                    }
                    if issues.is_empty() {
                        result_ok(&format!("valid: step '{}' (id={})", step.name, step.id))
                    } else {
                        result_error_typed(
                            McpErrorCode::Validation,
                            format!("issues: {}", issues.join(", ")),
                        )
                    }
                }
                Err(e) => {
                    result_error_typed(McpErrorCode::Validation, format!("invalid JSON: {e}"))
                }
            }
        })
    }
}

/// Inspect a step definition — return structured info.
pub struct StepInspect;

impl Tool for StepInspect {
    fn definition(&self) -> ToolDef {
        tool_def(
            "szal_step_inspect",
            "Inspect a step definition, returning its configuration details",
            json!({
                "step_json": { "type": "string", "description": "Step definition as JSON string" }
            }),
            vec!["step_json".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("step_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: step_json",
                    );
                }
            };

            match serde_json::from_str::<StepDef>(json_str) {
                Ok(step) => {
                    let info = json!({
                        "id": step.id.to_string(),
                        "name": step.name,
                        "description": step.description,
                        "timeout_ms": step.timeout_ms,
                        "max_retries": step.max_retries,
                        "retry_delay_ms": step.retry_delay_ms,
                        "rollbackable": step.rollbackable,
                        "dependency_count": step.depends_on.len(),
                        "depends_on": step.depends_on.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                    });
                    result_ok_json(&info)
                }
                Err(e) => {
                    result_error_typed(McpErrorCode::Validation, format!("invalid JSON: {e}"))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn step_create_basic() {
        let tool = StepCreate;
        let result = tool.call(json!({"name": "build"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let step: StepDef = serde_json::from_str(text).unwrap();
        assert_eq!(step.name, "build");
    }

    #[tokio::test]
    async fn step_create_full() {
        let tool = StepCreate;
        let result = tool
            .call(json!({
                "name": "deploy",
                "timeout_ms": 60000,
                "max_retries": 3,
                "retry_delay_ms": 5000,
                "rollbackable": true
            }))
            .await;
        assert_eq!(result["isError"], false);
        let step: StepDef =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(step.timeout_ms, 60_000);
        assert_eq!(step.max_retries, 3);
        assert!(step.rollbackable);
    }

    #[tokio::test]
    async fn step_create_missing_name() {
        let tool = StepCreate;
        let result = tool.call(json!({})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn step_validate_valid() {
        let step = StepDef::new("test");
        let json_str = serde_json::to_string(&step).unwrap();
        let tool = StepValidate;
        let result = tool.call(json!({"step_json": json_str})).await;
        assert_eq!(result["isError"], false);
    }

    #[tokio::test]
    async fn step_validate_bad_json() {
        let tool = StepValidate;
        let result = tool.call(json!({"step_json": "not json"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn step_inspect() {
        let step = StepDef::new("build").with_retries(3, 5000).with_rollback();
        let json_str = serde_json::to_string(&step).unwrap();
        let tool = StepInspect;
        let result = tool.call(json!({"step_json": json_str})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"rollbackable\": true"));
        assert!(text.contains("\"max_retries\": 3"));
    }
}
