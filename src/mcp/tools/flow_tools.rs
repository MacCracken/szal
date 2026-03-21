//! MCP tools for flow creation, validation, and manipulation.

use crate::flow::{FlowDef, FlowMode};
use crate::mcp::tool::{Tool, ToolDef, ToolResult};
use crate::step::StepDef;
use serde_json::json;
use std::future::Future;
use std::pin::Pin;

/// Create a flow definition.
pub struct FlowCreate;

impl Tool for FlowCreate {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_flow_create".into(),
            description: "Create a workflow flow definition with execution mode and options".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Flow name" },
                    "mode": {
                        "type": "string",
                        "enum": ["sequential", "parallel", "dag", "hierarchical"],
                        "description": "Execution mode"
                    },
                    "rollback_on_failure": { "type": "boolean", "description": "Rollback completed steps on failure (default: false)" },
                    "timeout_ms": { "type": "integer", "description": "Max flow duration in ms" },
                    "steps": {
                        "type": "array",
                        "items": { "type": "object" },
                        "description": "Array of step definitions to include"
                    }
                },
                "required": ["name", "mode"]
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return ToolResult::error("missing required field: name"),
            };
            let mode = match args.get("mode").and_then(|v| v.as_str()) {
                Some(m) => match parse_flow_mode(m) {
                    Some(mode) => mode,
                    None => return ToolResult::error(format!("invalid mode: {m}")),
                },
                None => return ToolResult::error("missing required field: mode"),
            };

            let mut flow = FlowDef::new(name, mode);

            if args.get("rollback_on_failure").and_then(|v| v.as_bool()).unwrap_or(false) {
                flow = flow.with_rollback();
            }
            if let Some(t) = args.get("timeout_ms").and_then(|v| v.as_u64()) {
                flow = flow.with_timeout(t);
            }
            if let Some(steps) = args.get("steps").and_then(|v| v.as_array()) {
                for step_val in steps {
                    let step_str = step_val.to_string();
                    match serde_json::from_str::<StepDef>(&step_str) {
                        Ok(step) => flow.add_step(step),
                        Err(e) => return ToolResult::error(format!("invalid step: {e}")),
                    }
                }
            }

            match serde_json::to_string_pretty(&flow) {
                Ok(json) => ToolResult::success(json),
                Err(e) => ToolResult::error(e.to_string()),
            }
        })
    }
}

/// Validate a flow definition (cycle detection, dependency checks).
pub struct FlowValidate;

impl Tool for FlowValidate {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_flow_validate".into(),
            description: "Validate a flow definition — checks for DAG cycles and missing dependencies".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "flow_json": { "type": "string", "description": "Flow definition as JSON string" }
                },
                "required": ["flow_json"]
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("flow_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: flow_json"),
            };

            let flow: FlowDef = match serde_json::from_str(json_str) {
                Ok(f) => f,
                Err(e) => return ToolResult::error(format!("invalid JSON: {e}")),
            };

            match flow.validate() {
                Ok(()) => ToolResult::success(format!(
                    "valid: flow '{}' ({} steps, mode={})",
                    flow.name,
                    flow.steps.len(),
                    flow.mode
                )),
                Err(e) => ToolResult::error(format!("validation failed: {e}")),
            }
        })
    }
}

/// Parse a flow from JSON.
pub struct FlowFromJson;

impl Tool for FlowFromJson {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_flow_from_json".into(),
            description: "Parse and inspect a flow definition from JSON".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "flow_json": { "type": "string", "description": "Flow definition as JSON string" }
                },
                "required": ["flow_json"]
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("flow_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: flow_json"),
            };

            let flow: FlowDef = match serde_json::from_str(json_str) {
                Ok(f) => f,
                Err(e) => return ToolResult::error(format!("invalid JSON: {e}")),
            };

            let info = json!({
                "id": flow.id.to_string(),
                "name": flow.name,
                "mode": flow.mode.to_string(),
                "step_count": flow.steps.len(),
                "rollback_on_failure": flow.rollback_on_failure,
                "timeout_ms": flow.timeout_ms,
                "steps": flow.steps.iter().map(|s| json!({
                    "id": s.id.to_string(),
                    "name": s.name,
                    "depends_on": s.depends_on.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            });

            ToolResult::success(serde_json::to_string_pretty(&info).unwrap_or_default())
        })
    }
}

/// List available flow execution modes.
pub struct FlowListModes;

impl Tool for FlowListModes {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_flow_list_modes".into(),
            description: "List available workflow execution modes with descriptions".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async {
            let modes = json!([
                { "mode": "sequential", "description": "Steps run one after another" },
                { "mode": "parallel", "description": "Steps run concurrently with no dependencies" },
                { "mode": "dag", "description": "Steps run based on dependency graph (Kahn's algorithm), with cycle detection" },
                { "mode": "hierarchical", "description": "Manager step delegates to sub-steps dynamically" },
            ]);
            ToolResult::success(serde_json::to_string_pretty(&modes).unwrap_or_default())
        })
    }
}

/// Add a step to an existing flow.
pub struct FlowAddStep;

impl Tool for FlowAddStep {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "szal_flow_add_step".into(),
            description: "Add a step to an existing flow definition, returning the updated flow".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "flow_json": { "type": "string", "description": "Existing flow definition JSON" },
                    "step_json": { "type": "string", "description": "Step definition JSON to add" }
                },
                "required": ["flow_json", "step_json"]
            }),
        }
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>> {
        Box::pin(async move {
            let flow_str = match args.get("flow_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: flow_json"),
            };
            let step_str = match args.get("step_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::error("missing required field: step_json"),
            };

            let mut flow: FlowDef = match serde_json::from_str(flow_str) {
                Ok(f) => f,
                Err(e) => return ToolResult::error(format!("invalid flow JSON: {e}")),
            };
            let step: StepDef = match serde_json::from_str(step_str) {
                Ok(s) => s,
                Err(e) => return ToolResult::error(format!("invalid step JSON: {e}")),
            };

            flow.add_step(step);

            match serde_json::to_string_pretty(&flow) {
                Ok(json) => ToolResult::success(json),
                Err(e) => ToolResult::error(e.to_string()),
            }
        })
    }
}

fn parse_flow_mode(s: &str) -> Option<FlowMode> {
    match s {
        "sequential" => Some(FlowMode::Sequential),
        "parallel" => Some(FlowMode::Parallel),
        "dag" => Some(FlowMode::Dag),
        "hierarchical" => Some(FlowMode::Hierarchical),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn flow_create_basic() {
        let tool = FlowCreate;
        let result = tool.call(json!({"name": "ci-cd", "mode": "dag"})).await;
        assert!(!result.is_error);
        let flow: FlowDef = serde_json::from_str(result.content[0].text.as_deref().unwrap()).unwrap();
        assert_eq!(flow.name, "ci-cd");
        assert_eq!(flow.mode, FlowMode::Dag);
    }

    #[tokio::test]
    async fn flow_create_with_options() {
        let tool = FlowCreate;
        let result = tool.call(json!({
            "name": "deploy",
            "mode": "sequential",
            "rollback_on_failure": true,
            "timeout_ms": 300000
        })).await;
        assert!(!result.is_error);
        let flow: FlowDef = serde_json::from_str(result.content[0].text.as_deref().unwrap()).unwrap();
        assert!(flow.rollback_on_failure);
        assert_eq!(flow.timeout_ms, Some(300_000));
    }

    #[tokio::test]
    async fn flow_create_invalid_mode() {
        let tool = FlowCreate;
        let result = tool.call(json!({"name": "x", "mode": "nope"})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn flow_validate_valid_dag() {
        let build = StepDef::new("build");
        let test = StepDef::new("test").depends_on(build.id);
        let mut flow = FlowDef::new("pipeline", FlowMode::Dag);
        flow.add_step(build);
        flow.add_step(test);
        let flow_json = serde_json::to_string(&flow).unwrap();

        let tool = FlowValidate;
        let result = tool.call(json!({"flow_json": flow_json})).await;
        assert!(!result.is_error);
        assert!(result.content[0].text.as_deref().unwrap().contains("valid"));
    }

    #[tokio::test]
    async fn flow_validate_cycle() {
        let mut a = StepDef::new("a");
        let mut b = StepDef::new("b");
        b.depends_on = vec![a.id];
        a.depends_on = vec![b.id];
        let mut flow = FlowDef::new("broken", FlowMode::Dag);
        flow.add_step(a);
        flow.add_step(b);
        let flow_json = serde_json::to_string(&flow).unwrap();

        let tool = FlowValidate;
        let result = tool.call(json!({"flow_json": flow_json})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn flow_list_modes() {
        let tool = FlowListModes;
        let result = tool.call(json!({})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("sequential"));
        assert!(text.contains("dag"));
    }

    #[tokio::test]
    async fn flow_add_step() {
        let flow = FlowDef::new("test", FlowMode::Sequential);
        let step = StepDef::new("build");
        let flow_json = serde_json::to_string(&flow).unwrap();
        let step_json = serde_json::to_string(&step).unwrap();

        let tool = FlowAddStep;
        let result = tool.call(json!({
            "flow_json": flow_json,
            "step_json": step_json
        })).await;
        assert!(!result.is_error);
        let updated: FlowDef = serde_json::from_str(result.content[0].text.as_deref().unwrap()).unwrap();
        assert_eq!(updated.steps.len(), 1);
    }

    #[tokio::test]
    async fn flow_from_json() {
        let mut flow = FlowDef::new("pipeline", FlowMode::Dag);
        flow.add_step(StepDef::new("build"));
        flow.add_step(StepDef::new("test"));
        let flow_json = serde_json::to_string(&flow).unwrap();

        let tool = FlowFromJson;
        let result = tool.call(json!({"flow_json": flow_json})).await;
        assert!(!result.is_error);
        let text = result.content[0].text.as_deref().unwrap();
        assert!(text.contains("\"step_count\": 2"));
    }
}
