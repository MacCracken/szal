//! Network and HTTP tools.

use crate::mcp::{Tool, tool_def};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

fn result_ok(text: &str) -> serde_json::Value {
    json!({"content": [{"type": "text", "text": text}], "isError": false})
}

fn result_error(msg: impl Into<String>) -> serde_json::Value {
    json!({"content": [{"type": "text", "text": msg.into()}], "isError": true})
}

/// HTTP request via curl.
pub struct HttpRequest;

impl Tool for HttpRequest {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_http",
            "Make an HTTP request via curl and return status, headers, and body",
            json!({
                "url": { "type": "string", "description": "URL to request" },
                "method": { "type": "string", "enum": ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"], "description": "HTTP method (default: GET)" },
                "headers": { "type": "object", "description": "Request headers as key-value pairs" },
                "body": { "type": "string", "description": "Request body (for POST/PUT/PATCH)" },
                "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default: 30)" }
            }),
            vec!["url".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let url = match args.get("url").and_then(|v| v.as_str()) {
                Some(u) => u,
                None => return result_error("missing required field: url"),
            };
            let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
            let timeout = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);

            let mut cmd = tokio::process::Command::new("curl");
            cmd.args(["-s", "-S", "-w", "\n%{http_code}", "-X", method]);
            cmd.args(["--max-time", &timeout.to_string()]);
            cmd.arg("-D").arg("-"); // dump headers to stdout

            if let Some(headers) = args.get("headers").and_then(|v| v.as_object()) {
                for (k, v) in headers {
                    if let Some(val) = v.as_str() {
                        cmd.args(["-H", &format!("{k}: {val}")]);
                    }
                }
            }

            if let Some(body) = args.get("body").and_then(|v| v.as_str()) {
                cmd.args(["-d", body]);
            }

            cmd.arg(url);

            match cmd.output().await {
                Ok(output) => {
                    let raw = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                    if !output.status.success() && !stderr.is_empty() {
                        return result_error(format!("curl failed: {stderr}"));
                    }

                    // Last line is status code from -w
                    let lines: Vec<&str> = raw.lines().collect();
                    let status_code = lines.last().and_then(|l| l.parse::<u16>().ok()).unwrap_or(0);

                    // Split headers from body (headers end at empty line)
                    let mut header_section = true;
                    let mut headers = Vec::new();
                    let mut body_lines = Vec::new();

                    for (i, line) in lines.iter().enumerate() {
                        if i == lines.len() - 1 {
                            continue; // skip status code line
                        }
                        if header_section {
                            if line.is_empty() {
                                header_section = false;
                            } else {
                                headers.push(*line);
                            }
                        } else {
                            body_lines.push(*line);
                        }
                    }

                    result_ok(&serde_json::to_string_pretty(&json!({
                        "status_code": status_code,
                        "headers": headers,
                        "body": body_lines.join("\n"),
                    })).unwrap_or_default())
                }
                Err(e) => result_error(format!("failed to execute curl: {e}")),
            }
        })
    }
}

/// DNS lookup.
pub struct DnsLookup;

impl Tool for DnsLookup {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_dns_lookup",
            "Perform a DNS lookup for a hostname",
            json!({ "hostname": { "type": "string", "description": "Hostname to resolve" } }),
            vec!["hostname".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let hostname = match args.get("hostname").and_then(|v| v.as_str()) {
                Some(h) => h,
                None => return result_error("missing required field: hostname"),
            };

            let addr = format!("{hostname}:0");
            match tokio::net::lookup_host(&addr).await {
                Ok(addrs) => {
                    let ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
                    result_ok(&serde_json::to_string_pretty(&json!({
                        "hostname": hostname,
                        "addresses": ips,
                    })).unwrap_or_default())
                }
                Err(e) => result_error(format!("DNS lookup failed for {hostname}: {e}")),
            }
        })
    }
}

/// Check if a TCP port is open.
pub struct PortCheck;

impl Tool for PortCheck {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_port_check",
            "Check if a TCP port is open on a host",
            json!({
                "host": { "type": "string", "description": "Host to check (default: 127.0.0.1)" },
                "port": { "type": "integer", "description": "TCP port number" },
                "timeout_ms": { "type": "integer", "description": "Connection timeout in ms (default: 3000)" }
            }),
            vec!["port".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let host = args.get("host").and_then(|v| v.as_str()).unwrap_or("127.0.0.1");
            let port = match args.get("port").and_then(|v| v.as_u64()) {
                Some(p) => p as u16,
                None => return result_error("missing required field: port"),
            };
            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(3000);

            let addr = format!("{host}:{port}");
            let open = tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                tokio::net::TcpStream::connect(&addr),
            )
            .await
            .is_ok_and(|r| r.is_ok());

            result_ok(&serde_json::to_string_pretty(&json!({
                "host": host,
                "port": port,
                "open": open,
            })).unwrap_or_default())
        })
    }
}

/// URL encode/decode.
pub struct UrlEncode;

impl Tool for UrlEncode {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_url_encode",
            "URL encode or decode a string",
            json!({
                "input": { "type": "string", "description": "String to encode/decode" },
                "operation": { "type": "string", "enum": ["encode", "decode"], "description": "Operation (default: encode)" }
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
                    let encoded: String = input.chars().map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                            c.to_string()
                        } else {
                            format!("%{:02X}", c as u32)
                        }
                    }).collect();
                    result_ok(&encoded)
                }
                "decode" => {
                    let mut result = String::new();
                    let mut chars = input.chars();
                    while let Some(c) = chars.next() {
                        if c == '%' {
                            let hex: String = chars.by_ref().take(2).collect();
                            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                result.push(byte as char);
                            } else {
                                result.push('%');
                                result.push_str(&hex);
                            }
                        } else if c == '+' {
                            result.push(' ');
                        } else {
                            result.push(c);
                        }
                    }
                    result_ok(&result)
                }
                _ => result_error(format!("invalid operation: {op}")),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dns_lookup_localhost() {
        let result = DnsLookup.call(json!({"hostname": "localhost"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("127.0.0.1") || text.contains("::1"));
    }

    #[tokio::test]
    async fn dns_lookup_invalid() {
        let result = DnsLookup.call(json!({"hostname": "this.host.does.not.exist.invalid"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn port_check_closed() {
        // Port 1 is almost certainly closed
        let result = PortCheck.call(json!({"port": 1, "timeout_ms": 500})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"open\": false"));
    }

    #[tokio::test]
    async fn url_encode() {
        let result = UrlEncode.call(json!({"input": "hello world & foo=bar"})).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "hello%20world%20%26%20foo%3Dbar");
    }

    #[tokio::test]
    async fn url_decode() {
        let result = UrlEncode.call(json!({"input": "hello%20world", "operation": "decode"})).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "hello world");
    }

    #[tokio::test]
    async fn url_encode_roundtrip() {
        let original = "spaces & symbols/here?yes=true";
        let encoded = UrlEncode.call(json!({"input": original})).await;
        let enc_text = encoded["content"][0]["text"].as_str().unwrap();
        let decoded = UrlEncode.call(json!({"input": enc_text, "operation": "decode"})).await;
        assert_eq!(decoded["content"][0]["text"].as_str().unwrap(), original);
    }
}
