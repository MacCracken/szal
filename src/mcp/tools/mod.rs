//! Built-in MCP tools for workflow orchestration.

pub mod engine_tools;
pub mod file_tools;
pub mod flow_tools;
pub mod hash_tools;
pub mod process_tools;
pub mod state_tools;
pub mod step_tools;
pub mod system_tools;
pub mod template_tools;

use crate::mcp::Tool;

/// Collect all built-in tools into a vec.
pub fn all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        // Step tools (3)
        Box::new(step_tools::StepCreate),
        Box::new(step_tools::StepValidate),
        Box::new(step_tools::StepInspect),
        // Flow tools (5)
        Box::new(flow_tools::FlowCreate),
        Box::new(flow_tools::FlowValidate),
        Box::new(flow_tools::FlowFromJson),
        Box::new(flow_tools::FlowListModes),
        Box::new(flow_tools::FlowAddStep),
        // State tools (3)
        Box::new(state_tools::StateCheck),
        Box::new(state_tools::StateTransition),
        Box::new(state_tools::StateLifecycle),
        // Engine tools (5)
        Box::new(engine_tools::EngineCreate),
        Box::new(engine_tools::ResultInspect),
        Box::new(engine_tools::StepStatusList),
        Box::new(engine_tools::ErrorList),
        Box::new(engine_tools::ServerInfo),
        // System tools (8)
        Box::new(system_tools::SystemInfo),
        Box::new(system_tools::Cwd),
        Box::new(system_tools::EnvGet),
        Box::new(system_tools::Timestamp),
        Box::new(system_tools::UuidGen),
        Box::new(system_tools::JsonDiff),
        Box::new(system_tools::JsonValidate),
        Box::new(system_tools::Base64Tool),
        // File tools (5)
        Box::new(file_tools::FileRead),
        Box::new(file_tools::FileWrite),
        Box::new(file_tools::DirList),
        Box::new(file_tools::FileStat),
        Box::new(file_tools::PathExists),
        // Process tools (3)
        Box::new(process_tools::Exec),
        Box::new(process_tools::Pid),
        Box::new(process_tools::Which),
        // Hash tools (3)
        Box::new(hash_tools::Sha256),
        Box::new(hash_tools::Md5),
        Box::new(hash_tools::RandomToken),
        // Template/text tools (5)
        Box::new(template_tools::TemplateRender),
        Box::new(template_tools::WordCount),
        Box::new(template_tools::TextReplace),
        Box::new(template_tools::TextSplit),
        Box::new(template_tools::TextJoin),
    ]
}
