//! Built-in MCP tools for workflow orchestration.

pub mod engine_tools;
pub mod flow_tools;
pub mod state_tools;
pub mod step_tools;

use crate::mcp::registry::Registry;

/// Register all built-in workflow tools with the registry.
pub fn register_all(registry: &mut Registry) {
    // Step tools
    registry.register_tool(step_tools::StepCreate);
    registry.register_tool(step_tools::StepValidate);
    registry.register_tool(step_tools::StepInspect);

    // Flow tools
    registry.register_tool(flow_tools::FlowCreate);
    registry.register_tool(flow_tools::FlowValidate);
    registry.register_tool(flow_tools::FlowFromJson);
    registry.register_tool(flow_tools::FlowListModes);
    registry.register_tool(flow_tools::FlowAddStep);

    // State tools
    registry.register_tool(state_tools::StateCheck);
    registry.register_tool(state_tools::StateTransition);
    registry.register_tool(state_tools::StateLifecycle);

    // Engine tools
    registry.register_tool(engine_tools::EngineCreate);
    registry.register_tool(engine_tools::ResultInspect);
    registry.register_tool(engine_tools::StepStatusList);
    registry.register_tool(engine_tools::ErrorList);
    registry.register_tool(engine_tools::ServerInfo);
}
