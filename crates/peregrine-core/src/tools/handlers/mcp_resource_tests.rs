#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use super::*;
use codex_mcp::ToolInfo;
use pretty_assertions::assert_eq;
use rmcp::model::AnnotateAble;
use serde_json::json;
use std::sync::Arc;

fn tool_info(server_name: &str, namespace: &str, tool_name: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool_name.to_string(),
        callable_namespace: namespace.to_string(),
        namespace_description: None,
        tool: rmcp::model::Tool::new(
            tool_name.to_string(),
            format!("{tool_name} test tool"),
            Arc::new(rmcp::model::object(json!({
                "type": "object",
                "properties": {},
            }))),
        ),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    }
}

fn resource(uri: &str, name: &str) -> Resource {
    rmcp::model::RawResource {
        uri: uri.to_string(),
        name: name.to_string(),
        title: None,
        description: None,
        mime_type: None,
        size: None,
        icons: None,
        meta: None,
    }
    .no_annotation()
}

fn template(uri_template: &str, name: &str) -> ResourceTemplate {
    rmcp::model::RawResourceTemplate {
        uri_template: uri_template.to_string(),
        name: name.to_string(),
        title: None,
        description: None,
        mime_type: None,
        icons: None,
    }
    .no_annotation()
}

#[test]
fn resource_with_server_serializes_server_field() {
    let entry = ResourceWithServer::new("test".to_string(), resource("memo://id", "memo"));
    let value = serde_json::to_value(&entry).expect("serialize resource");

    assert_eq!(value["server"], json!("test"));
    assert_eq!(value["uri"], json!("memo://id"));
    assert_eq!(value["name"], json!("memo"));
}

#[test]
fn mcp_server_inventory_groups_and_sorts_live_tools() {
    let payload = ListMcpServersPayload::from_tools(
        vec![
            tool_info("beta", "mcp__beta", "search"),
            tool_info("alpha", "mcp__alpha", "write"),
            tool_info("alpha", "mcp__alpha", "read"),
        ],
        None,
        0,
        100,
    )
    .expect("build inventory");

    assert_eq!(
        payload,
        ListMcpServersPayload {
            servers: vec![
                McpServerInventoryEntry {
                    name: "alpha".to_string(),
                    tools: vec![
                        McpToolInventoryEntry {
                            name: "read".to_string(),
                            callable_name: "read".to_string(),
                            namespace: "mcp__alpha".to_string(),
                        },
                        McpToolInventoryEntry {
                            name: "write".to_string(),
                            callable_name: "write".to_string(),
                            namespace: "mcp__alpha".to_string(),
                        },
                    ],
                },
                McpServerInventoryEntry {
                    name: "beta".to_string(),
                    tools: vec![McpToolInventoryEntry {
                        name: "search".to_string(),
                        callable_name: "search".to_string(),
                        namespace: "mcp__beta".to_string(),
                    }],
                },
            ],
            server_count: 2,
            tool_count: 3,
            next_cursor: None,
        }
    );
}

#[test]
fn mcp_server_inventory_filters_and_paginates_tools() {
    let tools = vec![
        tool_info("alpha", "mcp__alpha", "read"),
        tool_info("alpha", "mcp__alpha", "write"),
        tool_info("beta", "mcp__beta", "search"),
    ];

    let first_page =
        ListMcpServersPayload::from_tools(tools.clone(), Some("alpha"), 0, 1).expect("first page");
    assert_eq!(
        first_page,
        ListMcpServersPayload {
            servers: vec![McpServerInventoryEntry {
                name: "alpha".to_string(),
                tools: vec![McpToolInventoryEntry {
                    name: "read".to_string(),
                    callable_name: "read".to_string(),
                    namespace: "mcp__alpha".to_string(),
                }],
            }],
            server_count: 1,
            tool_count: 2,
            next_cursor: Some("1".to_string()),
        }
    );

    let second_page =
        ListMcpServersPayload::from_tools(tools, Some("alpha"), 1, 1).expect("second page");
    assert_eq!(
        second_page,
        ListMcpServersPayload {
            servers: vec![McpServerInventoryEntry {
                name: "alpha".to_string(),
                tools: vec![McpToolInventoryEntry {
                    name: "write".to_string(),
                    callable_name: "write".to_string(),
                    namespace: "mcp__alpha".to_string(),
                }],
            }],
            server_count: 1,
            tool_count: 2,
            next_cursor: None,
        }
    );
}

#[test]
fn list_resources_payload_from_single_server_copies_next_cursor() {
    let result = ListResourcesResult {
        meta: None,
        next_cursor: Some("cursor-1".to_string()),
        resources: vec![resource("memo://id", "memo")],
    };
    let payload = ListResourcesPayload::from_single_server("srv".to_string(), result);
    let value = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(value["server"], json!("srv"));
    assert_eq!(value["nextCursor"], json!("cursor-1"));
    let resources = value["resources"].as_array().expect("resources array");
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["server"], json!("srv"));
}

#[test]
fn list_resources_payload_from_all_servers_is_sorted() {
    let mut map = HashMap::new();
    map.insert("beta".to_string(), vec![resource("memo://b-1", "b-1")]);
    map.insert(
        "alpha".to_string(),
        vec![resource("memo://a-1", "a-1"), resource("memo://a-2", "a-2")],
    );

    let payload = ListResourcesPayload::from_all_servers(map);
    let value = serde_json::to_value(&payload).expect("serialize payload");
    let uris: Vec<String> = value["resources"]
        .as_array()
        .expect("resources array")
        .iter()
        .map(|entry| entry["uri"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(
        uris,
        vec![
            "memo://a-1".to_string(),
            "memo://a-2".to_string(),
            "memo://b-1".to_string()
        ]
    );
}

#[test]
fn call_tool_result_from_content_marks_success() {
    let result = call_tool_result_from_content("{}", Some(true));
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content.len(), 1);
}

#[test]
fn parse_arguments_handles_empty_and_json() {
    assert!(
        parse_arguments(" \n\t").unwrap().is_none(),
        "expected None for empty arguments"
    );

    assert!(
        parse_arguments("null").unwrap().is_none(),
        "expected None for null arguments"
    );

    let value = parse_arguments(r#"{"server":"figma"}"#)
        .expect("parse json")
        .expect("value present");
    assert_eq!(value["server"], json!("figma"));
}

#[test]
fn template_with_server_serializes_server_field() {
    let entry = ResourceTemplateWithServer::new("srv".to_string(), template("memo://{id}", "memo"));
    let value = serde_json::to_value(&entry).expect("serialize template");

    assert_eq!(
        value,
        json!({
            "server": "srv",
            "uriTemplate": "memo://{id}",
            "name": "memo"
        })
    );
}
