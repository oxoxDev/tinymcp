//! The stdio transport for an MCP server.
//!
//! Newline-delimited JSON-RPC: one request (or batch) per line in, one
//! response line out per line that needs an answer. Blank lines are skipped.
//! The whole stream is one client, so it is one [`ClientSession`] — the name
//! the client gives in `initialize` attributes everything it does after.
//!
//! The server runs until the request stream ends. Diagnostics never go to the
//! response stream; on a real stdio server that is the client's stdin.

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use super::{ClientSession, McpServerHandler, RequestHeaders, handle_line};
use crate::error::{Error, Result};

/// Serves MCP over a reader/writer pair until the reader ends.
///
/// # Errors
///
/// [`Error::ServerIo`] when a request cannot be read — including a line that
/// is not UTF-8 — or a response cannot be written.
pub async fn run_stdio<R, W>(
    handler: Arc<dyn McpServerHandler>,
    reader: R,
    mut writer: W,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut session = ClientSession::new(handler.source_type_prefix());
    // Stdio carries no transport headers.
    let headers = RequestHeaders::new();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.map_err(Error::server_io)? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_line(&*handler, &mut session, &headers, trimmed).await {
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(Error::server_io)?;
            writer.write_all(b"\n").await.map_err(Error::server_io)?;
            writer.flush().await.map_err(Error::server_io)?;
        }
    }
    tracing::debug!("[mcp_server] stdin closed; exiting");
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
