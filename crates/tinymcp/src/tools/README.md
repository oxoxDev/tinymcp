# MCP tools

This module adapts tools advertised by MCP servers into `tinytools::Tool`
values. `tools_for` builds one model-facing tool per usable server tool;
`McpToolInvoker` keeps execution behind the host's chosen registry and policy.

## Names and declarations

Names are provider-safe and stable across process restarts. When server labels
include a suffix derived from their server identity so adding or removing
another source cannot rebind a recorded tool name. `tool_parameters` sanitizes
remote descriptions and titles, ensures the root schema declares an object,
and replaces schemas over the model-facing size limit with an empty object
schema.

Each tool's `family` is its server label. Its `tags` are
`mcp.server:<label>`, `mcp.server_id:<id>` and `mcp.tool:<remote name>`, so a
host's `tinytools::ToolRules` can target one server's tools by the names the
server uses. The label in `mcp.server:` is the configured one, not the
sanitized `family` (`{ "tags": "mcp.tool:delete*" }`). The registered name is a slug
with a digest suffix, which a pattern cannot reliably split. The per-server
`allowed_tools` / `disallowed_tools` lists stay exact, fetch-time filters
inside the registry.

## Exposure and execution

`McpExposure` controls whether tools are offered directly or deferred for tool
search. The adapter does not grant call permission: invocation still requires
the live registry connection and any host-level approvals, auditing, or
screening. Results are converted and bounded before they are returned to a
model.

## Cache behavior

Tool sources may be built from live overviews or persistent cached overviews.
Cached listings are snapshots for discovery and prompt construction only;
they do not authorize invocation. Credential changes invalidate cached
listings, and a later successful connection can populate them again.
