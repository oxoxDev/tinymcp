//! Serving the Model Context Protocol.
//!
//! The client half of this crate talks *to* MCP servers; this half lets a host
//! *be* one. The host implements [`McpServerHandler`] — its identity, its
//! tools, its resources — and this module does the protocol around it.
//!
//! See `README.md` beside this file for the design and the wire guarantees.

pub mod args;
mod types;

pub use types::{
    DEFAULT_SOURCE_TYPE_PREFIX, McpServerHandler, RequestContext, RequestHeaders, ResourceSpec,
    ServerInfo, ServerToolSpec, ToolCallError,
};

#[cfg(test)]
mod test;
