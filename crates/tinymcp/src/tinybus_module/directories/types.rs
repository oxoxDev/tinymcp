//! The directory registry behind `Open`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use std::future::Future;
use std::task::Poll;

use tinybus::Connection;
use tinymcp_bus::{DIRECTORY_OBJECT_PREFIX, McpClientConfig, OBJECT_PATH};

use crate::error::{Error, Result};
use crate::registry::SupervisorConfig;
use crate::tinybus_module::{McpService, ModuleConfig};

/// How many directories one module process will serve.
///
/// See the module note on why an object is never unserved.
pub(in crate::tinybus_module) const MAX_OPEN_DIRECTORIES: usize = 32;

/// The root object's ability to serve additional directories.
pub(in crate::tinybus_module) struct DirectoryOpener {
    connection: Connection,
    /// Everything but the directory, shared by every object this opens: the
    /// servers, credentials, identity, and proxy are the host's, not the
    /// user's.
    client: McpClientConfig,
    supervisor: SupervisorConfig,
    /// Directory to object path.
    ///
    /// A `tokio` mutex held across the whole open: a lock released between the
    /// lookup and the insert would let two callers through, and that is the
    /// double open the map exists to prevent.
    served: tokio::sync::Mutex<HashMap<PathBuf, String>>,
}

impl std::fmt::Debug for DirectoryOpener {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DirectoryOpener")
            .field("supervisor", &self.supervisor)
            .finish_non_exhaustive()
    }
}

impl DirectoryOpener {
    /// Builds the opener for a module loaded with `config`.
    ///
    /// The load-time directory, when there is one, is already served at the
    /// root path, so it is recorded there rather than opened a second time.
    pub(in crate::tinybus_module) fn new(
        connection: Connection,
        config: &ModuleConfig,
        supervisor: SupervisorConfig,
    ) -> Result<Self> {
        let mut served = HashMap::new();
        if let Some(dir) = &config.data_dir {
            let absolute = std::path::absolute(dir).map_err(|source| Error::StoreIo {
                path: dir.clone(),
                source: Box::new(source),
            })?;
            served.insert(absolute, OBJECT_PATH.to_string());
        }

        Ok(Self {
            connection,
            client: config.client.clone(),
            supervisor,
            served: tokio::sync::Mutex::new(served),
        })
    }

    /// Serves `data_dir` as its own object and returns the object path.
    ///
    /// Idempotent per directory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] when the path is empty, relative, or
    /// the module already serves [`MAX_OPEN_DIRECTORIES`] directories,
    /// [`Error::Bus`] when the object cannot be served, plus whatever building
    /// the service returns. A failed open leaves nothing
    /// recorded, so it is retried rather than answered with a dead path.
    pub(in crate::tinybus_module) async fn open(&self, data_dir: &str) -> Result<String> {
        let dir = Path::new(data_dir);
        // Neither message echoes the value: it is a user's path.
        if data_dir.is_empty() || !dir.is_absolute() {
            return Err(Error::invalid_argument(
                "a data directory must be a non-empty absolute path",
            ));
        }

        let mut served = self.served.lock().await;
        if let Some(existing) = served.get(dir) {
            return Ok(existing.clone());
        }

        if served.len() >= MAX_OPEN_DIRECTORIES {
            return Err(Error::invalid_argument(format!(
                "this module already serves the maximum of {MAX_OPEN_DIRECTORIES} data directories"
            )));
        }

        let service = McpService::new(&ModuleConfig {
            data_dir: Some(dir.to_path_buf()),
            client: self.client.clone(),
        })?
        .with_maintenance(self.supervisor.clone())
        .await;

        // Numbered rather than derived from the directory: a path is arbitrary
        // user data and object-path elements are not. The map never shrinks,
        // so its length is never reused.
        let path = format!("{DIRECTORY_OBJECT_PREFIX}/d{}", served.len());
        let object_path = path.as_str().try_into().map_err(|error| Error::Bus {
            detail: format!("could not name an object for the directory: {error}"),
        })?;
        serve_without_blocking_the_reply(&self.connection, object_path, service).await?;

        served.insert(dir.to_path_buf(), path.clone());
        Ok(path)
    }
}

/// Exports `service` at `path` from inside a method call.
///
/// `TinyBus` dispatches a call while holding the read side of the lock over its
/// object tree, and [`Connection::serve_at`] takes the write side. A call that
/// awaited `serve_at` directly would therefore wait for a lock only its own
/// return releases, and the caller would see a timeout.
///
/// So the export is polled once here — which is enough to queue its write
/// request — and, when it cannot finish yet, handed to a task to complete once
/// this call has returned and released the lock. The queue is what makes that
/// safe to reply on: the lock is fair, so a call the caller makes to the new
/// path after receiving the reply is a reader arriving *behind* the queued
/// write and waits for the export rather than racing it.
///
/// The fair-queue behavior is `tokio`'s documented policy for its `RwLock`. If
/// `TinyBus` stops holding the tree lock across a handler, the first poll simply
/// completes and the task is never spawned.
pub(super) async fn serve_without_blocking_the_reply(
    connection: &Connection,
    path: tinybus::ObjectPath,
    service: McpService,
) -> Result<()> {
    let mut serve = Box::pin({
        let connection = connection.clone();
        async move { connection.serve_at(path, service).await }
    });

    let first_poll =
        std::future::poll_fn(|context| Poll::Ready(serve.as_mut().poll(context))).await;
    match first_poll {
        Poll::Ready(outcome) => outcome.map_err(|error| Error::Bus {
            detail: format!("could not serve the directory: {error}"),
        }),
        Poll::Pending => {
            tokio::spawn(async move {
                if let Err(error) = serve.await {
                    tracing::error!("could not serve an opened data directory: {error}");
                }
            });
            Ok(())
        }
    }
}
