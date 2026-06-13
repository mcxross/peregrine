use crate::artifacts::MovePackageContext;
use peregrine_analysis::{AnalysisReport, DynamicResultStatus};
use peregrine_sui_mcp_protocol::{CommandResult, CommandStatus, PackageSummary};
use serde_json::Value;

pub(crate) fn dynamic_command_result(
    context: &MovePackageContext,
    command: &str,
    capability: &str,
    report: &AnalysisReport,
) -> CommandResult {
    let result = report
        .dynamic_results
        .iter()
        .find(|result| result.capability == capability);
    let status = match result.map(|result| result.status) {
        Some(DynamicResultStatus::Completed) => CommandStatus::Completed,
        Some(DynamicResultStatus::Failed | DynamicResultStatus::Unavailable) | None => {
            CommandStatus::Failed
        }
    };
    let stdout = result
        .and_then(|result| result.result.get("stdout"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| result.and_then(|result| serde_json::to_string_pretty(&result.result).ok()))
        .unwrap_or_default();
    let stderr = result
        .into_iter()
        .flat_map(|result| &result.diagnostics)
        .chain(&report.diagnostics)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    CommandResult {
        status,
        package: PackageSummary {
            project_root: context.project_root.display().to_string(),
            package_root: context.package_root.display().to_string(),
            package_path: context.package_path.clone(),
            package_name: context.package_name.clone(),
        },
        command: command.to_string(),
        exit_code: Some(i32::from(status != CommandStatus::Completed)),
        stdout,
        stderr,
        truncated: false,
    }
}
