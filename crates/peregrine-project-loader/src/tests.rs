use crate::{
    load_project, ProjectLoadMode, ProjectLoadOptions, ProjectLoadReport, ProjectLoadStageStatus,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn manifest_only_package_reports_capability_limits() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        temp.path().join("Move.toml"),
        r#"
[package]
name = "manifest_only"
"#,
    )
    .expect("manifest");

    let project = load_project(
        temp.path(),
        ProjectLoadOptions {
            include_analyzer: true,
            mode: ProjectLoadMode::Detailed,
            ..ProjectLoadOptions::default()
        },
    )
    .expect("project");
    let package = project.move_packages.first().expect("package");
    let capabilities = project
        .load_report
        .capabilities
        .get(&package.manifest_path)
        .expect("capabilities");

    assert!(!package.has_source_files);
    assert!(!package.has_source_modules);
    assert_eq!(package.source_file_count, 0);
    assert!(!capabilities.can_show_dependency_graph);
    assert!(!capabilities.can_show_call_graph);
    assert!(!capabilities.can_show_type_graph);
    assert!(!capabilities.can_show_bytecode);
    assert!(!capabilities.can_run_static_analysis);
    assert!(project.load_report.analysis_reports.is_empty());
    assert_stage_status(
        &project.load_report,
        "sourceHealth",
        ProjectLoadStageStatus::Warning,
    );
    assert_stage_status(
        &project.load_report,
        "analyzer",
        ProjectLoadStageStatus::Skipped,
    );
}

#[test]
fn commented_only_source_skips_analyzer() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        temp.path().join("Move.toml"),
        r#"
[package]
name = "commented"
"#,
    )
    .expect("manifest");
    fs::create_dir_all(temp.path().join("sources")).expect("sources");
    fs::write(
        temp.path().join("sources/commented.move"),
        r#"/*
module commented::commented;
*/"#,
    )
    .expect("source");

    let project = load_project(
        temp.path(),
        ProjectLoadOptions {
            include_analyzer: true,
            mode: ProjectLoadMode::Detailed,
            ..ProjectLoadOptions::default()
        },
    )
    .expect("project");
    let package = project.move_packages.first().expect("package");

    assert!(package.has_source_files);
    assert!(!package.has_source_modules);
    assert_eq!(package.source_file_count, 1);
    assert!(project.load_report.analysis_reports.is_empty());
    assert_stage_status(
        &project.load_report,
        "sourceHealth",
        ProjectLoadStageStatus::Warning,
    );
    assert_stage_status(
        &project.load_report,
        "analyzer",
        ProjectLoadStageStatus::Skipped,
    );
}

#[test]
fn valid_source_runs_analyzer_as_load_step() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        temp.path().join("Move.toml"),
        r#"
[package]
name = "valid"
"#,
    )
    .expect("manifest");
    fs::create_dir_all(temp.path().join("sources")).expect("sources");
    fs::write(
        temp.path().join("sources/main.move"),
        r#"
module valid::main;

public fun ping() {}
"#,
    )
    .expect("source");

    let project = load_project(
        temp.path(),
        ProjectLoadOptions {
            include_analyzer: true,
            mode: ProjectLoadMode::Detailed,
            ..ProjectLoadOptions::default()
        },
    )
    .expect("project");
    let package = project.move_packages.first().expect("package");
    let report = project
        .load_report
        .analysis_reports
        .get(&package.manifest_path)
        .expect("analysis report");

    assert!(package.has_source_modules);
    assert!(!report.loaded_rulesets.is_empty());
    assert_stage_status(
        &project.load_report,
        "analyzer",
        ProjectLoadStageStatus::Passed,
    );
}

#[test]
fn shallow_load_reports_existing_package_summaries() {
    let temp = tempdir().expect("tempdir");

    fs::write(
        temp.path().join("Move.toml"),
        r#"
[package]
name = "with_summaries"
"#,
    )
    .expect("manifest");
    fs::create_dir_all(temp.path().join("sources")).expect("sources");
    fs::write(
        temp.path().join("sources/main.move"),
        r#"
module with_summaries::main;

public fun ping() {}
"#,
    )
    .expect("source");
    fs::create_dir_all(temp.path().join("package_summaries/with_summaries")).expect("summaries");

    let project = load_project(temp.path(), ProjectLoadOptions::default()).expect("project");

    assert_eq!(
        project.dependency_graph.summary_path.as_deref(),
        Some("package_summaries"),
    );
    assert_stage_status(
        &project.load_report,
        "packageSummaries",
        ProjectLoadStageStatus::Passed,
    );
}

fn assert_stage_status(
    report: &ProjectLoadReport,
    stage_id: &str,
    expected: ProjectLoadStageStatus,
) {
    let stage = report
        .stages
        .iter()
        .find(|stage| stage.id == stage_id)
        .expect("stage");

    assert_eq!(stage.status, expected);
}
