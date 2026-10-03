//! Refusing internal targets among the endpoints a server advertises.
//!
//! The authorization, registration and token endpoints come from the server
//! being signed in to, and the flow then POSTs to them **from the host**. An
//! unchecked endpoint is a server-side request-forgery primitive: a hostile
//! server can aim the flow at an internal service or the cloud metadata
//! address. A host serving untrusted servers turns this guard on with
//! [`OAuthFlow::require_public_endpoints`](super::OAuthFlow::require_public_endpoints).
//! It is off by default because a desktop host legitimately signs in to
//! loopback development servers.

use std::net::{IpAddr, ToSocketAddrs};

use reqwest::Url;

use crate::error::{Error, Result};

/// Refuses `raw` unless it is `https` and every address its host resolves to
/// is public. `what` names the endpoint in the error.
///
/// A single blocked record is enough to refuse, which also blunts DNS
/// rebinding to an internal address. Resolution runs on the blocking pool so a
/// slow resolver cannot stall the executor.
///
/// # Errors
///
/// [`Error::MalformedResponse`] naming the endpoint and why it was refused.
pub(super) async fn guard_endpoint(raw: &str, what: &str) -> Result<()> {
    let refuse = |why: String| Error::malformed(format!("{what} endpoint refused: {why}"));
    let url = Url::parse(raw).map_err(|error| refuse(format!("not a valid url: {error}")))?;
    if url.scheme() != "https" {
        return Err(refuse(format!("must use https, not `{}`", url.scheme())));
    }
    let host = url
        .host_str()
        .ok_or_else(|| refuse("it has no host".to_string()))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let port = url.port_or_known_default().unwrap_or(443);

    let addresses: Vec<IpAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![ip]
    } else {
        tokio::task::spawn_blocking(move || {
            (host.as_str(), port)
                .to_socket_addrs()
                .map(|found| found.map(|addr| addr.ip()).collect())
        })
        .await
        .map_err(|error| refuse(format!("resolution did not finish: {error}")))?
        .map_err(|error| refuse(format!("its host does not resolve: {error}")))?
    };
    if addresses.is_empty() {
        return Err(refuse("its host does not resolve".to_string()));
    }
    if addresses.iter().any(is_blocked_ip) {
        return Err(refuse("it resolves to a disallowed address".to_string()));
    }
    Ok(())
}

/// Whether an address is one the flow must never POST OAuth material to:
/// loopback, private or unique-local, link-local (the metadata address among
/// them), unspecified, broadcast, documentation, multicast, or `0.0.0.0/8`.
pub(super) fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_ip(&IpAddr::V4(v4));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
#[path = "endpoint_guard_tests.rs"]
mod tests;
