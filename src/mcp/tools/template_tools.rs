//! Template and text transformation tools.

use crate::mcp::{Tool, tool_def, result_ok, result_error};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;



/// Simple mustache-style template rendering.
pub struct TemplateRender;

impl Tool for TemplateRender {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_template_render",
            "Render a mustache-style template with {{variable}} substitution",
            json!({
                "template": { "type": "string", "description": "Template string with {{var}} placeholders" },
                "variables": { "type": "object", "description": "Key-value pairs for substitution" }
            }),
            vec!["template".into(), "variables".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let template = match args.get("template").and_then(|v| v.as_str()) {
                Some(t) => t.to_string(),
                None => return result_error("missing required field: template"),
            };
            let vars = match args.get("variables").and_then(|v| v.as_object()) {
                Some(v) => v,
                None => return result_error("missing required field: variables"),
            };

            let mut result = template;
            for (key, value) in vars {
                let placeholder = format!("{{{{{key}}}}}");
                let replacement = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                result = result.replace(&placeholder, &replacement);
            }

            result_ok(&result)
        })
    }
}

/// Count lines, words, and characters in text.
pub struct WordCount;

impl Tool for WordCount {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_wc",
            "Count lines, words, and characters in text",
            json!({
                "text": { "type": "string", "description": "Text to count (mutually exclusive with file)" },
                "file": { "type": "string", "description": "File path to count (mutually exclusive with text)" }
            }),
            vec![],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let text = if let Some(t) = args.get("text").and_then(|v| v.as_str()) {
                t.to_string()
            } else if let Some(path) = args.get("file").and_then(|v| v.as_str()) {
                match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => return result_error(format!("failed to read {path}: {e}")),
                }
            } else {
                return result_error("provide either 'text' or 'file'");
            };

            let lines = text.lines().count();
            let words = text.split_whitespace().count();
            let chars = text.chars().count();
            let bytes = text.len();

            result_ok(&serde_json::to_string_pretty(&json!({
                "lines": lines,
                "words": words,
                "chars": chars,
                "bytes": bytes,
            })).unwrap_or_default())
        })
    }
}

/// Search and replace in text.
pub struct TextReplace;

impl Tool for TextReplace {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_text_replace",
            "Search and replace text in a string",
            json!({
                "text": { "type": "string", "description": "Input text" },
                "search": { "type": "string", "description": "Text to find" },
                "replace": { "type": "string", "description": "Replacement text" },
                "all": { "type": "boolean", "description": "Replace all occurrences (default: true)" }
            }),
            vec!["text".into(), "search".into(), "replace".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let text = match args.get("text").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return result_error("missing required field: text"),
            };
            let search = match args.get("search").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return result_error("missing required field: search"),
            };
            let replace_with = match args.get("replace").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => return result_error("missing required field: replace"),
            };
            let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(true);

            let result = if all {
                text.replace(search, replace_with)
            } else {
                text.replacen(search, replace_with, 1)
            };

            result_ok(&result)
        })
    }
}

/// Split text into lines or by delimiter.
pub struct TextSplit;

impl Tool for TextSplit {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_text_split",
            "Split text by a delimiter and return as JSON array",
            json!({
                "text": { "type": "string", "description": "Text to split" },
                "delimiter": { "type": "string", "description": "Delimiter (default: newline)" }
            }),
            vec!["text".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let text = match args.get("text").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return result_error("missing required field: text"),
            };
            let delim = args.get("delimiter").and_then(|v| v.as_str()).unwrap_or("\n");

            let parts: Vec<&str> = text.split(delim).collect();
            result_ok(&serde_json::to_string_pretty(&parts).unwrap_or_default())
        })
    }
}

/// Join array elements into a string.
pub struct TextJoin;

impl Tool for TextJoin {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_text_join",
            "Join array elements into a single string with a separator",
            json!({
                "parts": { "type": "array", "items": { "type": "string" }, "description": "Array of strings to join" },
                "separator": { "type": "string", "description": "Separator (default: newline)" }
            }),
            vec!["parts".into()],
        )
    }

    fn call(&self, args: serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let parts = match args.get("parts").and_then(|v| v.as_array()) {
                Some(arr) => arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>(),
                None => return result_error("missing required field: parts"),
            };
            let sep = args.get("separator").and_then(|v| v.as_str()).unwrap_or("\n");
            result_ok(&parts.join(sep))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn template_render() {
        let result = TemplateRender.call(json!({
            "template": "Hello {{name}}, you are {{age}} years old",
            "variables": {"name": "Alice", "age": 30}
        })).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "Hello Alice, you are 30 years old");
    }

    #[tokio::test]
    async fn word_count() {
        let result = WordCount.call(json!({"text": "hello world\nfoo bar baz"})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"lines\": 2"));
        assert!(text.contains("\"words\": 5"));
    }

    #[tokio::test]
    async fn text_replace_all() {
        let result = TextReplace.call(json!({"text": "aaa", "search": "a", "replace": "b"})).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "bbb");
    }

    #[tokio::test]
    async fn text_replace_first() {
        let result = TextReplace.call(json!({"text": "aaa", "search": "a", "replace": "b", "all": false})).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "baa");
    }

    #[tokio::test]
    async fn text_split() {
        let result = TextSplit.call(json!({"text": "a,b,c", "delimiter": ","})).await;
        let text = result["content"][0]["text"].as_str().unwrap();
        let parts: Vec<String> = serde_json::from_str(text).unwrap();
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn text_join() {
        let result = TextJoin.call(json!({"parts": ["x", "y", "z"], "separator": "-"})).await;
        assert_eq!(result["content"][0]["text"].as_str().unwrap(), "x-y-z");
    }
}
