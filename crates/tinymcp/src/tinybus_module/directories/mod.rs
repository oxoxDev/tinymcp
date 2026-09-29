//! Serving more than one data directory from one loaded module.
//!
//! # The problem
//!
//! [`ModuleConfig`](super::ModuleConfig) carries one `data_dir`, supplied once
//! at load. A host whose storage moves while it runs — login rewrites the
//! active-user marker and the workspace changes from `users/local` to
//! `users/<id>`, logout reverses it — would keep being answered from the store
//! it had at load. That is worse than an error: the answer is wrong rather than
//! absent.
//!
//! # The seam
//!
//! The root object grows an `Open(data_dir)` member that serves a new object for
//! that directory and returns its path. Each such object is an ordinary
//! [`McpService`](super::McpService) exporting the identical interface over its
//! own store and audit log, so the contract does not change: a caller talking to
//! one path talks to one directory. The root object keeps answering for the
//! directory the module was loaded with.
//!
//! # One object per directory
//!
//! Opening the same directory twice returns the same path, and the root's own
//! directory maps to the root path. Two live handles to one `SQLite` file is
//! how migrations race each other, so the map is held across the whole open
//! rather than only the lookup.
//!
//! Directories are matched by the exact absolute path the caller supplied. A
//! host should pass the same spelling each time; the module does not resolve
//! symlinks, because a path that does not exist yet has nothing to resolve.
//!
//! # Bounds
//!
//! The bus cannot unserve an object a caller may still hold, so nothing here
//! ever closes one. [`MAX_OPEN_DIRECTORIES`] caps how many a process serves, far
//! above what a per-user host needs, so a caller opening directories in a loop
//! is refused by name rather than exhausting file descriptors.
//!
//! Each opened directory also runs its own boot pass and supervisor, so a user's
//! installed servers are connected after a login just as they were at load.

mod types;

pub(super) use types::DirectoryOpener;
#[cfg(test)]
pub(super) use types::MAX_OPEN_DIRECTORIES;

#[cfg(test)]
mod test;
