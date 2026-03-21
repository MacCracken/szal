//! MCP tools for flow creation, validation, and manipulation.

use crate::flow::{FlowDef, FlowMode};
use crate::mcp::{Tool, tool_def, result_ok, result_error};
use crate::step::StepDef;
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

fn parse_flow_mode(s: &str) -> Option<FlowMode> {
    match s {
        "sequential" => Some(FlowMode::Sequential),
        "parallel" => Some(FlowMode::Parallel),
        "dag" => Some(FlowMode::Dag),
        "hierarchical" => Some(FlowMode::Hierarchical),
        _ => None,
    }
}

pub struct FlowCreate;

impl Tool for FlowCreate {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_flow_create",
            "Create a workflow flow definition with execution mode and options",
            json!({
                "name": { "type": "string" },
                "mode": { "type": "string", "enum": ["sequential", "parallel", "dag", "hierarchical"] },
                "rollback_on_failure": { "type": "boolean" },
                "timeout_ms": { "type": "integer" },
                "steps": { "type": "array", "items": { "type": "object" } }
            }),
            vec!["name".into(), "mode".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return result_error("missing required field: name"),
            };
            let mode = match args.get("mode").and_then(|v| v.as_str()).and_then(parse_flow_mode) {
                Some(m) => m,
                None => return result_error("missing or invalid mode"),
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
                    match serde_json::from_value::<StepDef>(step_val.clone()) {
                        Ok(step) => flow.add_step(step),
                        Err(e) => return result_error(format!("invalid step: {e}")),
                    }
                }
            }
            result_ok(&serde_json::to_string_pretty(&flow).unwrap_or_default())
        })
    }
}

pub struct FlowValidate;

impl Tool for FlowValidate {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_flow_validate",
            "Validate a flow definition — checks for DAG cycles and missing dependencies",
            json!({ "flow_json": { "type": "string" } }),
            vec!["flow_json".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("flow_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: flow_json"),
            };
            let flow: FlowDef = match serde_json::from_str(json_str) {
                Ok(f) => f,
                Err(e) => return result_error(format!("invalid JSON: {e}")),
            };
            match flow.validate() {
                Ok(()) => result_ok(&format!("valid: flow '{}' ({} steps, mode={})", flow.name, flow.steps.len(), flow.mode)),
                Err(e) => result_error(format!("validation failed: {e}")),
            }
        })
    }
}

pub struct FlowFromJson;

impl Tool for FlowFromJson {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_flow_from_json",
            "Parse and inspect a flow definition from JSON",
            json!({ "flow_json": { "type": "string" } }),
            vec!["flow_json".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("flow_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: flow_json"),
            };
            let flow: FlowDef = match serde_json::from_str(json_str) {
                Ok(f) => f,
                Err(e) => return result_error(format!("invalid JSON: {e}")),
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
            result_ok(&serde_json::to_string_pretty(&info).unwrap_or_default())
        })
    }
}

pub struct FlowListModes;

impl Tool for FlowListModes {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_flow_list_modes", "List available workflow execution modes with descriptions", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            let modes = json!([
                { "mode": "sequential", "description": "Steps run one after another" },
                { "mode": "parallel", "description": "Steps run concurrently with no dependencies" },
                { "mode": "dag", "description": "Steps run based on dependency graph (Kahn's algorithm), with cycle detection" },
                { "mode": "hierarchical", "description": "Manager step delegates to sub-steps dynamically" },
            ]);
            result_ok(&serde_json::to_string_pretty(&modes).unwrap_or_default())
        })
    }
}

pub struct FlowAddStep;

impl Tool for FlowAddStep {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_flow_add_step",
            "Add a step to an existing flow definition, returning the updated flow",
            json!({
                "flow_json": { "type": "string" },
                "step_json": { "type": "string" }
            }),
            vec!["flow_json".into(), "step_json".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let flow_str = match args.get("flow_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: flow_json"),
            };
            let step_str = match args.get("step_json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: step_json"),
            };
            let mut flow: FlowDef = match serde_json::from_str(flow_str) {
                Ok(f) => f,
                Err(e) => return result_error(format!("invalid flow JSON: {e}")),
            };
            let step: StepDef = match serde_json::from_str(step_str) {
                Ok(s) => s,
                Err(e) => return result_error(format!("invalid step JSON: {e}")),
            };
            flow.add_step(step);
            result_ok(&serde_json::to_string_pretty(&flow).unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn flow_create_basic() {
        let result = FlowCreate.call(json!({"name": "ci-cd", "mode": "dag"})).await;
        assert_eq!(result["isError"], false);
        let flow: FlowDef = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(flow.name, "ci-cd");
        assert_eq!(flow.mode, FlowMode::Dag);
    }

    #[tokio::test]
    async fn flow_create_with_options() {
        let result = FlowCreate.call(json!({"name": "deploy", "mode": "sequential", "rollback_on_failure": true, "timeout_ms": 300000})).await;
        assert_eq!(result["isError"], false);
        let flow: FlowDef = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(flow.rollback_on_failure);
        assert_eq!(flow.timeout_ms, Some(300_000));
    }

    #[tokio::test]
    async fn flow_create_invalid_mode() {
        let result = FlowCreate.call(json!({"name": "x", "mode": "nope"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn flow_validate_valid_dag() {
        let build = StepDef::new("build");
        let test = StepDef::new("test").depends_on(build.id);
        let mut flow = FlowDef::new("pipeline", FlowMode::Dag);
        flow.add_step(build);
        flow.add_step(test);
        let flow_json = serde_json::to_string(&flow).unwrap();
        let result = FlowValidate.call(json!({"flow_json": flow_json})).await;
        assert_eq!(result["isError"], false);
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
        let result = FlowValidate.call(json!({"flow_json": serde_json::to_string(&flow).unwrap()})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn flow_list_modes() {
        let result = FlowListModes.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("sequential"));
        assert!(text.contains("dag"));
    }

    #[tokio::test]
    async fn flow_add_step() {
        let flow = FlowDef::new("test", FlowMode::Sequential);
        let step = StepDef::new("build");
        let result = FlowAddStep.call(json!({
            "flow_json": serde_json::to_string(&flow).unwrap(),
            "step_json": serde_json::to_string(&step).unwrap()
        })).await;
        assert_eq!(result["isError"], false);
        let updated: FlowDef = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(updated.steps.len(), 1);
    }

    #[tokio::test]
    async fn flow_from_json() {
        let mut flow = FlowDef::new("pipeline", FlowMode::Dag);
        flow.add_step(StepDef::new("build"));
        flow.add_step(StepDef::new("test"));
        let result = FlowFromJson.call(json!({"flow_json": serde_json::to_string(&flow).unwrap()})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"step_count\": 2"));
    }
}
