//! MCP JSON-RPC 2.0 protocol types.
//!
//! ```
//! use szal::mcp::protocol::{JsonRpcRequest, JsonRpcResponse, MCP_VERSION};
//!
//! assert_eq!(MCP_VERSION, "2025-03-26");
//!
//! let req = JsonRpcRequest {
//!     jsonrpc: "2.0".into(),
//!     id: serde_json::json!(1),
//!     method: "tools/list".into(),
//!     params: None,
//! };
//! assert_eq!(req.method, "tools/list");
//! ```

use serde::{Deserialize, Serialize};

/// MCP protocol version.
pub const MCP_VERSION: &str = "2025-03-26";

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// JSON-RPC 2.0 error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard JSON-RPC error codes.
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

/// MCP-specific error codes.
pub const TOOL_NOT_FOUND: i32 = -32001;
pub const TOOL_EXECUTION_ERROR: i32 = -32002;
pub const RESOURCE_NOT_FOUND: i32 = -32003;

/// MCP initialize request params.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

/// MCP client capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<serde_json::Value>,
}

/// MCP client info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// MCP initialize result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

/// MCP server capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<serde_json::Value>,
}

/// MCP server info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_success() {
        let resp = JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"ok": true}));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn response_error() {
        let resp = JsonRpcResponse::error(serde_json::json!(1), METHOD_NOT_FOUND, "not found");
        assert!(resp.result.is_none());
        assert_eq!(resp.error.as_ref().unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn request_serde_roundtrip() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(42),
            method: "tools/call".into(),
            params: Some(serde_json::json!({"name": "test"})),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.method, "tools/call");
    }

    #[test]
    fn initialize_result() {
        let result = InitializeResult {
            protocol_version: MCP_VERSION.into(),
            capabilities: ServerCapabilities {
                tools: Some(serde_json::json!({})),
                resources: None,
                prompts: None,
            },
            server_info: ServerInfo {
                name: "szal".into(),
                version: Some("0.22.0".into()),
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("szal"));
    }
}
