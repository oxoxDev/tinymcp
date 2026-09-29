//! Unit tests for per-session client provenance.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::{ClientSession, object_keys};

#[test]
fn normalize_client_name_accepts_ascii_client_names() {
    for (raw, expected) in [
        ("Claude Desktop", Some("claude-desktop")),
        ("Cursor", Some("cursor")),
        ("Windsurf", Some("windsurf")),
        ("  Zed: Nightly  ", Some("zed-nightly")),
        ("--VS  Code--", Some("vs-code")),
        ("会议记录", None),
        ("", None),
        ("   ", None),
    ] {
        assert_eq!(
            ClientSession::normalize_client_name(raw).as_deref(),
            expected,
            "raw client name: {raw:?}"
        );
    }
}

#[test]
fn a_fresh_session_reports_the_bare_prefix() {
    assert_eq!(ClientSession::new("mcp").source_type(), "mcp");
    assert_eq!(ClientSession::new("acme").source_type(), "acme");
}

#[test]
fn initialize_captures_the_normalized_client_name_under_the_prefix() {
    let mut session = ClientSession::new("acme");
    session.observe_initialize_params(&json!({"clientInfo": {"name": "Claude Desktop"}}));
    assert_eq!(session.source_type(), "acme:claude-desktop");
}

#[test]
fn a_missing_blank_or_unusable_name_locks_the_bare_prefix() {
    for params in [
        json!({}),
        json!(null),
        json!({"clientInfo": "not an object"}),
        json!({"clientInfo": {"name": 7}}),
        json!({"clientInfo": {"name": ""}}),
        json!({"clientInfo": {"name": "   "}}),
    ] {
        let mut session = ClientSession::new("mcp");
        session.observe_initialize_params(&params);
        assert_eq!(session.source_type(), "mcp", "params: {params}");
    }
}

#[test]
fn the_first_observation_wins() {
    // A named client cannot be renamed by a later initialize…
    let mut named = ClientSession::new("mcp");
    named.observe_initialize_params(&json!({"clientInfo": {"name": "Claude Desktop"}}));
    named.observe_initialize_params(&json!({"clientInfo": {"name": "Cursor"}}));
    named.observe_initialize_params(&json!({}));
    assert_eq!(named.source_type(), "mcp:claude-desktop");

    // …and an anonymous first initialize freezes the bare prefix.
    let mut anonymous = ClientSession::new("mcp");
    anonymous.observe_initialize_params(&json!({}));
    anonymous.observe_initialize_params(&json!({"clientInfo": {"name": "Cursor"}}));
    assert_eq!(anonymous.source_type(), "mcp");
}

#[test]
fn diagnostic_keys_are_sorted_and_empty_for_non_objects() {
    assert_eq!(object_keys(&json!({"b": 1, "a": 2})), ["a", "b"]);
    assert!(object_keys(&json!(null)).is_empty());
    assert!(object_keys(&json!([1])).is_empty());
}
