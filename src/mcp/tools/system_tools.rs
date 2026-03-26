//! System information and process tools.

use crate::mcp::{McpErrorCode, Tool, result_error_typed, result_ok, result_ok_json, tool_def};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

/// Get system information (hostname, OS, arch, CPUs, memory).
pub struct SystemInfo;

impl Tool for SystemInfo {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_system_info",
            "Get system hostname, OS, architecture, CPU count, and uptime",
            json!({}),
            vec![],
        )
    }

    fn call(
        &self,
        _args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            let hostname = tokio::fs::read_to_string("/etc/hostname")
                .await
                .unwrap_or_else(|_| "unknown".into())
                .trim()
                .to_string();
            let os = std::env::consts::OS;
            let arch = std::env::consts::ARCH;

            let cpus = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0);

            // Read uptime from /proc/uptime
            let uptime_secs = tokio::fs::read_to_string("/proc/uptime")
                .await
                .ok()
                .and_then(|s| s.split_whitespace().next().map(String::from))
                .and_then(|s| s.parse::<f64>().ok());

            result_ok_json(&json!({
                "hostname": hostname,
                "os": os,
                "arch": arch,
                "cpus": cpus,
                "uptime_secs": uptime_secs,
            }))
        })
    }
}

/// Get current working directory.
pub struct Cwd;

impl Tool for Cwd {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_cwd",
            "Get the current working directory",
            json!({}),
            vec![],
        )
    }

    fn call(
        &self,
        _args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            match std::env::current_dir() {
                Ok(p) => result_ok(&p.display().to_string()),
                Err(e) => result_error_typed(McpErrorCode::IoError, e.to_string()),
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

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: name",
                    );
                }
            };
            match std::env::var(name) {
                Ok(val) => result_ok(&val),
                Err(_) => result_error_typed(
                    McpErrorCode::NotFound,
                    format!("environment variable not set: {name}"),
                ),
            }
        })
    }
}

/// Get current timestamp in multiple formats.
pub struct Timestamp;

impl Tool for Timestamp {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_timestamp",
            "Get the current timestamp in ISO 8601 and Unix epoch formats",
            json!({}),
            vec![],
        )
    }

    fn call(
        &self,
        _args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            let now = chrono::Utc::now();
            result_ok_json(&json!({
                "iso8601": now.to_rfc3339(),
                "unix_secs": now.timestamp(),
                "unix_ms": now.timestamp_millis(),
            }))
        })
    }
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
        let result = EnvGet
            .call(json!({"name": "SZAL_NONEXISTENT_VAR_12345"}))
            .await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn timestamp() {
        let result = Timestamp.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("iso8601"));
    }
}
