//! A host serving MCP through the public API alone: implement the handler,
//! hand it to `run_stdio`, and speak JSON-RPC to it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use futures_util::future::BoxFuture;
use serde_json::{Map, Value, json};
use tinymcp::server::args::required_non_empty_string;
use tinymcp::{McpServerHandler, RequestContext, ServerInfo, ServerToolSpec, ToolCallError};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

struct Greeter;

impl McpServerHandler for Greeter {
    fn server_info(&self) -> ServerInfo {
        ServerInfo::new("greeter", "1.0.0")
    }

    fn list_tools<'a>(&'a self, _ctx: &'a RequestContext) -> BoxFuture<'a, Vec<ServerToolSpec>> {
        Box::pin(async {
            vec![ServerToolSpec::new(
                "greet",
                "Greet someone.",
                json!({"type": "object", "properties": {"name": {"type": "string"}}}),
            )]
        })
    }

    fn call_tool<'a>(
        &'a self,
        _ctx: &'a RequestContext,
        name: &'a str,
        arguments: Map<String, Value>,
    ) -> BoxFuture<'a, Result<Value, ToolCallError>> {
        Box::pin(async move {
            if name != "greet" {
                return Err(ToolCallError::InvalidParams(format!(
                    "unknown tool `{name}`"
                )));
            }
            let who = required_non_empty_string(&arguments, "name")?;
            Ok(json!({"content": [{"type": "text", "text": format!("hello, {who}")}]}))
        })
    }
}

#[tokio::test]
async fn a_host_handler_serves_initialize_list_and_call_over_stdio() {
    let (mut client_write, server_read) = duplex(4096);
    let (server_write, mut client_read) = duplex(4096);
    let server = tokio::spawn(tinymcp::run_stdio(
        Arc::new(Greeter),
        server_read,
        server_write,
    ));

    client_write
        .write_all(
            concat!(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"Test Client"}}}"#,
                "\n",
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
                "\n",
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"greet","arguments":"{\"name\":\"Ada\"}"}}"#,
                "\n",
                r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"greet","arguments":{}}}"#,
                "\n",
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    drop(client_write);

    let mut output = String::new();
    client_read.read_to_string(&mut output).await.unwrap();
    server.await.unwrap().unwrap();

    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 4, "{output}");
    let init: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "greeter");
    assert!(init["result"].get("instructions").is_none());
    let list: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(list["result"]["tools"][0]["name"], "greet");
    let call: Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(call["result"]["content"][0]["text"], "hello, Ada");
    assert_eq!(
        lines[3],
        r#"{"error":{"code":-32602,"data":"missing required argument `name`","message":"Invalid params"},"id":4,"jsonrpc":"2.0"}"#
    );
}
