use crate::stages::{
    AnalyzerStep, CollectPathsStep, DiscoverProjectStep, PackageSummaryStep, SourceHealthStep,
};
use peregrine_move_graphs::{
    MoveCallGraph, MoveStateAccessGraph, MoveTypeGraph, PackageDependencyGraph,
};
use peregrine_static_analysis::{AnalysisReport, MovePackage, MoveProject};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectLoadMode {
    Detailed,
    Shallow,
}

#[derive(Clone, Debug)]
pub struct ProjectLoadOptions {
    pub analyzer_plugin_root: Option<PathBuf>,
    pub include_analyzer: bool,
    pub mode: ProjectLoadMode,
}

impl Default for ProjectLoadOptions {
    fn default() -> Self {
        Self {
            analyzer_plugin_root: None,
            include_analyzer: false,
            mode: ProjectLoadMode::Shallow,
        }
    }
}

pub struct ProjectLoadPipeline {
    steps: Vec<Box<dyn ProjectLoadStep>>,
}

impl Default for ProjectLoadPipeline {
    fn default() -> Self {
        Self {
            steps: vec![
                Box::new(CollectPathsStep),
                Box::new(DiscoverProjectStep),
                Box::new(SourceHealthStep),
                Box::new(PackageSummaryStep),
                Box::new(AnalyzerStep),
            ],
        }
    }
}

impl ProjectLoadPipeline {
    pub fn load(
        &self,
        root_path: impl AsRef<Path>,
        options: ProjectLoadOptions,
    ) -> Result<LoadedProject, String> {
        let root_input = root_path.as_ref();
        let root = root_input.canonicalize().map_err(|error| {
            format!(
                "Could not read package directory {}: {error}",
                root_input.display()
            )
        })?;
        let root_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| root_input.to_str().unwrap_or("Move package"))
            .to_string();
        let mut context = ProjectLoadContext {
            analysis_reports: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            move_project: None,
            options,
            paths: Vec::new(),
            root,
        };
        let mut stages = Vec::with_capacity(self.steps.len());

        for step in &self.steps {
            stages.push(step.run(&mut context));
        }

        let move_project = context
            .move_project
            .ok_or_else(|| "Project discovery did not produce a Move project.".to_string())?;

        Ok(LoadedProject {
            root_path: context.root.to_string_lossy().into_owned(),
            root_name,
            is_detailed: matches!(context.options.mode, ProjectLoadMode::Detailed),
            paths: context.paths,
            move_packages: move_project.packages,
            dependency_graph: move_project.dependency_graph,
            call_graph: move_project.call_graph,
            type_graph: move_project.type_graph,
            state_access_graph: move_project.state_access_graph,
            load_report: ProjectLoadReport {
                stages,
                capabilities: context.capabilities,
                analysis_reports: context.analysis_reports,
            },
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedProject {
    pub root_path: String,
    pub root_name: String,
    pub is_detailed: bool,
    pub paths: Vec<String>,
    pub move_packages: Vec<MovePackage>,
    pub dependency_graph: PackageDependencyGraph,
    pub call_graph: MoveCallGraph,
    pub type_graph: MoveTypeGraph,
    pub state_access_graph: MoveStateAccessGraph,
    pub load_report: ProjectLoadReport,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoadReport {
    pub stages: Vec<ProjectLoadStageReport>,
    pub capabilities: BTreeMap<String, PackageLoadCapabilities>,
    pub analysis_reports: BTreeMap<String, AnalysisReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoadStageReport {
    pub id: &'static str,
    pub label: &'static str,
    pub status: ProjectLoadStageStatus,
    pub diagnostics: Vec<ProjectLoadDiagnostic>,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectLoadStageStatus {
    Failed,
    Passed,
    Running,
    Skipped,
    Warning,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoadDiagnostic {
    pub level: &'static str,
    pub source: &'static str,
    pub message: String,
    pub package_manifest_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageLoadCapabilities {
    pub package_name: String,
    pub package_path: String,
    pub manifest_path: String,
    pub has_manifest: bool,
    pub has_source_files: bool,
    pub has_parseable_modules: bool,
    pub has_package_summaries: bool,
    pub can_show_dependency_graph: bool,
    pub can_show_call_graph: bool,
    pub can_show_type_graph: bool,
    pub can_show_bytecode: bool,
    pub can_run_static_analysis: bool,
}

pub fn load_project(
    root_path: impl AsRef<Path>,
    options: ProjectLoadOptions,
) -> Result<LoadedProject, String> {
    ProjectLoadPipeline::default().load(root_path, options)
}

pub(crate) struct ProjectLoadContext {
    pub(crate) analysis_reports: BTreeMap<String, AnalysisReport>,
    pub(crate) capabilities: BTreeMap<String, PackageLoadCapabilities>,
    pub(crate) move_project: Option<MoveProject>,
    pub(crate) options: ProjectLoadOptions,
    pub(crate) paths: Vec<String>,
    pub(crate) root: PathBuf,
}

pub(crate) trait ProjectLoadStep {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn run(&self, context: &mut ProjectLoadContext) -> ProjectLoadStageReport;
}
