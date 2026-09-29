# Serving MCP from `tinymcp`

**Status:** Implemented. **Owner:** tinymcp maintainers.

## Problem

OpenHuman served MCP with a hand-rolled JSON-RPC layer, session handling and
two transports that knew nothing OpenHuman-specific, tangled with the host
policy that decides which tools exist and what running one means. Any other
host wanting to serve MCP would have copied the lot.

## Goals and non-goals

Goal: move the generic MCP server half out of OpenHuman behind a handler
trait. Non-goal: moving any OpenHuman policy, or changing a byte on the wire.

## Proposed behavior

- `tinymcp::server` owns the protocol: framing, batching, notifications,
  version negotiation over `SUPPORTED_PROTOCOL_VERSIONS`, method routing
  (`initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`,
  `resources/templates/list`, `resources/read`) and every error shape.
- A host supplies an `McpServerHandler`: server identity and instructions,
  source-type prefix, tool catalog and calls, resource catalog and reads.
  Handler errors are `ToolCallError`, whose variant picks the JSON-RPC code.
- `run_stdio(handler, reader, writer)` is always built.
  `run_http(handler, HttpServerConfig)` is behind the optional `server-http`
  feature (axum 0.8 + tokio-stream), so a host that does not listen on a
  socket links no server stack.
- Transport headers reach the handler through `RequestContext`; host-specific
  header semantics (OpenHuman's subagent depth) stay in the host.
- These are in-process Rust APIs. Nothing is added to `tinymcp-bus`, which
  stays transport-free.

## Invariants and constraints

- Wire output is byte-identical to OpenHuman's pre-move server. OpenHuman keeps
  golden fixtures that ran unchanged across the move; this crate's tests pin
  the same shapes against a demo handler.
- Host policy stays in the host: configuration, security policy, audit, agent
  turns, tool catalog, prompt resources, subagent depth, token minting.

## Acceptance criteria

- OpenHuman's golden protocol and HTTP fixtures pass unchanged against the
  moved implementation.
- `cargo test --workspace` passes with and without `--all-features`, and every
  file under `crates/tinymcp/src/server/` meets the 90% line-coverage gate.
- `cargo tree -p tinymcp-bus` shows no transport or runtime.

## Open questions

None.
