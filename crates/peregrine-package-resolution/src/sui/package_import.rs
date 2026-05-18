use base64::{engine::general_purpose, Engine};
use serde::Deserialize;
use std::time::Duration;

const SUI_PACKAGE_GRAPHQL_QUERY: &str = r#"
query PeregrinePackageById($address: SuiAddress!, $after: String) {
  object(address: $address) {
    asMovePackage {
      address
      version
      digest
      modules(first: 50, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          name
          bytes
          disassembly
        }
      }
    }
  }
}
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedMovePackage {
    pub address: String,
    pub version: u64,
    pub digest: String,
    pub modules: Vec<FetchedMoveModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedMoveModule {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub disassembly: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageGraphQlData {
    object: Option<PackageGraphQlObject>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageGraphQlObject {
    as_move_package: Option<GraphQlMovePackage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlMovePackage {
    address: String,
    version: u64,
    digest: String,
    modules: GraphQlMoveModuleConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlMoveModuleConnection {
    page_info: GraphQlPageInfo,
    nodes: Vec<GraphQlMoveModule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlMoveModule {
    name: String,
    bytes: Option<String>,
    disassembly: Option<String>,
}

pub fn normalize_sui_package_id(package_id: &str) -> Result<String, String> {
    let package_id = package_id.trim();
    let hex = package_id
        .strip_prefix("0x")
        .or_else(|| package_id.strip_prefix("0X"))
        .unwrap_or(package_id);

    if hex.is_empty()
        || hex.len() > 64
        || !hex.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err("Enter a Sui package ID as a 0x-prefixed hex address.".to_string());
    }

    let normalized = hex.trim_start_matches('0').to_ascii_lowercase();

    Ok(format!(
        "0x{}",
        if normalized.is_empty() {
            "0"
        } else {
            normalized.as_str()
        }
    ))
}

pub fn validated_graphql_url(graph_ql_url: &str) -> Result<String, String> {
    let graph_ql_url = graph_ql_url.trim();

    if !(graph_ql_url.starts_with("https://") || graph_ql_url.starts_with("http://")) {
        return Err("GraphQL endpoint must start with http:// or https://.".to_string());
    }

    Ok(graph_ql_url.to_string())
}

pub async fn fetch_move_package_from_graphql(
    graph_ql_url: &str,
    package_id: &str,
) -> Result<FetchedMovePackage, String> {
    let graph_ql_url = validated_graphql_url(graph_ql_url)?;
    let package_id = normalize_sui_package_id(package_id)?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| format!("Could not create GraphQL client: {error}"))?;
    let mut after: Option<String> = None;
    let mut package_ref: Option<(String, u64, String)> = None;
    let mut modules = Vec::new();

    loop {
        let request_body = serde_json::json!({
            "query": SUI_PACKAGE_GRAPHQL_QUERY,
            "variables": {
                "address": package_id,
                "after": after,
            },
        })
        .to_string();
        let response = client
            .post(&graph_ql_url)
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await
            .map_err(|error| {
                format!(
                    "Could not fetch package {package_id} from GraphQL endpoint {graph_ql_url}: {}",
                    describe_reqwest_error(&error)
                )
            })?;
        let status = response.status();
        let body = response.text().await.map_err(|error| {
            format!(
                "Could not read GraphQL response from {graph_ql_url}: {}",
                describe_reqwest_error(&error)
            )
        })?;

        if !status.is_success() {
            return Err(format!(
                "GraphQL endpoint returned HTTP {status} while fetching {package_id}: {body}"
            ));
        }

        let response: GraphQlResponse<PackageGraphQlData> = serde_json::from_str(&body)
            .map_err(|error| format!("Could not parse GraphQL response: {error}"))?;

        if !response.errors.is_empty() {
            let messages = response
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; ");

            return Err(format!("GraphQL package query failed: {messages}"));
        }

        let package = response
            .data
            .and_then(|data| data.object)
            .and_then(|object| object.as_move_package)
            .ok_or_else(|| {
                format!("{package_id} was not found as a Move package on this network.")
            })?;

        package_ref.get_or_insert_with(|| {
            (
                package.address.clone(),
                package.version,
                package.digest.clone(),
            )
        });

        for module in package.modules.nodes {
            let encoded = module.bytes.ok_or_else(|| {
                format!(
                    "GraphQL response did not include bytecode for module `{}`.",
                    module.name
                )
            })?;
            let bytecode = decode_graphql_module_bytes(&module.name, &encoded)?;

            modules.push(FetchedMoveModule {
                name: module.name,
                bytecode,
                disassembly: module.disassembly,
            });
        }

        if !package.modules.page_info.has_next_page {
            break;
        }

        after = Some(package.modules.page_info.end_cursor.ok_or_else(|| {
            format!(
                "GraphQL response for {package_id} indicated more modules but omitted endCursor."
            )
        })?);
    }

    let Some((address, version, digest)) = package_ref else {
        return Err(format!(
            "{package_id} was not found as a Move package on this network."
        ));
    };

    if modules.is_empty() {
        return Err(format!("{address} does not contain any Move modules."));
    }

    Ok(FetchedMovePackage {
        address: normalize_sui_package_id(&address)?,
        version,
        digest,
        modules,
    })
}

pub fn decode_graphql_module_bytes(module_name: &str, encoded: &str) -> Result<Vec<u8>, String> {
    let encoded = encoded.trim();

    if let Some(hex) = encoded
        .strip_prefix("0x")
        .or_else(|| encoded.strip_prefix("0X"))
    {
        return hex::decode(hex).map_err(|error| {
            format!("Could not decode hex bytecode for module `{module_name}`: {error}")
        });
    }

    general_purpose::STANDARD.decode(encoded).map_err(|error| {
        format!("Could not decode base64 bytecode for module `{module_name}`: {error}")
    })
}

fn describe_reqwest_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        format!("{error} (request timed out)")
    } else if error.is_connect() {
        format!("{error} (could not connect)")
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_sui_package_ids() {
        assert_eq!(normalize_sui_package_id("0x0002").unwrap(), "0x2");
        assert_eq!(normalize_sui_package_id("F5EA").unwrap(), "0xf5ea");
        assert!(normalize_sui_package_id("not-hex").is_err());
    }

    #[test]
    fn decodes_hex_and_base64_graphql_module_bytes() {
        assert_eq!(
            decode_graphql_module_bytes("m", "0x0102ff").unwrap(),
            vec![1, 2, 255]
        );
        assert_eq!(
            decode_graphql_module_bytes("m", "AQID").unwrap(),
            vec![1, 2, 3]
        );
    }
}
