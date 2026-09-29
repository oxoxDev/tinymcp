//! What an agent host needs to expose MCP to a model as tools.

mod arguments;
mod types;

pub use arguments::normalize_tool_arguments;
pub use types::ArgsError;

#[cfg(test)]
mod test;
