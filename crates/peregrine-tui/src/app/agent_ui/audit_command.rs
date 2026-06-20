mod executor;
mod output;
mod parser;
mod planner_prompt;

use peregrine_app_server_protocol::{AuditProfileParams, AuditReportFormat, AuditTargetParams};
use ratatui::text::Line;

pub(crate) use executor::execute_audit_command;
pub(crate) use output::{
    audit_activity_lines, audit_diagnostic_lines, audit_finding_update_lines,
    audit_stage_update_lines, audit_update_lines,
};
pub(crate) use parser::parse_audit_command;
pub(crate) use planner_prompt::audit_planner_prompt;

pub(crate) const AUDIT_USAGE: &str = concat!(
    "Usage: /audit --plan <local-path> | ",
    "/audit --plan --remote --network <network> --package <package-ref> [--chain <id>] | ",
    "/audit start <fingerprint> | /audit read|status <auditId> | ",
    "/audit report <auditId> [--json] | /audit artifact <auditId> <ref> | ",
    "/audit list [--cursor <cursor>] [--limit <n>] | ",
    "/audit pause|resume|cancel|delete <auditId>",
);

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AuditTargetRequest {
    pub(crate) target: AuditTargetParams,
    pub(crate) profile: Option<AuditProfileParams>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuditLifecycleAction {
    Pause,
    Resume,
    Cancel,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AuditCommand {
    Plan(AuditTargetRequest),
    Run(AuditTargetRequest),
    Start {
        fingerprint: String,
    },
    Read {
        audit_id: String,
    },
    Report {
        audit_id: String,
        format: AuditReportFormat,
    },
    Artifact {
        audit_id: String,
        artifact_ref: String,
    },
    List {
        cursor: Option<String>,
        limit: Option<u32>,
    },
    Lifecycle {
        action: AuditLifecycleAction,
        audit_id: String,
    },
}

#[derive(Debug)]
pub(crate) struct AuditCommandOutput {
    pub(crate) lines: Vec<Line<'static>>,
}
