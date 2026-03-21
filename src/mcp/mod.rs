//! MCP (Model Context Protocol) server implementation.
//!
//! Provides tool registration, discovery, and execution over multiple transports:
//! - Streamable HTTP (primary)
//! - Server-Sent Events (SSE)
//! - Stdio (for local integrations)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌──────────┐
//! │  Transport   │────▶│   Router     │────▶│ Registry │
//! │ HTTP/SSE/IO  │     │ JSON-RPC 2.0 │     │  Tools   │
//! └─────────────┘     └──────────────┘     │ Resources│
//!                                           │ Prompts  │
//!                                           └──────────┘
//! ```

pub mod protocol;
pub mod registry;
pub mod tool;
pub mod tools;
pub mod transport;
