//! Unit tests for the tool-argument validators.
//!
//! Every message here is wire text: it reaches the client as the JSON-RPC
//! error's `data`, and a model reads it to decide what to send next. The
//! assertions therefore pin whole messages rather than fragments.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Map, Value, json};

use super::{
    json_type_name, object_arguments, optional_i64, optional_non_empty_string,
    optional_positive_u64, optional_string_array, optional_u64, positive_u64_or_default,
    reject_unexpected_arguments, required_non_empty_string, required_non_empty_string_array,
};
use crate::server::ToolCallError;

fn args(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("fixture must be an object, got {other}"),
    }
}

fn invalid(result: Result<impl std::fmt::Debug, ToolCallError>) -> String {
    match result.expect_err("must be rejected") {
        ToolCallError::InvalidParams(message) => message,
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn allowed_arguments_pass_and_unexpected_ones_are_named_sorted() {
    let allowed = ["query", "k"];
    assert!(reject_unexpected_arguments(&args(json!({"query": "x", "k": 1})), &allowed).is_ok());
    assert!(reject_unexpected_arguments(&Map::new(), &[]).is_ok());
    assert_eq!(
        invalid(reject_unexpected_arguments(
            &args(json!({"query": "x", "zeta": 1, "alpha": 2})),
            &allowed
        )),
        "unexpected argument `alpha`, `zeta`"
    );
}

#[test]
fn object_arguments_accepts_null_and_objects_only() {
    assert!(object_arguments(Value::Null).unwrap().is_empty());
    assert_eq!(
        object_arguments(json!({"a": 1})).unwrap(),
        args(json!({"a": 1}))
    );
    assert_eq!(
        invalid(object_arguments(json!("query"))),
        "tools/call arguments must be an object, got string"
    );
}

#[test]
fn required_strings_are_trimmed_and_must_be_present_and_non_blank() {
    let map = args(json!({"a": "  value ", "blank": "   ", "number": 3}));
    assert_eq!(required_non_empty_string(&map, "a").unwrap(), "value");
    assert_eq!(
        invalid(required_non_empty_string(&map, "missing")),
        "missing required argument `missing`"
    );
    assert_eq!(
        invalid(required_non_empty_string(&map, "number")),
        "missing required argument `number`"
    );
    assert_eq!(
        invalid(required_non_empty_string(&map, "blank")),
        "argument `blank` must not be empty"
    );
}

#[test]
fn optional_strings_distinguish_absent_from_blank() {
    let map = args(json!({"a": " v ", "null": null, "blank": " ", "number": 1}));
    assert_eq!(
        optional_non_empty_string(&map, "a").unwrap().as_deref(),
        Some("v")
    );
    assert_eq!(optional_non_empty_string(&map, "missing").unwrap(), None);
    assert_eq!(optional_non_empty_string(&map, "null").unwrap(), None);
    assert_eq!(
        invalid(optional_non_empty_string(&map, "number")),
        "argument `number` must be a string"
    );
    assert_eq!(
        invalid(optional_non_empty_string(&map, "blank")),
        "argument `blank` must not be empty when provided"
    );
}

#[test]
fn optional_string_arrays_trim_drop_blanks_and_reject_other_shapes() {
    let map = args(json!({
        "tags": [" a ", "", "b", "   "],
        "null": null,
        "scalar": "email",
        "mixed": ["a", 1],
    }));
    assert_eq!(
        optional_string_array(&map, "tags").unwrap(),
        Some(vec!["a".to_string(), "b".to_string()])
    );
    assert_eq!(optional_string_array(&map, "missing").unwrap(), None);
    assert_eq!(optional_string_array(&map, "null").unwrap(), None);
    assert_eq!(
        invalid(optional_string_array(&map, "scalar")),
        "argument `scalar` must be an array of strings, got string"
    );
    assert_eq!(
        invalid(optional_string_array(&map, "mixed")),
        "argument `mixed` must contain only strings, got number entry"
    );
}

#[test]
fn required_string_arrays_need_at_least_one_non_blank_entry() {
    let map = args(json!({"tags": ["x"], "blank": ["", " "], "empty": []}));
    assert_eq!(
        required_non_empty_string_array(&map, "tags").unwrap(),
        vec!["x".to_string()]
    );
    assert_eq!(
        invalid(required_non_empty_string_array(&map, "missing")),
        "missing required argument `missing`"
    );
    for key in ["blank", "empty"] {
        assert_eq!(
            invalid(required_non_empty_string_array(&map, key)),
            format!("argument `{key}` must contain at least one non-empty string")
        );
    }
}

#[test]
fn optional_i64_accepts_the_signed_range_only() {
    let map = args(json!({
        "neg": -5,
        "null": null,
        "float": 1.5,
        "text": "yesterday",
        "huge": u64::MAX,
    }));
    assert_eq!(optional_i64(&map, "neg").unwrap(), Some(-5));
    assert_eq!(optional_i64(&map, "missing").unwrap(), None);
    assert_eq!(optional_i64(&map, "null").unwrap(), None);
    for key in ["float", "text", "huge"] {
        assert_eq!(
            invalid(optional_i64(&map, key)),
            format!("argument `{key}` must be an integer in the i64 range")
        );
    }
}

#[test]
fn optional_u64_rejects_negatives() {
    let map = args(json!({"ok": 10, "null": null, "neg": -1}));
    assert_eq!(optional_u64(&map, "ok").unwrap(), Some(10));
    assert_eq!(optional_u64(&map, "missing").unwrap(), None);
    assert_eq!(optional_u64(&map, "null").unwrap(), None);
    assert_eq!(
        invalid(optional_u64(&map, "neg")),
        "argument `neg` must be a non-negative integer"
    );
}

#[test]
fn a_defaulted_positive_bound_rejects_rather_than_clamps() {
    let map = args(json!({"at": 50, "over": 51, "zero": 0, "null": null, "text": "5"}));
    assert_eq!(
        positive_u64_or_default(&map, "missing", 10, 50).unwrap(),
        10
    );
    assert_eq!(positive_u64_or_default(&map, "at", 10, 50).unwrap(), 50);
    assert_eq!(
        invalid(positive_u64_or_default(&map, "over", 10, 50)),
        "argument `over` must not exceed 50 (got 51)"
    );
    assert_eq!(
        invalid(positive_u64_or_default(&map, "zero", 10, 50)),
        "argument `zero` must be greater than zero"
    );
    // Present-but-null is not "absent" here: the default applies only when
    // the key is missing entirely.
    for key in ["null", "text"] {
        assert_eq!(
            invalid(positive_u64_or_default(&map, key, 10, 50)),
            format!("argument `{key}` must be a positive integer")
        );
    }
}

#[test]
fn an_optional_positive_bound_treats_null_as_absent() {
    let map = args(json!({"ok": 20, "over": 21, "zero": 0, "null": null, "neg": -2}));
    assert_eq!(optional_positive_u64(&map, "ok", 20).unwrap(), Some(20));
    assert_eq!(optional_positive_u64(&map, "missing", 20).unwrap(), None);
    assert_eq!(optional_positive_u64(&map, "null", 20).unwrap(), None);
    assert_eq!(
        invalid(optional_positive_u64(&map, "over", 20)),
        "argument `over` must not exceed 20 (got 21)"
    );
    assert_eq!(
        invalid(optional_positive_u64(&map, "zero", 20)),
        "argument `zero` must be greater than zero"
    );
    assert_eq!(
        invalid(optional_positive_u64(&map, "neg", 20)),
        "argument `neg` must be a positive integer"
    );
}

#[test]
fn json_type_names_cover_every_variant() {
    assert_eq!(json_type_name(&Value::Null), "null");
    assert_eq!(json_type_name(&json!(true)), "bool");
    assert_eq!(json_type_name(&json!(1)), "number");
    assert_eq!(json_type_name(&json!("s")), "string");
    assert_eq!(json_type_name(&json!([])), "array");
    assert_eq!(json_type_name(&json!({})), "object");
}
