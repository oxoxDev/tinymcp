//! Persistence for the write-audit log.

mod types;

pub use types::AuditStore;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
