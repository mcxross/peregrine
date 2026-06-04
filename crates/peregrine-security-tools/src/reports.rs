use crate::{MovePackageContext, SecurityToolsError, SecurityToolsResult};
use peregrine_bytecode::{
    DecompiledMoveModule, MoveModuleBytecodeInput, decompile_package_bytecode_modules,
    load_package_bytecode,
};
use peregrine_move_graphs::{
    MoveProjectGraphs, MoveStateAccessGraph, discover_move_project_graphs_for_package,
    discover_move_state_access_graph_for_function,
};
use peregrine_move_insights::{
    attack_surface::{MovePackageSurface, package_surface_from_scanner_report},
    scanner_report::{MovePackageScannerReport, package_scanner_report_for_package},
};
use peregrine_move_model::{MovePackageModel, build_move_package};
use peregrine_scanner::{ScanInput, ScanReport, SourceMode, sui::scan_package};
use peregrine_static_analysis::{
    AnalysisConfig, AnalysisEngine, AnalysisEngineOptions, AnalysisReport, AnalysisRuleCatalog,
};
use serde::Serialize;
use std::fs;

pub fn static_rule_catalog(ctx: &MovePackageContext) -> AnalysisRuleCatalog {
    AnalysisEngine::new().catalog_with_options(
        &ctx.package_root,
        AnalysisConfig::default(),
        AnalysisEngineOptions::default(),
    )
}

pub fn static_analyze_package(ctx: &MovePackageContext) -> AnalysisReport {
    AnalysisEngine::new().analyze_package_with_options(
        &ctx.package_root,
        AnalysisConfig::default(),
        AnalysisEngineOptions::default(),
    )
}

pub fn sui_scanner_report(ctx: &MovePackageContext) -> SecurityToolsResult<ScanReport> {
    let model = build_package_model(ctx)?;
    Ok(scan_package(scanner_input(ctx, &model)))
}

pub fn sui_package_insights(
    ctx: &MovePackageContext,
) -> SecurityToolsResult<SuiPackageInsightsReport> {
    let model = build_package_model(ctx)?;
    let scanner_input = scanner_input(ctx, &model);
    let scanner_report = package_scanner_report_for_package(
        &model,
        scanner_input.package_root.clone(),
        scanner_input.build_root.clone(),
    );
    let attack_surface = package_surface_from_scanner_report(&model, &scanner_report);
    let raw_scanner_report = scan_package(scanner_input);

    Ok(SuiPackageInsightsReport {
        scanner_report,
        raw_scanner_report,
        attack_surface,
    })
}

fn build_package_model(ctx: &MovePackageContext) -> SecurityToolsResult<MovePackageModel> {
    let manifest_path = ctx.package_root.join("Move.toml");
    build_move_package(&ctx.project_root, &manifest_path, true).ok_or_else(|| {
        SecurityToolsError::Analysis(format!(
            "Could not build Move package model for {}",
            ctx.package_root.display()
        ))
    })
}

fn scanner_input<'a>(ctx: &MovePackageContext, model: &'a MovePackageModel) -> ScanInput<'a> {
    ScanInput {
        package_model: model,
        package_root: Some(ctx.package_root.clone()),
        build_root: Some(ctx.package_root.join("build").join(&ctx.package_name)),
        source_mode: SourceMode::BestAvailable,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiPackageInsightsReport {
    pub scanner_report: MovePackageScannerReport,
    pub raw_scanner_report: ScanReport,
    pub attack_surface: MovePackageSurface,
}

pub fn sui_graphs(ctx: &MovePackageContext) -> MoveProjectGraphs {
    discover_move_project_graphs_for_package(&ctx.project_root, &ctx.package_path)
}

pub fn sui_function_state_graph(
    ctx: &MovePackageContext,
    address: Option<String>,
    module_name: &str,
    function_name: &str,
) -> MoveStateAccessGraph {
    discover_move_state_access_graph_for_function(
        &ctx.project_root,
        &ctx.package_path,
        address,
        module_name,
        function_name,
    )
}

pub fn sui_bytecode_view(
    ctx: &MovePackageContext,
) -> SecurityToolsResult<peregrine_bytecode::MoveBytecodePackageView> {
    load_package_bytecode(&ctx.package_root, &ctx.package_name)
        .map_err(SecurityToolsError::Analysis)
}

pub fn sui_bytecode_decompile(
    ctx: &MovePackageContext,
) -> SecurityToolsResult<DecompiledPackageReport> {
    let bytecode = sui_bytecode_view(ctx)?;
    let mut inputs = Vec::new();
    for module in &bytecode.modules {
        if module.is_dependency {
            continue;
        }
        let bytes = fs::read(&module.bytecode_path).map_err(|error| {
            SecurityToolsError::Analysis(format!(
                "Could not read bytecode module {}: {error}",
                module.bytecode_path
            ))
        })?;
        inputs.push(MoveModuleBytecodeInput {
            name: module.name.clone(),
            bytecode: bytes,
            disassembly: Some(module.disassembly.clone()),
        });
    }

    if inputs.is_empty() {
        return Err(SecurityToolsError::Analysis(
            "No root package bytecode modules were found to decompile.".to_string(),
        ));
    }

    let modules = decompile_package_bytecode_modules(&inputs)
        .map_err(SecurityToolsError::Analysis)?
        .into_iter()
        .map(DecompiledModuleReport::from)
        .collect::<Vec<_>>();

    Ok(DecompiledPackageReport {
        package_name: bytecode.package_name,
        module_count: modules.len(),
        modules,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompiledPackageReport {
    pub package_name: String,
    pub module_count: usize,
    pub modules: Vec<DecompiledModuleReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompiledModuleReport {
    pub name: String,
    pub address: String,
    pub source: String,
    pub disassembly: String,
}

impl From<DecompiledMoveModule> for DecompiledModuleReport {
    fn from(module: DecompiledMoveModule) -> Self {
        Self {
            name: module.name,
            address: module.address,
            source: module.source,
            disassembly: module.disassembly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn static_rule_catalog_includes_builtin_sui_and_complexity_rules() {
        let temp = tempfile::tempdir().expect("tempdir");
        let ctx = MovePackageContext {
            project_root: temp.path().to_path_buf(),
            package_root: temp.path().to_path_buf(),
            package_path: ".".to_string(),
            package_name: "sample".to_string(),
        };

        let catalog = static_rule_catalog(&ctx);
        let ruleset_ids = catalog
            .rulesets
            .iter()
            .map(|ruleset| ruleset.id.as_str())
            .collect::<BTreeSet<_>>();
        for expected in [
            "bool_judgement",
            "infinite_loop",
            "precision_loss",
            "type_conversion",
            "unchecked_return",
            "unused_const",
            "unused_private_function",
            "unused_struct",
            "complexity",
        ] {
            assert!(
                ruleset_ids.contains(expected),
                "missing ruleset {expected} from {ruleset_ids:?}"
            );
        }

        let complexity = catalog
            .rulesets
            .iter()
            .find(|ruleset| ruleset.id == "complexity")
            .expect("complexity ruleset");
        let complexity_rule_ids = complexity
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(complexity_rule_ids.contains("function_complexity"));
        assert!(complexity_rule_ids.contains("module_complexity"));
    }
}
