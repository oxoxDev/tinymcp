# MCP tools

This module adapts tools advertised by MCP servers into `tinytools::Tool`
values. `tools_for` builds one model-facing tool per usable server tool;
`McpToolInvoker` keeps execution behind the host's chosen registry and policy.

## Names and declarations

Names are provider-safe and stable across process restarts. When server labels
would produce the same name, both tools receive a suffix derived from their
server identity so adding or removing another source cannot rebind a recorded
tool name. `tool_parameters` sanitizes remote descriptions and titles and
ensures the root schema declares an object.

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
