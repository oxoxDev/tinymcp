//! The seam between a tool and whatever actually calls the server.

use async_trait::async_trait;
use serde_json::Value;
use tinymcp_bus::McpToolResult;

use crate::config_servers::McpServerRegistry;
use crate::error::Result;
use crate::registry::McpRegistry;

/// Calls one tool on one server.
///
/// [`crate::tools::McpServerTool`] holds one of these rather than a registry,
/// so a host can put its own policy in front of every call — an approval
/// gate, an audit row, a liveness or injection re-check, a secret scrubber —
/// by wrapping one of the implementations below. tinymcp enforces only what it
/// owns: a static server's allow and deny lists, and that a dynamic server is
/// connected.
///
/// `server_id` is whatever the implementation addresses servers by: an
/// install identifier for [`McpRegistry`], a configured name for
/// [`McpServerRegistry`].
#[async_trait]
pub trait McpToolInvoker: Send + Sync + std::fmt::Debug {
    /// Calls `tool` on `server_id` with `arguments`.
    ///
    /// A tool that reports failure is a successful call with
    /// [`McpToolResult::is_error`] set.
    ///
    /// # Errors
    ///
    /// Whatever reaching the server returns: not connected, not allowed, a
    /// transport failure.
    async fn invoke(&self, server_id: &str, tool: &str, arguments: Value) -> Result<McpToolResult>;
}

#[async_trait]
impl McpToolInvoker for McpRegistry {
    async fn invoke(&self, server_id: &str, tool: &str, arguments: Value) -> Result<McpToolResult> {
        self.connections()
            .call_tool(server_id, tool, arguments)
            .await
            .map(|result| result.rendered)
    }
}

#[async_trait]
impl McpToolInvoker for McpServerRegistry {
    async fn invoke(&self, server_id: &str, tool: &str, arguments: Value) -> Result<McpToolResult> {
        self.call_tool(server_id, tool, arguments)
            .await
            .map(|result| result.rendered)
    }
}
