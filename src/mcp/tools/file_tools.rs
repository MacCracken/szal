//! File system tools — read, write, list, stat, search.

use crate::mcp::{Tool, tool_def, result_ok, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::path::Path;
use std::pin::Pin;



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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return result_error("missing required field: path"),
            };
            let max_bytes = args.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(1_048_576) as usize;

            match std::fs::read_to_string(path) {
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
                Err(e) => result_error(format!("failed to read {path}: {e}")),
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return result_error("missing required field: path"),
            };
            let content = match args.get("content").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return result_error("missing required field: content"),
            };
            let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

            let result = if append {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .and_then(|mut f| f.write_all(content.as_bytes()))
            } else {
                std::fs::write(path, content)
            };

            match result {
                Ok(()) => result_ok(&format!("wrote {} bytes to {path}", content.len())),
                Err(e) => result_error(format!("failed to write {path}: {e}")),
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
            let max = args.get("max_entries").and_then(|v| v.as_u64()).unwrap_or(500) as usize;

            let mut entries = Vec::new();
            if let Err(e) = collect_dir(Path::new(path), recursive, max, &mut entries) {
                return result_error(format!("failed to list {path}: {e}"));
            }

            result_ok(&serde_json::to_string_pretty(&entries).unwrap_or_default())
        })
    }
}

fn collect_dir(
    path: &Path,
    recursive: bool,
    max: usize,
    entries: &mut Vec<serde_json::Value>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        if entries.len() >= max {
            break;
        }
        let entry = entry?;
        let ft = entry.file_type()?;
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
            && let Ok(meta) = entry.metadata()
        {
            info["size"] = json!(meta.len());
        }

        entries.push(info);

        if recursive && ft.is_dir() && entries.len() < max {
            let _ = collect_dir(&entry.path(), true, max, entries);
        }
    }
    Ok(())
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return result_error("missing required field: path"),
            };

            match std::fs::symlink_metadata(path) {
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

                    result_ok(&serde_json::to_string_pretty(&json!({
                        "path": path,
                        "type": kind,
                        "size": meta.len(),
                        "readonly": meta.permissions().readonly(),
                        "modified": modified,
                    })).unwrap_or_default())
                }
                Err(e) => result_error(format!("failed to stat {path}: {e}")),
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

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let path = match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return result_error("missing required field: path"),
            };
            let p = Path::new(path);
            result_ok(&serde_json::to_string_pretty(&json!({
                "path": path,
                "exists": p.exists(),
                "is_file": p.is_file(),
                "is_dir": p.is_dir(),
            })).unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn file_read_write() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.txt");
        let path_str = path.display().to_string();

        let result = FileWrite.call(json!({"path": path_str, "content": "hello"})).await;
        assert_eq!(result["isError"], false);

        let result = FileRead.call(json!({"path": path_str})).await;
        assert_eq!(result["isError"], false);
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "hello");
    }

    #[tokio::test]
    async fn file_write_append() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("append.txt");
        let path_str = path.display().to_string();

        FileWrite.call(json!({"path": path_str, "content": "a"})).await;
        FileWrite.call(json!({"path": path_str, "content": "b", "append": true})).await;

        let result = FileRead.call(json!({"path": path_str})).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "ab");
    }

    #[tokio::test]
    async fn file_read_missing() {
        let result = FileRead.call(json!({"path": "/nonexistent/file/xyz"})).await;
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn dir_list() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "").unwrap();

        let result = DirList.call(json!({"path": tmp.path().display().to_string()})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let entries: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn file_stat() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("stat.txt");
        std::fs::write(&path, "12345").unwrap();

        let result = FileStat.call(json!({"path": path.display().to_string()})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"size\": 5"));
        assert!(text.contains("\"type\": \"file\""));
    }

    #[tokio::test]
    async fn path_exists() {
        let result = PathExists.call(json!({"path": "/tmp"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"exists\": true"));
        assert!(text.contains("\"is_dir\": true"));

        let result = PathExists.call(json!({"path": "/nonexistent_xyz_123"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"exists\": false"));
    }
}
