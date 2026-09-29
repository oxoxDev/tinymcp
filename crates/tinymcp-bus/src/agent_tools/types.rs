//! The agent-tool payload types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Why a tool call's `arguments` could not be read as an object.
///
/// Each variant names what actually arrived, because the message is read by
/// the model that sent it and "invalid arguments" gives it nothing to change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArgsError {
    /// The value was neither an object nor a string that could hold one.
    NotAnObject {
        /// The JSON type that arrived, with its article (`"an array"`).
        actual: &'static str,
    },
    /// The value was a string that did not decode to a JSON object.
    StringNotAnObject {
        /// The JSON type the string decoded to, or `None` when it was not
        /// JSON at all.
        decoded: Option<&'static str>,
    },
}

impl std::fmt::Display for ArgsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("tool arguments must be a JSON object, not ")?;
        match self {
            Self::NotAnObject { actual } => f.write_str(actual),
            Self::StringNotAnObject {
                decoded: Some(decoded),
            } => write!(f, "a string holding {decoded}"),
            Self::StringNotAnObject { decoded: None } => f.write_str("a string that is not JSON"),
        }
    }
}

impl std::error::Error for ArgsError {}

/// What calling a tool can do to the world.
///
/// A host maps this onto its own permission model; the contract only says
/// which of the three a tool is. Declared in increasing order of consequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentToolEffect {
    /// Observes only: browsing a catalog, listing, reporting status.
    Read,
    /// Acts without changing what is configured: connecting, calling a tool.
    Execute,
    /// Changes persistent configuration: uninstalling a server.
    Write,
}

/// One tool as a model sees it: its identity and what it may do.
///
/// `name`, `description` and `parameters` are prompt-cache and transcript
/// identity — a model's cached prefix and a resumed session's replay both hold
/// them verbatim — so a spec built here must not drift between releases
/// without a reason worth invalidating every cached conversation for.
///
/// Execution is not here. A spec says what a tool is; running it, gating it
/// behind an approval, and deciding whether it is shown at all are the host's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolSpec {
    /// The provider-safe tool name.
    pub name: String,
    /// What the model reads to decide whether to call it.
    pub description: String,
    /// The JSON Schema of the tool's arguments.
    pub parameters: Value,
    /// What calling it can do.
    pub effect: AgentToolEffect,
    /// Whether the tool is found through search rather than listed up front.
    ///
    /// A deferred tool costs nothing in the prompt until a model looks for it.
    pub deferred: bool,
}
