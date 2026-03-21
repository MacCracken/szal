//! MCP transport layer — stdio and HTTP/SSE.
//!
//! Stdio transport reads JSON-RPC messages from stdin and writes responses
//! to stdout, one message per line (newline-delimited JSON).

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::protocol::JsonRpcRequest;
use crate::mcp::registry::Registry;

/// Run the MCP server over stdio (one JSON-RPC message per line).
pub async fn serve_stdio(registry: Arc<Registry>) -> crate::Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => registry.handle_request(req).await,
            Err(e) => crate::mcp::protocol::JsonRpcResponse::error(
                serde_json::Value::Null,
                crate::mcp::protocol::PARSE_ERROR,
                e.to_string(),
            ),
        };

        let mut out = serde_json::to_string(&response).unwrap_or_default();
        out.push('\n');
        if stdout.write_all(out.as_bytes()).await.is_err() {
            break;
        }
        let _ = stdout.flush().await;
    }

    Ok(())
}

/// Configuration for the HTTP transport.
#[cfg(feature = "mcp")]
pub struct HttpConfig {
    pub host: String,
    pub port: u16,
}

#[cfg(feature = "mcp")]
impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 18790,
        }
    }
}

/// Run the MCP server over streamable HTTP + SSE.
#[cfg(feature = "mcp")]
pub async fn serve_http(
    registry: Arc<Registry>,
    config: HttpConfig,
) -> crate::Result<()> {
    use axum::{Router, routing::post};

    let app = Router::new()
        .route("/mcp", post(handle_mcp_post))
        .with_state(registry);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| crate::SzalError::Other(e.into()))?;
    tracing::info!("MCP HTTP server listening on {addr}");
    axum::serve(listener, app)
        .await
        .map_err(|e| crate::SzalError::Other(e.into()))?;

    Ok(())
}

#[cfg(feature = "mcp")]
async fn handle_mcp_post(
    axum::extract::State(registry): axum::extract::State<Arc<Registry>>,
    axum::Json(req): axum::Json<JsonRpcRequest>,
) -> axum::Json<crate::mcp::protocol::JsonRpcResponse> {
    axum::Json(registry.handle_request(req).await)
}
