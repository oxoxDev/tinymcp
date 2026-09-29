//! What an agent host needs to expose MCP to a model as tools.

mod action;
mod arguments;
mod registry_tools;
mod types;

pub use action::{
    ActionToolSpec, action_tool_spec, action_tool_specs, sanitize_schema_descriptions,
    searchable_name,
};
pub use arguments::normalize_tool_arguments;
pub use registry_tools::{RegistryTool, registry_tool_specs};
pub use types::{AgentToolEffect, AgentToolSpec, ArgsError};

#[cfg(test)]
mod test;
