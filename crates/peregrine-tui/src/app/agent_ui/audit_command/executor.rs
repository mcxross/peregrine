use color_eyre::eyre::Result;
use peregrine_app_server_protocol::{
    AuditArtifactReadParams, AuditLifecycleParams, AuditListParams, AuditPreflightParams,
    AuditReadParams, AuditReportReadParams, AuditStartParams,
};

use super::output::{
    artifact_output_lines, delete_output_lines, list_output_lines, plan_output_lines,
    report_output_lines, run_output_lines,
};
use super::{AuditCommand, AuditCommandOutput, AuditLifecycleAction};
use crate::agent::app_server_session::AppServerSession;

const DEFAULT_LIST_LIMIT: u32 = 25;

pub(crate) async fn execute_audit_command(
    app_server: &mut AppServerSession,
    command: AuditCommand,
) -> Result<AuditCommandOutput> {
    match command {
        AuditCommand::Plan(request) => {
            let preflight = app_server
                .audit_preflight(AuditPreflightParams {
                    target: request.target,
                    profile: request.profile,
                })
                .await?;
            let stored = app_server.audit_plan_store(preflight.plan.clone()).await?;
            Ok(AuditCommandOutput {
                lines: plan_output_lines(&stored.fingerprint, &stored.plan, &preflight.diagnostics),
            })
        }
        AuditCommand::Run(request) => {
            let preflight = app_server
                .audit_preflight(AuditPreflightParams {
                    target: request.target,
                    profile: request.profile,
                })
                .await?;
            let stored = app_server.audit_plan_store(preflight.plan).await?;
            let started = app_server
                .audit_start(AuditStartParams {
                    fingerprint: stored.fingerprint,
                })
                .await?;
            Ok(AuditCommandOutput {
                lines: run_output_lines("Audit started", &started.run, &preflight.diagnostics),
            })
        }
        AuditCommand::Start { fingerprint } => {
            let started = app_server
                .audit_start(AuditStartParams { fingerprint })
                .await?;
            Ok(AuditCommandOutput {
                lines: run_output_lines("Audit started", &started.run, &[]),
            })
        }
        AuditCommand::Read { audit_id } => {
            let response = app_server.audit_read(AuditReadParams { audit_id }).await?;
            Ok(AuditCommandOutput {
                lines: run_output_lines("Audit status", &response.run, &[]),
            })
        }
        AuditCommand::Report { audit_id, format } => {
            let response = app_server
                .audit_report_read(AuditReportReadParams {
                    audit_id,
                    format: Some(format),
                })
                .await?;
            Ok(AuditCommandOutput {
                lines: report_output_lines(&response),
            })
        }
        AuditCommand::Artifact {
            audit_id,
            artifact_ref,
        } => {
            let response = app_server
                .audit_artifact_read(AuditArtifactReadParams {
                    audit_id,
                    artifact_ref,
                })
                .await?;
            Ok(AuditCommandOutput {
                lines: artifact_output_lines(&response),
            })
        }
        AuditCommand::List { cursor, limit } => {
            let response = app_server
                .audit_list(AuditListParams {
                    cursor,
                    limit: Some(limit.unwrap_or(DEFAULT_LIST_LIMIT)),
                })
                .await?;
            Ok(AuditCommandOutput {
                lines: list_output_lines(&response),
            })
        }
        AuditCommand::Lifecycle { action, audit_id } => match action {
            AuditLifecycleAction::Pause => {
                let response = app_server
                    .audit_pause(AuditLifecycleParams { audit_id })
                    .await?;
                Ok(AuditCommandOutput {
                    lines: run_output_lines("Audit paused", &response.run, &[]),
                })
            }
            AuditLifecycleAction::Resume => {
                let response = app_server
                    .audit_resume(AuditLifecycleParams { audit_id })
                    .await?;
                Ok(AuditCommandOutput {
                    lines: run_output_lines("Audit resumed", &response.run, &[]),
                })
            }
            AuditLifecycleAction::Cancel => {
                let response = app_server
                    .audit_cancel(AuditLifecycleParams { audit_id })
                    .await?;
                Ok(AuditCommandOutput {
                    lines: run_output_lines("Audit cancelled", &response.run, &[]),
                })
            }
            AuditLifecycleAction::Delete => {
                let response = app_server
                    .audit_delete(AuditLifecycleParams {
                        audit_id: audit_id.clone(),
                    })
                    .await?;
                Ok(AuditCommandOutput {
                    lines: delete_output_lines(&audit_id, response.deleted),
                })
            }
        },
    }
}
