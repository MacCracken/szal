//! Math, conversion, and data tools.

use crate::mcp::{Tool, tool_def, result_ok, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;



/// Evaluate a basic math expression.
pub struct MathEval;

impl Tool for MathEval {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_math_eval",
            "Evaluate a basic math expression (+, -, *, /, %, ^)",
            json!({ "expression": { "type": "string", "description": "Math expression to evaluate (e.g. '2 + 3 * 4')" } }),
            vec!["expression".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let expr = match args.get("expression").and_then(|v| v.as_str()) {
                Some(e) => e,
                None => return result_error("missing required field: expression"),
            };

            let valid = expr.chars().all(|c| c.is_ascii_digit() || " +-*/.%()".contains(c));
            if !valid {
                return result_error("expression contains invalid characters — only digits, +, -, *, /, %, (, ), . allowed");
            }

            match eval_expr(expr) {
                Ok(val) => {
                    // Format: strip trailing zeros for clean output
                    if val.fract() == 0.0 && val.abs() < i64::MAX as f64 {
                        result_ok(&format!("{}", val as i64))
                    } else {
                        result_ok(&format!("{val}"))
                    }
                }
                Err(e) => result_error(e),
            }
        })
    }
}

/// Simple recursive descent math evaluator.
fn eval_expr(input: &str) -> Result<f64, String> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let result = parse_add_sub(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(format!("unexpected token at position {pos}"));
    }
    Ok(result)
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' => { chars.next(); }
            '+' => { tokens.push(Token::Plus); chars.next(); }
            '-' => {
                // Unary minus: after operator, open paren, or at start
                let is_unary = tokens.is_empty()
                    || matches!(tokens.last(), Some(Token::Plus | Token::Minus | Token::Mul | Token::Div | Token::Mod | Token::LParen));
                chars.next();
                if is_unary {
                    let mut num = String::with_capacity(16);
                    num.push('-');
                    while let Some(&d) = chars.peek() {
                        if d.is_ascii_digit() || d == '.' { num.push(d); chars.next(); } else { break; }
                    }
                    tokens.push(Token::Num(num.parse::<f64>().map_err(|e| e.to_string())?));
                } else {
                    tokens.push(Token::Minus);
                }
            }
            '*' => { tokens.push(Token::Mul); chars.next(); }
            '/' => { tokens.push(Token::Div); chars.next(); }
            '%' => { tokens.push(Token::Mod); chars.next(); }
            '(' => { tokens.push(Token::LParen); chars.next(); }
            ')' => { tokens.push(Token::RParen); chars.next(); }
            _ if c.is_ascii_digit() || c == '.' => {
                let mut num = String::with_capacity(16);
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' { num.push(d); chars.next(); } else { break; }
                }
                tokens.push(Token::Num(num.parse::<f64>().map_err(|e| e.to_string())?));
            }
            _ => return Err(format!("unexpected character: {c}")),
        }
    }
    Ok(tokens)
}

fn parse_add_sub(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_mul_div(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Plus => { *pos += 1; left += parse_mul_div(tokens, pos)?; }
            Token::Minus => { *pos += 1; left -= parse_mul_div(tokens, pos)?; }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_mul_div(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_atom(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Mul => { *pos += 1; left *= parse_atom(tokens, pos)?; }
            Token::Div => {
                *pos += 1;
                let right = parse_atom(tokens, pos)?;
                if right == 0.0 { return Err("division by zero".into()); }
                left /= right;
            }
            Token::Mod => {
                *pos += 1;
                let right = parse_atom(tokens, pos)?;
                if right == 0.0 { return Err("modulo by zero".into()); }
                left %= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_atom(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".into());
    }
    match &tokens[*pos] {
        Token::Num(n) => { let v = *n; *pos += 1; Ok(v) }
        Token::LParen => {
            *pos += 1;
            let val = parse_add_sub(tokens, pos)?;
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RParen) {
                return Err("missing closing parenthesis".into());
            }
            *pos += 1;
            Ok(val)
        }
        _ => Err(format!("unexpected token at position {pos}")),
    }
}

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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let value = match args.get("value").and_then(|v| v.as_str()) {
                Some(v) => v,
                None => return result_error("missing required field: value"),
            };
            let from = match args.get("from_base").and_then(|v| v.as_u64()) {
                Some(b) => b as u32,
                None => return result_error("missing required field: from_base"),
            };
            let to = match args.get("to_base").and_then(|v| v.as_u64()) {
                Some(b) => b as u32,
                None => return result_error("missing required field: to_base"),
            };

            if ![2, 8, 10, 16].contains(&from) || ![2, 8, 10, 16].contains(&to) {
                return result_error("supported bases: 2, 8, 10, 16");
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
                    result_ok(&serde_json::to_string_pretty(&json!({
                        "input": value,
                        "from_base": from,
                        "to_base": to,
                        "result": result,
                    })).unwrap_or_default())
                }
                Err(e) => result_error(format!("invalid number '{value}' for base {from}: {e}")),
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let bytes = match args.get("bytes").and_then(|v| v.as_u64()) {
                Some(b) => b,
                None => return result_error("missing required field: bytes"),
            };

            let (value, unit) = if bytes >= 1_099_511_627_776 {
                (bytes as f64 / 1_099_511_627_776.0, "TB")
            } else if bytes >= 1_073_741_824 {
                (bytes as f64 / 1_073_741_824.0, "GB")
            } else if bytes >= 1_048_576 {
                (bytes as f64 / 1_048_576.0, "MB")
            } else if bytes >= 1_024 {
                (bytes as f64 / 1_024.0, "KB")
            } else {
                (bytes as f64, "B")
            };

            result_ok(&serde_json::to_string_pretty(&json!({
                "bytes": bytes,
                "formatted": format!("{value:.2} {unit}"),
                "kb": bytes as f64 / 1_024.0,
                "mb": bytes as f64 / 1_048_576.0,
                "gb": bytes as f64 / 1_073_741_824.0,
            })).unwrap_or_default())
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let secs = match args.get("seconds").and_then(|v| v.as_f64()) {
                Some(s) => s,
                None => return result_error("missing required field: seconds"),
            };

            let total = secs as u64;
            let days = total / 86400;
            let hours = (total % 86400) / 3600;
            let minutes = (total % 3600) / 60;
            let remaining = total % 60;

            let mut parts = Vec::new();
            if days > 0 { parts.push(format!("{days}d")); }
            if hours > 0 { parts.push(format!("{hours}h")); }
            if minutes > 0 { parts.push(format!("{minutes}m")); }
            if remaining > 0 || parts.is_empty() { parts.push(format!("{remaining}s")); }

            result_ok(&serde_json::to_string_pretty(&json!({
                "seconds": secs,
                "formatted": parts.join(" "),
                "days": days,
                "hours": total / 3600,
                "minutes": total / 60,
            })).unwrap_or_default())
        })
    }
}

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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let json_str = match args.get("json").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: json"),
            };
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return result_error("missing required field: path"),
            };

            let value: serde_json::Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(e) => return result_error(format!("invalid JSON: {e}")),
            };

            let mut current = &value;
            for segment in path.split('.') {
                if let Ok(idx) = segment.parse::<usize>() {
                    match current.get(idx) {
                        Some(v) => current = v,
                        None => return result_error(format!("index {idx} not found at '{segment}'")),
                    }
                } else {
                    match current.get(segment) {
                        Some(v) => current = v,
                        None => return result_error(format!("key '{segment}' not found")),
                    }
                }
            }

            result_ok(&serde_json::to_string_pretty(current).unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn math_eval_basic() {
        let result = MathEval.call(json!({"expression": "2 + 3"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("5"));
    }

    #[tokio::test]
    async fn math_eval_complex() {
        let result = MathEval.call(json!({"expression": "10 * 5 + 3"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("53"));
    }

    #[tokio::test]
    async fn math_eval_rejects_injection() {
        let result = MathEval.call(json!({"expression": "1; rm -rf /"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn base_convert_dec_to_hex() {
        let result = BaseConvert.call(json!({"value": "255", "from_base": 10, "to_base": 16})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("0xff"));
    }

    #[tokio::test]
    async fn base_convert_hex_to_bin() {
        let result = BaseConvert.call(json!({"value": "0xFF", "from_base": 16, "to_base": 2})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("0b11111111"));
    }

    #[tokio::test]
    async fn base_convert_bin_to_dec() {
        let result = BaseConvert.call(json!({"value": "1010", "from_base": 2, "to_base": 10})).await;
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

    #[tokio::test]
    async fn json_path_nested() {
        let result = JsonPath.call(json!({
            "json": r#"{"data": {"items": [{"name": "first"}, {"name": "second"}]}}"#,
            "path": "data.items.1.name"
        })).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap().trim_matches('"'), "second");
    }

    #[tokio::test]
    async fn json_path_missing() {
        let result = JsonPath.call(json!({"json": "{\"a\": 1}", "path": "b"})).await;
        assert_eq!(result["isError"], true);
    }
}
