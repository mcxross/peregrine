mod executor;
mod output;
mod parser;

use peregrine_app_server_protocol::{AuditProfileParams, AuditReportFormat, AuditTargetParams};
use ratatui::text::Line;

pub(crate) use executor::execute_audit_command;
pub(crate) use output::{
    audit_diagnostic_lines, audit_finding_update_lines, audit_stage_update_lines,
    audit_update_lines,
};
pub(crate) use parser::parse_audit_command;

pub(crate) const AUDIT_USAGE: &str = concat!(
    "Usage: /audit [--plan] <local-path> | ",
    "/audit --remote --network <network> --package <package-ref> [--chain <id>] | ",
    "/audit start <fingerprint> | /audit read|status <auditId> | ",
    "/audit report <auditId> [--json] | /audit artifact <auditId> <ref> | ",
    "/audit list | /audit pause|resume|cancel|delete <auditId>",
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
    List,
    Lifecycle {
        action: AuditLifecycleAction,
        audit_id: String,
    },
}

#[derive(Debug)]
pub(crate) struct AuditCommandOutput {
    pub(crate) lines: Vec<Line<'static>>,
}
