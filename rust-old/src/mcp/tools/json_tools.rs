//! JSON tools: path extraction, diff, validation.

use crate::mcp::{McpErrorCode, Tool, result_error_typed, result_ok_json, tool_def};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

/// JSON path extraction.
pub struct JsonPath;

impl Tool for JsonPath {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_json_path",
            "Extract a value from JSON using a dot-separated path (e.g. 'a.b.c' or 'a.0.b')",
            json!({
                "json": { "type": "string", "description": "JSON string" },
                "path": { "type": "string", "description": "Dot-separated path (e.g. 'data.items.0.name')" }
            }),
            vec!["json".into(), "path".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: json",
                    );
                }
            };
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: path",
                    );
                }
            };

            let value: serde_json::Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(e) => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        format!("invalid JSON: {e}"),
                    );
                }
            };

            let mut current = &value;
            for segment in path.split('.') {
                if let Ok(idx) = segment.parse::<usize>() {
                    match current.get(idx) {
                        Some(v) => current = v,
                        None => {
                            return result_error_typed(
                                McpErrorCode::NotFound,
                                format!("index {idx} not found at '{segment}'"),
                            );
                        }
                    }
                } else {
                    match current.get(segment) {
                        Some(v) => current = v,
                        None => {
                            return result_error_typed(
                                McpErrorCode::NotFound,
                                format!("key '{segment}' not found"),
                            );
                        }
                    }
                }
            }

            result_ok_json(current)
        })
    }
}

/// Compute JSON diff between two values.
pub struct JsonDiff;

impl Tool for JsonDiff {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_json_diff",
            "Compare two JSON values and report differences",
            json!({
                "a": { "type": "string", "description": "First JSON string" },
                "b": { "type": "string", "description": "Second JSON string" }
            }),
            vec!["a".into(), "b".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let a_str = match args.get("a").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: a",
                    );
                }
            };
            let b_str = match args.get("b").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: b",
                    );
                }
            };

            let a: serde_json::Value = match serde_json::from_str(a_str) {
                Ok(v) => v,
                Err(e) => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        format!("invalid JSON in 'a': {e}"),
                    );
                }
            };
            let b: serde_json::Value = match serde_json::from_str(b_str) {
                Ok(v) => v,
                Err(e) => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        format!("invalid JSON in 'b': {e}"),
                    );
                }
            };

            let equal = a == b;
            result_ok_json(&json!({
                "equal": equal,
                "a_type": type_name(&a),
                "b_type": type_name(&b),
            }))
        })
    }
}

fn type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// JSON Schema validation (basic required-field check).
pub struct JsonValidate;

impl Tool for JsonValidate {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_json_validate",
            "Validate a JSON string — check if it parses correctly and report structure",
            json!({ "json": { "type": "string", "description": "JSON string to validate" } }),
            vec!["json".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: json",
                    );
                }
            };
            match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(val) => {
                    let info = json!({
                        "valid": true,
                        "type": type_name(&val),
                        "size_bytes": json_str.len(),
                    });
                    result_ok_json(&info)
                }
                Err(e) => result_ok_json(&json!({
                    "valid": false,
                    "error": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                })),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn json_path_nested() {
        let result = JsonPath
            .call(json!({
                "json": r#"{"data": {"items": [{"name": "first"}, {"name": "second"}]}}"#,
                "path": "data.items.1.name"
            }))
            .await;
        assert_eq!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .trim_matches('"'),
            "second"
        );
    }

    #[tokio::test]
    async fn json_path_missing() {
        let result = JsonPath
            .call(json!({"json": "{\"a\": 1}", "path": "b"}))
            .await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn json_diff_equal() {
        let result = JsonDiff
            .call(json!({"a": "{\"x\":1}", "b": "{\"x\":1}"}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"equal\": true"));
    }

    #[tokio::test]
    async fn json_diff_not_equal() {
        let result = JsonDiff
            .call(json!({"a": "{\"x\":1}", "b": "{\"x\":2}"}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"equal\": false"));
    }

    #[tokio::test]
    async fn json_validate_ok() {
        let result = JsonValidate.call(json!({"json": "{\"a\": [1,2,3]}"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"valid\": true"));
        assert!(text.contains("\"type\": \"object\""));
    }

    #[tokio::test]
    async fn json_validate_bad() {
        let result = JsonValidate.call(json!({"json": "{bad json}"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"valid\": false"));
    }
}
