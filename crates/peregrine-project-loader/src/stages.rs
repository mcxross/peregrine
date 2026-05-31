use crate::{
    pipeline::{
        PackageLoadCapabilities, ProjectLoadContext, ProjectLoadDiagnostic, ProjectLoadStageReport,
        ProjectLoadStageStatus, ProjectLoadStep,
    },
    tree_paths::collect_project_paths,
};
use peregrine_static_analysis::{
    AnalysisConfig, AnalysisEngine, AnalysisEngineOptions, MovePackage, discover_move_project_fast,
    discover_move_project_shallow,
};
use std::time::Instant;

pub(crate) struct CollectPathsStep;

impl ProjectLoadStep for CollectPathsStep {
    fn id(&self) -> &'static str {
        "collectPaths"
    }

    fn label(&self) -> &'static str {
        "Collect project paths"
    }

    fn run(&self, context: &mut ProjectLoadContext) -> ProjectLoadStageReport {
        timed_stage(self.id(), self.label(), || {
            let paths = collect_project_paths(&context.root)?;
            let count = paths.len();
            context.paths = paths;

            Ok(StageOutcome {
                diagnostics: vec![ProjectLoadDiagnostic {
                    level: "info",
                    source: self.id(),
                    message: format!("Collected {count} display paths."),
                    package_manifest_path: None,
                }],
                status: ProjectLoadStageStatus::Passed,
            })
        })
    }
}

pub(crate) struct DiscoverProjectStep;

impl ProjectLoadStep for DiscoverProjectStep {
    fn id(&self) -> &'static str {
        "discoverProject"
    }

    fn label(&self) -> &'static str {
        "Discover Move packages"
    }

    fn run(&self, context: &mut ProjectLoadContext) -> ProjectLoadStageReport {
        timed_stage(self.id(), self.label(), || {
            let move_project = match context.options.mode {
                crate::ProjectLoadMode::Detailed => discover_move_project_fast(&context.root),
                crate::ProjectLoadMode::Shallow => discover_move_project_shallow(&context.root),
            };
            let package_count = move_project.packages.len();
            context.move_project = Some(move_project);

            Ok(StageOutcome {
                diagnostics: vec![ProjectLoadDiagnostic {
                    level: "info",
                    source: self.id(),
                    message: format!("Discovered {package_count} Move packages."),
                    package_manifest_path: None,
                }],
                status: ProjectLoadStageStatus::Passed,
            })
        })
    }
}

pub(crate) struct SourceHealthStep;

impl ProjectLoadStep for SourceHealthStep {
    fn id(&self) -> &'static str {
        "sourceHealth"
    }

    fn label(&self) -> &'static str {
        "Check source module health"
    }

    fn run(&self, context: &mut ProjectLoadContext) -> ProjectLoadStageReport {
        timed_stage(self.id(), self.label(), || {
            let Some(project) = context.move_project.as_ref() else {
                return Ok(StageOutcome {
                    diagnostics: vec![ProjectLoadDiagnostic {
                        level: "error",
                        source: self.id(),
                        message: "Project discovery must run before source health.".to_string(),
                        package_manifest_path: None,
                    }],
                    status: ProjectLoadStageStatus::Failed,
                });
            };
            let mut diagnostics = Vec::new();
            let mut invalid_count = 0usize;

            for package in &project.packages {
                if package.has_source_modules {
                    continue;
                }

                invalid_count += 1;
                diagnostics.push(ProjectLoadDiagnostic {
                    level: "warning",
                    source: self.id(),
                    message: source_health_message(package),
                    package_manifest_path: Some(package.manifest_path.clone()),
                });
            }

            Ok(StageOutcome {
                diagnostics,
                status: if invalid_count == 0 {
                    ProjectLoadStageStatus::Passed
                } else {
                    ProjectLoadStageStatus::Warning
                },
            })
        })
    }
}

pub(crate) struct PackageSummaryStep;

impl ProjectLoadStep for PackageSummaryStep {
    fn id(&self) -> &'static str {
        "packageSummaries"
    }

    fn label(&self) -> &'static str {
        "Check package summaries"
    }

    fn run(&self, context: &mut ProjectLoadContext) -> ProjectLoadStageReport {
        timed_stage(self.id(), self.label(), || {
            let Some(project) = context.move_project.as_ref() else {
                return Ok(StageOutcome {
                    diagnostics: vec![ProjectLoadDiagnostic {
                        level: "error",
                        source: self.id(),
                        message: "Project discovery must run before package summary checks."
                            .to_string(),
                        package_manifest_path: None,
                    }],
                    status: ProjectLoadStageStatus::Failed,
                });
            };
            let has_package_summaries = project.dependency_graph.summary_path.is_some();
            let diagnostics = vec![ProjectLoadDiagnostic {
                level: if has_package_summaries {
                    "info"
                } else {
                    "warning"
                },
                source: self.id(),
                message: if let Some(path) = project.dependency_graph.summary_path.as_deref() {
                    format!("Package summaries found at {path}.")
                } else {
                    "No package_summaries artifacts were found; dependency graph detail is limited."
                        .to_string()
                },
                package_manifest_path: None,
            }];

            for package in &project.packages {
                context.capabilities.insert(
                    package.manifest_path.clone(),
                    PackageLoadCapabilities {
                        package_name: package.name.clone(),
                        package_path: package.path.clone(),
                        manifest_path: package.manifest_path.clone(),
                        has_manifest: true,
                        has_source_files: package.has_source_files,
                        has_parseable_modules: package.has_source_modules,
                        has_package_summaries,
                        can_show_dependency_graph: package.has_source_modules
                            && has_package_summaries,
                        can_show_call_graph: package.has_source_modules,
                        can_show_type_graph: package.has_source_modules,
                        can_show_bytecode: package.has_source_modules,
                        can_run_static_analysis: package.has_source_modules,
                    },
                );
            }

            Ok(StageOutcome {
                diagnostics,
                status: if has_package_summaries {
                    ProjectLoadStageStatus::Passed
                } else {
                    ProjectLoadStageStatus::Warning
                },
            })
        })
    }
}

pub(crate) struct AnalyzerStep;

impl ProjectLoadStep for AnalyzerStep {
    fn id(&self) -> &'static str {
        "analyzer"
    }

    fn label(&self) -> &'static str {
        "Run static analyzer"
    }

    fn run(&self, context: &mut ProjectLoadContext) -> ProjectLoadStageReport {
        timed_stage(self.id(), self.label(), || {
            if !context.options.include_analyzer {
                return Ok(StageOutcome {
                    diagnostics: vec![ProjectLoadDiagnostic {
                        level: "info",
                        source: self.id(),
                        message: "Analyzer runs during detailed project hydration.".to_string(),
                        package_manifest_path: None,
                    }],
                    status: ProjectLoadStageStatus::Skipped,
                });
            }

            let Some(project) = context.move_project.as_ref() else {
                return Ok(StageOutcome {
                    diagnostics: vec![ProjectLoadDiagnostic {
                        level: "error",
                        source: self.id(),
                        message: "Project discovery must run before analyzer.".to_string(),
                        package_manifest_path: None,
                    }],
                    status: ProjectLoadStageStatus::Failed,
                });
            };
            let valid_packages = project
                .packages
                .iter()
                .filter(|package| package.has_source_modules)
                .collect::<Vec<_>>();

            if valid_packages.is_empty() {
                return Ok(StageOutcome {
                    diagnostics: vec![ProjectLoadDiagnostic {
                        level: "info",
                        source: self.id(),
                        message: "Analyzer skipped because no parseable Move modules were found."
                            .to_string(),
                        package_manifest_path: None,
                    }],
                    status: ProjectLoadStageStatus::Skipped,
                });
            }

            let mut diagnostics = Vec::new();
            let mut has_analyzer_errors = false;

            for package in valid_packages {
                let package_root = context.root.join(&package.path);
                let config = match AnalysisConfig::load_from_package(&package_root) {
                    Ok(config) => config,
                    Err(message) => {
                        has_analyzer_errors = true;
                        diagnostics.push(ProjectLoadDiagnostic {
                            level: "error",
                            source: self.id(),
                            message,
                            package_manifest_path: Some(package.manifest_path.clone()),
                        });
                        continue;
                    }
                };
                let report = AnalysisEngine::new().analyze_package_with_options(
                    &package_root,
                    config,
                    AnalysisEngineOptions {
                        global_plugin_root: context.options.analyzer_plugin_root.clone(),
                        ..AnalysisEngineOptions::default()
                    },
                );

                has_analyzer_errors |= report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.level == "error");
                diagnostics.push(ProjectLoadDiagnostic {
                    level: "info",
                    source: self.id(),
                    message: format!(
                        "Analyzer completed for {}: {} findings, {} diagnostics.",
                        package.name,
                        report.findings.len(),
                        report.diagnostics.len()
                    ),
                    package_manifest_path: Some(package.manifest_path.clone()),
                });
                context
                    .analysis_reports
                    .insert(package.manifest_path.clone(), report);
            }

            Ok(StageOutcome {
                diagnostics,
                status: if has_analyzer_errors {
                    ProjectLoadStageStatus::Warning
                } else {
                    ProjectLoadStageStatus::Passed
                },
            })
        })
    }
}

struct StageOutcome {
    diagnostics: Vec<ProjectLoadDiagnostic>,
    status: ProjectLoadStageStatus,
}

fn timed_stage(
    id: &'static str,
    label: &'static str,
    run: impl FnOnce() -> Result<StageOutcome, String>,
) -> ProjectLoadStageReport {
    let started_at = Instant::now();

    match run() {
        Ok(outcome) => ProjectLoadStageReport {
            id,
            label,
            status: outcome.status,
            diagnostics: outcome.diagnostics,
            duration_ms: started_at.elapsed().as_millis() as u64,
        },
        Err(message) => ProjectLoadStageReport {
            id,
            label,
            status: ProjectLoadStageStatus::Failed,
            diagnostics: vec![ProjectLoadDiagnostic {
                level: "error",
                source: id,
                message,
                package_manifest_path: None,
            }],
            duration_ms: started_at.elapsed().as_millis() as u64,
        },
    }
}

fn source_health_message(package: &MovePackage) -> String {
    if package.source_file_count == 0 {
        return format!(
            "Move package {} contains a Move.toml manifest but no Move source files under sources/.",
            package.name
        );
    }

    format!(
        "Move package {} contains {} Move source {}, but no parseable Move modules were found. The source may be commented out or invalid.",
        package.name,
        package.source_file_count,
        if package.source_file_count == 1 {
            "file"
        } else {
            "files"
        }
    )
}
