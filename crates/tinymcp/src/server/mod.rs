//! Serving the Model Context Protocol.
//!
//! The client half of this crate talks *to* MCP servers; this half lets a host
//! *be* one. The host implements [`McpServerHandler`] — its identity, its
//! tools, its resources — and this module does the protocol around it.
//!
//! See `README.md` beside this file for the design and the wire guarantees.

pub mod args;
#[cfg(test)]
mod fixture;
mod protocol;
mod session;
mod types;

pub use protocol::{handle_line, handle_value};
pub use session::ClientSession;
pub use types::{
    DEFAULT_SOURCE_TYPE_PREFIX, McpServerHandler, RequestContext, RequestHeaders, ResourceSpec,
    ServerInfo, ServerToolSpec, ToolCallError,
};

#[cfg(test)]
mod test;
