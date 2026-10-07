//! Helpers both catalog adapters need.

use std::time::Duration;

use super::types::RegistryOperation;
use crate::error::{Error, Result};
use crate::registry::Store;

/// How much of an upstream failure body to keep.
///
/// These bodies reach a log line and an error message, and an upstream that
/// answers a failure with a whole HTML page would otherwise put all of it
/// there.
pub(super) const MAX_ERROR_BODY_BYTES: usize = 200;

/// Truncates on a character boundary.
pub(super) fn truncate(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }

    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }

    text.get(..end).unwrap_or_default().to_string()
}

/// Writes a response to the cache, treating a failure as unimportant.
///
/// A cache that cannot be written costs a round trip next time. Failing the
/// user's search over it would cost them the search.
pub(super) fn cache(store: &Store, cache_key: &str, body: &str) {
    if let Err(error) = store.cache(cache_key, body) {
        tracing::debug!(cache_key, "could not cache an upstream response: {error}");
    }
}

/// Sends a request and returns its body, judging the status first.
pub(super) async fn read_body(
    request: reqwest::RequestBuilder,
    url: &str,
    operation: RegistryOperation,
    timeout: Duration,
) -> Result<String> {
    let response = request
        .send()
        .await
        .map_err(|error| upstream_error(url, error, operation, timeout))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| upstream_error(url, error, operation, timeout))?;

    if !status.is_success() {
        return Err(Error::Http {
            endpoint: crate::redact_endpoint(url),
            status: status.as_u16(),
            body: truncate(&body, MAX_ERROR_BODY_BYTES),
        });
    }

    Ok(body)
}

/// The error for a request to `url` that failed before a status was judged.
///
/// A timeout becomes [`Error::RegistryTimeout`], so a caller can tell a
/// stalled catalog from an unreachable one; anything else is a transport
/// failure.
pub(super) fn upstream_error(
    url: &str,
    error: reqwest::Error,
    operation: RegistryOperation,
    timeout: Duration,
) -> Error {
    if error.is_timeout() {
        tracing::debug!(%operation, timeout_ms = timeout.as_millis(), "registry request timed out");
        return Error::RegistryTimeout {
            endpoint: crate::redact_endpoint(url),
            operation,
            timeout,
        };
    }

    Error::transport(url, error)
}
