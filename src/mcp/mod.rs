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

#[cfg(feature = "majra")]
pub mod pool;
#[cfg(feature = "majra")]
pub mod tenant;

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
#[must_use]
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
                rt.block_on(t.call(args))
            }),
        );
    }

    dispatcher
}

/// Build a successful MCP tool response.
#[must_use]
pub fn result_ok(text: &str) -> serde_json::Value {
    serde_json::json!({"content": [{"type": "text", "text": text}], "isError": false})
}

/// Build a successful MCP tool response from a JSON value (serialized once).
#[must_use]
pub fn result_ok_json(value: &serde_json::Value) -> serde_json::Value {
    let text = serde_json::to_string_pretty(value).unwrap_or_default();
    serde_json::json!({"content": [{"type": "text", "text": text}], "isError": false})
}

/// Build an error MCP tool response.
#[must_use]
pub fn result_error(msg: impl Into<String>) -> serde_json::Value {
    serde_json::json!({"content": [{"type": "text", "text": msg.into()}], "isError": true})
}

/// Structured error code for MCP tool responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum McpErrorCode {
    /// Permanent: bad input, missing fields, invalid format.
    Validation,
    /// Permanent: file, resource, or key not found.
    NotFound,
    /// Permanent: path outside working directory, security rejection.
    PermissionDenied,
    /// Transient: operation timed out.
    Timeout,
    /// Transient: filesystem or network I/O failure.
    IoError,
    /// Transient: unexpected internal error.
    Internal,
}

impl McpErrorCode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::Timeout => "timeout",
            Self::IoError => "io_error",
            Self::Internal => "internal",
        }
    }

    #[must_use]
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Timeout | Self::IoError | Self::Internal)
    }
}

/// Build an error MCP tool response with a structured error code.
#[must_use]
pub fn result_error_typed(code: McpErrorCode, msg: impl Into<String>) -> serde_json::Value {
    let msg = msg.into();
    serde_json::json!({
        "content": [{"type": "text", "text": msg}],
        "isError": true,
        "_meta": {
            "error_code": code.as_str(),
            "retryable": code.is_retryable()
        }
    })
}

/// Validate that a path resolves to a location under the current working directory.
/// For paths that don't exist yet (e.g. FileWrite to a new file), the parent must exist.
pub async fn validate_path(path: &str) -> Result<std::path::PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("failed to get cwd: {e}"))?;
    let cwd = tokio::fs::canonicalize(&cwd)
        .await
        .map_err(|e| format!("failed to resolve cwd: {e}"))?;

    let p = std::path::Path::new(path);

    // Resolve to absolute path
    let resolved = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };

    // Canonicalize to resolve symlinks and ..
    // For new files (FileWrite), parent must exist
    let canonical = if tokio::fs::metadata(&resolved).await.is_ok() {
        tokio::fs::canonicalize(&resolved)
            .await
            .map_err(|e| format!("failed to resolve path: {e}"))?
    } else {
        // For non-existent paths, canonicalize the parent
        let parent = resolved
            .parent()
            .ok_or_else(|| "invalid path".to_string())?;
        let canonical_parent = tokio::fs::canonicalize(parent)
            .await
            .map_err(|e| format!("failed to resolve parent path: {e}"))?;
        canonical_parent.join(resolved.file_name().unwrap_or_default())
    };

    if !canonical.starts_with(&cwd) {
        return Err(format!("path '{}' is outside working directory", path));
    }

    Ok(canonical)
}

/// Helper to build a bote ToolDef with common patterns.
#[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn validate_path_current_dir() {
        assert!(validate_path(".").await.is_ok());
    }

    #[tokio::test]
    async fn validate_path_rejects_outside_cwd() {
        let err = validate_path("/etc/passwd").await.unwrap_err();
        assert!(
            err.contains("outside working directory"),
            "expected 'outside working directory', got: {err}"
        );
    }

    #[tokio::test]
    async fn validate_path_rejects_traversal() {
        assert!(validate_path("../../etc/passwd").await.is_err());
    }

    #[tokio::test]
    async fn validate_path_new_file_in_valid_dir() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::TempDir::new_in(&cwd).unwrap();
        let new_file = tmp.path().join("newfile.txt");
        let result = validate_path(new_file.to_str().unwrap()).await;
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }
}
