//! Unit tests for the authorization detection payloads.
//!
//! The serde form is the wire form: a host and the module disagreeing about a
//! field name fail at runtime with a decode error, so it is pinned here.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::{AuthDetection, AuthKind};

#[test]
fn an_open_server_serializes_with_nothing_to_supply() {
    assert_eq!(
        serde_json::to_value(AuthDetection::open()).unwrap(),
        json!({ "kind": "none", "authorization_endpoint": null, "grant_types": [] }),
    );
}

#[test]
fn a_token_server_serializes_as_token() {
    assert_eq!(
        serde_json::to_value(AuthDetection::static_token()).unwrap(),
        json!({ "kind": "token", "authorization_endpoint": null, "grant_types": [] }),
    );
}

#[test]
fn an_oauth_server_serializes_its_endpoint_and_grants() {
    let detection = AuthDetection::oauth(
        "https://auth.example.test/authorize".into(),
        vec!["authorization_code".into()],
    );

    assert_eq!(
        serde_json::to_value(&detection).unwrap(),
        json!({
            "kind": "oauth",
            "authorization_endpoint": "https://auth.example.test/authorize",
            "grant_types": ["authorization_code"],
        }),
    );
}

#[test]
fn a_detection_with_only_a_kind_decodes_with_defaults() {
    let detection: AuthDetection = serde_json::from_value(json!({ "kind": "token" })).unwrap();

    assert_eq!(detection, AuthDetection::static_token());
}

#[test]
fn an_unknown_kind_is_rejected() {
    assert!(serde_json::from_value::<AuthDetection>(json!({ "kind": "saml" })).is_err());
}

#[test]
fn every_kind_transmits_as_its_stable_string() {
    for (kind, text) in [
        (AuthKind::None, "none"),
        (AuthKind::Token, "token"),
        (AuthKind::Oauth, "oauth"),
    ] {
        assert_eq!(kind.as_str(), text);
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(text));
    }
}
