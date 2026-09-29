//! One searchable tool per action a connected server advertises.
//!
//! A host that lists every remote tool up front pays for all of them in every
//! prompt. Instead each becomes a *deferred* tool — found by search, costing
//! nothing until a model looks for it — with a provider-safe name derived from
//! the server and the remote name, and every remote string sanitized before it
//! reaches the model.
//!
//! Admission is the host's. [`action_tool_specs`] takes a filter so a host can
//! apply its own prompt-injection policy to each server's definitions; this
//! module decides only how an admitted tool is named and described.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::types::{AgentToolEffect, AgentToolSpec};
use crate::registry::{ConnectedServerOverview, McpTool};
use crate::sanitize::sanitize_for_llm;

/// The longest slug a name carries, leaving room for the prefix and digest
/// under the 64-character limit providers put on tool names.
const MAX_SLUG_CHARS: usize = 42;

/// How many hex characters of the digest a name carries.
const DIGEST_HEX_CHARS: usize = 12;

/// The byte cap on a server's name or family as the model reads it.
const MAX_SERVER_TEXT_BYTES: usize = 120;

/// The byte cap on a tool description, and on each schema `description` or
/// `title`.
const MAX_TOOL_TEXT_BYTES: usize = 500;

/// An action tool's spec, and what a host needs to execute it.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionToolSpec {
    /// What the model sees: always deferred, always [`AgentToolEffect::Execute`].
    pub spec: AgentToolSpec,
    /// The family a host groups the tool under: the server's qualified name,
    /// sanitized.
    pub family: String,
    /// The server the call goes to.
    pub server_id: String,
    /// The tool's name on that server, verbatim — what the call names.
    pub tool_name: String,
}

/// A stable, provider-safe name for one server's tool.
///
/// `mcp_<slug>_<digest>`: the slug is the remote name lowercased with anything
/// but ASCII alphanumerics replaced by `_`, cut to 42 characters and trimmed of
/// `_`; the digest is the first 12 hex characters of SHA-256 over
/// `server_id \0 tool_name`, which tells apart equal names on different
/// servers and names that cut to the same slug.
///
/// # Examples
///
/// ```
/// # use tinymcp_bus::agent_tools::searchable_name;
/// assert_eq!(searchable_name("server-0", "forecast"), "mcp_forecast_183b86171ccb");
/// ```
#[must_use]
pub fn searchable_name(server_id: &str, tool_name: &str) -> String {
    let slug: String = tool_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .take(MAX_SLUG_CHARS)
        .collect();
    let slug = slug.trim_matches('_');
    let slug = if slug.is_empty() { "tool" } else { slug };
    let digest = Sha256::digest(format!("{server_id}\0{tool_name}").as_bytes());
    let mut hex = String::with_capacity(DIGEST_HEX_CHARS);
    for byte in digest.iter().take(DIGEST_HEX_CHARS / 2) {
        // Writing to a `String` cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    format!("mcp_{slug}_{hex}")
}

/// The spec for one tool on one connected server.
///
/// The description reads `MCP server <name>: <tool description>`, naming the
/// server by its display name, or its qualified name when that is blank, and
/// the tool by its name when it has no description. A schema that is not an
/// object becomes an empty object schema, and every `description` and `title`
/// inside it is sanitized.
#[must_use]
pub fn action_tool_spec(server: &ConnectedServerOverview, tool: &McpTool) -> ActionToolSpec {
    let server_name = if server.display_name.trim().is_empty() {
        &server.qualified_name
    } else {
        &server.display_name
    };
    let description = format!(
        "MCP server {}: {}",
        sanitize_for_llm(server_name, MAX_SERVER_TEXT_BYTES),
        sanitize_for_llm(
            tool.description.as_deref().unwrap_or(&tool.name),
            MAX_TOOL_TEXT_BYTES
        )
    );
    let mut parameters = if tool.input_schema.is_object() {
        tool.input_schema.clone()
    } else {
        json!({ "type": "object", "properties": {} })
    };
    sanitize_schema_descriptions(&mut parameters);

    ActionToolSpec {
        spec: AgentToolSpec {
            name: searchable_name(&server.server_id, &tool.name),
            description,
            parameters,
            effect: AgentToolEffect::Execute,
            deferred: true,
        },
        family: sanitize_for_llm(&server.qualified_name, MAX_SERVER_TEXT_BYTES),
        server_id: server.server_id.clone(),
        tool_name: tool.name.clone(),
    }
}

/// One spec per admitted tool on every connected server.
///
/// Servers are taken in `server_id` order and each server's admitted tools in
/// name order, so the same connections produce the same list and a prompt
/// built from it keeps its cached prefix. `admit` receives each server's id and
/// tools and returns the ones the host will expose. A blank remote name is
/// skipped, and a name already produced is not produced twice.
pub fn action_tool_specs<F>(
    servers: &[ConnectedServerOverview],
    mut admit: F,
) -> Vec<ActionToolSpec>
where
    F: FnMut(&str, Vec<McpTool>) -> Vec<McpTool>,
{
    let mut ordered: Vec<&ConnectedServerOverview> = servers.iter().collect();
    ordered.sort_by(|a, b| a.server_id.cmp(&b.server_id));

    let mut names = HashSet::new();
    let mut specs = Vec::new();
    for server in ordered {
        let mut tools = admit(&server.server_id, server.tools.clone());
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        for tool in &tools {
            if tool.name.trim().is_empty() {
                continue;
            }
            let action = action_tool_spec(server, tool);
            if names.insert(action.spec.name.clone()) {
                specs.push(action);
            }
        }
    }
    specs
}

/// Sanitizes every string-valued `description` and `title` in a schema.
///
/// Walks objects and arrays. A key named `description` or `title` whose value
/// is not a string — a property of that name — is walked like any other.
pub fn sanitize_schema_descriptions(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                match child {
                    Value::String(text) if key == "description" || key == "title" => {
                        *text = sanitize_for_llm(text, MAX_TOOL_TEXT_BYTES);
                    }
                    _ => sanitize_schema_descriptions(child),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                sanitize_schema_descriptions(item);
            }
        }
        _ => {}
    }
}
