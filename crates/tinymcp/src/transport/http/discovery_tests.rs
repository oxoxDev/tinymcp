//! Tests for well-known authorization discovery.
//!
//! Each test binds a loopback listener first, so the documents it serves can
//! name the origin they are served from, then drives a real client at it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::StatusCode as AxumStatus;
use axum::response::{IntoResponse, Response as AxumResponse};
use axum::routing::{get, post};
use serde_json::{Value, json};

use super::*;
use crate::Error;

const PRM_PATH: &str = "/.well-known/oauth-protected-resource";
const PRM_AT_MCP_PATH: &str = "/.well-known/oauth-protected-resource/mcp";
const AS_PATH: &str = "/.well-known/oauth-authorization-server";
const OIDC_PATH: &str = "/.well-known/openid-configuration";

/// A bound listener and the origin it will serve.
struct Origin {
    listener: tokio::net::TcpListener,
    base: String,
}

async fn origin() -> Origin {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    Origin { listener, base }
}

impl Origin {
    /// Serves `app` with an MCP endpoint at `/mcp` that answers every POST
    /// with a 401 carrying `challenge`, and returns that endpoint.
    fn serve(self, challenge: &'static str, app: Router) -> String {
        let app = app.route(
            "/mcp",
            post(move || async move {
                (
                    AxumStatus::UNAUTHORIZED,
                    [("WWW-Authenticate", challenge)],
                    "",
                )
                    .into_response()
            }),
        );
        tokio::spawn(async move { axum::serve(self.listener, app).await.unwrap() });
        format!("{}/mcp", self.base)
    }
}

const ZOMATO_CHALLENGE: &str =
    "Bearer error=\"invalid_token\", error_description=\"Authentication required\"";

/// Authorization-server metadata for `issuer`, with endpoints on `base`.
fn issuer_metadata(issuer: &str, base: &str) -> Value {
    json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "registration_endpoint": format!("{base}/register"),
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none", "client_secret_basic", "client_secret_post"],
    })
}

fn document(body: Value) -> axum::routing::MethodRouter {
    get(move || {
        let body = body.clone();
        async move { axum::Json(body) }
    })
}

fn client(endpoint: &str) -> McpHttpClient {
    McpHttpClient::new(endpoint, 5).unwrap()
}

/// The `resource_metadata` an `initialize` 401 carries.
async fn unauthorized_metadata(client: &McpHttpClient) -> Option<String> {
    match client.initialize().await.expect_err("a 401") {
        Error::Unauthorized {
            resource_metadata, ..
        } => resource_metadata,
        other => panic!("expected unauthorized, got {other:?}"),
    }
}

/// The Zomato shape: the MCP origin is its own authorization server and serves
/// that metadata at the protected-resource path, with no `resource` member.
async fn zomato_shaped() -> (String, String) {
    let origin = origin().await;
    let base = origin.base.clone();
    let metadata = issuer_metadata(&format!("{base}/"), &base);
    let app = Router::new()
        .route(PRM_PATH, document(metadata.clone()))
        .route(AS_PATH, document(metadata));
    (origin.serve(ZOMATO_CHALLENGE, app), base)
}

#[tokio::test]
async fn an_origin_serving_issuer_metadata_at_the_resource_path_is_discovered() {
    let (endpoint, base) = zomato_shaped().await;

    let context = client(&endpoint)
        .discover_authorization()
        .await
        .unwrap()
        .expect("a 401");

    assert_eq!(context.protected_resource_metadata, None);
    assert_eq!(context.authorization_server_metadata.len(), 1);
    let server = &context.authorization_server_metadata[0];
    assert_eq!(server.issuer, format!("{base}/"));
    assert_eq!(
        server.registration_endpoint.as_deref(),
        Some(format!("{base}/register").as_str())
    );
}

#[tokio::test]
async fn a_401_from_an_origin_serving_issuer_metadata_advertises_oauth() {
    let (endpoint, base) = zomato_shaped().await;
    let client = client(&endpoint);

    let error = client.initialize().await.expect_err("a 401");

    assert!(error.advertises_oauth(), "{error:?}");
    assert_eq!(
        unauthorized_metadata(&client).await,
        Some(format!("{base}{PRM_PATH}"))
    );
}

#[tokio::test]
async fn protected_resource_metadata_under_the_endpoint_path_is_followed() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new()
        .route(
            PRM_AT_MCP_PATH,
            document(json!({
                "resource": format!("{base}/mcp"),
                "authorization_servers": [base],
            })),
        )
        .route(AS_PATH, document(issuer_metadata(&base, &base)));
    let endpoint = origin.serve("Bearer realm=\"mcp\"", app);
    let client = client(&endpoint);

    let context = client.discover_authorization().await.unwrap().unwrap();

    let resource = context.protected_resource_metadata.expect("a resource");
    assert_eq!(resource.resource, format!("{base}/mcp"));
    assert_eq!(context.authorization_server_metadata.len(), 1);
    assert_eq!(
        unauthorized_metadata(&client).await,
        Some(format!("{base}{PRM_AT_MCP_PATH}"))
    );
}

#[tokio::test]
async fn a_resource_naming_no_authorization_server_falls_back_to_the_origin() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new()
        .route(
            PRM_PATH,
            document(json!({ "resource": format!("{base}/mcp") })),
        )
        .route(AS_PATH, document(issuer_metadata(&base, &base)));
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    let context = client.discover_authorization().await.unwrap().unwrap();

    assert!(context.protected_resource_metadata.is_some());
    assert_eq!(context.authorization_server_metadata[0].issuer, base);
    assert_eq!(
        unauthorized_metadata(&client).await,
        Some(format!("{base}{PRM_PATH}"))
    );
}

#[tokio::test]
async fn an_unreadable_named_authorization_server_is_not_cached_as_absent() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new().route(
        PRM_PATH,
        document(json!({
            "resource": format!("{base}/mcp"),
            "authorization_servers": [format!("{base}/missing")],
        })),
    );
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    assert_eq!(unauthorized_metadata(&client).await, None);
    assert_eq!(client.well_known.lock().clone(), None);
}

#[tokio::test]
async fn origin_rfc8414_metadata_alone_is_discovered() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new().route(AS_PATH, document(issuer_metadata(&base, &base)));
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    let context = client.discover_authorization().await.unwrap().unwrap();

    assert_eq!(context.protected_resource_metadata, None);
    assert_eq!(context.authorization_server_metadata.len(), 1);
    assert_eq!(
        unauthorized_metadata(&client).await,
        Some(format!("{base}{AS_PATH}"))
    );
}

#[tokio::test]
async fn origin_openid_metadata_alone_is_discovered() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new().route(OIDC_PATH, document(issuer_metadata(&base, &base)));
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    let context = client.discover_authorization().await.unwrap().unwrap();

    assert_eq!(context.authorization_server_metadata.len(), 1);
    assert_eq!(
        unauthorized_metadata(&client).await,
        Some(format!("{base}{OIDC_PATH}"))
    );
}

#[tokio::test]
async fn incomplete_rfc8414_metadata_is_completed_from_openid_metadata() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new()
        .route(
            AS_PATH,
            document(
                json!({ "issuer": base, "registration_endpoint": format!("{base}/register") }),
            ),
        )
        .route(OIDC_PATH, document(issuer_metadata(&base, &base)));
    let endpoint = origin.serve("Bearer", app);

    let context = client(&endpoint)
        .discover_authorization()
        .await
        .unwrap()
        .unwrap();

    let server = &context.authorization_server_metadata[0];
    assert!(server.authorization_endpoint.is_some());
    assert!(server.token_endpoint.is_some());
}

#[tokio::test]
async fn a_bearer_401_with_no_metadata_anywhere_wants_a_static_credential() {
    let origin = origin().await;
    let endpoint = origin.serve("Bearer realm=\"mcp\"", Router::new());
    let client = client(&endpoint);

    let context = client.discover_authorization().await.unwrap().unwrap();
    assert_eq!(context.authorization_server_metadata.len(), 0);
    assert_eq!(context.protected_resource_metadata, None);

    let error = client.initialize().await.expect_err("a 401");
    assert!(error.is_unauthorized());
    assert!(!error.advertises_oauth());
}

#[tokio::test]
async fn a_401_on_every_well_known_path_counts_as_absent() {
    let origin = origin().await;
    let app = Router::new().fallback(|| async { AxumStatus::UNAUTHORIZED });
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    assert_eq!(unauthorized_metadata(&client).await, None);
    assert_eq!(
        client.well_known.lock().clone(),
        Some(WellKnownOutcome::NotFound)
    );
}

#[tokio::test]
async fn a_basic_challenge_never_triggers_discovery() {
    let hits = Arc::new(AtomicUsize::new(0));
    let origin = origin().await;
    let base = origin.base.clone();
    let counted = Arc::clone(&hits);
    let metadata = issuer_metadata(&base, &base);
    let app = Router::new().route(
        AS_PATH,
        get(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            let metadata = metadata.clone();
            async move { axum::Json(metadata) }
        }),
    );
    let endpoint = origin.serve("Basic realm=\"mcp\"", app);
    let client = client(&endpoint);

    let context = client.discover_authorization().await.unwrap().unwrap();
    assert_eq!(context.authorization_server_metadata.len(), 0);
    assert_eq!(unauthorized_metadata(&client).await, None);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn issuer_metadata_for_another_origin_is_ignored() {
    let origin = origin().await;
    let base = origin.base.clone();
    let foreign = issuer_metadata("https://elsewhere.example", &base);
    let app = Router::new()
        .route(PRM_PATH, document(foreign.clone()))
        .route(AS_PATH, document(foreign));
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    let context = client.discover_authorization().await.unwrap().unwrap();
    assert_eq!(context.authorization_server_metadata.len(), 0);
    assert_eq!(unauthorized_metadata(&client).await, None);
}

#[tokio::test]
async fn an_issuer_with_a_path_on_the_same_origin_is_not_the_origin() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new().route(
        AS_PATH,
        document(issuer_metadata(&format!("{base}/tenant"), &base)),
    );
    let endpoint = origin.serve("Bearer", app);

    assert_eq!(unauthorized_metadata(&client(&endpoint)).await, None);
}

#[tokio::test]
async fn a_resource_on_another_origin_is_ignored() {
    let origin = origin().await;
    let base = origin.base.clone();
    let app = Router::new().route(
        PRM_PATH,
        document(json!({
            "resource": "https://elsewhere.example/mcp",
            "authorization_servers": [base],
        })),
    );
    let endpoint = origin.serve("Bearer", app);

    let context = client(&endpoint)
        .discover_authorization()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(context.protected_resource_metadata, None);
    assert_eq!(context.authorization_server_metadata.len(), 0);
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
    let hits = Arc::new(AtomicUsize::new(0));
    let target = origin().await;
    let target_base = target.base.clone();
    let counted = Arc::clone(&hits);
    let metadata = issuer_metadata(&target_base, &target_base);
    let target_app = Router::new().fallback(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        let metadata = metadata.clone();
        async move { axum::Json(metadata) }
    });
    target.serve("Bearer", target_app);

    let origin = origin().await;
    let app = Router::new().fallback(move |uri: axum::http::Uri| {
        let location = format!("{target_base}{}", uri.path());
        async move { (AxumStatus::FOUND, [("Location", location)]).into_response() }
    });
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    assert_eq!(unauthorized_metadata(&client).await, None);
    let context = client.discover_authorization().await.unwrap().unwrap();
    assert_eq!(context.authorization_server_metadata.len(), 0);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_oversized_document_is_ignored() {
    let origin = origin().await;
    let base = origin.base.clone();
    let mut metadata = issuer_metadata(&base, &base);
    metadata["padding"] = json!("x".repeat(MAX_DOCUMENT_BYTES));
    let app = Router::new().route(AS_PATH, document(metadata));
    let endpoint = origin.serve("Bearer", app);

    assert_eq!(unauthorized_metadata(&client(&endpoint)).await, None);
}

#[tokio::test]
async fn an_oversized_streamed_document_is_ignored() {
    let origin = origin().await;
    let base = origin.base.clone();
    let text = issuer_metadata(&base, &base).to_string();
    let app = Router::new().route(
        AS_PATH,
        get(move || {
            let chunks: Vec<Result<String, std::io::Error>> =
                vec![Ok(" ".repeat(MAX_DOCUMENT_BYTES)), Ok(text.clone())];
            async move { AxumResponse::new(Body::from_stream(futures_util::stream::iter(chunks))) }
        }),
    );
    let endpoint = origin.serve("Bearer", app);

    assert_eq!(unauthorized_metadata(&client(&endpoint)).await, None);
}

#[tokio::test]
async fn a_streamed_document_within_the_cap_is_read() {
    let origin = origin().await;
    let base = origin.base.clone();
    let text = issuer_metadata(&base, &base).to_string();
    let app = Router::new().route(
        AS_PATH,
        get(move || {
            let (head, tail) = text.split_at(10);
            let chunks: Vec<Result<String, std::io::Error>> =
                vec![Ok(head.to_string()), Ok(tail.to_string())];
            async move { AxumResponse::new(Body::from_stream(futures_util::stream::iter(chunks))) }
        }),
    );
    let endpoint = origin.serve("Bearer", app);

    assert_eq!(
        unauthorized_metadata(&client(&endpoint)).await,
        Some(format!("{base}{AS_PATH}"))
    );
}

#[tokio::test]
async fn a_document_that_is_not_json_is_ignored() {
    let origin = origin().await;
    let app = Router::new().route(AS_PATH, get(|| async { "<html>sign in</html>" }));
    let endpoint = origin.serve("Bearer", app);

    assert_eq!(unauthorized_metadata(&client(&endpoint)).await, None);
}

#[tokio::test]
async fn an_unexpected_client_error_counts_as_absent() {
    let origin = origin().await;
    let app = Router::new().fallback(|| async { AxumStatus::BAD_REQUEST });
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    assert_eq!(unauthorized_metadata(&client).await, None);
    assert_eq!(
        client.well_known.lock().clone(),
        Some(WellKnownOutcome::NotFound)
    );
}

#[tokio::test]
async fn a_server_error_is_retried_on_the_next_401() {
    let failures_left = Arc::new(AtomicUsize::new(1));
    let origin = origin().await;
    let base = origin.base.clone();
    let metadata = issuer_metadata(&base, &base);
    let app = Router::new().route(
        AS_PATH,
        get(move || {
            let failures_left = Arc::clone(&failures_left);
            let metadata = metadata.clone();
            async move {
                if failures_left.swap(0, Ordering::SeqCst) > 0 {
                    return AxumStatus::SERVICE_UNAVAILABLE.into_response();
                }
                axum::Json(metadata).into_response()
            }
        }),
    );
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    assert_eq!(unauthorized_metadata(&client).await, None);
    assert_eq!(client.well_known.lock().clone(), None);
    assert_eq!(
        unauthorized_metadata(&client).await,
        Some(format!("{base}{AS_PATH}"))
    );
}

#[tokio::test]
async fn a_definitive_answer_is_looked_up_once_per_client() {
    let hits = Arc::new(AtomicUsize::new(0));
    let origin = origin().await;
    let counted = Arc::clone(&hits);
    let app = Router::new().fallback(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        async { AxumStatus::NOT_FOUND }
    });
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    unauthorized_metadata(&client).await;
    let after_first = hits.load(Ordering::SeqCst);
    unauthorized_metadata(&client).await;

    assert!(after_first > 0);
    assert_eq!(hits.load(Ordering::SeqCst), after_first);
}

#[tokio::test]
async fn an_unreachable_well_known_path_is_transient() {
    let origin = origin().await;
    let app = Router::new().fallback(|| async {
        AxumResponse::new(Body::from_stream(futures_util::stream::iter(vec![Err::<
            String,
            std::io::Error,
        >(
            std::io::Error::other("reset"),
        )])))
    });
    let endpoint = origin.serve("Bearer", app);
    let client = client(&endpoint);

    assert_eq!(unauthorized_metadata(&client).await, None);
    assert_eq!(client.well_known.lock().clone(), None);
}

#[test]
fn an_origin_issuer_matches_with_or_without_one_trailing_slash() {
    let origin = Url::parse("https://mcp.example").unwrap();

    assert!(issuer_is_origin("https://mcp.example", &origin));
    assert!(issuer_is_origin("https://mcp.example/", &origin));
    assert!(!issuer_is_origin("https://mcp.example//", &origin));
    assert!(!issuer_is_origin("https://mcp.example/tenant", &origin));
    assert!(!issuer_is_origin("https://other.example", &origin));
}

#[test]
fn an_endpoint_without_an_http_origin_is_not_looked_up() {
    let client = McpHttpClient::new("file:///tmp/mcp", 1).unwrap();
    let outcome = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(client.well_known_authorization());

    assert_eq!(outcome, WellKnownOutcome::NotFound);
}
