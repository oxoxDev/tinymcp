//! The user's MCP servers as one `mcp.json` document.
//!
//! Installing a server need not be a catalog action. A user who found a
//! server declares it in the same `{ "mcpServers": { … } }` shape every desktop
//! MCP client already uses, and this module is the contract for that document
//! and the reconciliation of the install store against it.
//!
//! # One store, two notations
//!
//! The install rows and the document are the same [`Store`](super::Store).
//! [`render`] projects the store into the document; [`parse`] reads a document
//! back into declarations; and
//! [`McpRegistry::apply_config_doc`](super::McpRegistry::apply_config_doc)
//! applies the difference. Neither is an import format that can drift from
//! what is configured.
//!
//! # A host that keeps its own store
//!
//! A host that holds declarations itself rather than in the install store
//! reads the same document through [`parse_with`], which also accepts
//! `allowedTools`, `disallowedTools` and `timeoutSecs`, carries fields the
//! host registers in [`ParseOptions::host_fields`] through verbatim, and can
//! drop a bad entry instead of refusing the document. [`render_declared`]
//! writes those declarations back. [`parse`] keeps refusing the extension
//! fields, because the install store has nowhere to put them and accepting
//! them there would drop them silently.
//!
//! # Credentials are write-only
//!
//! A stdio server's `env` and an HTTP server's `headers` are secrets. They are
//! accepted on write and stored in the credential table; a read never echoes a
//! value, only the names (`envKeys`) and whether anything is stored
//! (`authConfigured`). An entry saved *without* an `env`/`headers` block
//! therefore keeps its stored credentials — a round trip cannot silently
//! de-authenticate a server. A key set to the empty string removes that one
//! stored value.
//!
//! # What it does not do
//!
//! **No events and no background connects.** Applying a document reports what
//! it installed, updated and removed, and which servers want connecting, in a
//! [`ConfigApplyReport`]. Publishing that, and dialling the servers without
//! holding up the editor that saved the document, are the host's.

mod apply;
mod document;
mod types;

pub use document::{
    ROOT_KEY, merge_credentials, parse, parse_with, render, render_declared, same_dial,
    to_installed,
};
pub use types::{
    AppliedServer, ConfigApplyReport, Declared, ParseOptions, ParseReport, RejectedEntry,
};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;

#[cfg(test)]
#[path = "mod_parse_with_tests.rs"]
mod parse_with_test;
