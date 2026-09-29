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
