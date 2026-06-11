//! Conversion tools: base convert, byte format, duration format.

use crate::mcp::{McpErrorCode, Tool, result_error_typed, result_ok_json, tool_def};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

const BYTES_PER_KB: f64 = 1_024.0;
const BYTES_PER_MB: f64 = 1_048_576.0;
const BYTES_PER_GB: f64 = 1_073_741_824.0;
const BYTES_PER_TB: f64 = 1_099_511_627_776.0;
const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 3_600;
const SECS_PER_DAY: u64 = 86_400;

/// Convert between number bases.
pub struct BaseConvert;

impl Tool for BaseConvert {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_base_convert",
            "Convert a number between bases (binary, octal, decimal, hexadecimal)",
            json!({
                "value": { "type": "string", "description": "Number to convert" },
                "from_base": { "type": "integer", "description": "Source base (2, 8, 10, 16)" },
                "to_base": { "type": "integer", "description": "Target base (2, 8, 10, 16)" }
            }),
            vec!["value".into(), "from_base".into(), "to_base".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let value = match args.get("value").and_then(|v| v.as_str()) {
                Some(v) => v,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: value",
                    );
                }
            };
            let from = match args.get("from_base").and_then(|v| v.as_u64()) {
                Some(b) => b as u32,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: from_base",
                    );
                }
            };
            let to = match args.get("to_base").and_then(|v| v.as_u64()) {
                Some(b) => b as u32,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: to_base",
                    );
                }
            };

            if ![2, 8, 10, 16].contains(&from) || ![2, 8, 10, 16].contains(&to) {
                return result_error_typed(
                    McpErrorCode::Validation,
                    "supported bases: 2, 8, 10, 16",
                );
            }

            // Strip common prefixes
            let clean = value
                .trim_start_matches("0x")
                .trim_start_matches("0X")
                .trim_start_matches("0b")
                .trim_start_matches("0B")
                .trim_start_matches("0o")
                .trim_start_matches("0O");

            match u128::from_str_radix(clean, from) {
                Ok(n) => {
                    let result = match to {
                        2 => format!("0b{n:b}"),
                        8 => format!("0o{n:o}"),
                        10 => n.to_string(),
                        16 => format!("0x{n:x}"),
                        _ => unreachable!(),
                    };
                    result_ok_json(&json!({
                        "input": value,
                        "from_base": from,
                        "to_base": to,
                        "result": result,
                    }))
                }
                Err(e) => result_error_typed(
                    McpErrorCode::Validation,
                    format!("invalid number '{value}' for base {from}: {e}"),
                ),
            }
        })
    }
}

/// Convert bytes to human-readable sizes.
pub struct ByteFormat;

impl Tool for ByteFormat {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_byte_format",
            "Convert bytes to human-readable format (KB, MB, GB, TB)",
            json!({ "bytes": { "type": "integer", "description": "Number of bytes" } }),
            vec!["bytes".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let bytes = match args.get("bytes").and_then(|v| v.as_u64()) {
                Some(b) => b,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: bytes",
                    );
                }
            };

            let (value, unit) = if bytes >= BYTES_PER_TB as u64 {
                (bytes as f64 / BYTES_PER_TB, "TB")
            } else if bytes >= BYTES_PER_GB as u64 {
                (bytes as f64 / BYTES_PER_GB, "GB")
            } else if bytes >= BYTES_PER_MB as u64 {
                (bytes as f64 / BYTES_PER_MB, "MB")
            } else if bytes >= BYTES_PER_KB as u64 {
                (bytes as f64 / BYTES_PER_KB, "KB")
            } else {
                (bytes as f64, "B")
            };

            result_ok_json(&json!({
                "bytes": bytes,
                "formatted": format!("{value:.2} {unit}"),
                "kb": bytes as f64 / BYTES_PER_KB,
                "mb": bytes as f64 / BYTES_PER_MB,
                "gb": bytes as f64 / BYTES_PER_GB,
            }))
        })
    }
}

/// Duration format — convert between seconds, minutes, hours, days.
pub struct DurationFormat;

impl Tool for DurationFormat {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_duration_format",
            "Convert seconds to human-readable duration (Xd Xh Xm Xs) and reverse",
            json!({
                "seconds": { "type": "number", "description": "Duration in seconds" }
            }),
            vec!["seconds".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let secs = match args.get("seconds").and_then(|v| v.as_f64()) {
                Some(s) => s,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: seconds",
                    );
                }
            };

            let total = secs as u64;
            let days = total / SECS_PER_DAY;
            let hours = (total % SECS_PER_DAY) / SECS_PER_HOUR;
            let minutes = (total % SECS_PER_HOUR) / SECS_PER_MINUTE;
            let remaining = total % SECS_PER_MINUTE;

            let mut parts = Vec::new();
            if days > 0 {
                parts.push(format!("{days}d"));
            }
            if hours > 0 {
                parts.push(format!("{hours}h"));
            }
            if minutes > 0 {
                parts.push(format!("{minutes}m"));
            }
            if remaining > 0 || parts.is_empty() {
                parts.push(format!("{remaining}s"));
            }

            result_ok_json(&json!({
                "seconds": secs,
                "formatted": parts.join(" "),
                "days": days,
                "hours": total / SECS_PER_HOUR,
                "minutes": total / SECS_PER_MINUTE,
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn base_convert_dec_to_hex() {
        let result = BaseConvert
            .call(json!({"value": "255", "from_base": 10, "to_base": 16}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("0xff"));
    }

    #[tokio::test]
    async fn base_convert_hex_to_bin() {
        let result = BaseConvert
            .call(json!({"value": "0xFF", "from_base": 16, "to_base": 2}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("0b11111111"));
    }

    #[tokio::test]
    async fn base_convert_bin_to_dec() {
        let result = BaseConvert
            .call(json!({"value": "1010", "from_base": 2, "to_base": 10}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"result\": \"10\""));
    }

    #[tokio::test]
    async fn byte_format() {
        let result = ByteFormat.call(json!({"bytes": 1_500_000})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("1.43 MB"));
    }

    #[tokio::test]
    async fn duration_format() {
        let result = DurationFormat.call(json!({"seconds": 90061})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("1d 1h 1m 1s"));
    }
}
