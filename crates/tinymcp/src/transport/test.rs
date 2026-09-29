//! Unit tests for the shared transport surface.
//!
//! [`redact_endpoint`] gets the most attention here. It is the single control
//! standing between an MCP endpoint — which routinely carries an API key in a
//! query parameter — and every log line, error message, and telemetry event
//! this crate produces, so its failure modes are worth enumerating rather than
//! sampling.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::validate_protocol_version;
use crate::Error;
use tinymcp_bus::{LATEST_PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS};

// ---------------------------------------------------------------------------
// validate_protocol_version
// ---------------------------------------------------------------------------

#[test]
fn every_supported_version_validates() {
    for version in SUPPORTED_PROTOCOL_VERSIONS {
        validate_protocol_version(version)
            .unwrap_or_else(|_| panic!("{version} is listed as supported but did not validate"));
    }
}

#[test]
fn the_latest_version_validates() {
    validate_protocol_version(LATEST_PROTOCOL_VERSION).expect("the latest version validates");
}

#[test]
fn an_unlisted_version_is_rejected_and_names_itself() {
    let error = validate_protocol_version("1999-01-01").expect_err("an unlisted version");

    match error {
        Error::UnsupportedProtocolVersion { version } => assert_eq!(version, "1999-01-01"),
        other => panic!("expected an unsupported-version error, got {other:?}"),
    }
}

#[test]
fn an_empty_version_is_rejected() {
    assert!(validate_protocol_version("").is_err());
}

#[test]
fn a_near_miss_version_is_rejected() {
    // Whitespace, a different separator, or a trailing character are all
    // rejections rather than near-enough matches.
    for version in [" 2025-11-25", "2025-11-25 ", "2025/11/25", "2025-11-250"] {
        assert!(
            validate_protocol_version(version).is_err(),
            "{version} was accepted"
        );
    }
}
