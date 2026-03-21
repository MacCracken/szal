//! Process and command execution tools.

use crate::mcp::{Tool, tool_def, result_ok, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;



/// Execute a shell command and return its output.
pub struct Exec;

impl Tool for Exec {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_exec",
            "Execute a shell command and return stdout, stderr, and exit code",
            json!({
                "command": { "type": "string", "description": "Command to execute" },
                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments" },
                "cwd": { "type": "string", "description": "Working directory (optional)" },
                "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds (default: 30000)" }
            }),
            vec!["command".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let command = match args.get("command").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return result_error("missing required field: command"),
            };

            // Reject commands containing path traversal or shell metacharacters
            if command.contains("..") || command.contains(';') || command.contains('|') || command.contains('&') || command.contains('`') || command.contains('$') {
                return result_error("command contains disallowed characters (.. ; | & ` $)");
            }

            let cmd_args: Vec<String> = args
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(30_000);

            let mut cmd = tokio::process::Command::new(command);
            cmd.args(&cmd_args);

            if let Some(cwd) = args.get("cwd").and_then(|v| v.as_str()) {
                cmd.current_dir(cwd);
            }

            let result = tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                cmd.output(),
            )
            .await;

            match result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let code = output.status.code().unwrap_or(-1);

                    result_ok(&serde_json::to_string_pretty(&json!({
                        "exit_code": code,
                        "stdout": stdout,
                        "stderr": stderr,
                        "success": output.status.success(),
                    })).unwrap_or_default())
                }
                Ok(Err(e)) => result_error(format!("command failed: {e}")),
                Err(_) => result_error(format!("command timed out after {timeout_ms}ms")),
            }
        })
    }
}

/// Get the current process ID.
pub struct Pid;

impl Tool for Pid {
    fn definition(&self) -> BoteToolDef {
        tool_def("szal_pid", "Get the current process ID", json!({}), vec![])
    }

    fn call(&self, _args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async {
            result_ok(&std::process::id().to_string())
        })
    }
}

/// Run a command and check if it succeeds (exit code 0).
pub struct Which;

impl Tool for Which {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_which",
            "Check if a command exists on PATH",
            json!({ "command": { "type": "string", "description": "Command name to look up" } }),
            vec!["command".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let command = match args.get("command").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return result_error("missing required field: command"),
            };

            let output = tokio::process::Command::new("which")
                .arg(command)
                .output()
                .await;

            match output {
                Ok(out) if out.status.success() => {
                    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    result_ok(&serde_json::to_string_pretty(&json!({
                        "command": command,
                        "found": true,
                        "path": path,
                    })).unwrap_or_default())
                }
                _ => result_ok(&serde_json::to_string_pretty(&json!({
                    "command": command,
                    "found": false,
                })).unwrap_or_default()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exec_echo() {
        let result = Exec.call(json!({"command": "echo", "args": ["hello"]})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"success\": true"));
        assert!(text.contains("hello"));
    }

    #[tokio::test]
    async fn exec_failing_command() {
        let result = Exec.call(json!({"command": "false"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"success\": false"));
    }

    #[tokio::test]
    async fn exec_nonexistent() {
        let result = Exec.call(json!({"command": "nonexistent_command_xyz_123"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn pid() {
        let result = Pid.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let pid: u32 = text.parse().unwrap();
        assert!(pid > 0);
    }

    #[tokio::test]
    async fn which_exists() {
        let result = Which.call(json!({"command": "ls"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"found\": true"));
    }

    #[tokio::test]
    async fn which_not_found() {
        let result = Which.call(json!({"command": "nonexistent_xyz_123"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"found\": false"));
    }
}
