//! Tests for [`SecretScrubber`]: which secrets it learns from a server's
//! credentials and endpoint, and what it leaves alone.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use tinymcp_bus::{HttpHeader, McpAuthConfig};

use super::scrub::SecretScrubber;

#[test]
fn scrubber_redacts_url_encoded_secrets_and_ignores_empty_values() {
    let scrubber = SecretScrubber::new(
        &McpAuthConfig::BearerToken {
            token: "a b/c".into(),
        },
        "https://example.com/mcp",
    );
    assert_eq!(
        scrubber.scrub("x a%20b%2Fc y a b/c"),
        "x [redacted] y [redacted]"
    );

    let empty = SecretScrubber::new(
        &McpAuthConfig::BearerToken { token: "  ".into() },
        "https://example.com/mcp?",
    );
    assert!(empty.secrets.is_empty());
    assert_eq!(empty.scrub("unchanged"), "unchanged");
}

#[test]
fn scrubber_collects_url_userinfo_credentials() {
    let scrubber =
        SecretScrubber::new(&McpAuthConfig::None, "https://short:pw@example.com/mcp");
    assert_eq!(
        scrubber.scrub("short pw shortpw"),
        "[redacted] [redacted] [redacted][redacted]"
    );
}

#[test]
fn scrubber_does_not_globally_redact_ordinary_short_query_values() {
    let scrubber = SecretScrubber::new(
        &McpAuthConfig::None,
        "https://example.com/mcp?v=2&format=json&api_token=abcdef1234",
    );
    assert_eq!(
        scrubber.scrub("v=2 and format=json are unrelated"),
        "v=2 and format=json are unrelated"
    );
    assert_eq!(
        scrubber.scrub("leaked abcdef1234 here"),
        "leaked [redacted] here"
    );
}

#[test]
fn scrubber_redacts_credential_query_value() {
    let scrubber = SecretScrubber::new(
        &McpAuthConfig::None,
        "https://example.com/mcp?credential=private12345",
    );
    assert_eq!(
        scrubber.scrub("server echoed private12345"),
        "server echoed [redacted]"
    );
}

#[test]
fn scrubber_redacts_short_query_credentials_even_inside_other_text() {
    let scrubber = SecretScrubber::new(
        &McpAuthConfig::None,
        "https://example.com/mcp?api_key=abc&v=2",
    );
    assert_eq!(
        scrubber.scrub("abc is a credential; prefixabc also contains it"),
        "[redacted] is a credential; prefix[redacted] also contains it"
    );
    assert_eq!(scrubber.scrub("v=2"), "v=2");
}

#[test]
fn short_query_credentials_do_not_rewrite_json_structure_keys() {
    let scrubber = SecretScrubber::new(
        &McpAuthConfig::None,
        "https://example.com/mcp?api_key=abc",
    );
    let mut value = json!({ "prefixabc": "prefixabc", "abc": "abc" });
    scrubber.scrub_value(&mut value);
    assert_eq!(value["prefixabc"], "prefix[redacted]");
    assert_eq!(value["[redacted]"], "[redacted]");
    assert!(value.get("prefix[redacted]").is_none());
}

#[test]
fn short_auth_values_do_not_rewrite_unrelated_words() {
    let scrubber = SecretScrubber::new(
        &McpAuthConfig::Basic {
            username: "abc".into(),
            password: "private12345".into(),
        },
        "https://example.com/mcp",
    );
    assert_eq!(
        scrubber.scrub("abc identifies the user; alphabet is unrelated"),
        "[redacted] identifies the user; alphabet is unrelated"
    );
}

#[test]
fn scrub_value_keeps_both_entries_when_keys_collide_after_redaction() {
    let scrubber = SecretScrubber::new(
        &McpAuthConfig::Headers {
            headers: vec![
                HttpHeader::new("one", "first-secret"),
                HttpHeader::new("two", "second-secret"),
            ],
        },
        "https://example.com/mcp",
    );
    let mut value = json!({
        "first-secret": "a",
        "second-secret": "b",
    });
    scrubber.scrub_value(&mut value);
    let map = value.as_object().expect("object");
    assert_eq!(map.len(), 2, "{value}");
    assert_eq!(map.get("[redacted]"), Some(&json!("a")));
    assert_eq!(map.get("[redacted] (2)"), Some(&json!("b")));
}

