use super::*;
use codex_tools::ResponsesApiWebSearchFilters;
use codex_tools::ResponsesApiWebSearchUserLocation;
use peregrine_types::config_types::WebSearchContextSize;
use peregrine_types::config_types::WebSearchFilters;
use peregrine_types::config_types::WebSearchUserLocation;
use peregrine_types::config_types::WebSearchUserLocationType;
use pretty_assertions::assert_eq;

#[test]
fn image_generation_tool_matches_expected_spec() {
    assert_eq!(
        create_image_generation_tool("png"),
        ToolSpec::ImageGeneration {
            output_format: "png".to_string(),
        }
    );
}

#[test]
fn web_search_tool_preserves_configured_options() {
    assert_eq!(
        create_web_search_tool(WebSearchToolOptions {
            web_search_mode: Some(WebSearchMode::Live),
            web_search_config: Some(&WebSearchConfig {
                filters: Some(WebSearchFilters {
                    allowed_domains: Some(vec!["example.com".to_string()]),
                }),
                user_location: Some(WebSearchUserLocation {
                    r#type: WebSearchUserLocationType::Approximate,
                    country: Some("US".to_string()),
                    region: None,
                    city: None,
                    timezone: Some("America/Los_Angeles".to_string()),
                }),
                search_context_size: Some(WebSearchContextSize::Low),
            }),
            web_search_tool_type: WebSearchToolType::TextAndImage,
        }),
        Some(ToolSpec::WebSearch {
            external_web_access: Some(true),
            filters: Some(ResponsesApiWebSearchFilters {
                allowed_domains: Some(vec!["example.com".to_string()]),
            }),
            user_location: Some(ResponsesApiWebSearchUserLocation {
                r#type: WebSearchUserLocationType::Approximate,
                country: Some("US".to_string()),
                region: None,
                city: None,
                timezone: Some("America/Los_Angeles".to_string()),
            }),
            search_context_size: Some(WebSearchContextSize::Low),
            search_content_types: Some(vec!["text".to_string(), "image".to_string()]),
        })
    );
}

#[test]
fn web_search_tool_is_absent_when_disabled() {
    assert_eq!(
        create_web_search_tool(WebSearchToolOptions {
            web_search_mode: Some(WebSearchMode::Disabled),
            web_search_config: None,
            web_search_tool_type: WebSearchToolType::Text,
        }),
        None
    );
}
