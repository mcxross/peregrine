use base64::{engine::general_purpose, Engine};
use peregrine_static_analysis::sui::{decompile_package_bytecode_modules, MoveModuleBytecodeInput};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const IMPORTED_PACKAGES_DIRECTORY: &str = "imported-packages";
const IMPORT_METADATA_FILE: &str = "import.json";
const PROJECT_METADATA_DIRECTORY: &str = ".peregrine";
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

pub struct MovePackageImportRequest {
    pub app_data_dir: PathBuf,
    pub network_id: String,
    pub graph_ql_url: String,
    pub package_id: String,
}

pub struct ImportedMovePackage {
    pub root_path: PathBuf,
}

struct MaterializedMoveModule {
    name: String,
    bytecode: Vec<u8>,
    decompiled: peregrine_static_analysis::sui::DecompiledMoveModule,
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

#[derive(Clone)]
struct FetchedMovePackage {
    address: String,
    version: u64,
    digest: String,
    modules: Vec<FetchedMoveModule>,
}

#[derive(Clone)]
struct FetchedMoveModule {
    name: String,
    bytecode: Vec<u8>,
    disassembly: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportedMovePackageMetadata {
    package_id: String,
    network_id: String,
    graph_ql_url: String,
    version: u64,
    digest: String,
    module_count: usize,
    imported_at: u64,
}

pub async fn import_move_package_by_id(
    request: MovePackageImportRequest,
) -> Result<ImportedMovePackage, String> {
    let package_id = normalize_sui_package_id(&request.package_id)?;
    let graph_ql_url = validated_graphql_url(&request.graph_ql_url)?;
    let fetched_package = fetch_move_package_from_graphql(&graph_ql_url, &package_id).await?;
    let root_path = materialize_imported_move_package(
        &request.app_data_dir,
        &request.network_id,
        &graph_ql_url,
        fetched_package,
    )?;

    Ok(ImportedMovePackage { root_path })
}

fn normalize_sui_package_id(package_id: &str) -> Result<String, String> {
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

fn validated_graphql_url(graph_ql_url: &str) -> Result<String, String> {
    let graph_ql_url = graph_ql_url.trim();

    if !(graph_ql_url.starts_with("https://") || graph_ql_url.starts_with("http://")) {
        return Err("GraphQL endpoint must start with http:// or https://.".to_string());
    }

    Ok(graph_ql_url.to_string())
}

async fn fetch_move_package_from_graphql(
    graph_ql_url: &str,
    package_id: &str,
) -> Result<FetchedMovePackage, String> {
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
            .post(graph_ql_url)
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
        address,
        version,
        digest,
        modules,
    })
}

fn decode_graphql_module_bytes(module_name: &str, encoded: &str) -> Result<Vec<u8>, String> {
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

fn materialize_imported_move_package(
    app_data_dir: &Path,
    network_id: &str,
    graph_ql_url: &str,
    fetched_package: FetchedMovePackage,
) -> Result<PathBuf, String> {
    let network_dir = sanitize_path_component(network_id);
    let package_dir = sanitize_path_component(fetched_package.address.trim_start_matches("0x"));
    let package_name = move_package_name_for_id(&fetched_package.address);
    let package_root = app_data_dir
        .join(IMPORTED_PACKAGES_DIRECTORY)
        .join(network_dir)
        .join(package_dir);
    let sources_dir = package_root.join("sources");
    let build_root = package_root.join("build").join(&package_name);
    let build_sources_dir = build_root.join("sources");
    let bytecode_dir = build_root.join("bytecode_modules");
    let decompiled_dir = package_root.join("decompiled");
    let metadata_dir = package_root.join(PROJECT_METADATA_DIRECTORY);
    let module_count = fetched_package.modules.len();

    if package_root.exists() {
        fs::remove_dir_all(&package_root).map_err(|error| {
            format!(
                "Could not replace previous import at {}: {error}",
                package_root.display()
            )
        })?;
    }

    fs::create_dir_all(&sources_dir)
        .map_err(|error| format!("Could not create {}: {error}", sources_dir.display()))?;
    fs::create_dir_all(&build_sources_dir)
        .map_err(|error| format!("Could not create {}: {error}", build_sources_dir.display()))?;
    fs::create_dir_all(&bytecode_dir)
        .map_err(|error| format!("Could not create {}: {error}", bytecode_dir.display()))?;
    fs::create_dir_all(&decompiled_dir)
        .map_err(|error| format!("Could not create {}: {error}", decompiled_dir.display()))?;
    fs::create_dir_all(&metadata_dir)
        .map_err(|error| format!("Could not create {}: {error}", metadata_dir.display()))?;

    let manifest = format!(
        r#"[package]
name = "{package_name}"
edition = "2024.beta"

[addresses]
{package_name} = "{}"
"#,
        fetched_package.address
    );
    fs::write(package_root.join("Move.toml"), manifest)
        .map_err(|error| format!("Could not write imported Move.toml: {error}"))?;

    let materialized_modules = decompile_fetched_modules(&fetched_package.modules)?;
    write_materialized_modules(
        &sources_dir,
        &build_sources_dir,
        &bytecode_dir,
        &decompiled_dir,
        materialized_modules,
    )?;

    let metadata = ImportedMovePackageMetadata {
        package_id: fetched_package.address,
        network_id: network_id.to_string(),
        graph_ql_url: graph_ql_url.to_string(),
        version: fetched_package.version,
        digest: fetched_package.digest,
        module_count,
        imported_at: now_unix_seconds(),
    };
    let metadata = serde_json::to_string_pretty(&metadata)
        .map_err(|error| format!("Could not serialize import metadata: {error}"))?;

    fs::write(metadata_dir.join(IMPORT_METADATA_FILE), metadata)
        .map_err(|error| format!("Could not write import metadata: {error}"))?;

    Ok(package_root)
}

pub fn refresh_imported_move_package_sources(package_root: &Path) -> Result<(), String> {
    if !package_root
        .join(PROJECT_METADATA_DIRECTORY)
        .join(IMPORT_METADATA_FILE)
        .is_file()
    {
        return Ok(());
    }

    let Some(build_root) = imported_package_build_root(package_root) else {
        return Ok(());
    };
    let bytecode_dir = build_root.join("bytecode_modules");
    let modules = load_materialized_bytecode_modules(package_root, &bytecode_dir)?;

    if modules.is_empty() {
        return Ok(());
    }

    let materialized_modules = decompile_fetched_modules(&modules)?;
    let sources_dir = package_root.join("sources");
    let build_sources_dir = build_root.join("sources");
    let decompiled_dir = package_root.join("decompiled");

    fs::create_dir_all(&sources_dir)
        .map_err(|error| format!("Could not create {}: {error}", sources_dir.display()))?;
    fs::create_dir_all(&build_sources_dir)
        .map_err(|error| format!("Could not create {}: {error}", build_sources_dir.display()))?;
    fs::create_dir_all(&decompiled_dir)
        .map_err(|error| format!("Could not create {}: {error}", decompiled_dir.display()))?;

    write_materialized_modules(
        &sources_dir,
        &build_sources_dir,
        &bytecode_dir,
        &decompiled_dir,
        materialized_modules,
    )
}

fn imported_package_build_root(package_root: &Path) -> Option<PathBuf> {
    let build_dir = package_root.join("build");
    let manifest_name = fs::read_to_string(package_root.join("Move.toml"))
        .ok()
        .and_then(|manifest| package_name(&manifest));

    if let Some(name) = manifest_name {
        let candidate = build_dir.join(name);

        if candidate.join("bytecode_modules").is_dir() {
            return Some(candidate);
        }
    }

    let mut candidates = fs::read_dir(build_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("bytecode_modules").is_dir())
        .collect::<Vec<_>>();
    candidates.sort();

    match candidates.as_slice() {
        [candidate] => Some(candidate.clone()),
        _ => None,
    }
}

fn load_materialized_bytecode_modules(
    package_root: &Path,
    bytecode_dir: &Path,
) -> Result<Vec<FetchedMoveModule>, String> {
    let mut bytecode_paths = fs::read_dir(bytecode_dir)
        .map_err(|error| format!("Could not read {}: {error}", bytecode_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("mv"))
        .collect::<Vec<_>>();
    bytecode_paths.sort();

    bytecode_paths
        .into_iter()
        .map(|bytecode_path| {
            let name = bytecode_path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("module")
                .to_string();
            let bytecode = fs::read(&bytecode_path)
                .map_err(|error| format!("Could not read {}: {error}", bytecode_path.display()))?;
            let disassembly_path = package_root
                .join("decompiled")
                .join(format!("{name}.moveasm"));
            let disassembly = fs::read_to_string(disassembly_path).ok();

            Ok(FetchedMoveModule {
                name,
                bytecode,
                disassembly,
            })
        })
        .collect()
}

fn decompile_fetched_modules(
    modules: &[FetchedMoveModule],
) -> Result<Vec<MaterializedMoveModule>, String> {
    let decompile_inputs = modules
        .iter()
        .map(|module| MoveModuleBytecodeInput {
            name: module.name.clone(),
            bytecode: module.bytecode.clone(),
            disassembly: module.disassembly.clone(),
        })
        .collect::<Vec<_>>();
    let decompiled_modules = decompile_package_bytecode_modules(&decompile_inputs)?;

    Ok(modules
        .iter()
        .cloned()
        .zip(decompiled_modules)
        .map(|(module, decompiled)| MaterializedMoveModule {
            name: module.name,
            bytecode: module.bytecode,
            decompiled,
        })
        .collect())
}

fn write_materialized_modules(
    sources_dir: &Path,
    build_sources_dir: &Path,
    bytecode_dir: &Path,
    decompiled_dir: &Path,
    modules: Vec<MaterializedMoveModule>,
) -> Result<(), String> {
    for module in modules {
        let file_stem = module_file_stem(&module.name, &module.decompiled.name);
        let source_path = sources_dir.join(format!("{file_stem}.move"));
        let build_source_path = build_sources_dir.join(format!("{file_stem}.move"));
        let bytecode_path = bytecode_dir.join(format!("{file_stem}.mv"));
        let disassembly_path = decompiled_dir.join(format!("{file_stem}.moveasm"));

        fs::write(&source_path, &module.decompiled.source)
            .map_err(|error| format!("Could not write {}: {error}", source_path.display()))?;
        fs::write(&build_source_path, &module.decompiled.source)
            .map_err(|error| format!("Could not write {}: {error}", build_source_path.display()))?;
        fs::write(&bytecode_path, module.bytecode)
            .map_err(|error| format!("Could not write {}: {error}", bytecode_path.display()))?;
        fs::write(&disassembly_path, module.decompiled.disassembly)
            .map_err(|error| format!("Could not write {}: {error}", disassembly_path.display()))?;
    }

    Ok(())
}

fn move_package_name_for_id(package_id: &str) -> String {
    let hex = package_id
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();

    if hex.is_empty() {
        "package".to_string()
    } else {
        let visible = hex.chars().take(8).collect::<String>();
        format!("pkg_{visible}")
    }
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package_section = false;

    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or("").trim();

        if line.starts_with('[') && line.ends_with(']') {
            in_package_section = line == "[package]";
            continue;
        }

        if !in_package_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if key.trim() != "name" {
            continue;
        }

        return Some(
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string(),
        );
    }

    None
}

fn module_file_stem(graphql_name: &str, decompiled_name: &str) -> String {
    let stem = sanitize_path_component(decompiled_name);

    if stem == "_" {
        sanitize_path_component(graphql_name)
    } else {
        stem
    }
}

fn sanitize_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('.').trim_matches('_');

    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized.to_string()
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
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
    use tempfile::tempdir;

    fn fixture_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("package-resolution crate should have a crates parent")
            .join("peregrine-indexer/tests/fixtures/sui")
            .join(relative)
    }

    #[test]
    fn refresh_imported_package_replaces_interface_stub_from_bytecode() {
        let temp = tempdir().expect("tempdir");
        let package_root = temp.path();
        let build_root = package_root.join("build/pkg_test");
        let bytecode_dir = build_root.join("bytecode_modules");

        fs::create_dir_all(package_root.join(".peregrine")).expect("metadata dir");
        fs::create_dir_all(package_root.join("sources")).expect("sources dir");
        fs::create_dir_all(&bytecode_dir).expect("bytecode dir");
        fs::write(package_root.join(".peregrine/import.json"), "{}").expect("import metadata");
        fs::write(
            package_root.join("Move.toml"),
            r#"
[package]
name = "pkg_test"
"#,
        )
        .expect("manifest");
        fs::write(
            package_root.join("sources/vault.move"),
            r#"
module bytecode_fixture::vault;

public fun create(): u64 {
    abort 0
}
"#,
        )
        .expect("old stub");
        fs::copy(
            fixture_path("bytecode_full_mode/build/bytecode_fixture/bytecode_modules/vault.mv"),
            bytecode_dir.join("vault.mv"),
        )
        .expect("copy bytecode");

        refresh_imported_move_package_sources(package_root).expect("refresh import");

        let source =
            fs::read_to_string(package_root.join("sources/vault.move")).expect("refreshed source");
        let build_source =
            fs::read_to_string(build_root.join("sources/vault.move")).expect("build source");

        assert!(!source.contains("Fallback interface"));
        assert!(source.contains("fun create"));
        assert!(source.contains("fun deposit"));
        assert!(source.contains("Vault {"));
        assert_eq!(source, build_source);
        assert!(package_root.join("decompiled/vault.moveasm").is_file());
    }
}
