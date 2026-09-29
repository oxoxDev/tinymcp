//! MCP server tools as [`tinytools::Tool`]s.
//!
//! Every tool a server advertises becomes one tool a model can call by name,
//! `mcp_<server>_<tool>`, rather than a call routed through a generic
//! "call this tool on that server" proxy. Each is either **direct** — sent to
//! the model every turn — or **deferred** — left out of the catalogue and
//! found through the harness's tool search, then called like any other.
//! Deferred is the default; see [`McpExposure`].
//!
//! # Feeding it from the cache
//!
//! [`McpToolSource`] is built from a [`tinymcp_bus::ConnectedServerOverview`]
//! or a configured server's tool list. Both can come from the persistent tool
//! cache — [`crate::McpRegistry::cached_overview`] and
//! [`crate::McpServerRegistry::cached_tools`] — so a host can offer every
//! tool at boot without dialling a single server.
//!
//! # What stays with the host
//!
//! Calls go through an [`McpToolInvoker`]. tinymcp implements it for both
//! registries; a host wraps one with its own approvals, audit, screening and
//! scrubbing. Nothing here decides whether a call is allowed beyond what
//! tinymcp already owns.
//!
//! # Example
//!
//! ```
//! use tinymcp::tools::{McpToolSource, McpExposure, naming};
//!
//! assert_eq!(naming::tool_name("@acme/ticktick-mcp", "readGoals"), "mcp_ticktick_read_goals");
//! ```

pub mod invoker;
pub mod naming;
mod result;
mod schema;
mod source;
mod tool;

use std::collections::HashSet;
use std::sync::Arc;

pub use invoker::McpToolInvoker;
pub use result::{MAX_LLM_BLOCK_BYTES, tool_result};
pub use schema::tool_parameters;
pub use source::{McpExposure, McpToolSource};
pub use tool::McpServerTool;

/// One tool per advertised tool of each source, named, described, and wired
/// to `invoker`.
///
/// Sources are taken in server order and tools in name order, so the result —
/// and which of two colliding tools gets the plain name — is the same on every
/// call. A tool with a blank name is skipped. A name two servers would share
/// goes to the first; the second gets
/// [`naming::disambiguated_tool_name`].
#[must_use]
pub fn tools_for(
    sources: &[McpToolSource],
    invoker: &Arc<dyn McpToolInvoker>,
) -> Vec<McpServerTool> {
    let mut ordered: Vec<&McpToolSource> = sources.iter().collect();
    ordered.sort_by(|left, right| left.server_id.cmp(&right.server_id));

    let mut taken: HashSet<String> = HashSet::new();
    let mut built = Vec::new();
    for source in ordered {
        let mut tools: Vec<&tinymcp_bus::McpTool> = source.tools.iter().collect();
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        for tool in tools {
            if tool.name.trim().is_empty() {
                continue;
            }
            let mut name = naming::tool_name(&source.label, &tool.name);
            if taken.contains(&name) {
                name = naming::disambiguated_tool_name(&source.server_id, &source.label, &tool.name);
            }
            if !taken.insert(name.clone()) {
                tracing::debug!(
                    server_id = %source.server_id,
                    tool = %tool.name,
                    "skipping a tool whose name is advertised twice"
                );
                continue;
            }
            built.push(McpServerTool::new(name, source, tool, Arc::clone(invoker)));
        }
    }
    tracing::debug!(sources = sources.len(), tools = built.len(), "built MCP tools");
    built
}

#[cfg(test)]
mod test;
