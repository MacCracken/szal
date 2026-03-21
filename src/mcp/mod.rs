//! MCP tool implementations for szal workflows.
//!
//! Szal provides workflow tools that register with [bote](https://crates.io/crates/bote)'s
//! MCP dispatcher. Bote owns the protocol, dispatch, and transport layers —
//! szal just implements tools.
//!
//! ```
//! use szal::mcp::register_tools;
//!
//! let dispatcher = register_tools();
//! // dispatcher is ready to handle JSON-RPC requests
//! ```

pub mod tools;

use bote::{Dispatcher, ToolDef, ToolRegistry, ToolSchema};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait that szal MCP tools implement.
pub trait Tool: Send + Sync {
    /// Tool definition for bote registry.
    fn definition(&self) -> ToolDef;

    /// Execute the tool — returns JSON result value.
    fn call(
        &self,
        args: serde_json::Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>>;
}

/// Register all szal workflow tools and return a ready-to-use bote dispatcher.
pub fn register_tools() -> Dispatcher {
    let tool_impls = tools::all_tools();
    let mut registry = ToolRegistry::new();

    for tool in &tool_impls {
        registry.register(tool.definition());
    }

    let mut dispatcher = Dispatcher::new(registry);

    for tool in tool_impls {
        let tool = Arc::new(tool);
        let tool_name = tool.definition().name.clone();
        let t = tool.clone();
        dispatcher.handle(
            tool_name,
            Arc::new(move |args: serde_json::Value| {
                let t = t.clone();
                let rt = tokio::runtime::Handle::current();
                std::thread::scope(|_| rt.block_on(async { t.call(args).await }))
            }),
        );
    }

    dispatcher
}

/// Build a successful MCP tool response.
pub fn result_ok(text: &str) -> serde_json::Value {
    serde_json::json!({"content": [{"type": "text", "text": text}], "isError": false})
}

/// Build an error MCP tool response.
pub fn result_error(msg: impl Into<String>) -> serde_json::Value {
    serde_json::json!({"content": [{"type": "text", "text": msg.into()}], "isError": true})
}

/// Helper to build a bote ToolDef with common patterns.
pub fn tool_def(
    name: impl Into<String>,
    description: impl Into<String>,
    properties: serde_json::Value,
    required: Vec<String>,
) -> ToolDef {
    let props: HashMap<String, serde_json::Value> = match properties {
        serde_json::Value::Object(map) => map.into_iter().collect(),
        _ => HashMap::new(),
    };
    ToolDef {
        name: name.into(),
        description: description.into(),
        input_schema: ToolSchema {
            schema_type: "object".into(),
            properties: props,
            required,
        },
    }
}
