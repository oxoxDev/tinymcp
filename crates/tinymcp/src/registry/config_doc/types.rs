//! The declarations a document makes, and what applying one did.

use std::collections::BTreeMap;

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
