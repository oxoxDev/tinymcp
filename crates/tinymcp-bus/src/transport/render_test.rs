//! Unit tests for endpoint redaction and tool-reply rendering.
//!
//! [`redact_endpoint`] gets the most attention here. It is the single control
//! standing between an MCP endpoint — which routinely carries an API key in a
//! query parameter — and every log line, error message, and telemetry event
//! the module and a host produce, so its failure modes are worth enumerating
//! rather than sampling.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{redact_endpoint, render_tool_result};
use serde_json::json;

// ---------------------------------------------------------------------------
// redact_endpoint
// ---------------------------------------------------------------------------

#[test]
fn redaction_keeps_the_scheme_and_authority() {
    assert_eq!(
        redact_endpoint("https://example.test"),
        "https://example.test"
    );
    assert_eq!(
        redact_endpoint("http://example.test"),
        "http://example.test"
    );
}

#[test]
fn redaction_keeps_a_port() {
    // The port is part of the authority and is often the only thing telling two
    // local servers apart in a log.
    assert_eq!(
        redact_endpoint("http://127.0.0.1:8080/mcp"),
        "http://127.0.0.1:8080"
    );
}

#[test]
fn redaction_drops_the_path_query_and_fragment() {
    // The query is where an API key most often lives.
    assert_eq!(
        redact_endpoint("https://example.test/mcp?api_key=secret"),
        "https://example.test"
    );
    assert_eq!(
        redact_endpoint("https://example.test/deep/path"),
        "https://example.test"
    );
    assert_eq!(
        redact_endpoint("https://example.test#fragment"),
        "https://example.test"
    );
}

#[test]
fn redaction_refuses_a_url_carrying_userinfo() {
    // Not "strips the userinfo" — refuses the whole URL. A host that put
    // credentials in the authority may have put more than one thing there.
    assert_eq!(
        redact_endpoint("https://user:password@example.test/mcp"),
        "<redacted>"
    );
    assert_eq!(redact_endpoint("https://token@example.test"), "<redacted>");
}

#[test]
fn redaction_refuses_a_scheme_it_does_not_recognise() {
    for endpoint in [
        "file:///etc/passwd",
        "ftp://example.test",
        "javascript:alert(1)",
        "example.test",
        "",
    ] {
        assert_eq!(
            redact_endpoint(endpoint),
            "<redacted>",
            "{endpoint} was not refused"
        );
    }
}

#[test]
fn redaction_refuses_a_url_with_no_authority() {
    assert_eq!(redact_endpoint("https://"), "<redacted>");
    assert_eq!(redact_endpoint("https:///path"), "<redacted>");
}

#[test]
fn redaction_ignores_surrounding_whitespace() {
    assert_eq!(
        redact_endpoint("  https://example.test/mcp \n"),
        "https://example.test"
    );
}

#[test]
fn redaction_is_case_sensitive_about_the_scheme() {
    // Refusing an uppercase scheme is the conservative reading: this function
    // decides what is safe to print, so an input it does not recognise exactly
    // should be refused rather than guessed at.
    assert_eq!(redact_endpoint("HTTPS://example.test"), "<redacted>");
}

// ---------------------------------------------------------------------------
// render_tool_result
// ---------------------------------------------------------------------------

#[test]
fn a_single_text_block_renders_as_its_text() {
    let rendered = render_tool_result(&json!({
        "content": [{ "type": "text", "text": "sunny" }],
    }));

    assert!(!rendered.is_error);
    assert_eq!(rendered.text(), "sunny");
}

#[test]
fn several_text_blocks_are_separated_by_a_blank_line() {
    let rendered = render_tool_result(&json!({
        "content": [
            { "type": "text", "text": "first" },
            { "type": "text", "text": "second" },
        ],
    }));

    assert_eq!(rendered.text(), "first\n\nsecond");
}

#[test]
fn non_text_blocks_are_skipped_when_text_is_present() {
    let rendered = render_tool_result(&json!({
        "content": [
            { "type": "image", "data": "..." },
            { "type": "text", "text": "caption" },
        ],
    }));

    assert_eq!(rendered.text(), "caption");
}

#[test]
fn a_reply_with_no_text_renders_as_its_own_json() {
    // A structured-only result should say something rather than nothing.
    let reply = json!({ "structuredContent": { "temperature": 21 } });
    let rendered = render_tool_result(&reply);

    assert_eq!(rendered.text(), reply.to_string());
    assert!(rendered.text().contains("temperature"));
}

#[test]
fn an_empty_reply_renders_as_its_own_json() {
    let rendered = render_tool_result(&json!({}));
    assert_eq!(rendered.text(), "{}");
}

#[test]
fn a_reply_flagged_is_error_renders_as_an_error_result() {
    let rendered = render_tool_result(&json!({
        "isError": true,
        "content": [{ "type": "text", "text": "city not found" }],
    }));

    assert!(rendered.is_error);
    assert_eq!(rendered.text(), "city not found");
}

#[test]
fn a_non_boolean_is_error_is_treated_as_no_error() {
    // Servers send odd things. Treating a malformed flag as "failed" would
    // report a failure that did not happen.
    let rendered = render_tool_result(&json!({
        "isError": "yes",
        "content": [{ "type": "text", "text": "fine" }],
    }));

    assert!(!rendered.is_error);
}

#[test]
fn a_reply_whose_content_is_not_an_array_renders_as_its_own_json() {
    let reply = json!({ "content": "not an array" });
    assert_eq!(render_tool_result(&reply).text(), reply.to_string());
}
