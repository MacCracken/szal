//! Hashing and checksum tools.

use crate::mcp::{Tool, result_error, result_ok, tool_def};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use sha2::Digest;
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

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let data = if let Some(input) = args.get("input").and_then(|v| v.as_str()) {
                input.as_bytes().to_vec()
            } else if let Some(path) = args.get("file").and_then(|v| v.as_str()) {
                let validated = match crate::mcp::validate_path(path) {
                    Ok(p) => p,
                    Err(e) => return result_error(e),
                };
                match std::fs::read(&validated) {
                    Ok(d) => d,
                    Err(e) => {
                        return result_error(format!(
                            "failed to read {}: {e}",
                            validated.display()
                        ));
                    }
                }
            } else {
                return result_error("provide either 'input' or 'file'");
            };

            let hash = sha2::Sha256::digest(&data);
            let hex = format!("{hash:x}");
            result_ok(
                &serde_json::to_string_pretty(&json!({
                    "algorithm": "sha256",
                    "hash": hex,
                    "input_bytes": data.len(),
                }))
                .unwrap_or_default(),
            )
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

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let input = match args.get("input").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: input"),
            };

            let hash = md5::Md5::digest(input.as_bytes());
            let hex = format!("{hash:x}");
            result_ok(&hex)
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

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let bytes = args
                .get("bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(32)
                .min(256) as usize;

            let mut buf = vec![0u8; bytes];
            use std::io::Read;
            let mut f = match std::fs::File::open("/dev/urandom") {
                Ok(f) => f,
                Err(e) => return result_error(format!("failed to open /dev/urandom: {e}")),
            };
            if let Err(e) = f.read_exact(&mut buf) {
                return result_error(format!("failed to read random bytes: {e}"));
            }
            use std::fmt::Write;
            let mut hex = String::with_capacity(bytes * 2);
            for b in &buf {
                write!(hex, "{b:02x}").unwrap();
            }
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
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::TempDir::new_in(cwd).unwrap();
        let path = tmp.path().join("test.bin");
        std::fs::write(&path, "test content").unwrap();
        let result = Sha256
            .call(json!({"file": path.display().to_string()}))
            .await;
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
