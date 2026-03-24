//! Encoding tools: UUID generation, base64 encode/decode.

use crate::mcp::{Tool, result_error, result_ok, result_ok_json, tool_def};
use base64::{Engine as B64Engine, engine::general_purpose::STANDARD};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

/// Maximum number of UUIDs that can be generated in one call.
const MAX_UUID_COUNT: u64 = 100;

/// Generate a UUID v4.
pub struct UuidGen;

impl Tool for UuidGen {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_uuid",
            "Generate one or more UUID v4 identifiers",
            json!({ "count": { "type": "integer", "description": "Number of UUIDs to generate (default: 1, max: 100)" } }),
            vec![],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let count = args
                .get("count")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                .min(MAX_UUID_COUNT) as usize;
            let uuids: Vec<String> = (0..count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect();
            if count == 1 {
                result_ok(&uuids[0])
            } else {
                result_ok_json(&json!(uuids))
            }
        })
    }
}

/// Base64 encode/decode.
pub struct Base64Tool;

impl Tool for Base64Tool {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_base64",
            "Encode or decode a string using base64",
            json!({
                "input": { "type": "string", "description": "Input string" },
                "operation": { "type": "string", "enum": ["encode", "decode"], "description": "Operation to perform (default: encode)" }
            }),
            vec!["input".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let input = match args.get("input").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: input"),
            };
            let op = args
                .get("operation")
                .and_then(|v| v.as_str())
                .unwrap_or("encode");

            match op {
                "encode" => {
                    let encoded = STANDARD.encode(input.as_bytes());
                    result_ok(&encoded)
                }
                "decode" => match STANDARD.decode(input) {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(s) => result_ok(&s),
                        Err(_) => result_error("decoded bytes are not valid UTF-8"),
                    },
                    Err(e) => result_error(format!("base64 decode error: {e}")),
                },
                _ => result_error(format!("invalid operation: {op}")),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn uuid_single() {
        let result = UuidGen.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(uuid::Uuid::parse_str(text).is_ok());
    }

    #[tokio::test]
    async fn uuid_multiple() {
        let result = UuidGen.call(json!({"count": 3})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let uuids: Vec<String> = serde_json::from_str(text).unwrap();
        assert_eq!(uuids.len(), 3);
    }

    #[tokio::test]
    async fn base64_encode_decode() {
        let result = Base64Tool.call(json!({"input": "hello world"})).await;
        assert_eq!(result["isError"], false);
        let encoded = result["content"][0]["text"].as_str().unwrap();
        assert_eq!(encoded, "aGVsbG8gd29ybGQ=");

        let result = Base64Tool
            .call(json!({"input": encoded, "operation": "decode"}))
            .await;
        assert_eq!(result["isError"], false);
        assert_eq!(
            result["content"][0]["text"].as_str().unwrap(),
            "hello world"
        );
    }
}
