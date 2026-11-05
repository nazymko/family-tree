#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The JSON-RPC layer without a transport, in both protocol eras.

mod common;

use common::*;
use gt_mcp::{RULES, handle_message};
use serde_json::{Value, json};

fn reply(body: &Value) -> Value {
    handle_message(&empty_host(), &ctx(), body).unwrap()
}

/// The spec's rules as the agent should read them: one line each, without notes to the builder.
fn rules_in_the_spec() -> Vec<String> {
    let spec =
        include_str!("../../../docs/superpowers/specs/2026-11-05-mcp-agent-access-design.md")
            .replace('\r', "");
    let section = spec
        .split("### Rules the agent receives")
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap();
    let mut rules: Vec<String> = Vec::new();
    for line in section.lines() {
        let trimmed = line.trim();
        let starts_a_rule = trimmed
            .split_once(". ")
            .is_some_and(|(n, _)| n.parse::<u32>().is_ok());
        if starts_a_rule {
            rules.push(trimmed.to_string());
        } else if !trimmed.is_empty()
            && let Some(rule) = rules.last_mut()
        {
            rule.push(' ');
            rule.push_str(trimmed);
        }
    }
    rules
        .into_iter()
        .map(|rule| match rule.split_once(" (Design call") {
            Some((text, _)) => text.to_string(),
            None => rule,
        })
        .collect()
}

#[test]
fn rules_are_the_owners_rules_verbatim_plus_living() {
    let spec = rules_in_the_spec();
    assert_eq!(spec.len(), 13, "{spec:?}");
    let ours: Vec<&str> = RULES.lines().take(13).collect();
    for (ours, theirs) in ours.iter().zip(&spec) {
        assert_eq!(ours, theirs);
    }
    let rest: Vec<&str> = RULES.lines().skip(13).filter(|l| !l.is_empty()).collect();
    assert_eq!(
        rest,
        [
            "You may never change whether a person counts as alive or dead. Leave a note and report it."
        ]
    );
    assert!(
        !RULES.contains('\r'),
        "the rules read the same on every system"
    );
}

#[test]
fn initialize_old_client_gets_rules() {
    for version in ["2025-03-26", "2025-06-18", "2025-11-25"] {
        let r = reply(&request(
            "initialize",
            json!({
                "protocolVersion": version,
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "1"},
            }),
        ));
        let result = &r["result"];
        assert_eq!(
            result["protocolVersion"], version,
            "the client's version is kept"
        );
        assert_eq!(result["instructions"], gt_mcp::INSTRUCTIONS);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "gene-tree");
        assert!(
            result.get("resultType").is_none(),
            "older results stay in their own shape"
        );
    }
}

#[test]
fn initialize_unknown_version_gets_the_newest_older_one() {
    let r = reply(&request(
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
    ));
    assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
}

#[test]
fn discover_new_client_gets_rules() {
    let r = reply(&modern_request("server/discover", json!({})));
    let result = &r["result"];
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["instructions"], gt_mcp::INSTRUCTIONS);
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(
        result["supportedVersions"],
        json!(["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"])
    );
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "gene-tree"
    );
    assert!(result["ttlMs"].as_u64().is_some());
    assert_eq!(result["cacheScope"], "private");
}

#[test]
fn unsupported_version_32022() {
    let mut body = modern_request("tools/list", json!({}));
    body["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] = json!("1900-01-01");
    let r = reply(&body);
    assert_eq!(r["id"], 1);
    assert_eq!(r["error"]["code"], -32022);
    assert_eq!(r["error"]["data"]["requested"], "1900-01-01");
    assert!(
        r["error"]["data"]["supported"]
            .as_array()
            .unwrap()
            .contains(&json!("2026-07-28"))
    );
}

#[test]
fn missing_required_meta_is_invalid_params() {
    let mut no_caps = modern_request("tools/list", json!({}));
    no_caps["params"]["_meta"]
        .as_object_mut()
        .unwrap()
        .remove("io.modelcontextprotocol/clientCapabilities");
    assert_eq!(reply(&no_caps)["error"]["code"], -32602);
    assert_eq!(
        reply(&request("server/discover", json!({})))["error"]["code"],
        -32602
    );
}

#[test]
fn result_type_complete_on_new() {
    let host = empty_host();
    for (method, params) in [
        ("tools/list", json!({})),
        ("tools/call", json!({"name": "get_rules", "arguments": {}})),
        ("ping", json!({})),
    ] {
        let r = handle_message(&host, &ctx(), &modern_request(method, params)).unwrap();
        assert_eq!(r["result"]["resultType"], "complete", "{method}");
        assert_eq!(
            r["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"], "gene-tree",
            "{method}"
        );
    }
    let list = handle_message(&host, &ctx(), &modern_request("tools/list", json!({}))).unwrap();
    assert!(list["result"]["ttlMs"].as_u64().is_some());
    assert_eq!(list["result"]["cacheScope"], "private");
}

#[test]
fn older_results_have_no_new_fields() {
    let host = empty_host();
    let list = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    for field in ["resultType", "ttlMs", "cacheScope", "_meta"] {
        assert!(list["result"].get(field).is_none(), "{field}");
    }
    let ping = handle_message(&host, &ctx(), &request("ping", json!({}))).unwrap();
    assert_eq!(ping["result"], json!({}));
}

#[test]
fn notification_no_reply() {
    let host = empty_host();
    for method in [
        "notifications/initialized",
        "notifications/cancelled",
        "anything/else",
    ] {
        let body = json!({"jsonrpc": "2.0", "method": method});
        assert_eq!(handle_message(&host, &ctx(), &body), None, "{method}");
    }
    // A client's reply to a request also gets no answer.
    let response = json!({"jsonrpc": "2.0", "id": 9, "result": {}});
    assert_eq!(handle_message(&host, &ctx(), &response), None);
}

#[test]
fn tools_list_deterministic() {
    let host = empty_host();
    let names = || -> Vec<String> {
        let r = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
        r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    };
    let first = names();
    assert_eq!(first, names());
    for tool in [
        "get_rules",
        "tree_summary",
        "search_people",
        "list_people",
        "get_person",
        "get_relatives",
        "get_family",
        "get_source",
        "get_note",
        "get_citation",
        "list_sources",
        "list_places",
        "get_media",
        "get_log",
        "get_history",
        "find_by_number",
        "get_event",
        "get_place",
        "get_repository",
        "list_media",
        "list_notes",
        "list_families",
        "list_repositories",
        "list_events",
        "list_citations",
        "begin_batch",
        "end_batch",
        "add_person",
        "link_people",
        "add_family",
        "add_name",
        "add_event",
        "add_note",
        "add_source",
        "add_citation",
        "add_sibling_marker",
        "set_number",
        "set_sex",
        "update_own_note",
        "update_own_event",
        "update_own_name",
        "update_own_citation",
    ] {
        assert!(first.iter().any(|n| n == tool), "{tool} is missing");
    }
    let list = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    for tool in list["result"]["tools"].as_array().unwrap() {
        assert!(
            tool["description"].as_str().is_some_and(|d| !d.is_empty()),
            "{tool}"
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
}

#[test]
fn unknown_method_and_unknown_tool_are_protocol_errors() {
    let host = empty_host();
    let r = handle_message(&host, &ctx(), &request("no/such", json!({}))).unwrap();
    assert_eq!(r["error"]["code"], -32601);
    let r = handle_message(
        &host,
        &ctx(),
        &request(
            "tools/call",
            json!({"name": "no_such_tool", "arguments": {}}),
        ),
    )
    .unwrap();
    assert_eq!(r["error"]["code"], -32602);
}

#[test]
fn malformed_requests_are_rejected() {
    let host = empty_host();
    for body in [
        json!([{"jsonrpc": "2.0", "id": 1, "method": "ping"}]),
        json!({"jsonrpc": "1.0", "id": 1, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": null, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": true, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": 1, "method": 5}),
    ] {
        let r = handle_message(&host, &ctx(), &body).unwrap();
        assert_eq!(r["error"]["code"], -32600, "{body}");
    }
}
