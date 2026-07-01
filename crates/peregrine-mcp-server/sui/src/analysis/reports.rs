use crate::{
    artifacts::MovePackageContext,
    error::{SecurityToolsError, SecurityToolsResult},
};
use peregrine_sui_bytecode::{
    DecompiledMoveModule, MoveModuleBytecodeInput, decompile_package_bytecode_modules,
    load_package_bytecode,
};
use peregrine_sui_mcp_protocol::{
    ModuleEntry, MoveSourceSummary, ScannerDiagnostic as ProtocolScannerDiagnostic,
    ScannerDiagnosticSeverity as ProtocolScannerDiagnosticSeverity, ScannerSourceMode,
    SignatureEntry, StaticAnalysisArgs, TestScannerReport,
};
use peregrine_sui_move_insights::{
    attack_surface::{MovePackageSurface, package_surface_from_scanner_report},
    scanner_report::{MovePackageScannerReport, package_scanner_report_for_package},
};
use peregrine_sui_move_model::{MovePackageModel, build_move_package};
use peregrine_sui_scanner::{
    ScanInput, ScanReport, ScannerDiagnosticSeverity, ScannerOutput, SourceMode,
    core::PackageScanner, scan_package, tests::TestsScanner,
};
use peregrine_sui_static_analysis::{
    AnalysisConfig, AnalysisEngine, AnalysisEngineOptions, AnalysisRuleCatalog,
};
use serde::Serialize;
use std::{fs, path::PathBuf};

pub fn static_rule_catalog(
    ctx: &MovePackageContext,
    args: &StaticAnalysisArgs,
) -> SecurityToolsResult<AnalysisRuleCatalog> {
    let config = AnalysisConfig::load_from_package(&ctx.package_root)
        .map_err(SecurityToolsError::Analysis)?;
    Ok(AnalysisEngine::new().catalog_with_options(
        &ctx.package_root,
        config,
        analysis_options(args),
    ))
}

fn analysis_options(args: &StaticAnalysisArgs) -> AnalysisEngineOptions {
    AnalysisEngineOptions {
        use_global_plugins: !args.no_global_plugins,
        extra_plugin_paths: args.plugins.iter().map(PathBuf::from).collect(),
        only_rulesets: args.rulesets.clone(),
        ..AnalysisEngineOptions::default()
    }
}

pub fn sui_test_scanner_report(
    ctx: &MovePackageContext,
    source_mode: ScannerSourceMode,
) -> SecurityToolsResult<TestScannerReport> {
    let mut model = build_package_model(ctx)?;
    let source_mode = match source_mode {
        ScannerSourceMode::BestAvailable => SourceMode::BestAvailable,
        ScannerSourceMode::SourceOnly => {
            model.modules.clear();
            SourceMode::SourceOnly
        }
    };
    let input = ScanInput {
        package_model: &model,
        package_root: Some(ctx.package_root.clone()),
        build_root: Some(ctx.package_root.join("build").join(&ctx.package_name)),
        source_mode,
    };
    let ScannerOutput::Tests(report) = TestsScanner.scan(&input) else {
        return Err(SecurityToolsError::Analysis(
            "tests scanner returned an object scan report".to_string(),
        ));
    };
    let random_test_count = report
        .unit_tests
        .iter()
        .filter(|finding| finding.is_random_test)
        .count();
    let diagnostics = report
        .diagnostics
        .into_iter()
        .map(|diagnostic| ProtocolScannerDiagnostic {
            severity: match diagnostic.severity {
                ScannerDiagnosticSeverity::Info => ProtocolScannerDiagnosticSeverity::Info,
                ScannerDiagnosticSeverity::Warning => ProtocolScannerDiagnosticSeverity::Warning,
                ScannerDiagnosticSeverity::Error => ProtocolScannerDiagnosticSeverity::Error,
            },
            message: diagnostic.message,
        })
        .collect();
    Ok(TestScannerReport {
        unit_test_count: report.unit_test_count,
        movy_invariant_test_count: report.movy_invariant_test_count,
        random_test_count,
        formal_prover_spec_count: report.formal_prover_spec_count,
        diagnostics,
    })
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

pub fn sui_signatures(ctx: &MovePackageContext) -> SecurityToolsResult<Vec<SignatureEntry>> {
    let model = build_package_model(ctx)?;
    let mut signatures = model
        .modules
        .into_iter()
        .flat_map(|module| {
            let module_name = module.name;
            let module_address = module.address;
            let file_path = module.file_path;
            module
                .functions
                .into_iter()
                .map(move |function| SignatureEntry {
                    module_name: module_name.clone(),
                    module_address: module_address.clone(),
                    file_path: file_path.clone(),
                    function_name: function.name,
                    visibility: function.visibility,
                    is_entry: function.is_entry,
                    is_transaction_callable: function.is_transaction_callable,
                    signature: function.signature,
                })
        })
        .collect::<Vec<_>>();
    signatures.sort_by(|left, right| {
        left.module_name
            .cmp(&right.module_name)
            .then_with(|| left.function_name.cmp(&right.function_name))
    });
    Ok(signatures)
}

pub fn sui_modules(
    ctx: &MovePackageContext,
) -> SecurityToolsResult<(MoveSourceSummary, Vec<ModuleEntry>)> {
    let model = build_package_model(ctx)?;
    let source = MoveSourceSummary {
        manifest_path: model.manifest_path.clone(),
        source_file_count: model.source_file_count,
        has_source_modules: model.has_source_modules,
    };
    let mut modules = model
        .modules
        .into_iter()
        .map(|module| ModuleEntry {
            module_name: module.name,
            module_address: module.address,
            file_path: module.file_path,
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.module_name.cmp(&right.module_name));
    Ok((source, modules))
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

pub fn sui_bytecode_view(
    ctx: &MovePackageContext,
) -> SecurityToolsResult<peregrine_sui_bytecode::MoveBytecodePackageView> {
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

        let args = StaticAnalysisArgs {
            package: peregrine_sui_mcp_protocol::PackageArgs {
                project_root: None,
                package_path: None,
                unbounded: false,
            },
            no_global_plugins: false,
            plugins: Vec::new(),
            rulesets: Vec::new(),
        };
        let catalog = static_rule_catalog(&ctx, &args).expect("static rule catalog");
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
