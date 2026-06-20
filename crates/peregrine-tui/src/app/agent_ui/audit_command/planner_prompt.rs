use peregrine_app_server_protocol::{AuditProfileParams, AuditTargetParams};

use super::AuditTargetRequest;

const ALLOWED_STAGES: &[&str] = &[
    "buildNormalize",
    "semanticGraphs",
    "classification",
    "threatModel",
    "attackSurface",
    "functionRiskMap",
    "invariants",
    "staticAnalysis",
    "graphAnalysis",
    "bytecodeReview",
    "attackHypotheses",
    "verificationPlanning",
    "targetedTests",
    "dynamicAnalysis",
    "invariantStress",
    "symbolicExecution",
    "economicSimulation",
    "exploitConfirmation",
    "adversarialReview",
    "evidenceAggregation",
    "findingValidation",
    "severityRanking",
    "auditReport",
    "auditTrace",
];

const CAPABILITY_EXAMPLES: &[&str] = &[
    "target.acquire",
    "target.normalize",
    "static.analysis",
    "graph.analysis",
    "bytecode.analysis",
    "dynamic.fuzzing",
    "formal.verification",
    "symbolic.execution",
    "economic.simulation",
    "exploit.replay",
];

pub(crate) fn audit_planner_prompt(command_text: &str, request: &AuditTargetRequest) -> String {
    let target_json = json_pretty(&request.target);
    let profile_json = request
        .profile
        .as_ref()
        .map(json_pretty)
        .unwrap_or_else(|| json_pretty(&default_profile()));
    format!(
        "Create a model-authored autonomous security audit plan for this target.\n\n\
         User command:\n{command_text}\n\n\
         Parsed audit target JSON:\n{target_json}\n\n\
         Audit profile JSON:\n{profile_json}\n\n\
         Requirements:\n\
         - This is planning only. Do not start the audit and do not modify the target repository.\n\
         - Inspect the target with read-only tools when useful before deciding the plan. For local packages, prefer bounded reads such as manifests, README/docs, tests, public entry points, and module names.\n\
         - Discover available analysis tools through the normal model-visible ToolRouter/tool_search path. Do not hardcode MCP server implementation names.\n\
         - Choose stages because they fit this specific contract. Do not use every stage unless the target actually justifies it.\n\
         - The stored stage list must start with buildNormalize and include auditReport. Include auditTrace when reproducibility artifacts are useful.\n\
         - Desired capabilities are best-effort evidence goals, not guaranteed tools. Use capability phrases, not tool names.\n\
         - Persist the final immutable plan by calling audit_store_plan exactly once. The target and profile passed to the tool must match the JSON above unless you found a read-only normalization issue that you explain.\n\
         - After audit_store_plan returns, show the fingerprint, the /audit start command, and a concise rationale for the selected stages.\n\n\
         Allowed stage IDs:\n{}\n\n\
         Capability phrase examples:\n{}\n\n\
         The audit_store_plan planner_output must include summary, rationale, focusAreas, nonGoals, stagePlans, and acceptanceCriteria. Each stage plan should explain the stage objective, why it belongs for this target, selected focus areas, desired capabilities, agent roles, and success criteria.",
        ALLOWED_STAGES.join(", "),
        CAPABILITY_EXAMPLES.join(", ")
    )
}

fn default_profile() -> AuditProfileParams {
    AuditProfileParams {
        model_token_budget: 500_000,
        wall_time_seconds: 14_400,
        max_hypotheses: 500,
        max_dependency_depth: 3,
        max_dependency_packages: 64,
    }
}

fn json_pretty(value: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "<unserializable>".to_string())
}
