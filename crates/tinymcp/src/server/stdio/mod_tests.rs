//! Unit tests for the stdio transport.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, duplex};

use super::run_stdio;
use crate::Error;
use crate::server::McpServerHandler;
use crate::server::fixture::DemoHandler;

fn demo() -> Arc<dyn McpServerHandler> {
    Arc::new(DemoHandler::new())
}

/// Feeds `input` to a stdio server and returns everything it wrote.
async fn serve(input: &'static [u8]) -> String {
    let (mut client_write, server_read) = duplex(4096);
    let (server_write, mut client_read) = duplex(4096);
    let server = tokio::spawn(run_stdio(demo(), server_read, server_write));
    client_write.write_all(input).await.unwrap();
    drop(client_write);
    let mut output = String::new();
    client_read.read_to_string(&mut output).await.unwrap();
    server.await.unwrap().unwrap();
    output
}

#[tokio::test]
async fn writes_one_line_per_response_and_nothing_for_notifications() {
    let output = serve(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n\
          \n   \n\
          {\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n\
          {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n",
    )
    .await;
    assert_eq!(
        output,
        "{\"id\":1,\"jsonrpc\":\"2.0\",\"result\":{}}\n{\"id\":2,\"jsonrpc\":\"2.0\",\"result\":{}}\n"
    );
}

#[tokio::test]
async fn one_connection_is_one_session() {
    // The client named in `initialize` is who later calls are attributed to.
    let output = serve(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"Cursor\"}}}\n\
          {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"echo\"}}\n",
    )
    .await;
    let responses = output
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(
        responses[1]["result"]["structuredContent"]["source_type"],
        "demo:cursor"
    );
}

#[tokio::test]
async fn an_unreadable_request_stream_is_a_server_io_error() {
    let err = run_stdio(demo(), &b"\xff\xfe\n"[..], tokio::io::sink())
        .await
        .expect_err("invalid utf-8 cannot be read as a line");
    assert!(matches!(err, Error::ServerIo { .. }), "{err:?}");
}

/// A response sink whose peer has gone away.
struct ClosedPipe;

impl AsyncWrite for ClosedPipe {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<io::Result<usize>> {
        Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe)))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn an_unwritable_response_stream_is_a_server_io_error() {
    let request = &b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n"[..];
    let err = run_stdio(demo(), request, ClosedPipe)
        .await
        .expect_err("a closed pipe cannot take a response");
    assert!(matches!(err, Error::ServerIo { .. }), "{err:?}");
}
