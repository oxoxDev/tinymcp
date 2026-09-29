//! Reconciling the install store against a document.

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use super::document::{merge_credentials, parse, render, same_dial, to_installed};
use super::types::{AppliedServer, ConfigApplyReport};
use crate::error::{Error, Result};
use crate::registry::ops::types::now_ms;
use crate::registry::{McpRegistry, Store};
use tinymcp_bus::InstalledServer;

impl McpRegistry {
    /// Renders the install store as the `mcp.json` document.
    ///
    /// Credential *names* ride along as `envKeys`; values never do.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Store`] when the installs cannot be listed.
    pub fn render_config_doc(&self) -> Result<Value> {
        let installed = self.installed_list()?;
        let stored_keys = stored_credential_names(self.store(), &installed);
        Ok(render(&installed, &stored_keys))
    }

    /// Replaces the install store with what `doc` declares.
    ///
    /// A replace, not a merge: a server absent from the document is
    /// uninstalled, one not yet installed is added, and one whose dial changed
    /// is rewritten in place under the same `server_id`. Credentials are the
    /// exception — a declaration that says nothing about them leaves the stored
    /// ones alone, because a read never shows them and a round trip must not
    /// wipe them.
    ///
    /// Nothing is dialled. Every added or updated server that is enabled is
    /// listed in [`ConfigApplyReport::connect_queued`] for the host to connect;
    /// a server whose credentials alone changed has its live session dropped
    /// first, because a new credential only takes effect on a fresh one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfigDoc`] when the document is refused — by
    /// [`parse`], or because two keys name one server once trimmed — before
    /// anything changes, and [`Error::Store`] when the store fails part way.
    pub async fn apply_config_doc(&self, doc: &Value) -> Result<ConfigApplyReport> {
        let declared = parse(doc)?;

        // Keys are trimmed on the way in, so two spellings of one name can
        // collide after the fact even though a JSON object cannot repeat a key.
        let mut seen = HashSet::new();
        for entry in &declared {
            if !seen.insert(entry.name.as_str()) {
                return Err(Error::ConfigDoc {
                    detail: format!("`{}` is declared twice", entry.name),
                });
            }
        }

        let store = self.store();
        let installed = self.installed_list()?;
        let mut report = ConfigApplyReport::default();

        // Removals first, so a server renamed in the document (drop one key,
        // add another) does not briefly hold two live connections to one
        // process.
        for server in &installed {
            if declared
                .iter()
                .any(|entry| entry.name == server.qualified_name)
            {
                continue;
            }
            tracing::debug!(
                server_id = %server.server_id,
                name = %server.qualified_name,
                "removing a server absent from the document"
            );
            self.uninstall(&server.server_id).await?;
            report.removed.push(applied(server));
        }

        for entry in &declared {
            let existing = installed
                .iter()
                .find(|server| server.qualified_name == entry.name);

            let Some(existing) = existing else {
                let server_id = uuid::Uuid::new_v4().to_string();
                let row = to_installed(entry, server_id.clone(), now_ms());
                store.insert_server(&row)?;
                if let Some(written) = &entry.credentials {
                    let merged = merge_credentials(&BTreeMap::new(), written);
                    write_credentials(store, &server_id, &merged)?;
                }
                tracing::debug!(
                    server_id = %server_id,
                    name = %entry.name,
                    transport = entry.transport.dispatch_kind(),
                    "added a server"
                );
                if entry.enabled {
                    report.connect_queued.push(server_id.clone());
                }
                report.installed.push(AppliedServer {
                    server_id,
                    name: entry.name.clone(),
                });
                continue;
            };

            let server_id = existing.server_id.clone();
            let dial_changed = !same_dial(entry, existing);
            let mut stored = store.load_env_values(&server_id)?;

            if dial_changed {
                // The store has no "update the dial" statement, so the row is
                // rewritten under the same id. Deleting it cascades the
                // credentials, which is why they were read first.
                self.connections().disconnect(&server_id).await;
                store.delete_server(&server_id)?;
                let row = to_installed(entry, server_id.clone(), existing.installed_at);
                store.insert_server(&row)?;
                write_credentials(store, &server_id, &stored)?;
                tracing::debug!(
                    server_id = %server_id,
                    name = %entry.name,
                    "rewrote a server whose dial changed"
                );
            }

            let credentials_changed = match &entry.credentials {
                Some(written) if !written.is_empty() => {
                    let merged = merge_credentials(&stored, written);
                    let changed = merged != stored;
                    if changed {
                        write_credentials(store, &server_id, &merged)?;
                        stored = merged;
                    }
                    changed
                }
                _ => false,
            };
            // Values are not held a moment longer than the comparison needs.
            drop(stored);

            if dial_changed || credentials_changed {
                if entry.enabled {
                    if credentials_changed && !dial_changed {
                        // A new credential only takes effect on a fresh
                        // session.
                        self.connections().disconnect(&server_id).await;
                    }
                    report.connect_queued.push(server_id.clone());
                }
                report.updated.push(AppliedServer {
                    server_id,
                    name: entry.name.clone(),
                });
            }
        }

        Ok(report)
    }
}

/// The report entry for an install.
fn applied(server: &InstalledServer) -> AppliedServer {
    AppliedServer {
        server_id: server.server_id.clone(),
        name: server.qualified_name.clone(),
    }
}

/// The credential *names* stored for each server, keyed by `server_id`.
///
/// Values are loaded and dropped here; only the names leave this function. A
/// server whose credentials cannot be read renders as having none rather than
/// failing the whole document.
fn stored_credential_names(
    store: &Store,
    installed: &[InstalledServer],
) -> BTreeMap<String, Vec<String>> {
    installed
        .iter()
        .map(|server| {
            let names = store
                .load_env_values(&server.server_id)
                .map(|values| values.into_keys().collect())
                .unwrap_or_default();
            (server.server_id.clone(), names)
        })
        .collect()
}

/// Stores a server's credentials and keeps the row's name list in step.
fn write_credentials(
    store: &Store,
    server_id: &str,
    values: &BTreeMap<String, String>,
) -> Result<()> {
    store.set_env_values(server_id, values)?;
    let names: Vec<String> = values.keys().cloned().collect();
    store.update_env_keys(server_id, &names)
}
