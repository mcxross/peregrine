use crate::{ScanInput, ScannerDiagnosticSeverity, SourceMode, scan_package};
use peregrine_analysis::{
    AnalysisDiagnostic, AnalysisError, AnalysisFuture, AnalysisLimits, AnalysisOptions,
    AnalysisStage, Artifact, ArtifactBundle, ChainId, DiagnosticSeverity, Evidence,
    PluginDescriptor, PluginOrigin, PluginStage, ResolvedTarget, Scanner, Symbol,
};
use peregrine_sui_move_model::{MoveModule, MovePackageModel, discover_move_packages};
use serde_json::{Value, json};

const PLUGIN_ID: &str = "peregrine.sui.scanner";

#[derive(Default)]
pub struct SuiScanner;

impl Scanner for SuiScanner {
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor {
            id: PLUGIN_ID.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            chain: ChainId::new("sui"),
            stage: PluginStage::Scanner,
            capabilities: vec![
                "moveSource".to_string(),
                "moveBytecode".to_string(),
                "objects".to_string(),
                "tests".to_string(),
            ],
            origin: PluginOrigin::BuiltIn,
            priority: 100,
        }
    }

    fn scan<'a>(
        &'a self,
        target: &'a ResolvedTarget,
        options: &'a AnalysisOptions,
        _limits: &'a AnalysisLimits,
    ) -> AnalysisFuture<'a, ArtifactBundle> {
        Box::pin(async move {
            let root = target.package_root.as_ref().ok_or_else(|| {
                AnalysisError::new(
                    "package_not_materialized",
                    "Sui scanner requires a locally materialized Move package",
                )
            })?;
            let source_mode = source_mode(options.get("sourceMode"));
            let packages = discover_move_packages(root, true);
            if packages.is_empty() {
                return Err(AnalysisError::new(
                    "move_package_not_found",
                    format!("no Move package found under {}", root.display()),
                ));
            }

            let mut artifacts = Vec::new();
            let mut symbols = Vec::new();
            let mut evidence = Vec::new();
            let mut diagnostics = Vec::new();
            let mut reports = Vec::new();
            for package in &packages {
                add_package_artifacts(
                    &target.target_id,
                    package,
                    &mut artifacts,
                    &mut symbols,
                    &mut evidence,
                );
                let package_root = if package.path.is_empty() {
                    root.clone()
                } else {
                    root.join(&package.path)
                };
                let report = scan_package(ScanInput {
                    package_model: package,
                    package_root: Some(package_root.clone()),
                    build_root: Some(package_root.join("build")),
                    source_mode,
                });
                diagnostics.extend(report.diagnostics.iter().map(|diagnostic| {
                    AnalysisDiagnostic {
                        stage: AnalysisStage::Scan,
                        plugin_id: Some(PLUGIN_ID.to_string()),
                        severity: match diagnostic.severity {
                            ScannerDiagnosticSeverity::Info => DiagnosticSeverity::Info,
                            ScannerDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
                            ScannerDiagnosticSeverity::Error => DiagnosticSeverity::Error,
                        },
                        code: format!("sui_scanner_{}", diagnostic.scanner_id.replace('.', "_")),
                        message: diagnostic.message.clone(),
                    }
                }));
                reports.push(serde_json::to_value(report).map_err(|error| {
                    AnalysisError::new(
                        "scanner_report_serialization_failed",
                        format!("could not serialize scanner report: {error}"),
                    )
                })?);
            }

            Ok(ArtifactBundle {
                chain: ChainId::new("sui"),
                target_id: target.target_id.clone(),
                package_root: Some(root.clone()),
                artifacts,
                symbols,
                evidence,
                diagnostics,
                metadata: json!({
                    "packageCount": packages.len(),
                    "scannerReports": reports,
                    "resolvedTarget": target.metadata,
                }),
            })
        })
    }
}

fn source_mode(value: Option<&Value>) -> SourceMode {
    match value.and_then(Value::as_str) {
        Some("compilerOnly") => SourceMode::CompilerOnly,
        Some("sourceOnly") => SourceMode::SourceOnly,
        _ => SourceMode::BestAvailable,
    }
}

fn add_package_artifacts(
    target_id: &str,
    package: &MovePackageModel,
    artifacts: &mut Vec<Artifact>,
    symbols: &mut Vec<Symbol>,
    evidence: &mut Vec<Evidence>,
) {
    let package_id = format!("{target_id}:package:{}", package.name);
    artifacts.push(Artifact {
        id: package_id.clone(),
        kind: "package".to_string(),
        name: package.name.clone(),
        path: Some(package.path.clone()),
        metadata: json!({
            "manifestPath": package.manifest_path,
            "sourceFileCount": package.source_file_count,
            "hasSourceModules": package.has_source_modules,
        }),
    });
    evidence.push(Evidence {
        source: PLUGIN_ID.to_string(),
        artifact_id: Some(package_id.clone()),
        span: None,
        message: format!("discovered Move package `{}`", package.name),
        metadata: json!({"manifestPath": package.manifest_path}),
    });

    for module in &package.modules {
        add_module_artifacts(&package_id, module, artifacts, symbols);
    }
}

fn add_module_artifacts(
    package_id: &str,
    module: &MoveModule,
    artifacts: &mut Vec<Artifact>,
    symbols: &mut Vec<Symbol>,
) {
    let qualified_module = module
        .address
        .as_ref()
        .map(|address| format!("{address}::{}", module.name))
        .unwrap_or_else(|| module.name.clone());
    let module_id = format!("{package_id}:module:{qualified_module}");
    artifacts.push(Artifact {
        id: module_id.clone(),
        kind: "module".to_string(),
        name: module.name.clone(),
        path: Some(module.file_path.clone()),
        metadata: json!({
            "address": module.address,
            "attributes": module.attributes,
        }),
    });
    symbols.push(Symbol {
        id: module_id.clone(),
        kind: "module".to_string(),
        qualified_name: qualified_module.clone(),
        span: None,
        metadata: json!({"filePath": module.file_path}),
    });

    for move_struct in &module.structs {
        symbols.push(Symbol {
            id: format!("{module_id}:struct:{}", move_struct.name),
            kind: "struct".to_string(),
            qualified_name: format!("{qualified_module}::{}", move_struct.name),
            span: None,
            metadata: serde_json::to_value(move_struct).unwrap_or_else(|_| json!({})),
        });
    }
    for function in &module.functions {
        symbols.push(Symbol {
            id: format!("{module_id}:function:{}", function.name),
            kind: "function".to_string(),
            qualified_name: format!("{qualified_module}::{}", function.name),
            span: None,
            metadata: serde_json::to_value(function).unwrap_or_else(|_| json!({})),
        });
    }
}
