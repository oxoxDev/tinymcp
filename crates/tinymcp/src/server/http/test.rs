//! Unit tests for the Streamable HTTP server transport.
//!
//! Every request goes over a real loopback socket. Status codes, plain-text
//! rejection bodies, content types, the session header and the SSE framing
//! are wire behavior a remote client depends on; the expectations match the
//! golden fixtures OpenHuman pinned before this transport moved here.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};
use tinymcp_bus::{
    HEADER_PROTOCOL_VERSION, HEADER_SESSION_ID, LATEST_PROTOCOL_VERSION, McpAuthConfig,
};
use tokio::sync::broadcast;

use super::{AppState, HttpServerConfig, SessionEvent, router, run_http, run_http_reporting};
use crate::server::McpServerHandler;
use crate::server::fixture::{DemoHandler, ECHOED_HEADER};
use crate::{Error, McpHttpClient};

const PLAIN_TEXT: &str = "text/plain; charset=utf-8";

fn demo() -> Arc<dyn McpServerHandler> {
    Arc::new(DemoHandler::new())
}

/// Starts a server through the public entry point; returns its endpoint.
async fn spawn(auth_token: Option<&str>) -> String {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let config = HttpServerConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        auth_token: auth_token.map(str::to_string),
    };
    tokio::spawn(run_http_reporting(demo(), config, Some(tx)));
    format!("http://{}/", rx.await.expect("bound address"))
}

/// Starts a server on the internal router, keeping its event channel.
async fn spawn_with_events() -> (String, broadcast::Sender<SessionEvent>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new(demo(), None);
    let events = state.event_tx.clone();
    tokio::spawn(async move { axum::serve(listener, router(state)).await });
    (format!("http://{addr}/"), events)
}

fn init_body(client_name: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": client_name, "version": "0"}
        }
    })
}

async fn post(
    endpoint: &str,
    session: Option<&str>,
    protocol: Option<&str>,
    body: &Value,
) -> Response {
    let mut request = Client::new().post(endpoint).json(body);
    if let Some(session) = session {
        request = request.header(HEADER_SESSION_ID, session);
    }
    if let Some(protocol) = protocol {
        request = request.header(HEADER_PROTOCOL_VERSION, protocol);
    }
    request.send().await.expect("post")
}

async fn initialize(endpoint: &str) -> String {
    let response = post(endpoint, None, None, &init_body("golden")).await;
    assert_eq!(response.status(), StatusCode::OK);
    session_of(&response)
}

fn session_of(response: &Response) -> String {
    response
        .headers()
        .get(HEADER_SESSION_ID)
        .and_then(|value| value.to_str().ok())
        .expect("session header")
        .to_string()
}

async fn assert_content(response: Response, status: StatusCode, content_type: &str, body: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some(content_type)
    );
    assert_eq!(response.text().await.expect("body"), body);
}

async fn assert_text(response: Response, status: StatusCode, body: &str) {
    assert_content(response, status, PLAIN_TEXT, body).await;
}

// ---------------------------------------------------------------------------
// Round trip against this crate's own client
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_http_client_round_trips_initialize_tools_and_calls() {
    let client = McpHttpClient::new(spawn(None).await, 5).expect("a client builds");

    let init = client.initialize().await.expect("initialize");
    assert_eq!(init.protocol_version, LATEST_PROTOCOL_VERSION);
    assert_eq!(init.server_info["name"], "demo-server");

    let tools = client.list_tools().await.expect("tools/list");
    assert!(tools.iter().any(|tool| tool.name == "echo"));

    let result = client
        .call_tool("echo", json!({"a": 1}))
        .await
        .expect("tools/call");
    assert_eq!(result.raw_result["content"][0]["text"], r#"{"a":1}"#);

    client.close_session().await.expect("DELETE session");
}

#[tokio::test]
async fn bearer_auth_is_enforced_for_the_client() {
    let endpoint = spawn(Some("phase1-secret")).await;
    let denied = McpHttpClient::builder(endpoint.clone())
        .timeout_secs(5)
        .auth(McpAuthConfig::BearerToken {
            token: "wrong".into(),
        })
        .build()
        .expect("a client builds");
    let err = denied.initialize().await.expect_err("bad token");
    assert!(err.to_string().contains("401"), "expected 401, got {err}");

    let allowed = McpHttpClient::builder(endpoint)
        .timeout_secs(5)
        .auth(McpAuthConfig::BearerToken {
            token: "phase1-secret".into(),
        })
        .build()
        .expect("a client builds");
    allowed.initialize().await.expect("authorized initialize");
}

// ---------------------------------------------------------------------------
// initialize
// ---------------------------------------------------------------------------

#[tokio::test]
async fn initialize_answers_json_and_mints_a_session() {
    let endpoint = spawn(None).await;
    let response = post(&endpoint, None, None, &init_body("golden")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    let session = session_of(&response);
    assert_eq!(session.len(), 36, "a hyphenated v4 uuid: {session}");
    assert_eq!(
        response.text().await.unwrap(),
        r#"{"id":1,"jsonrpc":"2.0","result":{"capabilities":{"resources":{"listChanged":false,"subscribe":false},"tools":{}},"instructions":"Use the demo tools.","protocolVersion":"2025-11-25","serverInfo":{"name":"demo-server","version":"9.9.9"}}}"#
    );
}

#[tokio::test]
async fn a_failed_initialize_answers_the_error_without_a_session() {
    let endpoint = spawn(None).await;
    let response = post(
        &endpoint,
        None,
        None,
        &json!({"jsonrpc": "1.0", "id": 1, "method": "initialize"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(HEADER_SESSION_ID).is_none());
    assert_eq!(
        response.text().await.unwrap(),
        r#"{"error":{"code":-32600,"data":"jsonrpc must be \"2.0\"","message":"Invalid Request"},"id":1,"jsonrpc":"2.0"}"#
    );
}

#[tokio::test]
async fn an_initialize_notification_answers_no_content() {
    let endpoint = spawn(None).await;
    let response = post(
        &endpoint,
        None,
        None,
        &json!({"jsonrpc": "2.0", "method": "initialize"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response.headers().get(HEADER_SESSION_ID).is_none());
}

// ---------------------------------------------------------------------------
// POST on a session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_and_protocol_rejections_are_plain_text() {
    let endpoint = spawn(None).await;
    let ping = json!({"jsonrpc": "2.0", "id": 2, "method": "ping"});

    let no_session = post(&endpoint, None, None, &ping).await;
    assert_text(
        no_session,
        StatusCode::BAD_REQUEST,
        "missing or invalid Mcp-Session-Id header",
    )
    .await;

    let unknown = post(&endpoint, Some("nope"), Some(LATEST_PROTOCOL_VERSION), &ping).await;
    assert_text(unknown, StatusCode::NOT_FOUND, "unknown or expired MCP session").await;

    let session = initialize(&endpoint).await;
    for protocol in [Some("2024-11-05"), None] {
        let rejected = post(&endpoint, Some(&session), protocol, &ping).await;
        assert_text(
            rejected,
            StatusCode::BAD_REQUEST,
            "missing or invalid MCP-Protocol-Version header",
        )
        .await;
    }
}

#[tokio::test]
async fn requests_notifications_and_batches_on_a_session() {
    let endpoint = spawn(None).await;
    let session = initialize(&endpoint).await;
    let protocol = Some(LATEST_PROTOCOL_VERSION);

    let ping = post(
        &endpoint,
        Some(&session),
        protocol,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
    )
    .await;
    assert_content(
        ping,
        StatusCode::OK,
        "application/json",
        r#"{"id":2,"jsonrpc":"2.0","result":{}}"#,
    )
    .await;

    let notification = post(
        &endpoint,
        Some(&session),
        protocol,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;
    assert_eq!(notification.status(), StatusCode::NO_CONTENT);
    assert_eq!(notification.text().await.unwrap(), "");

    // A batch body has no top-level `id`, so it is answered 204 even when it
    // holds requests. Pinned: it is the wire this transport has always had.
    let batch = post(
        &endpoint,
        Some(&session),
        protocol,
        &json!([{"jsonrpc": "2.0", "id": 3, "method": "ping"}]),
    )
    .await;
    assert_eq!(batch.status(), StatusCode::NO_CONTENT);

    let unknown_method = post(
        &endpoint,
        Some(&session),
        protocol,
        &json!({"jsonrpc": "2.0", "id": 4, "method": "nope"}),
    )
    .await;
    assert_content(
        unknown_method,
        StatusCode::OK,
        "application/json",
        r#"{"error":{"code":-32601,"data":"unsupported MCP method `nope`","message":"Method not found"},"id":4,"jsonrpc":"2.0"}"#,
    )
    .await;
}

#[tokio::test]
async fn request_headers_reach_the_handler_and_each_post_is_its_own_session() {
    let endpoint = spawn(None).await;
    let response = post(&endpoint, None, None, &init_body("Cursor")).await;
    let session = session_of(&response);

    let call = Client::new()
        .post(&endpoint)
        .header(HEADER_SESSION_ID, session.as_str())
        .header(HEADER_PROTOCOL_VERSION, LATEST_PROTOCOL_VERSION)
        .header(ECHOED_HEADER, "2")
        .json(&json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "echo"}}))
        .send()
        .await
        .unwrap();
    let body: Value = call.json().await.unwrap();
    assert_eq!(body["result"]["structuredContent"]["depth"], "2");
    // The client named in `initialize` is not carried to later POSTs: every
    // request is dispatched on a fresh session, so it reports the bare
    // prefix. Pinned as the transport's existing behavior.
    assert_eq!(body["result"]["structuredContent"]["source_type"], "demo");
}

#[tokio::test]
async fn a_non_json_body_is_rejected_by_the_extractor() {
    let endpoint = spawn(None).await;
    let response = Client::new()
        .post(&endpoint)
        .body("ping")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        response.text().await.unwrap(),
        "Expected request with `Content-Type: application/json`"
    );
}

// ---------------------------------------------------------------------------
// GET (events) and DELETE
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_and_delete_follow_the_session_lifecycle() {
    let endpoint = spawn(None).await;
    let http = Client::new();

    let get_missing = http.get(&endpoint).send().await.unwrap();
    assert_text(get_missing, StatusCode::BAD_REQUEST, "missing Mcp-Session-Id header").await;
    let get_unknown = http
        .get(&endpoint)
        .header(HEADER_SESSION_ID, "nope")
        .send()
        .await
        .unwrap();
    assert_text(get_unknown, StatusCode::NOT_FOUND, "unknown or expired MCP session").await;

    let session = initialize(&endpoint).await;
    let get_mismatch = http
        .get(&endpoint)
        .header(HEADER_SESSION_ID, session.as_str())
        .send()
        .await
        .unwrap();
    assert_text(
        get_mismatch,
        StatusCode::BAD_REQUEST,
        "missing or invalid MCP-Protocol-Version header",
    )
    .await;

    let delete_missing = http.delete(&endpoint).send().await.unwrap();
    assert_text(
        delete_missing,
        StatusCode::BAD_REQUEST,
        "missing Mcp-Session-Id header",
    )
    .await;
    for _ in 0..2 {
        // Deleting is idempotent: a second DELETE is still 204.
        let deleted = http
            .delete(&endpoint)
            .header(HEADER_SESSION_ID, session.as_str())
            .send()
            .await
            .unwrap();
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    }

    let after = post(
        &endpoint,
        Some(&session),
        Some(LATEST_PROTOCOL_VERSION),
        &json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
    )
    .await;
    assert_text(after, StatusCode::NOT_FOUND, "unknown or expired MCP session").await;
}

#[tokio::test]
async fn get_streams_only_this_sessions_events_as_sse() {
    let (endpoint, events) = spawn_with_events().await;
    let session = initialize(&endpoint).await;

    let stream = Client::new()
        .get(&endpoint)
        .header(HEADER_SESSION_ID, session.as_str())
        .header(HEADER_PROTOCOL_VERSION, LATEST_PROTOCOL_VERSION)
        .send()
        .await
        .unwrap();
    assert_eq!(stream.status(), StatusCode::OK);
    assert_eq!(
        stream
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );

    for event in [
        SessionEvent {
            session_id: "someone-else".into(),
            event: Some("leak".into()),
            data: "{}".into(),
        },
        SessionEvent {
            session_id: session.clone(),
            event: Some("test".into()),
            data: "{\"ok\":true}".into(),
        },
        SessionEvent {
            session_id: session.clone(),
            event: None,
            data: "plain".into(),
        },
    ] {
        events.send(event).expect("a subscriber is listening");
    }

    let mut body = stream.bytes_stream();
    let mut text = String::new();
    while !text.contains("data: plain\n\n") {
        let chunk = tokio::time::timeout(
            Duration::from_secs(2),
            futures_util::StreamExt::next(&mut body),
        )
        .await
        .expect("timely event chunk")
        .expect("event chunk")
        .expect("event bytes");
        text.push_str(&String::from_utf8_lossy(&chunk));
    }
    assert_eq!(text, "event: test\ndata: {\"ok\":true}\n\ndata: plain\n\n");
}

// ---------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bearer_auth_rejects_with_plain_text_before_anything_else() {
    let endpoint = spawn(Some("golden-secret")).await;
    let http = Client::new();

    // The auth rejection sets a bare `text/plain`, unlike the other rejections.
    let missing = post(&endpoint, None, None, &init_body("golden")).await;
    assert_content(missing, StatusCode::UNAUTHORIZED, "text/plain", "unauthorized").await;
    let wrong = http
        .post(&endpoint)
        .bearer_auth("wrong")
        .json(&init_body("golden"))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        http.get(&endpoint).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.delete(&endpoint).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    let allowed = http
        .post(&endpoint)
        .header("authorization", "Bearer  golden-secret ")
        .json(&init_body("golden"))
        .send()
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK, "the token is trimmed");
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_address_already_in_use_is_a_bind_error() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let bind_addr = taken.local_addr().unwrap();
    let err = run_http(
        demo(),
        HttpServerConfig {
            bind_addr,
            auth_token: None,
        },
    )
    .await
    .expect_err("the port is taken");
    match err {
        Error::ServerBind { addr, .. } => assert_eq!(addr, bind_addr),
        other => panic!("expected ServerBind, got {other:?}"),
    }
}
