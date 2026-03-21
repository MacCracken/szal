//! System information and process tools.

use crate::mcp::{Tool, tool_def, result_ok, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;



/// Get system information (hostname, OS, arch, CPUs, memory).
pub struct SystemInfo;

impl Tool for SystemInfo {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_system_info", "Get system hostname, OS, architecture, CPU count, and uptime", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            let hostname = std::fs::read_to_string("/etc/hostname")
                .unwrap_or_else(|_| "unknown".into())
                .trim()
                .to_string();
            let os = std::env::consts::OS;
            let arch = std::env::consts::ARCH;

            let cpus = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0);

            // Read uptime from /proc/uptime
            let uptime_secs = std::fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|s| s.split_whitespace().next().map(String::from))
                .and_then(|s| s.parse::<f64>().ok());

            result_ok(&serde_json::to_string_pretty(&json!({
                "hostname": hostname,
                "os": os,
                "arch": arch,
                "cpus": cpus,
                "uptime_secs": uptime_secs,
            })).unwrap_or_default())
        })
    }
}

/// Get current working directory.
pub struct Cwd;

impl Tool for Cwd {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_cwd", "Get the current working directory", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            match std::env::current_dir() {
                Ok(p) => result_ok(&p.display().to_string()),
                Err(e) => result_error(e.to_string()),
            }
        })
    }
}

/// Get an environment variable.
pub struct EnvGet;

impl Tool for EnvGet {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_env_get",
            "Get the value of an environment variable",
            json!({ "name": { "type": "string", "description": "Environment variable name" } }),
            vec!["name".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return result_error("missing required field: name"),
            };
            match std::env::var(name) {
                Ok(val) => result_ok(&val),
                Err(_) => result_error(format!("environment variable not set: {name}")),
            }
        })
    }
}

/// Get current timestamp in multiple formats.
pub struct Timestamp;

impl Tool for Timestamp {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_timestamp", "Get the current timestamp in ISO 8601 and Unix epoch formats", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            let now = chrono::Utc::now();
            result_ok(&serde_json::to_string_pretty(&json!({
                "iso8601": now.to_rfc3339(),
                "unix_secs": now.timestamp(),
                "unix_ms": now.timestamp_millis(),
            })).unwrap_or_default())
        })
    }
}

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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(1).min(100) as usize;
            let uuids: Vec<String> = (0..count).map(|_| uuid::Uuid::new_v4().to_string()).collect();
            if count == 1 {
                result_ok(&uuids[0])
            } else {
                result_ok(&serde_json::to_string_pretty(&uuids).unwrap_or_default())
            }
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let a_str = match args.get("a").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: a"),
            };
            let b_str = match args.get("b").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: b"),
            };

            let a: serde_json::Value = match serde_json::from_str(a_str) {
                Ok(v) => v,
                Err(e) => return result_error(format!("invalid JSON in 'a': {e}")),
            };
            let b: serde_json::Value = match serde_json::from_str(b_str) {
                Ok(v) => v,
                Err(e) => return result_error(format!("invalid JSON in 'b': {e}")),
            };

            let equal = a == b;
            result_ok(&serde_json::to_string_pretty(&json!({
                "equal": equal,
                "a_type": type_name(&a),
                "b_type": type_name(&b),
            })).unwrap_or_default())
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: json"),
            };
            match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(val) => {
                    let info = json!({
                        "valid": true,
                        "type": type_name(&val),
                        "size_bytes": json_str.len(),
                    });
                    result_ok(&serde_json::to_string_pretty(&info).unwrap_or_default())
                }
                Err(e) => result_ok(&serde_json::to_string_pretty(&json!({
                    "valid": false,
                    "error": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                })).unwrap_or_default()),
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let input = match args.get("input").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: input"),
            };
            let op = args.get("operation").and_then(|v| v.as_str()).unwrap_or("encode");

            match op {
                "encode" => {
                    let encoded = base64_encode(input.as_bytes());
                    result_ok(&encoded)
                }
                "decode" => {
                    match base64_decode(input) {
                        Ok(bytes) => match String::from_utf8(bytes) {
                            Ok(s) => result_ok(&s),
                            Err(_) => result_error("decoded bytes are not valid UTF-8"),
                        },
                        Err(e) => result_error(format!("base64 decode error: {e}")),
                    }
                }
                _ => result_error(format!("invalid operation: {op}")),
            }
        })
    }
}

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut result = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(B64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim_end_matches('=');
    let mut result = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for c in input.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return Err(format!("invalid base64 character: {c}")),
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn system_info() {
        let result = SystemInfo.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"os\":"));
        assert!(text.contains("\"arch\":"));
    }

    #[tokio::test]
    async fn cwd() {
        let result = Cwd.call(json!({})).await;
        assert_eq!(result["isError"], false);
    }

    #[tokio::test]
    async fn env_get_exists() {
        let result = EnvGet.call(json!({"name": "PATH"})).await;
        assert_eq!(result["isError"], false);
    }

    #[tokio::test]
    async fn env_get_missing() {
        let result = EnvGet.call(json!({"name": "SZAL_NONEXISTENT_VAR_12345"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn timestamp() {
        let result = Timestamp.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("iso8601"));
    }

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
    async fn json_diff_equal() {
        let result = JsonDiff.call(json!({"a": "{\"x\":1}", "b": "{\"x\":1}"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"equal\": true"));
    }

    #[tokio::test]
    async fn json_diff_not_equal() {
        let result = JsonDiff.call(json!({"a": "{\"x\":1}", "b": "{\"x\":2}"})).await;
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

    #[tokio::test]
    async fn base64_encode_decode() {
        let result = Base64Tool.call(json!({"input": "hello world"})).await;
        assert_eq!(result["isError"], false);
        let encoded = result["content"][0]["text"].as_str().unwrap();
        assert_eq!(encoded, "aGVsbG8gd29ybGQ=");

        let result = Base64Tool.call(json!({"input": encoded, "operation": "decode"})).await;
        assert_eq!(result["isError"], false);
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "hello world");
    }
}
