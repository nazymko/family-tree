#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The kind of a source and its document details through the agent API.

mod common;

use common::*;
use gt_session::{EditCommand, SourceInput};
use serde_json::{Value, json};

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Registry extracts"}))["batch"].clone()
}

#[test]
fn an_agent_adds_a_source_with_its_kind_and_reads_it_back() {
    let host = empty_host();
    let batch = begin(&host);
    let added = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Extract, Koval", "kind": "civil_registry_extract",
               "issuer": "CNAP of Lviv", "doc_number": "No 12/34", "doc_date": "5 MAR 2021"}),
    );
    let id = added["id"].clone();
    let got = call(&host, "get_source", json!({"id": id}));
    assert_eq!(got["kind"], "civil_registry_extract");
    assert_eq!(got["issuer"], "CNAP of Lviv");
    assert_eq!(got["docNumber"], "No 12/34");
    assert_eq!(got["docDate"], "5 Mar 2021");
    let listed = call(&host, "list_sources", json!({}));
    assert_eq!(listed["items"][0]["kind"], "civil_registry_extract");
}

#[test]
fn a_kind_outside_the_list_is_refused_and_the_tool_names_the_list() {
    let host = empty_host();
    let batch = begin(&host);
    let msg = call_err(
        &host,
        "add_source",
        json!({"batch": batch, "title": "X", "kind": "scroll"}),
    );
    assert!(msg.contains("metric_book"), "{msg}");
    let ok = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Y", "kind": "other:Family letter"}),
    );
    let got = call(&host, "get_source", json!({"id": ok["id"]}));
    assert_eq!(got["kind"], "other:Family letter");
}

#[test]
fn an_agent_fills_its_own_source_but_leaves_the_owners_kind_alone() {
    let host = empty_host();
    let batch = begin(&host);
    let mine = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Mine"}),
    )["id"]
        .clone();
    call(
        &host,
        "update_own_source",
        json!({"batch": batch, "source": mine, "kind": "photo", "issuer": "Studio"}),
    );
    assert_eq!(
        call(&host, "get_source", json!({"id": mine}))["kind"],
        "photo"
    );
    call(
        &host,
        "update_own_source",
        json!({"batch": batch, "source": mine, "kind": "gravestone"}),
    );
    assert_eq!(
        call(&host, "get_source", json!({"id": mine}))["kind"],
        "gravestone"
    );

    // A source the owner made and typed: the agent can fill the empty issuer, not change the kind.
    let owners = with(&host, |s| {
        s.apply(EditCommand::SaveSource {
            source: SourceInput {
                title: Some("Owner source".into()),
                kind: Some("metric_book".into()),
                ..Default::default()
            },
        })
        .unwrap()
        .touched[0]
    });
    let msg = call_err(
        &host,
        "update_own_source",
        json!({"batch": batch, "source": owners, "kind": "census"}),
    );
    assert!(!msg.is_empty());
    call(
        &host,
        "update_own_source",
        json!({"batch": batch, "source": owners, "issuer": "Parish office"}),
    );
    let got = call(&host, "get_source", json!({"id": owners}));
    assert_eq!(got["kind"], "metric_book");
    assert_eq!(got["issuer"], "Parish office");
}
