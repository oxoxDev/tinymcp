//! The `mcp.json` contract — what a read shows, what a write accepts, and
//! what counts as a change — and the reconciliation of a store against it.
//!
//! The document tests are ported verbatim from the host this moved out of;
//! the only edit is reading a refusal through `to_string()`, because a refusal
//! is now an [`Error`] rather than a bare `String`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use serde_json::json;
use tinymcp_bus::{
    CommandKind, InstalledServer, McpClientIdentityConfig, McpRegistryAuthConfig, Transport,
};

use super::*;
use crate::Error;
use crate::registry::{McpRegistry, Store};

fn stdio_row(name: &str) -> InstalledServer {
    InstalledServer {
        server_id: format!("id-{name}"),
        qualified_name: name.to_string(),
        display_name: name.to_string(),
        description: None,
        icon_url: None,
        command_kind: CommandKind::Node,
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@x/y".to_string()],
        env_keys: vec![],
        config: None,
        installed_at: 1,
        last_connected_at: None,
        transport: Transport::Stdio,
        enabled: true,
    }
}

fn http_row(name: &str) -> InstalledServer {
    InstalledServer {
        transport: Transport::HttpRemote {
            url: "https://x.test/mcp".to_string(),
        },
        command: String::new(),
        args: vec![],
        ..stdio_row(name)
    }
}

// ── render ───────────────────────────────────────────────────────────────────

#[test]
fn a_read_shows_the_dial_and_never_a_credential_value() {
    let mut keys = BTreeMap::new();
    keys.insert(
        "id-a".to_string(),
        vec!["API_KEY".to_string(), "__oauth".to_string()],
    );
    let doc = render(&[stdio_row("a"), http_row("b")], &keys);

    assert_eq!(
        doc,
        json!({
            "mcpServers": {
                "a": {
                    "command": "npx",
                    "args": ["-y", "@x/y"],
                    "envKeys": ["API_KEY"],
                    "authConfigured": true,
                },
                "b": {
                    "url": "https://x.test/mcp",
                    "authConfigured": false,
                }
            }
        })
    );
}

#[test]
fn a_read_is_sorted_and_says_when_a_server_is_off() {
    let mut off = stdio_row("zeta");
    off.enabled = false;
    off.description = Some("the last one".to_string());
    let doc = render(&[off, stdio_row("alpha")], &BTreeMap::new());
    let names: Vec<&String> = doc["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(names, ["alpha", "zeta"]);
    assert_eq!(doc["mcpServers"]["zeta"]["enabled"], json!(false));
    assert_eq!(
        doc["mcpServers"]["zeta"]["description"],
        json!("the last one")
    );
    assert!(doc["mcpServers"]["alpha"].get("enabled").is_none());
}

// ── parse ────────────────────────────────────────────────────────────────────

#[test]
fn a_write_reads_both_spellings() {
    let declared = parse(&json!({
        "mcpServers": {
            "local": { "command": "uvx", "args": ["thing"], "env": { "TOKEN": "t" } },
            "hosted": { "type": "http", "url": "https://h.test/mcp", "headers": { "Authorization": "Bearer x" }, "enabled": false },
        }
    }))
    .unwrap();

    let local = declared.iter().find(|d| d.name == "local").unwrap();
    assert_eq!(local.transport, Transport::Stdio);
    assert_eq!(local.command, "uvx");
    assert_eq!(local.args, ["thing"]);
    assert_eq!(
        local.credentials.as_ref().unwrap().get("TOKEN").unwrap(),
        "t"
    );
    assert!(local.enabled);

    let hosted = declared.iter().find(|d| d.name == "hosted").unwrap();
    assert_eq!(
        hosted.transport,
        Transport::HttpRemote {
            url: "https://h.test/mcp".to_string()
        }
    );
    assert!(!hosted.enabled);
    assert_eq!(
        hosted
            .credentials
            .as_ref()
            .unwrap()
            .get("Authorization")
            .unwrap(),
        "Bearer x"
    );
}

#[test]
fn a_write_without_a_credential_block_leaves_it_unsaid() {
    let declared = parse(&json!({ "mcpServers": { "a": { "command": "npx" } } })).unwrap();
    assert_eq!(declared[0].credentials, None);
}

#[test]
fn the_echoed_fields_are_ignored_on_write() {
    let declared = parse(&json!({
        "mcpServers": { "a": { "url": "https://a.test", "envKeys": ["X"], "authConfigured": true } }
    }))
    .unwrap();
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].credentials, None);
}

#[test]
fn refusals_name_the_entry_and_the_field() {
    let cases: Vec<(serde_json::Value, &str)> = vec![
        (json!([]), "holds an object"),
        (json!({}), "no `mcpServers` key"),
        (json!({ "mcpServers": 1 }), "maps a server name"),
        (
            json!({ "mcpServers": { "": {} } }),
            "one entry's key is empty",
        ),
        (json!({ "mcpServers": { "a": "x" } }), "`a` holds an object"),
        (json!({ "mcpServers": { "a": {} } }), "`a` needs a `url`"),
        (
            json!({ "mcpServers": { "a": { "url": "u", "command": "c" } } }),
            "both a `url` and a `command`",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "cwd": "/x" } } }),
            "`cwd` field this host doesn't understand",
        ),
        (
            json!({ "mcpServers": { "a": { "url": "u", "args": ["x"] } } }),
            "`args` only apply to a `command`",
        ),
        (
            json!({ "mcpServers": { "a": { "url": "u", "env": {} } } }),
            "takes `headers`, not `env`",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "env": { "__x": "1" } } } }),
            "reserved",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "env": { "K": 1 } } } }),
            "`a`.env.K holds a string",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "enabled": "yes" } } }),
            "`a`.enabled is true or false",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "   " } } }),
            "`a`.command is empty",
        ),
    ];
    for (doc, expected) in cases {
        let error = parse(&doc).expect_err("refused").to_string();
        assert!(
            error.contains(expected),
            "{doc}: expected {expected:?} in {error:?}"
        );
    }
}

// ── reconciliation helpers ───────────────────────────────────────────────────

#[test]
fn a_re_save_of_what_is_installed_is_not_a_change() {
    let row = stdio_row("a");
    let declared = parse(&render(std::slice::from_ref(&row), &BTreeMap::new())).unwrap();
    assert!(same_dial(&declared[0], &row));
}

#[test]
fn a_changed_argument_or_flag_is_a_change() {
    let row = stdio_row("a");
    let mut declared = parse(&render(std::slice::from_ref(&row), &BTreeMap::new())).unwrap();
    declared[0].args.push("--verbose".to_string());
    assert!(!same_dial(&declared[0], &row));

    let mut declared = parse(&render(std::slice::from_ref(&row), &BTreeMap::new())).unwrap();
    declared[0].enabled = false;
    assert!(!same_dial(&declared[0], &row));
}

#[test]
fn a_declaration_becomes_a_row_keyed_by_its_name() {
    let declared =
        parse(&json!({ "mcpServers": { "fs": { "command": "npx", "args": ["-y", "x"] } } }))
            .unwrap();
    let row = to_installed(&declared[0], "id-1".to_string(), 42);
    assert_eq!(row.server_id, "id-1");
    assert_eq!(row.qualified_name, "fs");
    assert_eq!(row.display_name, "fs");
    assert_eq!(row.command_kind, CommandKind::Node);
    assert_eq!(row.installed_at, 42);
    assert_eq!(row.env_keys.len(), 0);
}

#[test]
fn merging_credentials_replaces_removes_and_keeps() {
    let mut stored = BTreeMap::new();
    stored.insert("A".to_string(), "old".to_string());
    stored.insert("B".to_string(), "keep".to_string());
    stored.insert("C".to_string(), "gone".to_string());
    let mut written = BTreeMap::new();
    written.insert("A".to_string(), "new".to_string());
    written.insert("C".to_string(), String::new());
    written.insert("D".to_string(), "added".to_string());

    let merged = merge_credentials(&stored, &written);
    assert_eq!(merged.get("A").unwrap(), "new");
    assert_eq!(merged.get("B").unwrap(), "keep");
    assert!(!merged.contains_key("C"));
    assert_eq!(merged.get("D").unwrap(), "added");
}

#[test]
fn a_refusal_is_a_config_doc_error_carrying_the_sentence_verbatim() {
    let error = parse(&json!({})).expect_err("refused");
    assert!(matches!(error, Error::ConfigDoc { .. }), "{error:?}");
    assert_eq!(
        error.to_string(),
        "no `mcpServers` key — every server lives under it"
    );
}

#[test]
fn a_null_args_list_and_a_null_flag_read_as_absent() {
    let declared = parse(&json!({
        "mcpServers": { "a": { "command": "c", "args": null, "enabled": null, "env": null, "url": null } }
    }))
    .unwrap();
    assert_eq!(declared[0].args.len(), 0);
    assert!(declared[0].enabled);
    assert_eq!(declared[0].credentials, None);
}

#[test]
fn further_refusals_name_the_entry_and_the_field() {
    let cases: Vec<(serde_json::Value, &str)> = vec![
        (
            json!({ "mcpServers": { "a": { "command": "c", "args": "x" } } }),
            "`a`.args is a list of strings",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "args": [1] } } }),
            "`a`.args holds strings only",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "headers": {} } } }),
            "takes `env`, not `headers`",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "env": { " ": "v" } } } }),
            "`a`.env has an empty key",
        ),
        (
            json!({ "mcpServers": { "a": { "url": "u", "headers": [] } } }),
            "`a`.headers maps a name to a string value",
        ),
        (
            json!({ "mcpServers": { "a": { "command": 3 } } }),
            "`a`.command holds a string",
        ),
    ];
    for (doc, expected) in cases {
        let error = parse(&doc).expect_err("refused").to_string();
        assert!(
            error.contains(expected),
            "{doc}: expected {expected:?} in {error:?}"
        );
    }
}

// ── applying a document to a registry ────────────────────────────────────────

fn registry() -> McpRegistry {
    McpRegistry::new(
        Store::open_in_memory().expect("a store"),
        McpRegistryAuthConfig::default(),
        McpClientIdentityConfig::default(),
        None,
    )
    .expect("the facade builds")
}

fn names(applied: &[AppliedServer]) -> Vec<&str> {
    applied.iter().map(|server| server.name.as_str()).collect()
}

#[tokio::test]
async fn applying_to_an_empty_store_installs_every_declared_server() {
    let registry = registry();
    let report = registry
        .apply_config_doc(&json!({
            "mcpServers": {
                "local": { "command": "uvx", "args": ["thing"], "env": { "TOKEN": "t" } },
                "hosted": { "url": "https://h.test/mcp", "enabled": false },
            }
        }))
        .await
        .unwrap();

    assert_eq!(names(&report.installed), ["hosted", "local"]);
    assert_eq!(report.updated.len(), 0);
    assert_eq!(report.removed.len(), 0);

    let local = registry
        .store()
        .find_server_by_qualified_name("local")
        .unwrap()
        .unwrap();
    assert_eq!(local.command_kind, CommandKind::Python);
    assert_eq!(local.env_keys, ["TOKEN"]);
    assert_eq!(
        registry.store().load_env_values(&local.server_id).unwrap()["TOKEN"],
        "t"
    );
    // Only an enabled server is connected; a disabled one is only recorded.
    assert_eq!(
        report.connect_queued,
        std::slice::from_ref(&local.server_id)
    );
    let installed_ids: Vec<&str> = report
        .installed
        .iter()
        .map(|s| s.server_id.as_str())
        .collect();
    assert!(installed_ids.contains(&local.server_id.as_str()));
}

#[tokio::test]
async fn a_server_absent_from_the_document_is_removed() {
    let registry = registry();
    registry.store().insert_server(&stdio_row("gone")).unwrap();
    registry.store().insert_server(&stdio_row("kept")).unwrap();

    let report = registry
        .apply_config_doc(&json!({
            "mcpServers": { "kept": { "command": "npx", "args": ["-y", "@x/y"] } }
        }))
        .await
        .unwrap();

    assert_eq!(names(&report.removed), ["gone"]);
    assert_eq!(report.removed[0].server_id, "id-gone");
    assert_eq!(report.installed.len(), 0);
    assert_eq!(report.updated.len(), 0);
    assert_eq!(report.connect_queued.len(), 0);
    assert!(registry.store().find_server("id-gone").unwrap().is_none());
}

#[tokio::test]
async fn re_applying_what_a_read_shows_changes_nothing_and_keeps_credentials() {
    // The round trip a user makes by opening the editor and saving: the read
    // shows no credential values, and saving it must not wipe them.
    let registry = registry();
    registry.store().insert_server(&stdio_row("a")).unwrap();
    let mut stored = BTreeMap::new();
    stored.insert("API_KEY".to_string(), "secret".to_string());
    registry.store().set_env_values("id-a", &stored).unwrap();

    let doc = registry.render_config_doc().unwrap();
    assert_eq!(doc["mcpServers"]["a"]["envKeys"], json!(["API_KEY"]));
    assert!(!doc.to_string().contains("secret"));

    let report = registry.apply_config_doc(&doc).await.unwrap();

    assert_eq!(report.installed.len(), 0);
    assert_eq!(report.updated.len(), 0);
    assert_eq!(report.removed.len(), 0);
    assert_eq!(report.connect_queued.len(), 0);
    assert_eq!(registry.store().load_env_values("id-a").unwrap(), stored);
}

#[tokio::test]
async fn a_changed_dial_is_rewritten_in_place_keeping_its_id_and_credentials() {
    let registry = registry();
    registry.store().insert_server(&stdio_row("a")).unwrap();
    let mut stored = BTreeMap::new();
    stored.insert("API_KEY".to_string(), "secret".to_string());
    registry.store().set_env_values("id-a", &stored).unwrap();

    let report = registry
        .apply_config_doc(&json!({
            "mcpServers": { "a": { "command": "npx", "args": ["-y", "@x/z"] } }
        }))
        .await
        .unwrap();

    assert_eq!(names(&report.updated), ["a"]);
    assert_eq!(report.updated[0].server_id, "id-a");
    assert_eq!(report.connect_queued, ["id-a"]);
    let row = registry.store().get_server("id-a").unwrap();
    assert_eq!(row.args, ["-y", "@x/z"]);
    assert_eq!(row.installed_at, 1);
    assert_eq!(row.env_keys, ["API_KEY"]);
    assert_eq!(registry.store().load_env_values("id-a").unwrap(), stored);
}

#[tokio::test]
async fn a_credential_change_alone_updates_and_reconnects_an_enabled_server() {
    let registry = registry();
    registry.store().insert_server(&stdio_row("a")).unwrap();
    let mut stored = BTreeMap::new();
    stored.insert("OLD".to_string(), "1".to_string());
    stored.insert("KEEP".to_string(), "2".to_string());
    registry.store().set_env_values("id-a", &stored).unwrap();

    let report = registry
        .apply_config_doc(&json!({
            "mcpServers": { "a": {
                "command": "npx", "args": ["-y", "@x/y"],
                "env": { "OLD": "", "NEW": "3" }
            } }
        }))
        .await
        .unwrap();

    assert_eq!(names(&report.updated), ["a"]);
    assert_eq!(report.connect_queued, ["id-a"]);
    let values = registry.store().load_env_values("id-a").unwrap();
    assert_eq!(values.keys().collect::<Vec<_>>(), ["KEEP", "NEW"]);
    assert_eq!(
        registry.store().get_server("id-a").unwrap().env_keys,
        ["KEEP", "NEW"]
    );
}

#[tokio::test]
async fn a_credential_block_that_changes_nothing_is_not_an_update() {
    let registry = registry();
    registry.store().insert_server(&stdio_row("a")).unwrap();
    let mut stored = BTreeMap::new();
    stored.insert("K".to_string(), "v".to_string());
    registry.store().set_env_values("id-a", &stored).unwrap();

    for env in [json!({ "K": "v" }), json!({})] {
        let report = registry
            .apply_config_doc(&json!({
                "mcpServers": { "a": { "command": "npx", "args": ["-y", "@x/y"], "env": env } }
            }))
            .await
            .unwrap();
        assert!(report.updated.is_empty(), "{env}");
        assert!(report.connect_queued.is_empty(), "{env}");
    }
}

#[tokio::test]
async fn a_disabled_server_is_updated_but_not_connected() {
    let registry = registry();
    registry.store().insert_server(&stdio_row("a")).unwrap();

    let report = registry
        .apply_config_doc(&json!({
            "mcpServers": { "a": { "command": "npx", "args": ["-y", "@x/y"], "enabled": false } }
        }))
        .await
        .unwrap();

    assert_eq!(names(&report.updated), ["a"]);
    assert_eq!(report.connect_queued.len(), 0);
    assert!(!registry.store().get_server("id-a").unwrap().enabled);
}

#[tokio::test]
async fn two_spellings_of_one_name_are_refused_before_anything_changes() {
    let registry = registry();
    registry.store().insert_server(&stdio_row("other")).unwrap();

    let error = registry
        .apply_config_doc(&json!({
            "mcpServers": { "a": { "command": "x" }, " a ": { "command": "y" } }
        }))
        .await
        .expect_err("a duplicate is refused");

    assert_eq!(error.to_string(), "`a` is declared twice");
    assert!(registry.store().find_server("id-other").unwrap().is_some());
}

#[tokio::test]
async fn an_invalid_document_is_refused_before_anything_changes() {
    let registry = registry();
    registry.store().insert_server(&stdio_row("a")).unwrap();

    let error = registry
        .apply_config_doc(&json!({ "servers": {} }))
        .await
        .expect_err("refused");

    assert!(matches!(error, Error::ConfigDoc { .. }), "{error:?}");
    assert!(registry.store().find_server("id-a").unwrap().is_some());
}

#[test]
fn a_render_of_the_store_names_credentials_and_hides_bookkeeping() {
    let registry = registry();
    registry.store().insert_server(&http_row("b")).unwrap();
    let mut stored = BTreeMap::new();
    stored.insert("Authorization".to_string(), "Bearer x".to_string());
    stored.insert("__oauth".to_string(), "{}".to_string());
    registry.store().set_env_values("id-b", &stored).unwrap();

    let doc = registry.render_config_doc().unwrap();

    assert_eq!(
        doc,
        json!({ "mcpServers": { "b": {
            "url": "https://x.test/mcp",
            "envKeys": ["Authorization"],
            "authConfigured": true,
        } } })
    );
}
