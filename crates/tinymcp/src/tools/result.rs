//! Converting a rendered MCP result into a `tinytools` result.

use serde_json::{Value, json};
use tinymcp_bus::{McpToolContent, McpToolResult};
use tinytools::{ToolContent, ToolResult};

/// The most a pass-through content block may serialize to before its payload
/// is elided.
///
/// A block kind this build does not model is carried through as JSON rather
/// than dropped, and such a block can be a base64 image or audio — megabytes.
/// Above this the payload is replaced with a marker that keeps the block type.
pub const MAX_LLM_BLOCK_BYTES: usize = 64 * 1024;

/// Maps a rendered MCP result onto [`ToolResult`].
///
/// The two shapes match by construction, so this is a mapping rather than a
/// translation. It is written once because spelled out at each call site it
/// would be as many chances to get the error flag the wrong way round.
#[must_use]
pub fn tool_result(result: McpToolResult) -> ToolResult {
    ToolResult {
        content: result
            .content
            .into_iter()
            .map(|block| match block {
                McpToolContent::Text { text } => ToolContent::Text { text },
                McpToolContent::Json { data } => ToolContent::Json { data },
                // The contract's block enum is `#[non_exhaustive]`: a kind this
                // build does not model travels as its JSON, bounded.
                #[allow(unreachable_patterns)]
                other => ToolContent::Json {
                    data: elide_oversized_block(&other),
                },
            })
            .collect(),
        is_error: result.is_error,
        markdown_formatted: result.markdown_formatted,
        ..ToolResult::default()
    }
}

/// An unrecognized block as JSON, its payload elided above
/// [`MAX_LLM_BLOCK_BYTES`].
fn elide_oversized_block(block: &McpToolContent) -> Value {
    let value = serde_json::to_value(block).unwrap_or(Value::Null);
    let serialized = serde_json::to_string(&value).unwrap_or_default();
    if serialized.len() <= MAX_LLM_BLOCK_BYTES {
        return value;
    }
    let kind = value.get("type").cloned().unwrap_or(Value::Null);
    json!({
        "type": kind,
        "data": format!("[{} bytes elided]", serialized.len()),
    })
}
