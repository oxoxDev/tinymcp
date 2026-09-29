//! What a host needs to expose MCP to a model as tools.
//!
//! A host that puts MCP in front of a model gives it two kinds of tool: the
//! fixed registry tools ([`RegistryTool`]) — browse the catalog, connect,
//! call. They are described here as [`AgentToolSpec`]s: the name, description
//! and schema a model reads, plus what calling the tool can do
//! ([`AgentToolEffect`]). The per-action tools a connected server advertises
//! are adapted by `tinymcp::tools` (the `tools` feature), not here.
//!
//! [`normalize_tool_arguments`] is the other half: models do not always send
//! `arguments` as the object MCP requires, and every path that forwards a call
//! reads them through it.
//!
//! # Why this is in the contract crate
//!
//! A tool's name, description and schema are prompt-cache and transcript
//! identity. Two hosts — or one host and this module's own adapter — that
//! described the same tool differently would each invalidate the other's cached
//! prompts, and a hand-kept copy drifts. The specs are pure data and the
//! normalization is pure `serde_json`, so they cost this crate nothing.
//!
//! # What is not here
//!
//! **Execution and policy.** Running a tool, mapping [`AgentToolEffect`] onto
//! a permission model, approvals, deciding which remote tools pass a
//! prompt-injection scan, and whether a tool is shown at all are the host's.
//! [`action_tool_specs`] takes the admission decision as a filter for exactly
//! that reason.

mod arguments;
mod registry_tools;
mod types;

pub use arguments::normalize_tool_arguments;
pub use registry_tools::{RegistryTool, registry_tool_specs};
pub use types::{AgentToolEffect, AgentToolSpec, ArgsError};

#[cfg(test)]
mod test;
