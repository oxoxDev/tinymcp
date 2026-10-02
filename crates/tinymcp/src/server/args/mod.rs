//! Validators for a tool call's arguments.
//!
//! A server handler reads its `arguments` object through these, so every tool
//! rejects bad input with the same wording. That wording is wire text: it is
//! the JSON-RPC error's `data`, and it is what a model reads to decide what to
//! send next.
//!
//! Two rules run through all of them:
//!
//! - **Explicit rejection over silent correction.** An over-cap number, a
//!   blank required string, an unexpected key — each is refused with a
//!   message naming it, never clamped or dropped. A model that silently gets
//!   less than it asked for cannot correct itself.
//! - **Absent is not the same as blank.** An optional field that is missing or
//!   `null` is `None`; one that is present but blank is an error, so the
//!   caller learns to omit it.
//!
//! Every function fails with [`ToolCallError::InvalidParams`].

use serde_json::{Map, Value};

use super::ToolCallError;

/// Rejects any key of `args` not in `allowed`, naming every offender in
/// sorted order.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when `args` holds a key outside
/// `allowed`.
pub fn reject_unexpected_arguments(
    args: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), ToolCallError> {
    let mut unexpected = args
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if unexpected.is_empty() {
        return Ok(());
    }
    unexpected.sort();
    Err(ToolCallError::InvalidParams(format!(
        "unexpected argument `{}`",
        unexpected.join("`, `")
    )))
}

/// Reads `arguments` as an object: `null` is an empty one.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] for anything other than an object or
/// `null`.
pub fn object_arguments(arguments: Value) -> Result<Map<String, Value>, ToolCallError> {
    match arguments {
        Value::Null => Ok(Map::new()),
        Value::Object(map) => Ok(map),
        other => Err(ToolCallError::InvalidParams(format!(
            "tools/call arguments must be an object, got {}",
            json_type_name(&other)
        ))),
    }
}

/// The trimmed string at `key`.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when the key is missing or not a string,
/// or when the string is blank.
pub fn required_non_empty_string(
    args: &Map<String, Value>,
    key: &str,
) -> Result<String, ToolCallError> {
    let raw = args.get(key).and_then(Value::as_str).ok_or_else(|| {
        ToolCallError::InvalidParams(format!("missing required argument `{key}`"))
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must not be empty"
        )));
    }
    Ok(trimmed.to_string())
}

/// The trimmed string at `key`, or `None` when it is missing or `null`.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when the value is not a string, or is a
/// blank one — present-but-blank is a client bug worth surfacing, so the next
/// call drops the field instead of resending whitespace.
pub fn optional_non_empty_string(
    args: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    let Some(value) = present(args, key) else {
        return Ok(None);
    };
    let Some(raw) = value.as_str() else {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must be a string"
        )));
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must not be empty when provided"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// The array of strings at `key`, trimmed, with blank entries dropped; `None`
/// when the key is missing or `null`.
///
/// Blank entries are tolerated because the intent survives them: `["",
/// "email"]` after a partial selection still means "email".
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when the value is not an array, or holds
/// anything other than strings.
pub fn optional_string_array(
    args: &Map<String, Value>,
    key: &str,
) -> Result<Option<Vec<String>>, ToolCallError> {
    let Some(value) = present(args, key) else {
        return Ok(None);
    };
    let Some(items) = value.as_array() else {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must be an array of strings, got {}",
            json_type_name(value)
        )));
    };
    let mut out = Vec::with_capacity(items.len());
    let mut dropped_blank = 0usize;
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(ToolCallError::InvalidParams(format!(
                "argument `{key}` must contain only strings, got {} entry",
                json_type_name(item)
            )));
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            dropped_blank += 1;
            continue;
        }
        out.push(trimmed.to_string());
    }
    if dropped_blank > 0 {
        // The caller never learns how many entries were skipped; a "the
        // filter didn't match" report is much faster to triage with this.
        tracing::trace!(
            "[mcp_server] optional_string_array key={key} dropped_blank_entries={dropped_blank}"
        );
    }
    Ok(Some(out))
}

/// Like [`optional_string_array`], but the list must be present and keep at
/// least one entry after blanks are dropped.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] for everything [`optional_string_array`]
/// rejects, and when the key is missing, `null`, or holds no non-blank entry.
pub fn required_non_empty_string_array(
    args: &Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, ToolCallError> {
    let entries = optional_string_array(args, key)?.ok_or_else(|| {
        ToolCallError::InvalidParams(format!("missing required argument `{key}`"))
    })?;
    if entries.is_empty() {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must contain at least one non-empty string"
        )));
    }
    Ok(entries)
}

/// The signed integer at `key`, or `None` when it is missing or `null`.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when the value is not an integer in the
/// `i64` range.
pub fn optional_i64(args: &Map<String, Value>, key: &str) -> Result<Option<i64>, ToolCallError> {
    let Some(value) = present(args, key) else {
        return Ok(None);
    };
    value.as_i64().map(Some).ok_or_else(|| {
        ToolCallError::InvalidParams(format!(
            "argument `{key}` must be an integer in the i64 range"
        ))
    })
}

/// The unsigned integer at `key`, or `None` when it is missing or `null`.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when the value is not a non-negative
/// integer.
pub fn optional_u64(args: &Map<String, Value>, key: &str) -> Result<Option<u64>, ToolCallError> {
    let Some(value) = present(args, key) else {
        return Ok(None);
    };
    value.as_u64().map(Some).ok_or_else(|| {
        ToolCallError::InvalidParams(format!("argument `{key}` must be a non-negative integer"))
    })
}

/// The integer at `key`, in `1..=max`, or `default` when the key is missing.
///
/// Only a *missing* key takes the default: an explicit `null` is refused like
/// any other non-integer.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when the value is not a positive integer
/// or exceeds `max`. Over-cap values are refused, not clamped: the schema
/// advertises the cap, so exceeding it is a client bug.
pub fn positive_u64_or_default(
    args: &Map<String, Value>,
    key: &str,
    default: u64,
    max: u64,
) -> Result<u64, ToolCallError> {
    match args.get(key) {
        None => Ok(default),
        Some(value) => bounded_positive(key, value, max),
    }
}

/// The integer at `key`, in `1..=max`, or `None` when it is missing or
/// `null`.
///
/// # Errors
///
/// [`ToolCallError::InvalidParams`] when the value is not a positive integer
/// or exceeds `max`.
pub fn optional_positive_u64(
    args: &Map<String, Value>,
    key: &str,
    max: u64,
) -> Result<Option<u64>, ToolCallError> {
    present(args, key)
        .map(|value| bounded_positive(key, value, max))
        .transpose()
}

/// The JSON type of `value`, as the validators' messages name it.
#[must_use]
pub fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// The value at `key`, treating `null` as absent.
fn present<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|value| !value.is_null())
}

fn bounded_positive(key: &str, value: &Value, max: u64) -> Result<u64, ToolCallError> {
    let Some(number) = value.as_u64() else {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must be a positive integer"
        )));
    };
    if number == 0 {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must be greater than zero"
        )));
    }
    if number > max {
        return Err(ToolCallError::InvalidParams(format!(
            "argument `{key}` must not exceed {max} (got {number})"
        )));
    }
    Ok(number)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
