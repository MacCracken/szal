//! Hashing and checksum tools.

use crate::mcp::{Tool, tool_def, result_ok, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;



/// Compute SHA-256 hash of a string or file.
pub struct Sha256;

impl Tool for Sha256 {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_sha256",
            "Compute SHA-256 hash of a string or file contents",
            json!({
                "input": { "type": "string", "description": "String to hash (mutually exclusive with file)" },
                "file": { "type": "string", "description": "File path to hash (mutually exclusive with input)" }
            }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let data = if let Some(input) = args.get("input").and_then(|v| v.as_str()) {
                input.as_bytes().to_vec()
            } else if let Some(path) = args.get("file").and_then(|v| v.as_str()) {
                match std::fs::read(path) {
                    Ok(d) => d,
                    Err(e) => return result_error(format!("failed to read {path}: {e}")),
                }
            } else {
                return result_error("provide either 'input' or 'file'");
            };

            // Use system sha256sum for real hashing
            let output = tokio::process::Command::new("sha256sum")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn();

            match output {
                Ok(mut child) => {
                    use tokio::io::AsyncWriteExt;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&data).await;
                        drop(stdin);
                    }
                    match child.wait_with_output().await {
                        Ok(out) => {
                            let hash = String::from_utf8_lossy(&out.stdout)
                                .split_whitespace()
                                .next()
                                .unwrap_or("")
                                .to_string();
                            result_ok(&json!({
                                "algorithm": "sha256",
                                "hash": hash,
                                "input_bytes": data.len(),
                            }).to_string())
                        }
                        Err(e) => result_error(format!("sha256sum failed: {e}")),
                    }
                }
                Err(e) => result_error(format!("sha256sum not available: {e}")),
            }
        })
    }
}

/// Compute MD5 hash (for checksums, not security).
pub struct Md5;

impl Tool for Md5 {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_md5",
            "Compute MD5 hash of a string (for checksums, not security)",
            json!({
                "input": { "type": "string", "description": "String to hash" }
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

            let output = tokio::process::Command::new("md5sum")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn();

            match output {
                Ok(mut child) => {
                    use tokio::io::AsyncWriteExt;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(input.as_bytes()).await;
                        drop(stdin);
                    }
                    match child.wait_with_output().await {
                        Ok(out) => {
                            let hash = String::from_utf8_lossy(&out.stdout)
                                .split_whitespace()
                                .next()
                                .unwrap_or("")
                                .to_string();
                            result_ok(&hash)
                        }
                        Err(e) => result_error(format!("md5sum failed: {e}")),
                    }
                }
                Err(e) => result_error(format!("md5sum not available: {e}")),
            }
        })
    }
}

/// Generate a random hex token.
pub struct RandomToken;

impl Tool for RandomToken {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_random_token",
            "Generate a cryptographically random hex token",
            json!({ "bytes": { "type": "integer", "description": "Number of random bytes (default: 32, output is 2x hex chars)" } }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let bytes = args.get("bytes").and_then(|v| v.as_u64()).unwrap_or(32).min(256) as usize;

            // Use UUIDs as entropy source (no external deps needed)
            let mut hex = String::with_capacity(bytes * 2);
            while hex.len() < bytes * 2 {
                hex.push_str(&uuid::Uuid::new_v4().simple().to_string());
            }
            hex.truncate(bytes * 2);
            result_ok(&hex)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sha256_string() {
        let result = Sha256.call(json!({"input": "hello"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        // SHA-256 of "hello" is well-known
        assert!(text.contains("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"));
    }

    #[tokio::test]
    async fn sha256_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "test content").unwrap();
        let result = Sha256.call(json!({"file": tmp.path().display().to_string()})).await;
        assert_eq!(result["isError"], false);
    }

    #[tokio::test]
    async fn sha256_no_input() {
        let result = Sha256.call(json!({})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn md5_string() {
        let result = Md5.call(json!({"input": "hello"})).await;
        assert_eq!(result["isError"], false);
    }

    #[tokio::test]
    async fn random_token_default() {
        let result = RandomToken.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert_eq!(text.len(), 64); // 32 bytes = 64 hex chars
    }

    #[tokio::test]
    async fn random_token_custom() {
        let result = RandomToken.call(json!({"bytes": 16})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert_eq!(text.len(), 32); // 16 bytes = 32 hex chars
    }
}
