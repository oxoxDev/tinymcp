//! Unit tests for the error-name table.
//!
//! The names are strings a host matches on, so what matters is that they are
//! well-formed, distinct, and stable. That each one is *produced* by the
//! module's `Error` is asserted in `crates/tinymcp`, which can see both sides.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{ALL, PREFIX, REGISTRY_TIMEOUT, UNAUTHORIZED};

#[test]
fn every_name_shares_the_prefix_and_has_a_suffix() {
    for name in ALL {
        let suffix = name
            .strip_prefix(PREFIX)
            .unwrap_or_else(|| panic!("{name} does not start with {PREFIX}"));
        assert!(!suffix.is_empty(), "{name} has no suffix");
        assert!(
            suffix.chars().all(|c| c.is_ascii_alphanumeric()),
            "{name} has something other than ASCII alphanumerics after the prefix"
        );
    }
}

#[test]
fn no_name_is_listed_twice() {
    let mut sorted = ALL.to_vec();
    sorted.sort_unstable();
    let mut deduplicated = sorted.clone();
    deduplicated.dedup();

    assert_eq!(sorted, deduplicated);
}

#[test]
fn the_prefix_is_distinct_from_the_bus_own_error_namespace() {
    // TinyBus's own failures travel under `ai.tinyhumans.tinybus.Error.`. A
    // module name in that namespace would be indistinguishable from one.
    assert!(!PREFIX.starts_with("ai.tinyhumans.tinybus."));
}

#[test]
fn the_unauthorized_name_is_pinned() {
    // A host anchors its needs-auth classification on this string.
    assert_eq!(UNAUTHORIZED, "ai.tinyhumans.tinymcp.Error.Unauthorized");
}

#[test]
fn the_registry_timeout_name_is_pinned() {
    assert_eq!(
        REGISTRY_TIMEOUT,
        "ai.tinyhumans.tinymcp.Error.RegistryTimeout"
    );
}
