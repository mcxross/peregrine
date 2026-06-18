use peregrine_app_server_protocol::AuditListResponse;
use peregrine_types::harness::{AuditPlan, AuditRun, AuditRunStatus, AuditTarget};
use ratatui::style::Stylize;
use ratatui::text::Line;
use serde_json::Value as JsonValue;

pub(crate) fn audit_update_lines(audit_id: &str, run: &JsonValue) -> Vec<Line<'static>> {
    if let Some(run) = parse_run(run) {
        vec![
            vec![
                "audit ".cyan(),
                run.id.cyan(),
                " updated: ".into(),
                status_label(&run.status).into(),
                " at ".into(),
                format!("{:?}", run.current_stage).dim(),
            ]
            .into(),
        ]
    } else {
        vec![
            vec![
                "audit ".cyan(),
                audit_id.to_string().cyan(),
                " updated".into(),
            ]
            .into(),
        ]
    }
}

pub(crate) fn audit_diagnostic_lines(audit_id: Option<&str>, message: &str) -> Vec<Line<'static>> {
    let prefix = audit_id
        .map(|audit_id| format!("audit {audit_id} diagnostic: "))
        .unwrap_or_else(|| "audit diagnostic: ".to_string());
    vec![vec![prefix.yellow(), message.to_string().into()].into()]
}

pub(super) fn plan_output_lines(
    fingerprint: &str,
    plan_value: &JsonValue,
    diagnostics: &[String],
) -> Vec<Line<'static>> {
    let mut lines = vec!["Audit plan stored".green().bold().into()];
    lines.push(kv_line("fingerprint", fingerprint));
    if let Some(plan) = parse_plan(plan_value) {
        lines.push(kv_line("target", &target_label(&plan.target)));
        lines.push(kv_line("stages", &plan.stages.len().to_string()));
        lines.push(kv_line(
            "required capabilities",
            &plan.required_capabilities.len().to_string(),
        ));
        lines.push(kv_line(
            "budget",
            &format!(
                "{} tokens, {}s, {} hypotheses",
                plan.profile.model_token_budget,
                plan.profile.wall_time_seconds,
                plan.profile.max_hypotheses
            ),
        ));
    }
    append_diagnostics(&mut lines, diagnostics);
    lines.push(
        vec![
            "start with ".dim(),
            format!("/audit start {fingerprint}").cyan(),
        ]
        .into(),
    );
    lines
}

pub(super) fn run_output_lines(
    title: &str,
    run_value: &JsonValue,
    diagnostics: &[String],
) -> Vec<Line<'static>> {
    let mut lines = vec![title.to_string().green().bold().into()];
    if let Some(run) = parse_run(run_value) {
        lines.extend(run_summary_lines(&run));
    } else {
        lines.push(kv_line("run", "received, but could not parse summary"));
    }
    append_diagnostics(&mut lines, diagnostics);
    lines
}

pub(super) fn list_output_lines(response: &AuditListResponse) -> Vec<Line<'static>> {
    if response.data.is_empty() {
        return vec!["No audit runs found.".dim().into()];
    }

    let mut lines = vec!["Audit runs".green().bold().into()];
    for value in &response.data {
        if let Some(run) = parse_run(value) {
            lines.push(
                vec![
                    run.id.cyan(),
                    "  ".into(),
                    status_label(&run.status).into(),
                    "  ".into(),
                    format!("{:?}", run.current_stage).dim(),
                    "  updated ".dim(),
                    run.updated_at.to_string().dim(),
                ]
                .into(),
            );
        } else {
            lines.push("unparseable audit run".yellow().into());
        }
    }
    if let Some(cursor) = &response.next_cursor {
        lines.push(kv_line("next cursor", cursor));
    }
    lines
}

pub(super) fn delete_output_lines(audit_id: &str, deleted: bool) -> Vec<Line<'static>> {
    if deleted {
        vec![vec!["Deleted audit ".green(), audit_id.to_string().cyan()].into()]
    } else {
        vec![
            vec![
                "Audit ".yellow(),
                audit_id.to_string().cyan(),
                " was not found".yellow(),
            ]
            .into(),
        ]
    }
}

fn run_summary_lines(run: &AuditRun) -> Vec<Line<'static>> {
    let available_capabilities = run
        .capabilities
        .iter()
        .filter(|capability| capability.available)
        .count();
    let mut lines = vec![
        kv_line("auditId", &run.id),
        kv_line("status", status_label(&run.status)),
        kv_line("stage", &format!("{:?}", run.current_stage)),
        kv_line("target", &target_label(&run.target)),
        kv_line("plan", &run.plan_fingerprint),
    ];
    if let Some(thread_id) = &run.coordinator_thread_id {
        lines.push(kv_line("coordinator thread", thread_id));
    }
    if let Some(goal_id) = &run.goal_id {
        lines.push(kv_line("goal", goal_id));
    }
    if let Some(adapter_id) = &run.adapter_id {
        lines.push(kv_line("adapter", adapter_id));
    }
    lines.push(kv_line(
        "capabilities",
        &format!("{available_capabilities}/{}", run.capabilities.len()),
    ));
    lines.push(kv_line(
        "coverage gaps",
        &run.coverage_gaps.len().to_string(),
    ));
    lines.push(kv_line("work items", &run.work_items.len().to_string()));
    lines.push(kv_line(
        "evidence refs",
        &run.evidence_refs.len().to_string(),
    ));
    lines
}

fn append_diagnostics(lines: &mut Vec<Line<'static>>, diagnostics: &[String]) {
    if diagnostics.is_empty() {
        return;
    }
    lines.push("diagnostics".yellow().bold().into());
    for diagnostic in diagnostics.iter().take(5) {
        lines.push(vec!["  - ".dim(), diagnostic.clone().into()].into());
    }
    if diagnostics.len() > 5 {
        lines.push(
            format!("  - {} more diagnostics", diagnostics.len() - 5)
                .dim()
                .into(),
        );
    }
}

fn kv_line(key: &str, value: &str) -> Line<'static> {
    vec![format!("{key}: ").dim(), value.to_string().into()].into()
}

fn parse_plan(value: &JsonValue) -> Option<AuditPlan> {
    serde_json::from_value(value.clone()).ok()
}

fn parse_run(value: &JsonValue) -> Option<AuditRun> {
    serde_json::from_value(value.clone()).ok()
}

fn target_label(target: &AuditTarget) -> String {
    match target {
        AuditTarget::LocalPackage { chain_id, path, .. } => {
            format!("{chain_id} local {path}")
        }
        AuditTarget::RemotePackage {
            chain_id,
            network_id,
            package_ref,
            ..
        } => format!("{chain_id} {network_id} package {package_ref}"),
    }
}

fn status_label(status: &AuditRunStatus) -> &'static str {
    match status {
        AuditRunStatus::Pending => "pending",
        AuditRunStatus::Running => "running",
        AuditRunStatus::Paused => "paused",
        AuditRunStatus::Completed => "completed",
        AuditRunStatus::CompletedWithGaps => "completed with gaps",
        AuditRunStatus::Failed => "failed",
        AuditRunStatus::Cancelled => "cancelled",
    }
}
