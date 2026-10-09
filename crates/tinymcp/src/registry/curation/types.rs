//! The canonical-server list and the catalog filters.

use serde_json::json;

pub use super::servers::CURATED_SERVERS;
use crate::registry::sources::SOURCE_MCP_OFFICIAL;
use tinymcp_bus::{ExtraFields, RegistryConnection, RegistryServerDetail, RegistryServerSummary};

/// The value `auth_kind` takes for a server declaring a static credential.
const AUTH_KIND_API_KEY: &str = "api_key";

/// How a curated server's endpoint speaks MCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CuratedTransport {
    /// Streamable HTTP.
    StreamableHttp,
    /// Server-sent events.
    Sse,
}

impl CuratedTransport {
    /// The connection type a catalog detail record uses for this transport.
    #[must_use]
    pub const fn connection_type(self) -> &'static str {
        match self {
            Self::StreamableHttp => "http",
            Self::Sse => "sse",
        }
    }
}

/// How a curated server's endpoint authenticates a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CuratedAuth {
    /// OAuth, with dynamic client registration.
    Oauth,
    /// OAuth for clients the vendor has registered in advance only; dynamic
    /// client registration is refused.
    OauthPreregistered,
    /// A static token sent in the `Authorization` header.
    Token,
    /// No authentication.
    None,
}

/// A canonical first-party server, with what it takes to reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CuratedServer {
    /// The exact registry qualified name.
    pub qualified_name: &'static str,
    /// The vendor's name for it.
    pub display_name: &'static str,
    /// A short description.
    pub description: &'static str,
    /// The vendor-hosted endpoint.
    pub remote_url: &'static str,
    /// How the endpoint speaks MCP.
    pub transport: CuratedTransport,
    /// How the endpoint authenticates a client.
    pub auth: CuratedAuth,
    /// The icon the registry publishes for it, when it publishes one.
    pub icon_url: Option<&'static str>,
}

impl CuratedServer {
    /// This server as a catalog row from the official registry.
    #[must_use]
    pub fn to_summary(&self) -> RegistryServerSummary {
        RegistryServerSummary {
            qualified_name: self.qualified_name.to_string(),
            display_name: self.display_name.to_string(),
            description: Some(self.description.to_string()),
            icon_url: self.icon_url.map(ToString::to_string),
            use_count: 0,
            is_deployed: true,
            source: SOURCE_MCP_OFFICIAL.to_string(),
            official: false,
            website_url: None,
            auth_kind: (self.auth == CuratedAuth::Token).then(|| AUTH_KIND_API_KEY.to_string()),
            extra: ExtraFields::new(),
        }
    }

    /// This server as a catalog detail record with its one hosted connection.
    #[must_use]
    pub fn to_detail(&self) -> RegistryServerDetail {
        let config_schema = (self.auth == CuratedAuth::Token).then(|| {
            json!({
                "properties": { "Authorization": { "x-secret": true } },
                "required": ["Authorization"],
            })
        });

        RegistryServerDetail {
            qualified_name: self.qualified_name.to_string(),
            display_name: self.display_name.to_string(),
            description: Some(self.description.to_string()),
            icon_url: self.icon_url.map(ToString::to_string),
            connections: vec![RegistryConnection {
                r#type: self.transport.connection_type().to_string(),
                deployment_url: Some(self.remote_url.to_string()),
                config_schema,
                example_config: None,
                published: true,
                extra: ExtraFields::new(),
            }],
            source: SOURCE_MCP_OFFICIAL.to_string(),
            extra: ExtraFields::new(),
        }
    }
}

/// The curated entry for `qualified_name`, by exact match.
#[must_use]
pub fn curated_server(qualified_name: &str) -> Option<&'static CuratedServer> {
    CURATED_SERVERS
        .iter()
        .find(|server| server.qualified_name == qualified_name)
}

/// The qualified names of [`CURATED_SERVERS`], in the same order.
pub(super) const fn curated_names() -> [&'static str; CURATED_SERVERS.len()] {
    let mut names = [""; CURATED_SERVERS.len()];
    let mut index = 0;
    while index < CURATED_SERVERS.len() {
        names[index] = CURATED_SERVERS[index].qualified_name;
        index += 1;
    }
    names
}

/// The qualified names of [`CURATED_SERVERS`], held so a slice of them can be
/// borrowed for the whole program.
const CURATED_NAMES: [&str; CURATED_SERVERS.len()] = curated_names();

/// Canonical first-party servers, by exact registry qualified name.
///
/// The names of [`CURATED_SERVERS`], in the same order. These get the badge;
/// every other server is shown without one.
pub const OFFICIAL_SERVERS: &[&str] = &CURATED_NAMES;

/// Marks the canonical first-party server for each known service.
///
/// Sets the badge on an exact qualified-name match and clears it otherwise, so
/// a row arriving with the flag already set cannot keep it. See the module note
/// on why the match is never a substring.
///
/// # Examples
///
/// ```
/// # use tinymcp::registry::curation::tag_official;
/// # use tinymcp_bus::RegistryServerSummary;
/// let mut servers: Vec<RegistryServerSummary> = serde_json::from_value(serde_json::json!([
///     { "qualified_name": "com.notion/mcp", "display_name": "Notion" },
///     { "qualified_name": "ai.smithery/smithery-notion", "display_name": "Notion-ish" },
/// ]))?;
///
/// tag_official(&mut servers);
///
/// assert!(servers[0].official);
/// assert!(!servers[1].official, "merely containing 'notion' is not official");
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn tag_official(servers: &mut [RegistryServerSummary]) {
    for server in servers.iter_mut() {
        server.official = OFFICIAL_SERVERS.contains(&server.qualified_name.as_str());
    }
}

/// Whether a row says enough, from its metadata alone, to be installed and
/// connected without guessing.
///
/// That means a non-blank vendor website — the user's destination for getting a
/// key, and a signal somebody stands behind the server — and a declared static
/// credential.
#[must_use]
pub fn is_perfect_server(server: &RegistryServerSummary) -> bool {
    server
        .website_url
        .as_deref()
        .is_some_and(|url| !url.trim().is_empty())
        && server.auth_kind.as_deref() == Some("api_key")
}

/// Keeps only the rows [`is_perfect_server`] accepts, returning how many went.
///
/// This drops OAuth-only, open, and under-declared servers. It is a deliberate
/// quality-over-quantity trade: a user browsing the catalog only sees servers
/// that can be installed and connected with confidence, rather than a longer
/// list where some fraction will fail in ways they cannot diagnose.
///
/// The count is returned so a caller can log what was trimmed. A filter that
/// silently removes most of a catalog reads as "there is nothing here".
pub fn retain_perfect_servers(servers: &mut Vec<RegistryServerSummary>) -> usize {
    let before = servers.len();
    servers.retain(is_perfect_server);
    before - servers.len()
}

/// Floats the badged servers to the top, keeping relevance order below.
///
/// A stable sort, so everything that is not badged stays in the order the
/// upstream ranked it.
pub fn float_official_first(servers: &mut [RegistryServerSummary]) {
    servers.sort_by_key(|server| !server.official);
}
