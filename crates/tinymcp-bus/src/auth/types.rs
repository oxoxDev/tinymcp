//! The authorization detection payloads.

use serde::{Deserialize, Serialize};

/// What a server wants before it will talk.
///
/// Drives which control a caller offers the user, and getting it wrong costs
/// them a sign-in that cannot work or a token field for a server that will
/// never accept one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AuthDetection {
    /// `none`, `token`, or `oauth`.
    pub kind: AuthKind,
    /// Where to send the user, for an OAuth challenge.
    #[serde(default)]
    pub authorization_endpoint: Option<String>,
    /// The grant types the authorization server listed, if it listed any.
    #[serde(default)]
    pub grant_types: Vec<String>,
}

impl AuthDetection {
    /// A server that wants nothing.
    #[must_use]
    pub fn open() -> Self {
        Self {
            kind: AuthKind::None,
            authorization_endpoint: None,
            grant_types: Vec::new(),
        }
    }

    /// A server that wants a static credential.
    #[must_use]
    pub fn static_token() -> Self {
        Self {
            kind: AuthKind::Token,
            authorization_endpoint: None,
            grant_types: Vec::new(),
        }
    }

    /// A server that wants a browser sign-in.
    #[must_use]
    pub fn oauth(authorization_endpoint: String, grant_types: Vec<String>) -> Self {
        Self {
            kind: AuthKind::Oauth,
            authorization_endpoint: Some(authorization_endpoint),
            grant_types,
        }
    }
}

/// The three kinds of thing a server can want.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AuthKind {
    /// The server answered without a challenge. Nothing to supply.
    None,
    /// A static bearer token or API key, which the user pastes.
    Token,
    /// A browser sign-in.
    Oauth,
}

impl AuthKind {
    /// The stable string this kind is transmitted as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Token => "token",
            Self::Oauth => "oauth",
        }
    }
}
