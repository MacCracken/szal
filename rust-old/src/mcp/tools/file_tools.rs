//! File system tools — read, write, list, stat, search.

use crate::mcp::{
    McpErrorCode, Tool, result_error_typed, result_ok, result_ok_json, tool_def, validate_path,
};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::path::Path;
use std::pin::Pin;
use tokio::io::AsyncWriteExt;

/// Maximum bytes to read from a file by default (1 MB).
const DEFAULT_MAX_READ_BYTES: usize = 1_048_576;
/// Maximum entries returned by directory listing.
const MAX_DIR_ENTRIES: u64 = 10_000;
/// Default entries returned by directory listing.
const DEFAULT_DIR_ENTRIES: u64 = 500;
/// Maximum recursion depth for directory traversal.
const MAX_DIR_DEPTH: usize = 20;

/// Read a file's contents.
pub struct FileRead;

impl Tool for FileRead {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_file_read",
            "Read a file's contents as text",
            json!({
                "path": { "type": "string", "description": "File path to read" },
                "max_bytes": { "type": "integer", "description": "Max bytes to read (default: 1MB)" }
            }),
            vec!["path".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: path",
                    );
                }
            };
            let path = match validate_path(path).await {
                Ok(p) => p.display().to_string(),
                Err(e) => return result_error_typed(McpErrorCode::PermissionDenied, e),
            };
            let path = path.as_str();
            let max_bytes = args
                .get("max_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(DEFAULT_MAX_READ_BYTES as u64) as usize;

            match tokio::fs::read_to_string(path).await {
                Ok(content) => {
                    if content.len() > max_bytes {
                        let end = content[..max_bytes]
                            .char_indices()
                            .last()
                            .map(|(i, c)| i + c.len_utf8())
                            .unwrap_or(0);
                        result_ok(&content[..end])
                    } else {
                        result_ok(&content)
                    }
                }
                Err(e) => {
                    result_error_typed(McpErrorCode::IoError, format!("failed to read {path}: {e}"))
                }
            }
        })
    }
}

/// Write content to a file.
pub struct FileWrite;

impl Tool for FileWrite {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_file_write",
            "Write text content to a file (creates or overwrites)",
            json!({
                "path": { "type": "string", "description": "File path to write" },
                "content": { "type": "string", "description": "Content to write" },
                "append": { "type": "boolean", "description": "Append instead of overwrite (default: false)" }
            }),
            vec!["path".into(), "content".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: path",
                    );
                }
            };
            let path = match validate_path(path).await {
                Ok(p) => p.display().to_string(),
                Err(e) => return result_error_typed(McpErrorCode::PermissionDenied, e),
            };
            let path = path.as_str();
            let content = match args.get("content").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: content",
                    );
                }
            };
            let append = args
                .get("append")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let result = if append {
                let file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await;
                match file {
                    Ok(mut f) => f.write_all(content.as_bytes()).await,
                    Err(e) => Err(e),
                }
            } else {
                tokio::fs::write(path, content).await
            };

            match result {
                Ok(()) => result_ok(&format!("wrote {} bytes to {path}", content.len())),
                Err(e) => result_error_typed(
                    McpErrorCode::IoError,
                    format!("failed to write {path}: {e}"),
                ),
            }
        })
    }
}

/// List directory contents.
pub struct DirList;

impl Tool for DirList {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_dir_list",
            "List files and directories in a path",
            json!({
                "path": { "type": "string", "description": "Directory path (default: current dir)" },
                "recursive": { "type": "boolean", "description": "Recurse into subdirectories (default: false)" },
                "max_entries": { "type": "integer", "description": "Max entries to return (default: 500)" }
            }),
            vec![],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let path = match validate_path(path).await {
                Ok(p) => p.display().to_string(),
                Err(e) => return result_error_typed(McpErrorCode::PermissionDenied, e),
            };
            let path = path.as_str();
            let recursive = args
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let max = args
                .get("max_entries")
                .and_then(|v| v.as_u64())
                .unwrap_or(DEFAULT_DIR_ENTRIES)
                .min(MAX_DIR_ENTRIES) as usize;

            let mut entries = Vec::new();
            if let Err(e) = collect_dir(Path::new(path), recursive, max, &mut entries, 0).await {
                return result_error_typed(
                    McpErrorCode::IoError,
                    format!("failed to list {path}: {e}"),
                );
            }

            result_ok_json(&json!(entries))
        })
    }
}

fn collect_dir<'a>(
    path: &'a Path,
    recursive: bool,
    max: usize,
    entries: &'a mut Vec<serde_json::Value>,
    depth: usize,
) -> Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if depth > MAX_DIR_DEPTH {
            return Ok(());
        }
        let mut read_dir = tokio::fs::read_dir(path).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            if entries.len() >= max {
                break;
            }
            let ft = entry.file_type().await?;
            let name = entry.file_name().to_string_lossy().to_string();
            let kind = if ft.is_dir() {
                "directory"
            } else if ft.is_symlink() {
                "symlink"
            } else {
                "file"
            };

            let mut info = json!({
                "name": name,
                "path": entry.path().display().to_string(),
                "type": kind,
            });

            if ft.is_file()
                && let Ok(meta) = entry.metadata().await
            {
                info["size"] = json!(meta.len());
            }

            entries.push(info);

            if recursive && ft.is_dir() && entries.len() < max {
                let entry_path = entry.path();
                if let Err(e) = collect_dir(&entry_path, true, max, entries, depth + 1).await {
                    tracing::debug!(path = %entry_path.display(), error = %e, "skipping unreadable subdirectory");
                }
            }
        }
        Ok(())
    })
}

/// Get file or directory metadata.
pub struct FileStat;

impl Tool for FileStat {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_file_stat",
            "Get metadata for a file or directory (size, permissions, timestamps)",
            json!({ "path": { "type": "string", "description": "File or directory path" } }),
            vec!["path".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: path",
                    );
                }
            };
            let path = match validate_path(path).await {
                Ok(p) => p.display().to_string(),
                Err(e) => return result_error_typed(McpErrorCode::PermissionDenied, e),
            };
            let path = path.as_str();

            match tokio::fs::symlink_metadata(path).await {
                Ok(meta) => {
                    let kind = if meta.is_dir() {
                        "directory"
                    } else if meta.is_symlink() {
                        "symlink"
                    } else {
                        "file"
                    };

                    let modified = meta.modified().ok().map(|t| {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        dt.to_rfc3339()
                    });

                    result_ok_json(&json!({
                        "path": path,
                        "type": kind,
                        "size": meta.len(),
                        "readonly": meta.permissions().readonly(),
                        "modified": modified,
                    }))
                }
                Err(e) => {
                    result_error_typed(McpErrorCode::IoError, format!("failed to stat {path}: {e}"))
                }
            }
        })
    }
}

/// Check if a path exists.
pub struct PathExists;

impl Tool for PathExists {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_path_exists",
            "Check if a file or directory exists",
            json!({ "path": { "type": "string" } }),
            vec!["path".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return result_error_typed(
                        McpErrorCode::Validation,
                        "missing required field: path",
                    );
                }
            };
            let validated = match validate_path(path).await {
                Ok(p) => p,
                Err(e) => return result_error_typed(McpErrorCode::PermissionDenied, e),
            };
            let (exists, is_file, is_dir) = match tokio::fs::metadata(&validated).await {
                Ok(meta) => (true, meta.is_file(), meta.is_dir()),
                Err(_) => (false, false, false),
            };
            result_ok_json(&json!({
                "path": validated.display().to_string(),
                "exists": exists,
                "is_file": is_file,
                "is_dir": is_dir,
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a TempDir under the current working directory so paths pass validation.
    fn cwd_tempdir() -> TempDir {
        let cwd = std::env::current_dir().unwrap();
        TempDir::new_in(cwd).unwrap()
    }

    #[tokio::test]
    async fn file_read_write() {
        let tmp = cwd_tempdir();
        let path = tmp.path().join("test.txt");
        let path_str = path.display().to_string();

        let result = FileWrite
            .call(json!({"path": path_str, "content": "hello"}))
            .await;
        assert_eq!(result["isError"], false);

        let result = FileRead.call(json!({"path": path_str})).await;
        assert_eq!(result["isError"], false);
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "hello");
    }

    #[tokio::test]
    async fn file_write_append() {
        let tmp = cwd_tempdir();
        let path = tmp.path().join("append.txt");
        let path_str = path.display().to_string();

        FileWrite
            .call(json!({"path": path_str, "content": "a"}))
            .await;
        FileWrite
            .call(json!({"path": path_str, "content": "b", "append": true}))
            .await;

        let result = FileRead.call(json!({"path": path_str})).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "ab");
    }

    #[tokio::test]
    async fn file_read_missing() {
        // Path outside cwd should be rejected
        let result = FileRead
            .call(json!({"path": "/nonexistent/file/xyz"}))
            .await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn file_read_rejects_path_traversal() {
        let result = FileRead.call(json!({"path": "/etc/passwd"})).await;
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("outside working directory"));
    }

    #[tokio::test]
    async fn dir_list() {
        let tmp = cwd_tempdir();
        std::fs::write(tmp.path().join("a.txt"), "").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "").unwrap();

        let result = DirList
            .call(json!({"path": tmp.path().display().to_string()}))
            .await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let entries: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn file_stat() {
        let tmp = cwd_tempdir();
        let path = tmp.path().join("stat.txt");
        std::fs::write(&path, "12345").unwrap();

        let result = FileStat
            .call(json!({"path": path.display().to_string()}))
            .await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"size\": 5"));
        assert!(text.contains("\"type\": \"file\""));
    }

    #[tokio::test]
    async fn path_exists_cwd() {
        // "." is always under cwd
        let result = PathExists.call(json!({"path": "."})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"exists\": true"));
        assert!(text.contains("\"is_dir\": true"));
    }

    #[tokio::test]
    async fn path_exists_rejects_outside_cwd() {
        let result = PathExists.call(json!({"path": "/tmp"})).await;
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("outside working directory"));
    }
}
