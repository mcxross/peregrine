use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub fn create_list_mcp_servers_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "server".to_string(),
            JsonSchema::string(Some(
                "Exact MCP server name to inspect. Omit to list every server that currently exposes tools to the model."
                    .to_string(),
            )),
        ),
        (
            "cursor".to_string(),
            JsonSchema::string(Some(
                "Opaque cursor from a previous list_mcp_servers call; omit for the first page."
                    .to_string(),
            )),
        ),
        (
            "limit".to_string(),
            JsonSchema::integer(Some(
                "Maximum number of tool entries to return. Defaults to 100 and cannot exceed 200."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_mcp_servers".to_string(),
        description: "Lists MCP servers and the tools they currently expose to the model. Use this when the user asks which MCP servers, MCPs, or MCP tools are available. This reports both user-configured servers and host-provided default servers through the same live MCP client infrastructure."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, /*required*/ None, Some(false.into())),
        output_schema: None,
    })
}

pub fn create_list_mcp_resources_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "server".to_string(),
            JsonSchema::string(Some(
                "MCP server name. Omit to list resources from every configured server.".to_string(),
            )),
        ),
        (
            "cursor".to_string(),
            JsonSchema::string(Some(
                "Opaque cursor from a previous list_mcp_resources call; omit for the first page."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_mcp_resources".to_string(),
        description: "Lists resources provided by MCP servers. Resources allow servers to share data that provides context to language models, such as files, database schemas, or application-specific information. Do not use this to answer which MCP servers or tools are available; call list_mcp_servers instead. An empty resource result does not mean MCP tools are unavailable. Prefer resources over web search when possible.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, /*required*/ None, Some(false.into())),
        output_schema: None,
    })
}

pub fn create_list_mcp_resource_templates_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "server".to_string(),
            JsonSchema::string(Some(
                "MCP server name. Omit to list resource templates from every configured server."
                    .to_string(),
            )),
        ),
        (
            "cursor".to_string(),
            JsonSchema::string(Some(
                "Opaque cursor from a previous list_mcp_resource_templates call; omit for the first page."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_mcp_resource_templates".to_string(),
        description: "Lists resource templates provided by MCP servers. Parameterized resource templates allow servers to share data that takes parameters and provides context to language models, such as files, database schemas, or application-specific information. Do not use this to answer which MCP servers or tools are available; call list_mcp_servers instead. An empty resource-template result does not mean MCP tools are unavailable. Prefer resource templates over web search when possible.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, /*required*/ None, Some(false.into())),
        output_schema: None,
    })
}

pub fn create_read_mcp_resource_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "server".to_string(),
            JsonSchema::string(Some(
                "MCP server name exactly as configured. Must match the 'server' field returned by list_mcp_resources."
                    .to_string(),
            )),
        ),
        (
            "uri".to_string(),
            JsonSchema::string(Some(
                "Resource URI to read. Must be one of the URIs returned by list_mcp_resources."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "read_mcp_resource".to_string(),
        description:
            "Read a specific resource from an MCP server given the server name and resource URI."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["server".to_string(), "uri".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "mcp_resource_spec_tests.rs"]
mod tests;
