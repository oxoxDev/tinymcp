//! Unit tests for the agent-tool contract: argument normalization, the
//! registry tool specs, and the per-action spec builder.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};

use super::{ArgsError, normalize_tool_arguments};

// ---------------------------------------------------------------------------
// Argument normalization
// ---------------------------------------------------------------------------

#[test]
fn missing_arguments_normalize_to_an_empty_object() {
    assert_eq!(normalize_tool_arguments(None).unwrap(), serde_json::Map::new());
}

#[test]
fn null_arguments_normalize_to_an_empty_object() {
    assert_eq!(
        normalize_tool_arguments(Value::Null).unwrap(),
        serde_json::Map::new()
    );
}

#[test]
fn an_object_is_passed_through_unchanged() {
    let arguments = json!({ "city": "London", "days": 3, "nested": { "a": [1, 2] } });
    let normalized = normalize_tool_arguments(arguments.clone()).unwrap();
    assert_eq!(Value::Object(normalized), arguments);
}

#[test]
fn a_json_encoded_empty_object_string_is_decoded() {
    // The reported failure: a model sent `"arguments": "{}"`.
    assert_eq!(
        normalize_tool_arguments(json!("{}")).unwrap(),
        serde_json::Map::new()
    );
}

#[test]
fn a_json_encoded_object_string_is_decoded() {
    let normalized = normalize_tool_arguments(json!(r#"{"city":"Paris","days":2}"#)).unwrap();
    assert_eq!(Value::Object(normalized), json!({ "city": "Paris", "days": 2 }));
}

#[test]
fn surrounding_whitespace_on_an_encoded_object_is_tolerated() {
    let normalized = normalize_tool_arguments(json!("  \n{\"a\": 1}\n ")).unwrap();
    assert_eq!(Value::Object(normalized), json!({ "a": 1 }));
}

#[test]
fn a_markdown_fenced_object_string_is_decoded() {
    for fenced in [
        "```json\n{\"a\": 1}\n```",
        "```JSON\n{\"a\": 1}```",
        "```\n{\"a\": 1}\n```",
        "  ```json {\"a\": 1} ```  ",
    ] {
        let normalized = normalize_tool_arguments(json!(fenced))
            .unwrap_or_else(|error| panic!("{fenced:?}: {error}"));
        assert_eq!(Value::Object(normalized), json!({ "a": 1 }), "{fenced:?}");
    }
}

#[test]
fn a_value_of_another_type_is_refused_naming_the_type() {
    let cases = [
        (json!(true), "a boolean"),
        (json!(42), "a number"),
        (json!([1, 2]), "an array"),
    ];
    for (value, actual) in cases {
        let error = normalize_tool_arguments(value.clone()).expect_err("refused");
        assert_eq!(error, ArgsError::NotAnObject { actual }, "{value}");
        assert!(error.to_string().contains(actual), "{error}");
    }
}

#[test]
fn a_string_that_is_not_json_is_refused() {
    let error = normalize_tool_arguments(json!("city=London")).expect_err("refused");
    assert_eq!(error, ArgsError::StringNotAnObject { decoded: None });
    assert!(error.to_string().contains("not JSON"), "{error}");
}

#[test]
fn a_string_holding_json_that_is_not_an_object_is_refused_naming_the_type() {
    let cases = [
        (json!("[1, 2]"), "an array"),
        (json!("\"nested\""), "a string"),
        (json!("7"), "a number"),
        (json!("null"), "null"),
    ];
    for (value, decoded) in cases {
        let error = normalize_tool_arguments(value.clone()).expect_err("refused");
        assert_eq!(
            error,
            ArgsError::StringNotAnObject {
                decoded: Some(decoded)
            },
            "{value}"
        );
        assert!(error.to_string().contains(decoded), "{error}");
    }
}

#[test]
fn an_empty_string_is_refused() {
    let error = normalize_tool_arguments(json!("   ")).expect_err("refused");
    assert_eq!(error, ArgsError::StringNotAnObject { decoded: None });
}

#[test]
fn the_error_is_a_std_error() {
    let error: Box<dyn std::error::Error> = Box::new(ArgsError::NotAnObject { actual: "a number" });
    assert!(error.to_string().starts_with("tool arguments must be a JSON object"));
}
