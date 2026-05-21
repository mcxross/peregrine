use peregrine_move_graphs::{
    discover_move_project_model, discover_move_project_model_fast,
    discover_move_project_model_shallow, MoveCallGraph, MoveProjectModel, MoveStateAccessGraph,
    MoveTypeGraph, PackageDependencyGraph,
};
use peregrine_move_insights::attack_surface::{package_surface_for_package, MovePackageSurface};
use peregrine_move_model::{MoveModule, MovePackageModel};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveProject {
    pub packages: Vec<MovePackage>,
    pub dependency_graph: PackageDependencyGraph,
    pub call_graph: MoveCallGraph,
    pub type_graph: MoveTypeGraph,
    pub state_access_graph: MoveStateAccessGraph,
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

pub fn discover_move_project(root: &Path) -> MoveProject {
    let model = discover_move_project_model(root);
    build_move_project(root, model)
}

pub fn discover_move_project_fast(root: &Path) -> MoveProject {
    let model = discover_move_project_model_fast(root);
    build_move_project(root, model)
}

pub fn discover_move_project_shallow(root: &Path) -> MoveProject {
    let model = discover_move_project_model_shallow(root);
    build_move_project(root, model)
}

fn build_move_project(root: &Path, model: MoveProjectModel) -> MoveProject {
    let packages = model
        .packages
        .into_iter()
        .map(|package| build_move_package(root, package))
        .collect::<Vec<_>>();

    MoveProject {
        packages,
        dependency_graph: model.dependency_graph,
        call_graph: model.call_graph,
        type_graph: model.type_graph,
        state_access_graph: model.state_access_graph,
    }
}

fn build_move_package(root: &Path, model: MovePackageModel) -> MovePackage {
    let package_root = root.join(&model.path);
    let build_root = package_root.join("build").join(&model.name);
    let surface = package_surface_for_package(&model, Some(package_root), Some(build_root));

    MovePackage {
        name: model.name,
        path: model.path,
        manifest_path: model.manifest_path,
        surface,
        modules: model.modules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_move_insights::attack_surface::package_surface;
    use peregrine_move_model::parse_module_declarations;
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

    #[test]
    fn package_surface_builds_object_lifecycle_maps() {
        let root = Path::new("/workspace");
        let module = parse_module_declaration(
            r#"
module lifecycle::vault {
    public struct AdminCap has key, store { id: UID }
    public struct Vault has key, store { id: UID }
    public struct VaultReceipt has key, store { id: UID }
    public struct ReceiptWrapper has key, store { id: UID, receipt: VaultReceipt }

    fun create_internal(ctx: &mut TxContext): Vault {
        Vault { id: object::new(ctx) }
    }

    public entry fun create_vault(ctx: &mut TxContext) {
        let vault = Vault { id: object::new(ctx) };
        transfer::transfer(vault, tx_context::sender(ctx));
    }

    public entry fun route_create(ctx: &mut TxContext) {
        let _vault = create_internal(ctx);
    }

    public entry fun deposit(vault: &mut Vault) {}

    public entry fun share_vault(vault: Vault) {
        transfer::share_object(vault);
    }

    public entry fun freeze_vault(vault: Vault) {
        transfer::freeze_object(vault);
    }

    public entry fun party_vault(vault: Vault, party: Party) {
        transfer::party_transfer(vault, party);
    }

    public entry fun delete_vault(vault: Vault) {
        let Vault { id } = vault;
        id.delete();
    }

    public fun issue_receipt(ctx: &mut TxContext): VaultReceipt {
        VaultReceipt { id: object::new(ctx) }
    }

    public entry fun leak_cap(ctx: &mut TxContext) {
        transfer::transfer(AdminCap { id: object::new(ctx) }, @0x1);
    }
}
"#,
            root,
            Path::new("/workspace/sources/vault.move"),
        )
        .expect("lifecycle module should parse");

        let surface = package_surface(&[module]);
        let vault = surface
            .object_lifecycle_maps
            .iter()
            .find(|lifecycle| lifecycle.qualified_name == "vault::Vault")
            .expect("vault lifecycle map");
        let stage_kinds = vault
            .stages
            .iter()
            .map(|stage| stage.kind.as_str())
            .collect::<Vec<_>>();

        assert!(stage_kinds.contains(&"created"));
        assert!(stage_kinds.contains(&"owned"));
        assert!(stage_kinds.contains(&"mutated"));
        assert!(stage_kinds.contains(&"transferred"));
        assert!(stage_kinds.contains(&"shared"));
        assert!(stage_kinds.contains(&"immutable"));
        assert!(stage_kinds.contains(&"party"));
        assert!(stage_kinds.contains(&"deleted"));
        assert!(vault.touched_by.iter().any(|function| {
            function.qualified_name == "vault::route_create"
                && !function.direct
                && function.call_path == ["vault::route_create", "vault::create_internal"]
        }));

        let receipt = surface
            .object_lifecycle_maps
            .iter()
            .find(|lifecycle| lifecycle.qualified_name == "vault::VaultReceipt")
            .expect("receipt lifecycle map");

        assert!(receipt.stages.iter().any(|stage| stage.kind == "wrapped"));
        assert!(receipt
            .risks
            .iter()
            .any(|risk| risk.kind == "longLivedReceiptOrPosition"));

        let admin_cap = surface
            .object_lifecycle_maps
            .iter()
            .find(|lifecycle| lifecycle.qualified_name == "vault::AdminCap")
            .expect("admin cap lifecycle map");

        assert!(admin_cap.is_capability_like);
        assert!(admin_cap
            .risks
            .iter()
            .any(|risk| risk.kind == "privilegedObjectLeak"));
    }

    #[test]
    fn package_surface_excludes_test_only_lifecycle_code() {
        let root = Path::new("/workspace");
        let module = parse_module_declaration(
            r#"
module lifecycle::objects {
    public struct LiveObject has key, store { id: UID }
    public struct TestOnlyObject has key, store { id: UID }

    public fun create_live(ctx: &mut TxContext): LiveObject {
        LiveObject { id: object::new(ctx) }
    }

    #[test]
    public fun create_test(ctx: &mut TxContext): TestOnlyObject {
        TestOnlyObject { id: object::new(ctx) }
    }
}
"#,
            root,
            Path::new("/workspace/sources/objects.move"),
        )
        .expect("objects module should parse");
        let test_only_module = parse_module_declaration(
            r#"
#[test_only]
module lifecycle::test_support {
    public struct FixtureObject has key, store { id: UID }

    public fun fixture(ctx: &mut TxContext): FixtureObject {
        FixtureObject { id: object::new(ctx) }
    }
}
"#,
            root,
            Path::new("/workspace/sources/test_support.move"),
        )
        .expect("test-only module should parse");

        let surface = package_surface(&[module, test_only_module]);
        let lifecycle_names = surface
            .object_lifecycle_maps
            .iter()
            .map(|lifecycle| lifecycle.qualified_name.as_str())
            .collect::<Vec<_>>();

        assert!(lifecycle_names.contains(&"objects::LiveObject"));
        assert!(lifecycle_names.contains(&"objects::TestOnlyObject"));
        assert!(!surface.object_lifecycle_maps.iter().any(|lifecycle| {
            lifecycle
                .touched_by
                .iter()
                .any(|function| function.qualified_name == "objects::create_test")
        }));
        assert!(!lifecycle_names.contains(&"test_support::FixtureObject"));
    }

    #[test]
    fn lifecycle_scanner_avoids_read_only_false_ownership_events() {
        let root = Path::new("/workspace");
        let module = parse_module_declaration(
            r#"
module savings_personal::savings_personal {
    public struct SavingsVault<phantom Asset> has key, store { id: UID }
    public struct SavingsTreasury has key { id: UID, address: address }
    public struct VaultReceipt has key, store { id: UID, vault_id: ID }

    fun init(ctx: &mut TxContext) {
        transfer::share_object(SavingsTreasury {
            id: object::new(ctx),
            address: ctx.sender(),
        });
    }

    public fun register_account<Asset>(
        vault: &mut SavingsVault<Asset>,
        ctx: &mut TxContext,
    ): VaultReceipt {
        let receipt_uid = object::new(ctx);
        let vault_id = object::id(vault);

        VaultReceipt {
            id: receipt_uid,
            vault_id,
        }
    }

    public fun emergency_withdraw_from_bucket<Asset>(
        treasury: &SavingsTreasury,
        receipt: &VaultReceipt,
        penalty_coin: Coin<Asset>,
    ) {
        let _receipt_id = object::id(receipt);
        transfer::public_transfer(penalty_coin, treasury_address(treasury));
    }

    public fun deposit_to_bucket<Asset>(
        vault: &mut SavingsVault<Asset>,
        receipt: &VaultReceipt,
    ) {
        let receipt_id = object::id(receipt);
        let account = vault.borrow_account_mut(receipt_id);
        vault::set_bucket_balance(account, 10);
    }

    public fun withdraw_from_bucket<Asset>(
        vault: &mut SavingsVault<Asset>,
        receipt: &VaultReceipt,
    ) {
        let receipt_id = object::id(receipt);
        withdraw_unlocked_amount(vault, receipt_id);
    }

    fun withdraw_unlocked_amount<Asset>(
        vault: &mut SavingsVault<Asset>,
        receipt_id: ID,
    ) {
        let account = vault.borrow_account_mut(receipt_id);
        vault::set_bucket_balance(account, 0);
    }

    public fun get_bucket_info<Asset>(
        vault: &SavingsVault<Asset>,
        receipt: &VaultReceipt,
    ): u64 {
        let receipt_id = object::id(receipt);
        let _account = vault.borrow_account(receipt_id);
        0
    }

    public fun treasury_address(treasury: &SavingsTreasury): address {
        treasury.address
    }

    #[test_only]
    public fun destroy_receipt_for_testing(receipt: VaultReceipt) {
        let VaultReceipt { id, vault_id: _ } = receipt;
        object::delete(id);
    }
}
"#,
            root,
            Path::new("/workspace/sources/savings_personal.move"),
        )
        .expect("savings-like module should parse");

        let surface = package_surface(&[module]);
        let receipt = surface
            .object_lifecycle_maps
            .iter()
            .find(|lifecycle| lifecycle.qualified_name == "savings_personal::VaultReceipt")
            .expect("receipt lifecycle map");
        let receipt_stages = receipt
            .stages
            .iter()
            .map(|stage| stage.kind.as_str())
            .collect::<Vec<_>>();

        assert!(receipt_stages.contains(&"created"));
        assert!(receipt_stages.contains(&"owned"));
        assert!(receipt_stages.contains(&"mutated"));
        assert!(receipt_stages.contains(&"transferred"));
        assert!(!receipt_stages.contains(&"deleted"));
        assert!(receipt
            .touched_by
            .iter()
            .any(|function| function.qualified_name == "savings_personal::register_account"));
        assert!(!receipt.touched_by.iter().any(|function| {
            function.qualified_name == "savings_personal::emergency_withdraw_from_bucket"
        }));
        assert!(receipt.touched_by.iter().any(|function| {
            function.qualified_name == "savings_personal::deposit_to_bucket"
                && function
                    .evidence
                    .iter()
                    .any(|item| item.contains("mutates state keyed by"))
        }));
        assert!(receipt.touched_by.iter().any(|function| {
            function.qualified_name == "savings_personal::withdraw_from_bucket"
                && function
                    .evidence
                    .iter()
                    .any(|item| item.contains("mutates state keyed by"))
        }));
        assert!(receipt.touched_by.iter().any(|function| {
            function.qualified_name == "savings_personal::register_account"
                && function
                    .evidence
                    .iter()
                    .any(|item| item.contains("returned to transaction caller"))
        }));
        assert!(!receipt
            .touched_by
            .iter()
            .any(|function| { function.qualified_name == "savings_personal::get_bucket_info" }));

        let receipt_ownership = surface
            .object_ownership_findings
            .iter()
            .find(|finding| {
                finding.qualified_name == "savings_personal::VaultReceipt"
                    && finding.ownership_kind == "addressOwned"
            })
            .expect("receipt address-owned finding");

        assert_eq!(
            receipt_ownership.related_functions,
            vec!["savings_personal::register_account"]
        );
        assert!(!surface.object_ownership_findings.iter().any(|finding| {
            finding.qualified_name == "savings_personal::VaultReceipt"
                && finding
                    .related_functions
                    .iter()
                    .any(|function| function == "savings_personal::emergency_withdraw_from_bucket")
        }));

        let treasury = surface
            .object_lifecycle_maps
            .iter()
            .find(|lifecycle| lifecycle.qualified_name == "savings_personal::SavingsTreasury")
            .expect("treasury lifecycle map");

        assert!(treasury.stages.iter().any(|stage| stage.kind == "shared"));
    }
}
