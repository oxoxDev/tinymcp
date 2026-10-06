//! Rendering the store as the document, and reading a document back.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::types::{Declared, ParseOptions, ParseReport, RejectedEntry};
use crate::error::{Error, Result};
use tinymcp_bus::{InstalledServer, Transport};

/// The document's root key.
pub const ROOT_KEY: &str = "mcpServers";

/// Fields a read echoes for the reader and a write ignores.
const ECHOED_FIELDS: &[&str] = &["envKeys", "authConfigured"];

/// Fields a write accepts without acting on, because other clients emit them.
const TOLERATED_FIELDS: &[&str] = &["type", "transport"];

/// The fields every entry may carry.
const STANDARD_FIELDS: &[&str] = &[
    "url",
    "command",
    "args",
    "env",
    "headers",
    "description",
    "enabled",
];

/// The fields [`parse_with`] reads and [`parse`] refuses, because the install
/// store has nowhere to keep them.
const EXTENSION_FIELDS: &[&str] = &["allowedTools", "disallowedTools", "timeoutSecs"];

/// Credential names beginning this way are the store's own bookkeeping (the
/// OAuth refresh bundle) and are neither rendered nor overwritten from here.
const INTERNAL_KEY_PREFIX: &str = "__";

/// Projects the install store into the document.
///
/// Servers are keyed by `qualified_name` and sorted, so the same store renders
/// the same text and a re-save is not an edit. `authConfigured` is derived from
/// the stored credential names the caller passes in, never from values.
#[must_use]
pub fn render(servers: &[InstalledServer], stored_keys: &BTreeMap<String, Vec<String>>) -> Value {
    let mut entries: Vec<&InstalledServer> = servers.iter().collect();
    entries.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));

    let mut out = Map::new();
    for server in entries {
        let mut entry = Map::new();
        if let Transport::HttpRemote { url } = &server.transport {
            entry.insert("url".into(), json!(url));
        } else {
            entry.insert("command".into(), json!(server.command));
            if !server.args.is_empty() {
                entry.insert("args".into(), json!(server.args));
            }
        }
        if let Some(description) = server.description.as_deref().filter(|d| !d.is_empty()) {
            entry.insert("description".into(), json!(description));
        }
        if !server.enabled {
            entry.insert("enabled".into(), json!(false));
        }

        let keys: Vec<&String> = stored_keys
            .get(&server.server_id)
            .map(|keys| {
                keys.iter()
                    .filter(|key| !key.starts_with(INTERNAL_KEY_PREFIX))
                    .collect()
            })
            .unwrap_or_default();
        if !keys.is_empty() {
            entry.insert("envKeys".into(), json!(keys));
        }
        entry.insert("authConfigured".into(), json!(!keys.is_empty()));

        out.insert(server.qualified_name.clone(), Value::Object(entry));
    }

    json!({ ROOT_KEY: out })
}

/// Reads a document into declarations, refusing anything it cannot honour.
///
/// The message names the entry and the field, because the text is still on
/// the user's screen and "invalid document" would send them hunting.
///
/// # Errors
///
/// Returns [`Error::ConfigDoc`] carrying a sentence for the first problem
/// found: a missing or non-object `mcpServers`, an entry that is not an object,
/// an entry with neither or both of `url` and `command`, a field of the wrong
/// type, or a field this host does not understand (a `cwd`, say, which the
/// install store cannot carry).
pub fn parse(doc: &Value) -> Result<Vec<Declared>> {
    parse_servers(doc).map_err(|detail| Error::ConfigDoc { detail })
}

/// [`parse`], with the refusal as the bare sentence.
fn parse_servers(doc: &Value) -> std::result::Result<Vec<Declared>, String> {
    let mut declared = Vec::new();
    for (name, entry) in server_entries(doc)? {
        declared.push(read_entry(name, entry, None)?);
    }
    Ok(declared)
}

/// Reads a document into declarations for a host that keeps its own store.
///
/// As [`parse`], plus `allowedTools`, `disallowedTools` and `timeoutSecs`, the
/// host's own fields from [`ParseOptions::host_fields`], and a refusal of two
/// keys that name one server once trimmed. Under [`ParseOptions::lenient`] a
/// refused entry is dropped and reported in [`ParseReport::rejected`] rather
/// than refusing the document.
///
/// # Errors
///
/// Returns [`Error::ConfigDoc`] when the document's root is unreadable, and,
/// unless lenient, for the first refused entry.
pub fn parse_with(doc: &Value, options: &ParseOptions<'_>) -> Result<ParseReport> {
    let config_doc = |detail| Error::ConfigDoc { detail };
    let mut report = ParseReport::default();
    for (name, entry) in server_entries(doc).map_err(config_doc)? {
        let read = if report.declared.iter().any(|seen| seen.name == name) {
            Err(format!("`{name}` is declared twice"))
        } else {
            read_entry(name, entry, Some(options))
        };
        match read {
            Ok(declared) => report.declared.push(declared),
            Err(detail) if options.lenient => {
                tracing::debug!(name, "dropping a refused mcp.json entry: {detail}");
                report.rejected.push(RejectedEntry {
                    name: name.to_string(),
                    detail,
                });
            }
            Err(detail) => return Err(config_doc(detail)),
        }
    }
    Ok(report)
}

/// The document's entries by trimmed key, refusing an unreadable root.
fn server_entries(doc: &Value) -> std::result::Result<Vec<(&str, &Value)>, String> {
    let Some(root) = doc.as_object() else {
        return Err(format!("mcp.json holds an object with an `{ROOT_KEY}` key"));
    };
    let Some(servers) = root.get(ROOT_KEY) else {
        return Err(format!("no `{ROOT_KEY}` key — every server lives under it"));
    };
    let Some(servers) = servers.as_object() else {
        return Err(format!("`{ROOT_KEY}` maps a server name to its settings"));
    };
    Ok(servers
        .iter()
        .map(|(name, entry)| (name.trim(), entry))
        .collect())
}

/// Reads one keyed entry, refusing an empty key or a non-object value.
fn read_entry(
    name: &str,
    entry: &Value,
    extension: Option<&ParseOptions<'_>>,
) -> std::result::Result<Declared, String> {
    if name.is_empty() {
        return Err("a server needs a name — one entry's key is empty".to_string());
    }
    let Some(entry) = entry.as_object() else {
        return Err(format!(
            "`{name}` holds an object, e.g. {{ \"command\": \"npx\", \"args\": [\"-y\", \"…\"] }} or {{ \"url\": \"https://…\" }}"
        ));
    };
    parse_entry(name, entry, extension)
}

/// Reads one entry. `extension` is `None` for [`parse`], which refuses the
/// extension and host fields.
fn parse_entry(
    name: &str,
    entry: &Map<String, Value>,
    extension: Option<&ParseOptions<'_>>,
) -> std::result::Result<Declared, String> {
    let host_fields = extension.map_or(&[][..], |options| options.host_fields);
    for key in entry.keys() {
        let key = key.as_str();
        let known = STANDARD_FIELDS.contains(&key)
            || ECHOED_FIELDS.contains(&key)
            || TOLERATED_FIELDS.contains(&key)
            || (extension.is_some() && EXTENSION_FIELDS.contains(&key))
            || host_fields.contains(&key);
        if !known {
            return Err(unknown_field(name, key, extension.is_some(), host_fields));
        }
    }

    let url = optional_string(name, entry, "url")?;
    let command = optional_string(name, entry, "command")?;

    let (transport, command) = match (url, command) {
        (Some(_), Some(_)) => {
            return Err(format!(
                "`{name}` names both a `url` and a `command`; a server is dialled one way"
            ));
        }
        (Some(url), None) => (Transport::HttpRemote { url }, String::new()),
        (None, Some(command)) => (Transport::Stdio, command),
        (None, None) => {
            return Err(format!(
                "`{name}` needs a `url` (hosted) or a `command` (run locally)"
            ));
        }
    };

    let args = match entry.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("`{name}`.args holds strings only"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?,
        Some(_) => return Err(format!("`{name}`.args is a list of strings")),
    };
    if matches!(transport, Transport::HttpRemote { .. }) && !args.is_empty() {
        return Err(format!(
            "`{name}` is a hosted server; `args` only apply to a `command`"
        ));
    }

    let credentials = parse_credentials(name, entry, &transport)?;

    let description = optional_string(name, entry, "description")?;
    let enabled = match entry.get("enabled") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => return Err(format!("`{name}`.enabled is true or false")),
    };

    Ok(Declared {
        name: name.to_string(),
        transport,
        command,
        args,
        credentials,
        description,
        enabled,
        allowed_tools: tool_list(name, entry, "allowedTools")?,
        disallowed_tools: tool_list(name, entry, "disallowedTools")?,
        timeout_secs: timeout_secs(name, entry)?,
        host_fields: host_field_values(entry, host_fields),
    })
}

/// The refusal for a field this read does not accept, listing what it does.
fn unknown_field(name: &str, key: &str, extended: bool, host_fields: &[&str]) -> String {
    let mut accepted: Vec<&str> = vec![
        "url",
        "headers",
        "command",
        "args",
        "env",
        "description",
        "enabled",
    ];
    if extended {
        accepted.extend(EXTENSION_FIELDS);
        accepted.extend(
            host_fields.iter().filter(|field| {
                !STANDARD_FIELDS.contains(field) && !EXTENSION_FIELDS.contains(field)
            }),
        );
    }
    let (last, rest) = accepted.split_last().unwrap_or((&"", &[]));
    format!(
        "`{name}` has a `{key}` field this host doesn't understand; it accepts {} and {last}",
        rest.join(", ")
    )
}

/// A list of tool names, trimmed, refusing a blank or non-string one. Absent
/// reads as empty.
fn tool_list(
    name: &str,
    entry: &Map<String, Value>,
    key: &str,
) -> std::result::Result<Vec<String>, String> {
    match entry.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::trim)
                    .filter(|tool| !tool.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| format!("`{name}`.{key} holds tool names only"))
            })
            .collect(),
        Some(_) => Err(format!("`{name}`.{key} is a list of tool names")),
    }
}

/// `timeoutSecs`, refusing anything but a whole number of seconds above zero.
fn timeout_secs(
    name: &str,
    entry: &Map<String, Value>,
) -> std::result::Result<Option<u64>, String> {
    match entry.get("timeoutSecs") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .filter(|secs| *secs > 0)
            .map(Some)
            .ok_or_else(|| format!("`{name}`.timeoutSecs is a whole number of seconds above zero")),
    }
}

/// The host's registered fields an entry carries, verbatim, skipping nulls
/// and any name that is a standard field.
fn host_field_values(entry: &Map<String, Value>, host_fields: &[&str]) -> BTreeMap<String, Value> {
    host_fields
        .iter()
        .filter(|field| {
            !STANDARD_FIELDS.contains(field)
                && !EXTENSION_FIELDS.contains(field)
                && !ECHOED_FIELDS.contains(field)
                && !TOLERATED_FIELDS.contains(field)
        })
        .filter_map(|field| {
            let value = entry.get(*field).filter(|value| !value.is_null())?;
            Some(((*field).to_string(), value.clone()))
        })
        .collect()
}

/// Writes declarations back as the document [`parse_with`] reads.
///
/// For a host that keeps declarations itself; [`render`] is the install
/// store's projection. Servers are sorted by name, the extension and host
/// fields are emitted when set, and credentials never are: a declaration's
/// `env` or `headers` is write-only, as everywhere else in this module.
#[must_use]
pub fn render_declared(declared: &[Declared]) -> Value {
    let mut entries: Vec<&Declared> = declared.iter().collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let mut out = Map::new();
    for server in entries {
        let mut entry = Map::new();
        for (key, value) in &server.host_fields {
            entry.insert(key.clone(), value.clone());
        }
        if let Transport::HttpRemote { url } = &server.transport {
            entry.insert("url".into(), json!(url));
        } else {
            entry.insert("command".into(), json!(server.command));
            if !server.args.is_empty() {
                entry.insert("args".into(), json!(server.args));
            }
        }
        if let Some(description) = server.description.as_deref().filter(|d| !d.is_empty()) {
            entry.insert("description".into(), json!(description));
        }
        if !server.enabled {
            entry.insert("enabled".into(), json!(false));
        }
        if !server.allowed_tools.is_empty() {
            entry.insert("allowedTools".into(), json!(server.allowed_tools));
        }
        if !server.disallowed_tools.is_empty() {
            entry.insert("disallowedTools".into(), json!(server.disallowed_tools));
        }
        if let Some(secs) = server.timeout_secs {
            entry.insert("timeoutSecs".into(), json!(secs));
        }
        out.insert(server.name.clone(), Value::Object(entry));
    }

    json!({ ROOT_KEY: out })
}

/// Reads an entry's credential block: `env` for a stdio server, `headers` for
/// an HTTP one, refusing the other spelling.
///
/// `None` when the block is absent, which leaves stored values alone.
fn parse_credentials(
    name: &str,
    entry: &Map<String, Value>,
    transport: &Transport,
) -> std::result::Result<Option<BTreeMap<String, String>>, String> {
    // `env` is the stdio spelling and `headers` the HTTP one; both land in the
    // same credential table, because that is what the transport reads.
    let credential_field = match transport {
        Transport::Stdio => "env",
        _ => "headers",
    };
    let wrong_field = match transport {
        Transport::Stdio => "headers",
        _ => "env",
    };
    if entry.contains_key(wrong_field) {
        return Err(format!(
            "`{name}` takes `{credential_field}`, not `{wrong_field}`, for a `{}` server",
            match transport {
                Transport::Stdio => "command",
                _ => "url",
            }
        ));
    }
    let credentials = match entry.get(credential_field) {
        None | Some(Value::Null) => None,
        Some(Value::Object(values)) => {
            let mut out = BTreeMap::new();
            for (key, value) in values {
                let key = key.trim();
                if key.is_empty() {
                    return Err(format!("`{name}`.{credential_field} has an empty key"));
                }
                if key.starts_with(INTERNAL_KEY_PREFIX) {
                    return Err(format!(
                        "`{name}`.{credential_field}.{key} — names beginning `{INTERNAL_KEY_PREFIX}` are reserved"
                    ));
                }
                let Some(value) = value.as_str() else {
                    return Err(format!("`{name}`.{credential_field}.{key} holds a string"));
                };
                out.insert(key.to_string(), value.to_string());
            }
            Some(out)
        }
        Some(_) => {
            return Err(format!(
                "`{name}`.{credential_field} maps a name to a string value"
            ));
        }
    };
    Ok(credentials)
}

/// A string field that may be absent, refusing a blank or non-string one.
fn optional_string(
    name: &str,
    entry: &Map<String, Value>,
    key: &str,
) -> std::result::Result<Option<String>, String> {
    match entry.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(format!("`{name}`.{key} is empty"));
            }
            Ok(Some(trimmed.to_string()))
        }
        Some(_) => Err(format!("`{name}`.{key} holds a string")),
    }
}

/// Whether a declaration describes the same dial as an install row.
///
/// Credentials are not compared: they are write-only, so a declaration cannot
/// know what is stored, and a re-save must not read as a change.
#[must_use]
pub fn same_dial(declared: &Declared, server: &InstalledServer) -> bool {
    declared.transport == server.transport
        && declared.command == server.command
        && declared.args == server.args
        && declared.description.as_deref().unwrap_or_default()
            == server.description.as_deref().unwrap_or_default()
        && declared.enabled == server.enabled
}

/// Builds the install row a declaration becomes.
///
/// `server_id` is the caller's: a fresh id for a new server, the existing one
/// for a server being rewritten in place, so the row the frontend and the
/// connection map address stays the same row.
#[must_use]
pub fn to_installed(declared: &Declared, server_id: String, installed_at: i64) -> InstalledServer {
    let command_kind = crate::transport::stdio::spawn_env::required_runtime(&declared.command);
    InstalledServer {
        server_id,
        qualified_name: declared.name.clone(),
        display_name: declared.name.clone(),
        description: declared.description.clone(),
        icon_url: None,
        command_kind,
        command: declared.command.clone(),
        args: declared.args.clone(),
        env_keys: Vec::new(),
        config: None,
        installed_at,
        last_connected_at: None,
        transport: declared.transport.clone(),
        enabled: declared.enabled,
    }
}

/// Merges a write's credentials over what is stored.
///
/// A non-empty value replaces; an empty string removes; a name the write did
/// not mention is kept. Internal bookkeeping is never touched.
#[must_use]
pub fn merge_credentials(
    stored: &BTreeMap<String, String>,
    written: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut merged = stored.clone();
    for (key, value) in written {
        if value.is_empty() {
            merged.remove(key);
        } else {
            merged.insert(key.clone(), value.clone());
        }
    }
    merged
}
