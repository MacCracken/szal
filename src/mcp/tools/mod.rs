//! Built-in MCP tools for workflow orchestration.

pub mod engine_tools;
pub mod flow_tools;
pub mod state_tools;
pub mod step_tools;
pub mod system_tools;

use crate::mcp::Tool;

/// Collect all built-in tools into a vec.
pub fn all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        // Step tools
        Box::new(step_tools::StepCreate),
        Box::new(step_tools::StepValidate),
        Box::new(step_tools::StepInspect),
        // Flow tools
        Box::new(flow_tools::FlowCreate),
        Box::new(flow_tools::FlowValidate),
        Box::new(flow_tools::FlowFromJson),
        Box::new(flow_tools::FlowListModes),
        Box::new(flow_tools::FlowAddStep),
        // State tools
        Box::new(state_tools::StateCheck),
        Box::new(state_tools::StateTransition),
        Box::new(state_tools::StateLifecycle),
        // Engine tools
        Box::new(engine_tools::EngineCreate),
        Box::new(engine_tools::ResultInspect),
        Box::new(engine_tools::StepStatusList),
        Box::new(engine_tools::ErrorList),
        Box::new(engine_tools::ServerInfo),
        // System tools
        Box::new(system_tools::SystemInfo),
        Box::new(system_tools::Cwd),
        Box::new(system_tools::EnvGet),
        Box::new(system_tools::Timestamp),
        Box::new(system_tools::UuidGen),
        Box::new(system_tools::JsonDiff),
        Box::new(system_tools::JsonValidate),
        Box::new(system_tools::Base64Tool),
    ]
}
