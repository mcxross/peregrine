use super::audit_scope;
use crate::session::turn_context::TurnContext;
use crate::tools::context::ToolPayload;
use crate::tools::flat_tool_name;
use crate::tools::registry::PostToolUsePayload;
use chrono::Utc;
use codex_tools::ToolName;
use peregrine_audit_store::AuditStore;
use peregrine_types::{
    AuditEvidence, AuditEvidenceAttestation, Metadata, SourcePrecision, VerificationMethod,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

const MAX_OBSERVATION_BYTES: usize = 8_000;
const MAX_SUMMARY_BYTES: usize = 512;

pub(crate) struct RouterEvidenceCapture {
    pub(crate) tool_input: Value,
    pub(crate) tool_response: Value,
    pub(crate) output_preview: String,
}

pub(crate) fn router_evidence_capture(
    turn: &TurnContext,
    tool_name: &ToolName,
    payload: &ToolPayload,
    post_tool_use_payload: Option<&PostToolUsePayload>,
    output_preview: String,
) -> Result<Option<String>, String> {
    if !should_capture_tool(tool_name) {
        return Ok(None);
    }
    let Some(scope) = audit_scope(turn) else {
        return Ok(None);
    };
    let capture = RouterEvidenceCapture {
        tool_input: post_tool_use_payload
            .map(|payload| payload.tool_input.clone())
            .unwrap_or_else(|| payload_as_value(payload)),
        tool_response: post_tool_use_payload
            .map(|payload| payload.tool_response.clone())
            .unwrap_or_else(|| Value::String(output_preview.clone())),
        output_preview,
    };
    let flat_name = flat_tool_name(tool_name).into_owned();
    let evidence = AuditEvidence {
        id: String::new(),
        audit_run_id: scope.audit_id.clone(),
        work_item_id: None,
        verification_method: verification_method(&flat_name),
        provider_id: provider_id(tool_name),
        adapter_id: None,
        tool_name: flat_name.clone(),
        tool_version: None,
        input_hash: sha256_json(&capture.tool_input)?,
        source_precision: source_precision(&flat_name),
        attestation: AuditEvidenceAttestation::RouterCaptured,
        summary: bounded_text(
            &format!("Router-captured successful output from `{flat_name}`."),
            MAX_SUMMARY_BYTES,
        ),
        observation: bounded_observation(&capture),
        execution_trace_ref: None,
        artifact_refs: Vec::new(),
        created_at: Utc::now().timestamp(),
        metadata: Metadata::new(),
    };
    let store = AuditStore::open(&scope.peregrine_home).map_err(|error| error.to_string())?;
    store
        .record_router_evidence_for_current_work(&scope.audit_id, evidence)
        .map(|recorded| recorded.map(|(_, evidence_ref)| evidence_ref))
        .map_err(|error| error.to_string())
}

fn should_capture_tool(tool_name: &ToolName) -> bool {
    let flat_name = flat_tool_name(tool_name);
    if flat_name.starts_with("audit_")
        || matches!(
            flat_name.as_ref(),
            "update_plan"
                | "get_goal"
                | "create_goal"
                | "update_goal"
                | "request_user_input"
                | "request_permissions"
                | "tool_search"
                | "list_available_plugins_to_install"
                | "request_plugin_install"
                | "apply_patch"
                | "view_image"
                | "spawn_agent"
                | "send_input"
                | "wait_agent"
                | "close_agent"
                | "resume_agent"
                | "spawn_agents_on_csv"
                | "report_agent_job_result"
                | "list_mcp_servers"
                | "list_mcp_resources"
                | "list_mcp_resource_templates"
                | "read_mcp_resource"
        )
    {
        return false;
    }
    tool_name.namespace.is_some()
        || flat_name.starts_with("mcp__")
        || matches!(flat_name.as_ref(), "exec_command" | "shell_command")
}

fn provider_id(tool_name: &ToolName) -> String {
    tool_name
        .namespace
        .as_ref()
        .map_or_else(|| "native".to_string(), std::clone::Clone::clone)
}

fn verification_method(tool_name: &str) -> VerificationMethod {
    let lower = tool_name.to_ascii_lowercase();
    if lower.contains("fuzz") || lower.contains("movy") {
        VerificationMethod::Fuzzing
    } else if lower.contains("symbolic") || lower.contains("smt") || lower.contains("z3") {
        VerificationMethod::SymbolicExecution
    } else if lower.contains("prover") || lower.contains("formal") {
        VerificationMethod::FormalVerification
    } else if lower.contains("bytecode") {
        VerificationMethod::BytecodeAnalysis
    } else if lower.contains("graph") || lower.contains("callgraph") || lower.contains("cfg") {
        VerificationMethod::GraphAnalysis
    } else if lower.contains("economic") || lower.contains("oracle") || lower.contains("liquidity")
    {
        VerificationMethod::EconomicSimulation
    } else if lower.contains("replay")
        || lower.contains("dry_run")
        || lower.contains("dev_inspect")
        || lower.contains("exploit")
    {
        VerificationMethod::ExploitReplay
    } else if matches!(tool_name, "exec_command" | "shell_command") {
        VerificationMethod::GeneratedTest
    } else {
        VerificationMethod::StaticAnalysis
    }
}

fn source_precision(tool_name: &str) -> SourcePrecision {
    let lower = tool_name.to_ascii_lowercase();
    if lower.contains("bytecode") {
        SourcePrecision::Bytecode
    } else if lower.contains("compiler") || lower.contains("prover") {
        SourcePrecision::Compiler
    } else if lower.contains("graph") || lower.contains("static") || lower.contains("source") {
        SourcePrecision::SourceMap
    } else {
        SourcePrecision::Summary
    }
}

fn sha256_json(value: &impl Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn payload_as_value(payload: &ToolPayload) -> Value {
    match payload {
        ToolPayload::Function { arguments } => {
            serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.clone()))
        }
        ToolPayload::ToolSearch { arguments } => Value::String(arguments.query.clone()),
        ToolPayload::Custom { input } => Value::String(input.clone()),
    }
}

fn bounded_observation(capture: &RouterEvidenceCapture) -> String {
    let response = serde_json::to_string(&capture.tool_response)
        .unwrap_or_else(|error| format!("failed to serialize tool response: {error}"));
    if response.is_empty() || response == "null" {
        bounded_text(&capture.output_preview, MAX_OBSERVATION_BYTES)
    } else {
        bounded_text(&response, MAX_OBSERVATION_BYTES)
    }
}

fn bounded_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let suffix = "...";
    let mut end = max_bytes.saturating_sub(suffix.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &value[..end], suffix)
}
