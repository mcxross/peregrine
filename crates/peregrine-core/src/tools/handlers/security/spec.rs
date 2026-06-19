use codex_tools::{JsonSchema, ResponsesApiTool, ToolSpec};
use serde_json::json;
use std::collections::BTreeMap;

pub const READ_RUN_TOOL_NAME: &str = "audit_read_run";
pub const CLAIM_WORK_TOOL_NAME: &str = "audit_claim_work";
pub const CLAIM_AGENT_ASSIGNMENT_TOOL_NAME: &str = "audit_claim_agent_assignment";
pub const SET_AGENT_ASSIGNMENT_THREAD_TOOL_NAME: &str = "audit_set_agent_assignment_thread";
pub const FINISH_AGENT_ASSIGNMENT_TOOL_NAME: &str = "audit_finish_agent_assignment";
pub const RECORD_PACKET_TOOL_NAME: &str = "audit_record_packet";
pub const RECORD_EVIDENCE_TOOL_NAME: &str = "audit_record_evidence";
pub const RECORD_AGENT_CONCLUSION_TOOL_NAME: &str = "audit_record_agent_conclusion";
pub const FINISH_WORK_TOOL_NAME: &str = "audit_finish_work";
pub const FINALIZE_REPORT_TOOL_NAME: &str = "audit_finalize_report";

pub fn read_run_tool() -> ToolSpec {
    function_tool(
        READ_RUN_TOOL_NAME,
        "Read a bounded summary of the current persisted audit run, queue, coverage gaps, and artifact counts.",
        BTreeMap::new(),
        Vec::new(),
    )
}

pub fn claim_work_tool() -> ToolSpec {
    function_tool(
        CLAIM_WORK_TOOL_NAME,
        "Atomically claim the next pending item in the current audit's deterministic stage queue.",
        BTreeMap::from([(
            "worker_id".to_string(),
            JsonSchema::string(Some(
                "Stable coordinator or agent identifier that will finish the work.".to_string(),
            )),
        )]),
        vec!["worker_id"],
    )
}

pub fn claim_agent_assignment_tool() -> ToolSpec {
    function_tool(
        CLAIM_AGENT_ASSIGNMENT_TOOL_NAME,
        "Atomically claim a pending audit agent assignment for an already claimed work item before spawning the configured role with spawn_agent.",
        BTreeMap::from([
            (
                "work_item_id".to_string(),
                id_schema("Claimed work item ID."),
            ),
            (
                "assignment_id".to_string(),
                id_schema("Pending audit agent assignment ID."),
            ),
            (
                "worker_id".to_string(),
                id_schema("Worker identifier used when claiming the work item."),
            ),
        ]),
        vec!["work_item_id", "assignment_id", "worker_id"],
    )
}

pub fn set_agent_assignment_thread_tool() -> ToolSpec {
    function_tool(
        SET_AGENT_ASSIGNMENT_THREAD_TOOL_NAME,
        "Attach the spawned child agent task/thread identity returned by spawn_agent to an already claimed audit agent assignment.",
        BTreeMap::from([
            (
                "work_item_id".to_string(),
                id_schema("Claimed work item ID."),
            ),
            (
                "assignment_id".to_string(),
                id_schema("Spawned audit agent assignment ID."),
            ),
            (
                "agent_thread_id".to_string(),
                id_schema("Child agent task name or thread identifier returned by spawn_agent."),
            ),
        ]),
        vec!["work_item_id", "assignment_id", "agent_thread_id"],
    )
}

pub fn finish_agent_assignment_tool() -> ToolSpec {
    function_tool(
        FINISH_AGENT_ASSIGNMENT_TOOL_NAME,
        "Mark an audit agent assignment failed or cancelled when no public conclusion artifact can be recorded. Successful assignments should complete through audit_record_agent_conclusion.",
        BTreeMap::from([
            (
                "work_item_id".to_string(),
                id_schema("Claimed work item ID."),
            ),
            (
                "assignment_id".to_string(),
                id_schema("Audit agent assignment ID."),
            ),
            (
                "status".to_string(),
                JsonSchema::string_enum(
                    vec![json!("failed"), json!("cancelled")],
                    Some("Terminal non-success assignment state.".to_string()),
                ),
            ),
        ]),
        vec!["work_item_id", "assignment_id", "status"],
    )
}

pub fn record_packet_tool() -> ToolSpec {
    function_tool(
        RECORD_PACKET_TOOL_NAME,
        "Persist a bounded structured stage packet under the current audit workspace. The store chooses the path; this tool never accepts filesystem paths.",
        BTreeMap::from([
            (
                "work_item_id".to_string(),
                id_schema("Claimed work item ID."),
            ),
            (
                "packet_kind".to_string(),
                JsonSchema::string(Some(
                    "Chain-neutral packet kind, such as knowledgeGraph or attackSurface."
                        .to_string(),
                )),
            ),
            (
                "summary".to_string(),
                JsonSchema::string(Some("Concise packet summary.".to_string())),
            ),
            (
                "packet".to_string(),
                JsonSchema::object(BTreeMap::new(), None, Some(true.into())),
            ),
        ]),
        vec!["work_item_id", "packet_kind", "summary", "packet"],
    )
}

pub fn record_evidence_tool() -> ToolSpec {
    function_tool(
        RECORD_EVIDENCE_TOOL_NAME,
        "Persist normalized candidate evidence observed from a registered tool. Records created by this model-visible tool are marked modelSubmitted and cannot independently confirm a finding. Generated code alone is not evidence.",
        BTreeMap::from([
            (
                "work_item_id".to_string(),
                id_schema("Claimed work item ID."),
            ),
            (
                "verification_method".to_string(),
                JsonSchema::string_enum(
                    vec![
                        json!("staticAnalysis"),
                        json!("graphAnalysis"),
                        json!("bytecodeAnalysis"),
                        json!("generatedTest"),
                        json!("fuzzing"),
                        json!("symbolicExecution"),
                        json!("formalVerification"),
                        json!("economicSimulation"),
                        json!("exploitReplay"),
                        json!("humanReview"),
                    ],
                    Some("Verification class that produced the observation.".to_string()),
                ),
            ),
            (
                "provider_id".to_string(),
                id_schema("Capability provider ID."),
            ),
            (
                "adapter_id".to_string(),
                id_schema("Optional blockchain adapter ID."),
            ),
            ("tool_name".to_string(), id_schema("Registered tool name.")),
            (
                "tool_version".to_string(),
                id_schema("Optional tool version."),
            ),
            (
                "input_hash".to_string(),
                id_schema("Hash of the verified input."),
            ),
            (
                "source_precision".to_string(),
                JsonSchema::string_enum(
                    vec![
                        json!("compiler"),
                        json!("sourceMap"),
                        json!("bytecode"),
                        json!("summary"),
                        json!("heuristic"),
                        json!("unknown"),
                    ],
                    Some("Precision of the source attribution.".to_string()),
                ),
            ),
            (
                "summary".to_string(),
                JsonSchema::string(Some("Concise evidence claim.".to_string())),
            ),
            (
                "observation".to_string(),
                JsonSchema::string(Some(
                    "Observed result, counterexample, or execution outcome.".to_string(),
                )),
            ),
            (
                "execution_trace_ref".to_string(),
                id_schema("Optional existing trace reference under traces/."),
            ),
            (
                "artifact_refs".to_string(),
                JsonSchema::array(
                    JsonSchema::string(None),
                    Some("Existing audit-owned artifact references.".to_string()),
                ),
            ),
        ]),
        vec![
            "work_item_id",
            "verification_method",
            "provider_id",
            "tool_name",
            "input_hash",
            "source_precision",
            "summary",
            "observation",
        ],
    )
}

pub fn record_agent_conclusion_tool() -> ToolSpec {
    function_tool(
        RECORD_AGENT_CONCLUSION_TOOL_NAME,
        "Persist a bounded Researcher/Skeptic/Exploiter/Judge conclusion for the current audit work item. The Judge must receive normalized evidence, exploit results, and role conclusions only; do not include hidden reasoning. Conclusions are artifacts, not verification evidence.",
        BTreeMap::from([
            (
                "work_item_id".to_string(),
                id_schema("Claimed work item ID."),
            ),
            (
                "role".to_string(),
                JsonSchema::string_enum(
                    vec![
                        json!("researcher"),
                        json!("skeptic"),
                        json!("exploiter"),
                        json!("judge"),
                    ],
                    Some("Audit agent role that produced the conclusion.".to_string()),
                ),
            ),
            (
                "agent_thread_id".to_string(),
                id_schema("Optional child thread ID for the subagent."),
            ),
            (
                "status".to_string(),
                JsonSchema::string_enum(
                    vec![
                        json!("candidate"),
                        json!("supported"),
                        json!("refuted"),
                        json!("needsValidation"),
                        json!("discarded"),
                        json!("accepted"),
                    ],
                    Some("Role conclusion state for the candidate or work item.".to_string()),
                ),
            ),
            (
                "summary".to_string(),
                JsonSchema::string(Some(
                    "Bounded public conclusion. Cite artifact and evidence references instead of embedding traces.".to_string(),
                )),
            ),
            (
                "candidate_ids".to_string(),
                JsonSchema::array(
                    JsonSchema::string(None),
                    Some("Candidate or hypothesis IDs addressed by this conclusion.".to_string()),
                ),
            ),
            (
                "evidence_refs".to_string(),
                JsonSchema::array(
                    JsonSchema::string(None),
                    Some("Existing audit evidence references used by this conclusion.".to_string()),
                ),
            ),
            (
                "artifact_refs".to_string(),
                JsonSchema::array(
                    JsonSchema::string(None),
                    Some("Existing audit-owned artifacts used by this conclusion.".to_string()),
                ),
            ),
            (
                "metadata".to_string(),
                JsonSchema::object(BTreeMap::new(), None, Some(true.into())),
            ),
        ]),
        vec!["work_item_id", "role", "status", "summary"],
    )
}

pub fn finish_work_tool() -> ToolSpec {
    function_tool(
        FINISH_WORK_TOOL_NAME,
        "Finish a claimed audit work item. Evidence references must already have been persisted by audit_record_evidence.",
        BTreeMap::from([
            (
                "work_item_id".to_string(),
                id_schema("Claimed work item ID."),
            ),
            (
                "worker_id".to_string(),
                id_schema("Worker identifier used when claiming the item."),
            ),
            (
                "status".to_string(),
                JsonSchema::string_enum(
                    vec![json!("completed"), json!("failed"), json!("blocked")],
                    Some("Terminal work item status.".to_string()),
                ),
            ),
            (
                "evidence_refs".to_string(),
                JsonSchema::array(
                    JsonSchema::string(None),
                    Some("Previously recorded evidence references.".to_string()),
                ),
            ),
        ]),
        vec!["work_item_id", "worker_id", "status"],
    )
}

pub fn finalize_report_tool() -> ToolSpec {
    function_tool(
        FINALIZE_REPORT_TOOL_NAME,
        "Finalize the terminal audit report. The store validates every finding against persisted evidence and rejects confirmed findings without two independent verification classes plus adapter replay evidence.",
        BTreeMap::from([
            (
                "findings".to_string(),
                JsonSchema::array(
                    JsonSchema::object(BTreeMap::new(), None, Some(true.into())),
                    Some("Final report findings. Confirmed findings must cite validated evidence references.".to_string()),
                ),
            ),
            (
                "metadata".to_string(),
                JsonSchema::object(
                    BTreeMap::new(),
                    None,
                    Some(true.into()),
                ),
            ),
        ]),
        vec!["findings"],
    )
}

fn id_schema(description: &str) -> JsonSchema {
    JsonSchema::string(Some(description.to_string()))
}

fn function_tool(
    name: &str,
    description: &str,
    properties: BTreeMap<String, JsonSchema>,
    required: Vec<&str>,
) -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: name.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(required.into_iter().map(str::to_string).collect()),
            Some(false.into()),
        ),
        output_schema: None,
    })
}
