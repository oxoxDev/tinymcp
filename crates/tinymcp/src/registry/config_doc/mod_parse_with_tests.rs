//! [`parse_with`] and [`render_declared`]: the reading of `mcp.json` for a
//! host that keeps its own store — extension fields, host fields, lenient
//! reads — and writing those declarations back.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tinymcp_bus::Transport;

use super::*;
use crate::Error;

const HOST_FIELDS: &[&str] = &["readOnlyTools", "authSecret"];

fn host() -> ParseOptions<'static> {
    ParseOptions {
        host_fields: HOST_FIELDS,
        lenient: false,
    }
}

fn lenient() -> ParseOptions<'static> {
    ParseOptions {
        lenient: true,
        ..host()
    }
}

fn refusal(doc: &Value, options: &ParseOptions<'_>) -> String {
    match parse_with(doc, options).unwrap_err() {
        Error::ConfigDoc { detail } => detail,
        other => panic!("expected a config-doc refusal, got {other:?}"),
    }
}

#[test]
fn the_extension_fields_are_read() {
    let report = parse_with(
        &json!({ "mcpServers": { "notion": {
            "url": "https://mcp.notion.test/mcp",
            "allowedTools": [" search ", "fetch"],
            "disallowedTools": ["delete"],
            "timeoutSecs": 45,
        } } }),
        &ParseOptions::default(),
    )
    .unwrap();
    assert!(report.rejected.is_empty());
    let notion = &report.declared[0];
    assert_eq!(notion.allowed_tools, ["search", "fetch"]);
    assert_eq!(notion.disallowed_tools, ["delete"]);
    assert_eq!(notion.timeout_secs, Some(45));
    assert!(notion.host_fields.is_empty());
}

#[test]
fn the_strict_parse_still_refuses_the_extension_fields() {
    let error = parse(&json!({ "mcpServers": { "a": { "command": "npx", "timeoutSecs": 5 } } }))
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "`a` has a `timeoutSecs` field this host doesn't understand; it accepts url, headers, command, args, env, description and enabled"
    );
    let declared = parse(&json!({ "mcpServers": { "a": { "command": "npx" } } })).unwrap();
    assert!(declared[0].allowed_tools.is_empty());
    assert_eq!(declared[0].timeout_secs, None);
    assert!(declared[0].host_fields.is_empty());
}

#[test]
fn registered_host_fields_are_carried_verbatim() {
    let report = parse_with(
        &json!({ "mcpServers": { "a": {
            "url": "https://a.test/mcp",
            "readOnlyTools": ["list"],
            "authSecret": null,
        } } }),
        &host(),
    )
    .unwrap();
    assert_eq!(
        report.declared[0].host_fields,
        BTreeMap::from([("readOnlyTools".to_string(), json!(["list"]))])
    );
}

#[test]
fn an_unregistered_field_is_refused_naming_what_is_accepted() {
    let doc = json!({ "mcpServers": { "a": { "url": "https://a.test/mcp", "cwd": "/tmp" } } });
    assert_eq!(
        refusal(&doc, &host()),
        "`a` has a `cwd` field this host doesn't understand; it accepts url, headers, command, args, env, description, enabled, allowedTools, disallowedTools, timeoutSecs, readOnlyTools and authSecret"
    );
}

#[test]
fn a_host_field_named_like_a_standard_one_reads_as_the_standard_one() {
    let report = parse_with(
        &json!({ "mcpServers": { "a": { "url": "https://a.test/mcp", "enabled": false } } }),
        &ParseOptions {
            host_fields: &["enabled", "url"],
            lenient: false,
        },
    )
    .unwrap();
    assert!(!report.declared[0].enabled);
    assert!(report.declared[0].host_fields.is_empty());
}

#[test]
fn extension_field_refusals_name_the_entry_and_the_field() {
    let cases = [
        (
            json!({ "allowedTools": "search" }),
            "`a`.allowedTools is a list of tool names",
        ),
        (
            json!({ "allowedTools": [1] }),
            "`a`.allowedTools holds tool names only",
        ),
        (
            json!({ "disallowedTools": ["  "] }),
            "`a`.disallowedTools holds tool names only",
        ),
        (
            json!({ "timeoutSecs": 0 }),
            "`a`.timeoutSecs is a whole number of seconds above zero",
        ),
        (
            json!({ "timeoutSecs": "30" }),
            "`a`.timeoutSecs is a whole number of seconds above zero",
        ),
        (
            json!({ "timeoutSecs": -1 }),
            "`a`.timeoutSecs is a whole number of seconds above zero",
        ),
    ];
    for (extra, expected) in cases {
        let mut entry = json!({ "url": "https://a.test/mcp" });
        entry
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let doc = json!({ "mcpServers": { "a": entry } });
        assert_eq!(refusal(&doc, &host()), expected);
    }
}

#[test]
fn null_extension_fields_read_as_absent() {
    let report = parse_with(
        &json!({ "mcpServers": { "a": {
            "url": "https://a.test/mcp",
            "allowedTools": null,
            "timeoutSecs": null,
        } } }),
        &host(),
    )
    .unwrap();
    assert!(report.declared[0].allowed_tools.is_empty());
    assert_eq!(report.declared[0].timeout_secs, None);
}

#[test]
fn a_strict_read_refuses_the_document_on_the_first_bad_entry() {
    let doc = json!({ "mcpServers": {
        "bad": { "url": "https://b.test/mcp", "command": "npx" },
        "good": { "url": "https://g.test/mcp" },
    } });
    assert_eq!(
        refusal(&doc, &host()),
        "`bad` names both a `url` and a `command`; a server is dialled one way"
    );
}

#[test]
fn a_lenient_read_drops_bad_entries_and_keeps_the_rest() {
    let doc = json!({ "mcpServers": {
        "bad": { "url": "https://b.test/mcp", "command": "npx" },
        "good": { "url": "https://g.test/mcp" },
        "list": [1],
        "  ": { "url": "https://e.test/mcp" },
        "odd": { "url": "https://o.test/mcp", "cwd": "/" },
    } });
    let report = parse_with(&doc, &lenient()).unwrap();
    assert_eq!(
        report
            .declared
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        ["good"]
    );
    let rejected: Vec<&str> = report.rejected.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(rejected, ["", "bad", "list", "odd"]);
    assert_eq!(
        report.rejected[0].detail,
        "a server needs a name — one entry's key is empty"
    );
    assert!(
        report.rejected[2]
            .detail
            .starts_with("`list` holds an object")
    );
}

#[test]
fn two_keys_naming_one_server_are_refused_or_the_second_dropped() {
    let doc = json!({ "mcpServers": {
        "a": { "url": "https://one.test/mcp" },
        "a ": { "url": "https://two.test/mcp" },
    } });
    assert_eq!(refusal(&doc, &host()), "`a` is declared twice");

    let report = parse_with(&doc, &lenient()).unwrap();
    assert_eq!(report.declared.len(), 1);
    assert_eq!(
        report.declared[0].transport,
        Transport::HttpRemote {
            url: "https://one.test/mcp".into()
        }
    );
    assert_eq!(report.rejected[0].detail, "`a` is declared twice");
}

#[test]
fn an_unreadable_root_is_refused_even_when_lenient() {
    for doc in [json!([]), json!({}), json!({ "mcpServers": [] })] {
        assert!(matches!(
            parse_with(&doc, &lenient()),
            Err(Error::ConfigDoc { .. })
        ));
    }
}

#[test]
fn a_rendered_declaration_reads_back_as_itself_without_credentials() {
    let doc = json!({ "mcpServers": {
        "notion": {
            "url": "https://mcp.notion.test/mcp",
            "headers": { "Authorization": "Bearer secret" },
            "description": "Notes",
            "enabled": false,
            "allowedTools": ["search"],
            "disallowedTools": ["delete"],
            "timeoutSecs": 30,
            "readOnlyTools": ["search"],
        },
        "fs": { "command": "npx", "args": ["-y", "fs"], "env": { "ROOT": "/" } },
    } });
    let declared = parse_with(&doc, &host()).unwrap().declared;
    let rendered = render_declared(&declared);

    assert_eq!(
        rendered,
        json!({ "mcpServers": {
            "fs": { "command": "npx", "args": ["-y", "fs"] },
            "notion": {
                "url": "https://mcp.notion.test/mcp",
                "description": "Notes",
                "enabled": false,
                "allowedTools": ["search"],
                "disallowedTools": ["delete"],
                "timeoutSecs": 30,
                "readOnlyTools": ["search"],
            },
        } })
    );
    assert!(!rendered.to_string().contains("secret"));

    let reread = parse_with(&rendered, &host()).unwrap().declared;
    let without_credentials: Vec<Declared> = declared
        .into_iter()
        .map(|mut d| {
            d.credentials = None;
            d
        })
        .collect();
    let mut expected = without_credentials;
    expected.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(reread, expected);
}

#[test]
fn a_command_without_args_renders_without_an_args_list() {
    let declared = parse_with(
        &json!({ "mcpServers": { "a": { "command": "uvx", "description": "" } } }),
        &ParseOptions::default(),
    );
    assert!(declared.is_err(), "a blank description is refused");
    let declared = parse_with(
        &json!({ "mcpServers": { "a": { "command": "uvx" } } }),
        &ParseOptions::default(),
    )
    .unwrap()
    .declared;
    assert_eq!(
        render_declared(&declared),
        json!({ "mcpServers": { "a": { "command": "uvx" } } })
    );
}
