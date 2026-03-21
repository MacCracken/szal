//! Git repository tools.

use crate::mcp::{Tool, tool_def, result_ok, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;



/// Reject values that look like git options to prevent option injection.
fn validate_git_ref(s: &str) -> Result<(), String> {
    if s.starts_with('-') {
        Err(format!("invalid ref: '{s}' — must not start with '-'"))
    } else {
        Ok(())
    }
}

async fn git_cmd(args: &[&str], cwd: Option<&str>) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    match cmd.output().await {
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => Err(format!("git not available: {e}")),
    }
}

/// Get git status of a repository.
pub struct GitStatus;

impl Tool for GitStatus {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_git_status",
            "Get git status (branch, modified/staged/untracked files)",
            json!({ "path": { "type": "string", "description": "Repository path (default: current dir)" } }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let cwd = args.get("path").and_then(|v| v.as_str());

            let branch = git_cmd(&["rev-parse", "--abbrev-ref", "HEAD"], cwd).await.unwrap_or_default();
            let status = git_cmd(&["status", "--porcelain"], cwd).await.unwrap_or_default();

            let mut modified = 0;
            let mut staged = 0;
            let mut untracked = 0;
            for line in status.lines() {
                let bytes = line.as_bytes();
                if bytes.len() < 2 { continue; }
                match (bytes[0], bytes[1]) {
                    (b'?', b'?') => untracked += 1,
                    (b' ', _) => modified += 1,
                    (_, b' ') => staged += 1,
                    _ => { modified += 1; staged += 1; }
                }
            }

            let clean = status.is_empty();
            result_ok(&serde_json::to_string_pretty(&json!({
                "branch": branch,
                "clean": clean,
                "modified": modified,
                "staged": staged,
                "untracked": untracked,
            })).unwrap_or_default())
        })
    }
}

/// Get recent git log.
pub struct GitLog;

impl Tool for GitLog {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_git_log",
            "Get recent git commits",
            json!({
                "path": { "type": "string", "description": "Repository path (default: current dir)" },
                "count": { "type": "integer", "description": "Number of commits (default: 10, max: 100)" }
            }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let cwd = args.get("path").and_then(|v| v.as_str());
            let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10).min(100);

            let format = "--format=%H|%h|%an|%ae|%aI|%s";
            let log = git_cmd(&["log", &format!("-{count}"), format], cwd).await;

            match log {
                Ok(output) => {
                    let commits: Vec<serde_json::Value> = output
                        .lines()
                        .filter_map(|line| {
                            let parts: Vec<&str> = line.splitn(6, '|').collect();
                            if parts.len() == 6 {
                                Some(json!({
                                    "hash": parts[0],
                                    "short_hash": parts[1],
                                    "author": parts[2],
                                    "email": parts[3],
                                    "date": parts[4],
                                    "message": parts[5],
                                }))
                            } else {
                                None
                            }
                        })
                        .collect();
                    result_ok(&serde_json::to_string_pretty(&commits).unwrap_or_default())
                }
                Err(e) => result_error(e),
            }
        })
    }
}

/// Get git diff.
pub struct GitDiff;

impl Tool for GitDiff {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_git_diff",
            "Get git diff (staged, unstaged, or between refs)",
            json!({
                "path": { "type": "string", "description": "Repository path (default: current dir)" },
                "staged": { "type": "boolean", "description": "Show staged changes (default: false)" },
                "ref1": { "type": "string", "description": "First ref for comparison" },
                "ref2": { "type": "string", "description": "Second ref for comparison" },
                "stat_only": { "type": "boolean", "description": "Show only file stats, not full diff (default: false)" }
            }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let cwd = args.get("path").and_then(|v| v.as_str());
            let staged = args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
            let stat_only = args.get("stat_only").and_then(|v| v.as_bool()).unwrap_or(false);

            let mut git_args = vec!["diff"];
            if staged { git_args.push("--cached"); }
            if stat_only { git_args.push("--stat"); }

            if let Some(r1) = args.get("ref1").and_then(|v| v.as_str()) {
                if let Err(e) = validate_git_ref(r1) { return result_error(e); }
                git_args.push(r1);
                if let Some(r2) = args.get("ref2").and_then(|v| v.as_str()) {
                    if let Err(e) = validate_git_ref(r2) { return result_error(e); }
                    git_args.push(r2);
                }
            }

            match git_cmd(&git_args, cwd).await {
                Ok(diff) => {
                    if diff.is_empty() {
                        result_ok("no changes")
                    } else {
                        result_ok(&diff)
                    }
                }
                Err(e) => result_error(e),
            }
        })
    }
}

/// Get current git branch and tag info.
pub struct GitBranch;

impl Tool for GitBranch {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_git_branch",
            "List git branches and show current branch",
            json!({
                "path": { "type": "string", "description": "Repository path (default: current dir)" },
                "all": { "type": "boolean", "description": "Include remote branches (default: false)" }
            }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let cwd = args.get("path").and_then(|v| v.as_str());
            let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);

            let current = git_cmd(&["rev-parse", "--abbrev-ref", "HEAD"], cwd).await.unwrap_or_default();

            let branch_args = if all {
                vec!["branch", "-a", "--format=%(refname:short)"]
            } else {
                vec!["branch", "--format=%(refname:short)"]
            };

            let branches = git_cmd(&branch_args.to_vec(), cwd)
                .await
                .unwrap_or_default();

            let branch_list: Vec<&str> = branches.lines().collect();

            result_ok(&serde_json::to_string_pretty(&json!({
                "current": current,
                "branches": branch_list,
                "count": branch_list.len(),
            })).unwrap_or_default())
        })
    }
}

/// Git blame a file.
pub struct GitBlame;

impl Tool for GitBlame {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_git_blame",
            "Show git blame for a file (who last modified each line)",
            json!({
                "file": { "type": "string", "description": "File path to blame" },
                "path": { "type": "string", "description": "Repository path (default: current dir)" }
            }),
            vec!["file".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let file = match args.get("file").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return result_error("missing required field: file"),
            };
            let cwd = args.get("path").and_then(|v| v.as_str());

            match git_cmd(&["blame", "--porcelain", file], cwd).await {
                Ok(output) => {
                    // Summarize: count commits per author
                    let mut authors: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
                    for line in output.lines() {
                        if let Some(author) = line.strip_prefix("author ") {
                            *authors.entry(author.to_string()).or_default() += 1;
                        }
                    }
                    let total_lines = authors.values().sum::<usize>();
                    let mut author_list: Vec<_> = authors.into_iter().collect();
                    author_list.sort_by(|a, b| b.1.cmp(&a.1));

                    result_ok(&serde_json::to_string_pretty(&json!({
                        "file": file,
                        "total_lines": total_lines,
                        "authors": author_list.iter().map(|(name, count)| json!({
                            "name": name,
                            "lines": count,
                        })).collect::<Vec<_>>(),
                    })).unwrap_or_default())
                }
                Err(e) => result_error(e),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn git_status_current_repo() {
        let result = GitStatus.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"branch\""));
    }

    #[tokio::test]
    async fn git_log_current_repo() {
        let result = GitLog.call(json!({"count": 3})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let commits: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert!(!commits.is_empty());
        assert!(commits[0].get("hash").is_some());
    }

    #[tokio::test]
    async fn git_diff_current_repo() {
        let result = GitDiff.call(json!({"stat_only": true})).await;
        assert_eq!(result["isError"], false);
    }

    #[tokio::test]
    async fn git_branch_current_repo() {
        let result = GitBranch.call(json!({})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"current\""));
    }

    #[tokio::test]
    async fn git_blame_file() {
        let result = GitBlame.call(json!({"file": "Cargo.toml"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"total_lines\""));
    }
}
