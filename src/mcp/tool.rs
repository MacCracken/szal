//! MCP tool trait and types.
//!
//! ```
//! use szal::mcp::tool::{ToolDef, ToolResult, ToolContent, ContentType};
//!
//! let tool = ToolDef {
//!     name: "workflow_validate".into(),
//!     description: "Validate a workflow DAG".into(),
//!     input_schema: serde_json::json!({
//!         "type": "object",
//!         "properties": {
//!             "flow_json": { "type": "string" }
//!         },
//!         "required": ["flow_json"]
//!     }),
//! };
//! assert_eq!(tool.name, "workflow_validate");
//! ```

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

/// MCP tool definition (returned by tools/list).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP tool call parameters (from tools/call).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Content type for tool results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Text,
    Image,
    Resource,
}

/// A single content block in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: ContentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl ToolContent {
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            content_type: ContentType::Text,
            text: Some(s.into()),
        }
    }
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: true,
        }
    }
}

/// Trait that all MCP tools implement.
pub trait Tool: Send + Sync {
    /// Tool definition for discovery.
    fn definition(&self) -> ToolDef;

    /// Execute the tool with the given arguments.
    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_result_success() {
        let r = ToolResult::success("done");
        assert!(!r.is_error);
        assert_eq!(r.content[0].text.as_deref(), Some("done"));
    }

    #[test]
    fn tool_result_error() {
        let r = ToolResult::error("failed");
        assert!(r.is_error);
    }

    #[test]
    fn tool_def_serde() {
        let def = ToolDef {
            name: "test".into(),
            description: "A test tool".into(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let json = serde_json::to_string(&def).unwrap();
        let back: ToolDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
    }

    #[test]
    fn tool_content_text() {
        let c = ToolContent::text("hello");
        assert_eq!(c.text.as_deref(), Some("hello"));
    }
}
