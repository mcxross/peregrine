use super::super::discover_move_project_model;
use std::{fs, path::Path};
use tempfile::tempdir;

fn write_package(root: &Path, source: &str) {
    fs::write(
        root.join("Move.toml"),
        r#"
[package]
name = "demo"
"#,
    )
    .expect("manifest");
    fs::create_dir_all(root.join("sources")).expect("sources");
    fs::write(root.join("sources/main.move"), source).expect("source");
}

#[test]
fn call_graph_resolves_local_qualified_alias_and_external_calls() {
    let temp = tempdir().expect("tempdir");
    write_package(
        temp.path(),
        r#"
module demo::helper {
    public fun ping() {}
}

module demo::main {
    use demo::helper;
    use demo::helper::{ping as imported_ping};

    fun local() {}

    public fun run() {
        local();
        helper::ping();
        imported_ping();
        sui::transfer::share_object(0);
    }
}
"#,
    );

    let project = discover_move_project_model(temp.path());
    let run_id = project
        .call_graph
        .nodes
        .iter()
        .find(|node| node.module_name == "main" && node.function_name == "run")
        .expect("run node")
        .id
        .clone();
    let local_id = project
        .call_graph
        .nodes
        .iter()
        .find(|node| node.module_name == "main" && node.function_name == "local")
        .expect("local node")
        .id
        .clone();
    let ping_id = project
        .call_graph
        .nodes
        .iter()
        .find(|node| node.module_name == "helper" && node.function_name == "ping")
        .expect("ping node")
        .id
        .clone();

    assert!(
        project
            .call_graph
            .edges
            .iter()
            .any(|edge| edge.source == run_id && edge.target == local_id && edge.is_resolved)
    );
    assert!(
        project
            .call_graph
            .edges
            .iter()
            .filter(|edge| edge.source == run_id && edge.target == ping_id)
            .map(|edge| edge.call_count)
            .sum::<usize>()
            == 2
    );
    assert!(
        project
            .call_graph
            .unresolved_calls
            .iter()
            .any(|call| call.raw_target == "sui::transfer::share_object")
    );
}

#[test]
fn call_graph_preserves_use_fun_methods_and_unknown_methods() {
    let temp = tempdir().expect("tempdir");
    write_package(
        temp.path(),
        r#"
module demo::main {
    public struct Box has drop { value: u64 }

    public fun get(box: &Box): u64 { box.value }

    use fun get as Box.value;

    public fun run(box: &Box) {
        box.value();
        box.unknown();
    }
}
"#,
    );

    let project = discover_move_project_model(temp.path());

    assert!(
        project
            .call_graph
            .edges
            .iter()
            .any(|edge| edge.call_kind == "method"
                && edge.raw_target == ".value"
                && edge.is_resolved)
    );
    assert!(
        project
            .call_graph
            .unresolved_calls
            .iter()
            .any(|call| call.raw_target == ".unknown")
    );
    let node_ids = project
        .call_graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        project
            .call_graph
            .edges
            .iter()
            .all(|edge| node_ids.contains(edge.source.as_str())
                && node_ids.contains(edge.target.as_str()))
    );
}

#[test]
fn graph_resolution_respects_explicit_addresses() {
    let temp = tempdir().expect("tempdir");
    write_package(
        temp.path(),
        r#"
module demo::transfer {
    public fun share_object() {}
}

module demo::object {
    public struct UID has store {}
}

module demo::main {
    public struct Holder has store { id: sui::object::UID }

    public fun run() {
        sui::transfer::share_object();
    }
}
"#,
    );

    let project = discover_move_project_model(temp.path());
    let local_share_object_id = project
        .call_graph
        .nodes
        .iter()
        .find(|node| {
            node.module_name == "transfer"
                && node.function_name == "share_object"
                && !node.is_external
        })
        .expect("local transfer::share_object")
        .id
        .clone();

    assert!(
        !project
            .call_graph
            .edges
            .iter()
            .any(|edge| edge.target == local_share_object_id)
    );
    assert!(project.call_graph.edges.iter().any(|edge| {
        edge.raw_target == "sui::transfer::share_object" && edge.is_external && !edge.is_resolved
    }));

    let local_uid_id = project
        .type_graph
        .nodes
        .iter()
        .find(|node| {
            node.module_name.as_deref() == Some("object") && node.name == "UID" && !node.is_external
        })
        .expect("local object::UID")
        .id
        .clone();

    assert!(!project.type_graph.edges.iter().any(|edge| {
        edge.relationship == "field"
            && edge.field_name.as_deref() == Some("id")
            && edge.target == local_uid_id
    }));
    assert!(project.type_graph.edges.iter().any(|edge| {
        edge.relationship == "field"
            && edge.field_name.as_deref() == Some("id")
            && project.type_graph.nodes.iter().any(|node| {
                node.id == edge.target
                    && node.is_external
                    && node.qualified_name == "sui::object::UID"
            })
    }));
}

#[test]
fn type_graph_extracts_fields_signatures_construction_and_annotations() {
    let temp = tempdir().expect("tempdir");
    write_package(
        temp.path(),
        r#"
module demo::main {
    public struct Coin<phantom T> has key, store { id: UID, amount: u64 }
    public struct Receipt has store { coin: Coin<u64> }

    public fun make(amount: u64): Receipt {
        let receipt: Receipt = Receipt { coin: Coin<u64> { id: object::new(), amount } };
        receipt
    }

    public fun unwrap(receipt: Receipt): Coin<u64> {
        let Receipt { coin } = receipt;
        coin
    }
}
"#,
    );

    let project = discover_move_project_model(temp.path());

    let builtin_abilities = |name: &str| {
        project
            .type_graph
            .nodes
            .iter()
            .find(|node| node.kind == "builtin" && node.name == name)
            .unwrap_or_else(|| panic!("builtin node {name}"))
            .abilities
            .clone()
    };

    assert_eq!(builtin_abilities("u64"), ["copy", "drop", "store"]);
    assert_eq!(builtin_abilities("bool"), ["copy", "drop", "store"]);
    assert_eq!(builtin_abilities("address"), ["copy", "drop", "store"]);
    assert_eq!(builtin_abilities("signer"), ["drop"]);
    assert_eq!(builtin_abilities("vector"), ["copy", "drop", "store"]);

    let coin_node = project
        .type_graph
        .nodes
        .iter()
        .find(|node| node.name == "Coin")
        .expect("Coin node");
    assert!(coin_node.span.is_some());
    assert_eq!(coin_node.type_parameters.len(), 1);
    assert_eq!(coin_node.type_parameters[0].name, "T");
    assert!(coin_node.type_parameters[0].is_phantom);

    assert!(
        project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "field" && edge.field_name.as_deref() == Some("coin"))
    );
    let receipt_coin_edge = project
        .type_graph
        .edges
        .iter()
        .find(|edge| edge.relationship == "field" && edge.field_name.as_deref() == Some("coin"))
        .expect("Receipt.coin field edge");
    assert_eq!(receipt_coin_edge.confidence, "syntactic");
    assert_eq!(
        receipt_coin_edge.type_expression.as_deref(),
        Some("Coin<u64>")
    );
    assert_eq!(
        receipt_coin_edge.declaring_field_name.as_deref(),
        Some("coin")
    );
    assert!(!receipt_coin_edge.source_spans.is_empty());
    assert!(!receipt_coin_edge.evidence.is_empty());

    assert!(project.type_graph.edges.iter().any(|edge| {
        edge.relationship == "genericArgument"
            && edge.type_argument_name.as_deref() == Some("T")
            && edge.type_expression.as_deref() == Some("u64")
            && edge.declaring_field_name.as_deref() == Some("coin")
    }));
    assert!(
        project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "return")
    );
    assert!(
        project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "construction")
    );
    assert!(
        project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "destructuring")
    );
    assert!(
        project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "annotation")
    );
    assert!(
        project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "genericArgument")
    );
}

#[test]
fn summary_enrichment_adds_external_datatype_edges() {
    let temp = tempdir().expect("tempdir");
    write_package(
        temp.path(),
        r#"
module demo::main {
    public fun ping() {}
}
"#,
    );
    fs::create_dir_all(temp.path().join("package_summaries/demo")).expect("summary dir");
    fs::write(
        temp.path().join("package_summaries/address_mapping.json"),
        r#"{"demo":"0x1","external":"0x2","sui":"0x2"}"#,
    )
    .expect("mapping");
    fs::write(
        temp.path().join("package_summaries/demo/main.json"),
        r#"{
  "id": { "address": "demo", "name": "main" },
  "functions": {},
  "structs": {
    "Holder": {
      "fields": {
        "positional_fields": false,
        "fields": {
          "item": {
            "type_": {
              "Datatype": {
                "module": { "address": "external", "name": "asset" },
                "name": "Coin",
                "type_arguments": []
              }
            }
          },
          "id": {
            "type_": {
              "Datatype": {
                "module": { "address": "sui", "name": "object" },
                "name": "ID",
                "type_arguments": []
              }
            }
          },
          "uid": {
            "type_": {
              "Datatype": {
                "module": { "address": "sui", "name": "object" },
                "name": "UID",
                "type_arguments": []
              }
            }
          },
          "table": {
            "type_": {
              "Datatype": {
                "module": { "address": "sui", "name": "table" },
                "name": "Table",
                "type_arguments": []
              }
            }
          }
        }
      }
    }
  },
  "enums": {}
}"#,
    )
    .expect("summary");

    let project = discover_move_project_model(temp.path());

    assert!(
        project
            .type_graph
            .nodes
            .iter()
            .any(|node| node.is_external && node.qualified_name == "external::asset::Coin")
    );
    let summary_abilities = |qualified_name: &str| {
        project
            .type_graph
            .nodes
            .iter()
            .find(|node| node.qualified_name == qualified_name)
            .unwrap_or_else(|| panic!("summary node {qualified_name}"))
            .abilities
            .clone()
    };
    assert_eq!(
        summary_abilities("sui::object::ID"),
        ["copy", "drop", "store"]
    );
    assert_eq!(summary_abilities("sui::object::UID"), ["store"]);
    assert_eq!(summary_abilities("sui::table::Table"), ["key", "store"]);
    let table_node = project
        .type_graph
        .nodes
        .iter()
        .find(|node| node.qualified_name == "sui::table::Table")
        .expect("table node");
    assert_eq!(
        table_node
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        ["K", "V"]
    );
    assert!(
        project
            .type_graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "field" && edge.field_name.as_deref() == Some("item"))
    );
    assert!(project.type_graph.edges.iter().any(|edge| {
        edge.relationship == "field"
            && edge.field_name.as_deref() == Some("item")
            && edge.confidence == "heuristic"
            && !edge.evidence.is_empty()
    }));
}
