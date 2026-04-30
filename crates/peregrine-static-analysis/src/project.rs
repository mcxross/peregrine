mod dependency_graph;
mod source_parser;
mod surface;

use dependency_graph::build_package_dependency_graph;
use serde::Serialize;
use source_parser::discover_modules;
use std::{
    fs,
    path::{Path, PathBuf},
};
use surface::package_surface;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveProject {
    pub packages: Vec<MovePackage>,
    pub dependency_graph: PackageDependencyGraph,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackage {
    pub name: String,
    pub path: String,
    pub manifest_path: String,
    pub surface: MovePackageSurface,
    pub modules: Vec<MoveModule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageSurface {
    pub entry_function_count: usize,
    pub capability_count: usize,
    pub shared_object_count: usize,
    pub address_owned_object_count: usize,
    pub immutable_object_count: usize,
    pub wrapped_object_count: usize,
    pub party_object_count: usize,
    pub admin_control_count: usize,
    pub external_call_count: usize,
    pub public_package_relationship_count: usize,
    pub capability_structs: Vec<String>,
    pub capability_findings: Vec<CapabilityFinding>,
    pub shared_object_structs: Vec<String>,
    pub object_ownership_findings: Vec<ObjectOwnershipFinding>,
    pub admin_control_findings: Vec<AdminControlFinding>,
    pub external_call_findings: Vec<ExternalCallFinding>,
    pub public_package_relationships: Vec<PublicPackageRelationship>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityFinding {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub protected_functions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOwnershipFinding {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub ownership_kind: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub related_functions: Vec<String>,
    pub wrapped_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminControlFinding {
    pub function_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub guarding_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCallFinding {
    pub caller_module: String,
    pub caller_function: String,
    pub target_module: String,
    pub target_function: String,
    pub target: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPackageRelationship {
    pub source_module: String,
    pub source_function: String,
    pub target_module: String,
    pub target_function: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveModule {
    pub name: String,
    pub address: Option<String>,
    pub file_path: String,
    pub structs: Vec<MoveStructSignature>,
    pub functions: Vec<MoveFunctionSignature>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructSignature {
    pub name: String,
    pub abilities: Vec<String>,
    pub fields: Vec<MoveStructField>,
    pub signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructField {
    pub name: String,
    pub type_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFunctionSignature {
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub signature: String,
    pub body: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyGraph {
    pub root: Option<String>,
    pub nodes: Vec<PackageDependencyNode>,
    pub edges: Vec<PackageDependencyEdge>,
    pub summary_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyNode {
    pub id: String,
    pub address: Option<String>,
    pub module_count: usize,
    pub public_function_count: usize,
    pub entry_function_count: usize,
    pub is_root: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyEdge {
    pub source: String,
    pub target: String,
    pub dependency_count: usize,
    pub dependency_kind: String,
}

pub fn discover_move_project(root: &Path) -> MoveProject {
    let mut manifest_paths = Vec::new();

    collect_move_manifests(root, root, &mut manifest_paths);
    manifest_paths.sort();

    let packages = manifest_paths
        .into_iter()
        .filter_map(|manifest_path| build_move_package(root, &manifest_path))
        .collect::<Vec<_>>();
    let graph = build_package_dependency_graph(root, &packages);

    MoveProject {
        packages,
        dependency_graph: graph,
    }
}

fn collect_move_manifests(root: &Path, directory: &Path, manifest_paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_manifests(root, &path, manifest_paths);
            continue;
        }

        if file_type.is_file() && entry.file_name() == "Move.toml" && path.starts_with(root) {
            manifest_paths.push(path);
        }
    }
}

pub(crate) fn root_package_name(packages: &[MovePackage]) -> Option<String> {
    packages
        .iter()
        .find(|move_package| move_package.path.is_empty())
        .or_else(|| packages.first())
        .map(|move_package| move_package.name.clone())
}

fn build_move_package(root: &Path, manifest_path: &Path) -> Option<MovePackage> {
    let package_root = manifest_path.parent()?;
    let manifest = fs::read_to_string(manifest_path).ok()?;
    let name = package_name(&manifest).unwrap_or_else(|| {
        package_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Move package")
            .to_string()
    });
    let path = relative_path(root, package_root)?;
    let manifest_path = relative_path(root, manifest_path)?;
    let mut modules = discover_modules(root, package_root);

    modules.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    let surface = package_surface(&modules);

    Some(MovePackage {
        name,
        path,
        manifest_path,
        surface,
        modules,
    })
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

pub(crate) fn relative_path(root: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}

#[cfg(test)]
mod tests {
    use super::source_parser::parse_module_declarations;
    use super::surface::package_surface;
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn parse_module_declaration(source: &str, root: &Path, path: &Path) -> Option<MoveModule> {
        parse_module_declarations(source, root, path)
            .into_iter()
            .next()
    }

    #[test]
    fn ast_parser_extracts_modern_module_surface() {
        let root = Path::new("/workspace");
        let modules = parse_module_declarations(
            r#"
module demo::modern;

public struct Config<phantom T> has key, store {
    id: UID,
    value: T,
}

native struct Marker has copy;

public(package) fun configure(config: &mut Config<u64>) {
    assert!(true, 0);
}

public(friend) fun friend_only() {}

public entry fun enter() {}

native fun native_noop();
"#,
            root,
            Path::new("/workspace/sources/helper/modern.move"),
        );

        assert_eq!(modules.len(), 1);
        let module = &modules[0];

        assert_eq!(module.name, "modern");
        assert_eq!(module.address.as_deref(), Some("demo"));
        assert_eq!(module.file_path, "sources/helper/modern.move");
        assert_eq!(module.structs.len(), 2);
        assert_eq!(module.structs[0].name, "Config");
        assert_eq!(module.structs[0].abilities, ["key", "store"]);
        assert_eq!(module.structs[0].fields[0].type_name, "UID");
        assert_eq!(module.structs[0].fields[1].type_name, "T");
        assert_eq!(module.structs[1].name, "Marker");
        assert!(module.structs[1].fields.is_empty());

        let configure = module
            .functions
            .iter()
            .find(|function| function.name == "configure")
            .expect("configure function");
        assert_eq!(configure.visibility, "public(package)");
        assert!(!configure.is_transaction_callable);
        assert!(configure
            .body
            .as_deref()
            .is_some_and(|body| body.contains("assert!")));

        let friend_only = module
            .functions
            .iter()
            .find(|function| function.name == "friend_only")
            .expect("friend function");
        assert_eq!(friend_only.visibility, "public(friend)");

        let enter = module
            .functions
            .iter()
            .find(|function| function.name == "enter")
            .expect("entry function");
        assert!(enter.is_entry);
        assert!(enter.is_transaction_callable);

        let native_noop = module
            .functions
            .iter()
            .find(|function| function.name == "native_noop")
            .expect("native function");
        assert!(native_noop.body.is_none());
    }

    #[test]
    fn ast_parser_extracts_modules_from_address_blocks() {
        let root = Path::new("/workspace");
        let modules = parse_module_declarations(
            r#"
address demo {
    module first {
        public fun a() {}
    }

    module second {
        public struct Coin has copy, drop {}
    }
}
"#,
            root,
            Path::new("/workspace/sources/group.move"),
        );
        let names = modules
            .iter()
            .map(|module| (module.address.as_deref(), module.name.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![(Some("demo"), "first"), (Some("demo"), "second")]
        );
    }

    #[test]
    fn discover_project_preserves_nested_source_paths() {
        let temp = tempdir().expect("tempdir");

        fs::write(
            temp.path().join("Move.toml"),
            r#"
[package]
name = "nested_paths"
"#,
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources/helper")).expect("sources");
        fs::write(
            temp.path().join("sources/helper/oracle.move"),
            r#"
module nested_paths::oracle;

public fun ping() {}
"#,
        )
        .expect("module");

        let project = discover_move_project(temp.path());
        let package = project.packages.first().expect("package");
        let module = package.modules.first().expect("module");

        assert_eq!(package.name, "nested_paths");
        assert_eq!(module.name, "oracle");
        assert_eq!(module.file_path, "sources/helper/oracle.move");
    }

    #[test]
    fn dependency_graph_still_reads_package_summaries() {
        let temp = tempdir().expect("tempdir");

        fs::write(
            temp.path().join("Move.toml"),
            r#"
[package]
name = "root_pkg"
"#,
        )
        .expect("manifest");
        fs::create_dir_all(temp.path().join("sources")).expect("sources");
        fs::write(
            temp.path().join("sources/root.move"),
            r#"
module root_pkg::root;

public fun ping() {}
"#,
        )
        .expect("module");
        fs::create_dir_all(temp.path().join("package_summaries")).expect("summaries");
        fs::write(
            temp.path().join("package_summaries/address_mapping.json"),
            r#"{"root_pkg":"0x1","dependency":"0x2"}"#,
        )
        .expect("mapping");
        fs::write(
            temp.path().join("package_summaries/root.json"),
            r#"{
  "id": { "address": "root_pkg", "name": "root" },
  "immediate_dependencies": [{ "address": "dependency", "name": "dep" }],
  "functions": {
    "entry_fn": { "visibility": "Public", "entry": true },
    "internal_fn": { "visibility": "Internal", "entry": false }
  }
}"#,
        )
        .expect("root summary");
        fs::write(
            temp.path().join("package_summaries/dep.json"),
            r#"{
  "id": { "address": "dependency", "name": "dep" },
  "immediate_dependencies": [],
  "functions": {
    "public_fn": { "visibility": "Public", "entry": false }
  }
}"#,
        )
        .expect("dep summary");

        let project = discover_move_project(temp.path());
        let graph = project.dependency_graph;

        assert_eq!(graph.root.as_deref(), Some("root_pkg"));
        assert_eq!(graph.summary_path.as_deref(), Some("package_summaries"));
        assert!(graph.nodes.iter().any(|node| {
            node.id == "root_pkg"
                && node.address.as_deref() == Some("0x1")
                && node.module_count == 1
                && node.public_function_count == 1
                && node.entry_function_count == 1
                && node.is_root
        }));
        assert!(graph.nodes.iter().any(|node| {
            node.id == "dependency"
                && node.address.as_deref() == Some("0x2")
                && node.module_count == 1
                && node.public_function_count == 1
                && node.entry_function_count == 0
                && !node.is_root
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.source == "root_pkg" && edge.target == "dependency" && edge.dependency_count == 1
        }));
    }

    #[test]
    fn package_surface_detects_sui_object_ownership_patterns() {
        let root = Path::new("/workspace");
        let vault = parse_module_declaration(
            r#"
module savings_personal::vault {
    public struct SavingsAdminCap has key, store { id: UID }
    public struct SavingsTreasury has key, store { id: UID }
    public struct SavingsVault<phantom Asset> has key, store { id: UID }

    public entry fun init(ctx: &mut TxContext) {
        transfer::transfer(SavingsAdminCap { id: object::new(ctx) }, ctx.sender());
        transfer::share_object(SavingsTreasury { id: object::new(ctx) });
        let vault = SavingsVault<u64> { id: object::new(ctx) };
        transfer::share_object(vault);
    }

    public entry fun update_config(cap: &SavingsAdminCap, vault: &mut SavingsVault<u64>) {
        assert!(true, 0);
    }
}
"#,
            root,
            Path::new("/workspace/sources/vault.move"),
        )
        .expect("vault module should parse");
        let savings = parse_module_declaration(
            r#"
module savings_personal::savings_personal {
    public struct VaultReceipt has key, store { id: UID }
    public struct ReceiptWrapper has key, store { id: UID, receipt: VaultReceipt }

    public fun register_account(ctx: &mut TxContext): VaultReceipt {
        VaultReceipt { id: object::new(ctx) }
    }

    public(package) fun borrow_receipt(receipt: &VaultReceipt): ID {
        object::id(receipt)
    }
}
"#,
            root,
            Path::new("/workspace/sources/savings_personal.move"),
        )
        .expect("savings module should parse");

        let surface = package_surface(&[vault, savings]);

        assert!(surface.entry_function_count >= 3);
        assert!(surface.capability_count >= 1);
        assert!(surface.shared_object_count >= 2);
        assert!(surface.address_owned_object_count >= 1);
        assert!(surface.wrapped_object_count >= 1);
        assert!(surface.admin_control_count >= 1);
    }
}
