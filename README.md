# Rust Template

A production-ready Rust 2024 TinyBus module template used by TinyHumans AI. It
ships the workspace layout, TinyBus ABI adapter, error handling, testing,
documentation, CI, and multi-platform release workflow that every new
integration in this organization starts from.

It is a two-crate cargo workspace. `crates/tinymcp-bus` is the wire contract —
member names, payload types, and the contract version, with no transport and no
behavior — and `crates/tinymcp` is the implementation, built as both an `rlib`
and the `cdylib` TinyBus loads. A host that only makes calls depends on the
contract crate alone and compiles neither the module nor `tinybus` itself.

## Use This Template

Choose **Use this template** on GitHub, create a repository, then work through
the checklist at the top of [`AGENTS.md`](AGENTS.md):

- rename the `crates/tinymcp` and `crates/tinymcp-bus` directories and the
  `name` fields in their manifests, and set the shared `description`,
  `repository`, `keywords`, and `categories`;
- update this README and the crate documentation in `crates/tinymcp/src/lib.rs`;
- replace the placeholder `greeting` module with the first real feature area, in
  both crates: the payload types in the contract, the behavior in the module;
- rename the TinyBus interface, object path, and member constants in
  `crates/tinymcp-bus/src/names/`, and the matching `provides` / `methods`
  declarations in `crates/tinymcp/src/tinybus_module/`;
- update the security contact and repository links in the community files;
- replace `ROADMAP.md` with the real plan, or delete it;
- change the license if GPL-3.0-only is not appropriate.

Search for `template` and `tinymcp_bus` to find every remaining
template-specific value.

## What You Get

| Area | What is configured |
| --- | --- |
| Layout | A cargo workspace under `crates/`, split into a dependency-light wire contract and the module that implements it; directory modules with `mod.rs` / `types.rs` / `test.rs`, a crate-wide error type, integration tests, and a runnable example |
| Lints | `unsafe_code` forbidden, `missing_docs`, clippy `all` + `pedantic`, no `unwrap`/`expect`/`panic`/`todo` in library code — all declared once in `[workspace.lints]` so every crate, local run, and CI run agree |
| CI | Format, clippy, build, test (default and all features), a run of the bundled example, an assertion that the contract crate stays transport-free, at least 90% line coverage in every source file, rustdoc with `-D warnings`, an MSRV build, and a `cargo-deny` supply-chain check |
| Release | Manual `workflow_dispatch` bump that validates, versions, tags, and creates installable native module packages for every supported platform |
| Community | Issue and pull request templates, Dependabot, contributing, security, support, and code of conduct docs |
| Agents | [`AGENTS.md`](AGENTS.md) as the single source of truth, symlinked as `CLAUDE.md`, plus a `.claude/settings.json` allowlist for the standard commands |
| Vendor | TinyBus host types and module SDK pinned as the `vendor/tinybus` build-time submodule |

## Layout

```text
Cargo.toml              # virtual workspace: members, shared metadata, lints
crates/
├── tinymcp-bus/       # the wire contract — what crosses the bus
│   ├── README.md       # why the contract is its own crate
│   └── src/
│       ├── lib.rs      # crate docs + the entire public re-export surface
│       ├── names/      # interface, object path, one constant per member
│       ├── greeting/   # payload types, one directory per family
│       │   ├── mod.rs
│       │   ├── types.rs
│       │   └── test.rs
│       └── version/    # contract version and the host bind rule
└── template/           # the module — behavior, adapter, and the cdylib
    ├── src/
    │   ├── lib.rs      # crate docs + public surface, re-exporting the contract
    │   ├── error/      # crate-wide `Error` and `Result<T>`
    │   ├── greeting/   # one directory per feature area
    │   └── tinybus_module/   # bus interface, setup, and ABI v1 exports
    ├── tests/
    │   └── public_api.rs     # integration tests against the public API only
    └── examples/
        ├── basic.rs                  # ordinary library API usage
        ├── verify_module.rs          # local dynamic-module verification
        └── verify_github_release.rs  # tagged-release download and bus call
vendor/
└── tinybus/            # pinned TinyBus git submodule
docs/
├── README.md           # documentation index and conventions
├── specs/              # behavior and architecture specifications
├── plans/              # implementation-ordered delivery plans
└── adr/                # immutable architecture decision records
```

The split is the point. A payload type describes what a frame carries; the
behavior that answers it is a different obligation. `template` depends on
`tinymcp-bus` and re-exports all of it, so `tinymcp::GreetRequest` and
`tinymcp_bus::GreetRequest` are the *same* type rather than structural twins,
and a host is never forced to choose between linking the whole module and
redefining the vocabulary. See
[`crates/tinymcp-bus/README.md`](crates/tinymcp-bus/README.md).

Within each crate, feature areas use directory modules: implementation and
exports live in `mod.rs`, substantial types move to `types.rs`, and unit tests
live in `test.rs`. [`AGENTS.md`](AGENTS.md) holds the complete repository
guidance, and `CLAUDE.md` is a symlink to it so every coding agent reads one
source of truth.

## Development

Clone with submodules, or initialize them before building:

```sh
git submodule update --init --recursive
```

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
cargo run -p tinymcp --example basic
cargo build -p tinymcp --release --lib   # produces the installable cdylib
```

Those four checks are exactly what CI runs. Optional extras:

```sh
cargo doc --no-deps --all-features   # CI builds this with RUSTDOCFLAGS="-D warnings"
cargo deny check all                 # supply-chain check; see deny.toml
cargo install cargo-llvm-cov         # once, before running the coverage gate
.github/scripts/check-file-coverage.sh 90 coverage.json
```

## Releasing

Run the **Release** workflow from the Actions tab with a `patch`, `minor`, or
`major` bump. Use `current` only to resume an interrupted release whose version
commit and tag already exist. The workflow revalidates the workspace, versions
and tags it — one `[workspace.package]` version that every member inherits —
builds `crates/tinymcp` as a TinyBus `cdylib`, and creates a GitHub release.
Assets follow `template-<version>-<platform>.<tar.gz|zip>` and contain the
native module, its SHA-256 `modules.toml`, license, and
[`MODULE.md`](MODULE.md). Every release also publishes `checksum.toml`, which
TinyBus uses to verify an archive before extraction. The workflow loads the
published Ubuntu archive through TinyBus's GitHub release API and calls its
`Greet` method before declaring the release successful. TinyBus itself is not
shipped by this repository; the pinned submodule is the build-time SDK. The stable native
matrix covers Ubuntu 22.04 and 24.04 on x86_64 and ARM64; Fedora 43 and 44 on
x86_64 and ARM64; rolling Arch Linux on its officially supported x86_64
architecture; macOS 15 and 26 on Intel and Apple Silicon; Windows Server 2022
and 2025 on x86_64; and Windows 11 on ARM64. Preview, deprecated, and unofficial
architecture images are not release gates. Do not hand-edit the version in the
root `Cargo.toml`.

## Documentation

- [`AGENTS.md`](AGENTS.md) — repository guidelines for humans and agents
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — how to propose a change
- [`docs/specs/`](docs/specs/README.md) — behavior and architecture specs
- [`docs/plans/`](docs/plans/README.md) — test-first implementation plans
- [`docs/adr/`](docs/adr/0001-record-architecture-decisions.md) — architecture
  decision records
- [`SECURITY.md`](SECURITY.md) — how to report a vulnerability

## License

GPL-3.0-only. See [LICENSE](LICENSE).

## Agent tools

Enable the `tools` feature to expose each server tool as a
[`tinytools::Tool`](https://github.com/tinyhumansai/tinytools) through
`tinymcp::tools`:

- **Names** read `mcp_<server>_<tool>_<server-id digest>` (for example,
  `mcp_ticktick_read_goals_a1b2c3`). The server identity keeps a name stable
  when another server with the same readable label is added or removed.
  provider-safe and at most 64 bytes. A digest suffix is added only when a
  name is too long or two servers would collide.
- **Exposure** is deferred by default, so a tool is found through the
  harness's tool search. A server, or individual tools on it, can be made
  direct (`McpExposure`).
- **A persistent tool cache** (`mcp_tool_cache` in the store) is written on
  every successful listing and keyed by a fingerprint of the server's
  definition. `McpRegistry::cached_overview` and
  `McpServerRegistry::cached_tools` answer from it without dialling, so a
  host can offer every tool at boot.
- **Calls** go through an `McpToolInvoker`. Both registries implement it; a
  host wraps one with its own approvals, audit and screening.
- **The generic bridge** is three tools over one configured-server registry:
  `McpListServersTool`, `McpListToolsTool` and `McpCallTool`
  (`mcp_list_servers`, `mcp_list_tools`, `mcp_call_tool`). `McpCallTool` asks a
  host-supplied `ActGate` before it sends anything. Server and tool names lose
  the backticks and asterisks a model wraps them in (`` `docs` ``, `**docs**`,
  `` `docs`. ``); `_`, `.` and other characters are kept.
- **The call outcome.** Every `mcp_call_tool` result for a call the act gate
  allowed and that names a server, a tool and `arguments` carries a
  `tinymcp_bus::McpCallOutcome` in `ToolResult::metadata`, with
  `kind` set to `MCP_CALL_RESULT_KIND` (`"mcp_call"`): the server and tool,
  `ok` when the server answered, and otherwise the error's wire name and
  whether it was a 401 that advertised OAuth. It is host-only — never rendered
  to the model — so a host meters answered calls and surfaces failures from it
  instead of parsing text. A call missing one of those fields, or refused by
  the gate, fails before a result exists and carries no outcome. The server and
  tool are reported as the caller named them, unscrubbed. Read it back with
  `McpCallOutcome::from_metadata`; decoding rejects a wrong `kind` or an `ok`
  that disagrees with `error`.
- **`SecretScrubber`** removes a server's own credentials (tokens, basic-auth
  pairs, header and query values, URL userinfo) from whatever it echoes back;
  every bridge tool applies it, and a host wraps its own invokers with it.
  `with_secrets` adds values the host keeps outside the server's config, each
  redacted as a strict credential: anywhere in the text, even when short.
- **`tool_result`** maps a rendered MCP result onto `tinytools::ToolResult`,
  bounding oversized blocks.

`tinytools` is a git dependency so a host that links another checkout of it can
`[patch]` the two into one package.

## `mcp.json` and OAuth for hosts with their own store

`registry::config_doc` reads and writes the `{ "mcpServers": { … } }` document.
`parse` and `render` are the install store's reading and projection.
A host that keeps declarations itself uses `parse_with` and `render_declared`
instead: `parse_with` also reads `allowedTools`, `disallowedTools` and
`timeoutSecs`, carries the fields the host registers in
`ParseOptions::host_fields` through verbatim in `Declared::host_fields`, and
with `lenient` set drops a bad entry into `ParseReport::rejected` rather than
refusing the document. Credentials are write-only in both.

`registry::OAuthFlow::refresh` refreshes an expired token from a host's
`OAuthCredentialStore`, re-checking the token endpoint when
`require_public_endpoints()` is set; the free `refresh_if_expired` does not
check. `registry::OAuthBundle` is the stored refresh bundle's shape.
`OAuthFlow::with_client_name` sets the name dynamic registration sends, which
the authorization server shows on its consent screen; it defaults to
`DEFAULT_CLIENT_NAME` (`TinyMCP`).

## Static linking

Enable the `static-link` feature when compiling this module into a Rust host. It
exposes `tinybus_module::TINYBUS_MODULE_ABI_V1`,
`tinybus_module::tinybus_module_manifest_v1`, and
`tinybus_module::tinybus_module_init_v1` as Rust-addressable entries for the
TinyBus linked-module loader. The default build retains the native loadable
module exports. Build without `static-link` when packaging a `cdylib`.
Since `--all-features` includes `static-link`, its `cdylib` has Rust-addressable
entries rather than exported C ABI symbols; run `verify_module` against a
default-feature build. The static entry test covers the all-features mode,
while `verify_module` parses and checks the generated manifest in dynamic mode.
