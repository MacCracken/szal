//! Tool registry — dynamic registration and lookup.
//!
//! ```
//! use szal::mcp::registry::Registry;
//!
//! let registry = Registry::new();
//! assert_eq!(registry.list_tools().len(), 0);
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use crate::mcp::protocol::{
    InitializeResult, JsonRpcRequest, JsonRpcResponse, MCP_VERSION, ServerCapabilities, ServerInfo,
};
use crate::mcp::tool::{Tool, ToolCallParams, ToolDef, ToolResult};

/// Central registry for MCP tools, resources, and prompts.
pub struct Registry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Replaces any existing tool with the same name.
    pub fn register_tool(&mut self, tool: impl Tool + 'static) {
        let def = tool.definition();
        self.tools.insert(def.name.clone(), Arc::new(tool));
    }

    /// List all registered tool definitions.
    pub fn list_tools(&self) -> Vec<ToolDef> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Look up a tool by name.
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Execute a tool by name.
    pub async fn call_tool(&self, name: &str, args: serde_json::Value) -> ToolResult {
        match self.get_tool(name) {
            Some(tool) => tool.call(args).await,
            None => ToolResult::error(format!("tool not found: {name}")),
        }
    }

    /// Number of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Build the MCP initialize response.
    pub fn initialize_result(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: MCP_VERSION.into(),
            capabilities: ServerCapabilities {
                tools: Some(serde_json::json!({})),
                resources: None,
                prompts: None,
            },
            server_info: ServerInfo {
                name: "szal".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            },
        }
    }

    /// Route a JSON-RPC request to the appropriate handler.
    pub async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        match req.method.as_str() {
            "initialize" => {
                let result = self.initialize_result();
                JsonRpcResponse::success(
                    req.id,
                    serde_json::to_value(result).unwrap_or_default(),
                )
            }
            "tools/list" => {
                let tools = self.list_tools();
                JsonRpcResponse::success(
                    req.id,
                    serde_json::json!({ "tools": tools }),
                )
            }
            "tools/call" => {
                let params: ToolCallParams = match req.params {
                    Some(p) => match serde_json::from_value(p) {
                        Ok(p) => p,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                req.id,
                                crate::mcp::protocol::INVALID_PARAMS,
                                e.to_string(),
                            )
                        }
                    },
                    None => {
                        return JsonRpcResponse::error(
                            req.id,
                            crate::mcp::protocol::INVALID_PARAMS,
                            "missing params",
                        )
                    }
                };
                let result = self.call_tool(&params.name, params.arguments).await;
                JsonRpcResponse::success(
                    req.id,
                    serde_json::to_value(result).unwrap_or_default(),
                )
            }
            "ping" => JsonRpcResponse::success(req.id, serde_json::json!({})),
            _ => JsonRpcResponse::error(
                req.id,
                crate::mcp::protocol::METHOD_NOT_FOUND,
                format!("unknown method: {}", req.method),
            ),
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool::ToolDef;
    use std::pin::Pin;

    struct EchoTool;

    impl Tool for EchoTool {
        fn definition(&self) -> ToolDef {
            ToolDef {
                name: "echo".into(),
                description: "Echo input back".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "message": { "type": "string" } },
                    "required": ["message"]
                }),
            }
        }

        fn call(
            &self,
            args: serde_json::Value,
        ) -> Pin<Box<dyn std::future::Future<Output = ToolResult> + Send + '_>> {
            Box::pin(async move {
                let msg = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("no message");
                ToolResult::success(msg)
            })
        }
    }

    #[test]
    fn register_and_list() {
        let mut reg = Registry::new();
        reg.register_tool(EchoTool);
        assert_eq!(reg.tool_count(), 1);
        let tools = reg.list_tools();
        assert_eq!(tools[0].name, "echo");
    }

    #[tokio::test]
    async fn call_registered_tool() {
        let mut reg = Registry::new();
        reg.register_tool(EchoTool);
        let result = reg
            .call_tool("echo", serde_json::json!({"message": "hello"}))
            .await;
        assert!(!result.is_error);
        assert_eq!(result.content[0].text.as_deref(), Some("hello"));
    }

    #[tokio::test]
    async fn call_missing_tool() {
        let reg = Registry::new();
        let result = reg.call_tool("nope", serde_json::json!({})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn handle_initialize() {
        let reg = Registry::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "initialize".into(),
            params: None,
        };
        let resp = reg.handle_request(req).await;
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], MCP_VERSION);
        assert_eq!(result["serverInfo"]["name"], "szal");
    }

    #[tokio::test]
    async fn handle_tools_list() {
        let mut reg = Registry::new();
        reg.register_tool(EchoTool);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(2),
            method: "tools/list".into(),
            params: None,
        };
        let resp = reg.handle_request(req).await;
        let tools = &resp.result.unwrap()["tools"];
        assert_eq!(tools.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn handle_tools_call() {
        let mut reg = Registry::new();
        reg.register_tool(EchoTool);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(3),
            method: "tools/call".into(),
            params: Some(serde_json::json!({
                "name": "echo",
                "arguments": { "message": "test" }
            })),
        };
        let resp = reg.handle_request(req).await;
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap_or(true));
    }

    #[tokio::test]
    async fn handle_unknown_method() {
        let reg = Registry::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(4),
            method: "unknown/method".into(),
            params: None,
        };
        let resp = reg.handle_request(req).await;
        assert!(resp.error.is_some());
    }

    #[tokio::test]
    async fn handle_ping() {
        let reg = Registry::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(5),
            method: "ping".into(),
            params: None,
        };
        let resp = reg.handle_request(req).await;
        assert!(resp.result.is_some());
    }
}
