//! The declarations a document makes, and what applying one did.

use std::collections::BTreeMap;

use serde_json::Value;
use tinymcp_bus::Transport;

/// One server as the document declares it, after validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The document key; doubles as the store's `qualified_name`.
    pub name: String,
    /// How the server is dialled.
    pub transport: Transport,
    /// The launcher, for stdio; empty for HTTP.
    pub command: String,
    /// Arguments to the launcher, for stdio.
    pub args: Vec<String>,
    /// Credentials named in this write: stdio `env` or HTTP `headers`.
    ///
    /// `None` when the block was absent, which leaves stored values alone.
    pub credentials: Option<BTreeMap<String, String>>,
    /// Free text shown beside the row.
    pub description: Option<String>,
    /// Whether the server is brought up and exposed.
    pub enabled: bool,
    /// `allowedTools`: the only tools the server may expose. Empty means every
    /// tool. Read by [`parse_with`](super::parse_with) only.
    pub allowed_tools: Vec<String>,
    /// `disallowedTools`: tools that are always blocked. Read by
    /// [`parse_with`](super::parse_with) only.
    pub disallowed_tools: Vec<String>,
    /// `timeoutSecs`: how long a call may take, when the entry says. Read by
    /// [`parse_with`](super::parse_with) only.
    pub timeout_secs: Option<u64>,
    /// The host's own fields, by document key, verbatim.
    ///
    /// Only the names a host registers in [`ParseOptions::host_fields`] land
    /// here; a `null` value reads as absent.
    pub host_fields: BTreeMap<String, Value>,
}

/// How [`parse_with`](super::parse_with) reads a document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseOptions<'a> {
    /// Entry fields the host understands beyond the standard ones, by
    /// document key (`"readOnlyTools"`, say). Their values are carried in
    /// [`Declared::host_fields`] untouched; validating them is the host's. A
    /// name that is already a standard field is read as that field.
    pub host_fields: &'a [&'a str],
    /// Whether a refused entry is dropped and reported instead of refusing
    /// the whole document.
    ///
    /// For documents a user did not write against this host, such as a
    /// bundle copied from a vendor's instructions. A document whose root is
    /// unreadable is still refused.
    pub lenient: bool,
}

/// What [`parse_with`](super::parse_with) read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseReport {
    /// The entries that were read, in document-key order.
    pub declared: Vec<Declared>,
    /// The entries that were dropped, in document-key order. Always empty
    /// unless [`ParseOptions::lenient`] is set.
    pub rejected: Vec<RejectedEntry>,
}

/// One entry a lenient read dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedEntry {
    /// The entry's key, trimmed.
    pub name: String,
    /// Why it was dropped, naming the entry and the field.
    pub detail: String,
}

/// One server a document application touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedServer {
    /// The install's identifier.
    pub server_id: String,
    /// The document key, which is the install's qualified name.
    pub name: String,
}

/// What applying a document changed.
///
/// Each list is in document-key order (removals in store order), so a host
/// that reports them reports the same thing for the same change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigApplyReport {
    /// Servers the document declared that were not installed, now installed.
    pub installed: Vec<AppliedServer>,
    /// Installed servers whose dial was rewritten in place or whose stored
    /// credentials changed. Their identifier is unchanged.
    pub updated: Vec<AppliedServer>,
    /// Installed servers the document no longer declares, now uninstalled.
    pub removed: Vec<AppliedServer>,
    /// Enabled servers that were installed or updated and want connecting, by
    /// identifier. Nothing has dialled them; a host connects them, typically in
    /// the background so whoever saved the document does not wait on it.
    pub connect_queued: Vec<String>,
}
