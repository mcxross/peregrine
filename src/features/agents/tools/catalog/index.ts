import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { createBytecodeTools } from "@/features/agents/tools/catalog/bytecode-tools";
import { createDynamicTools } from "@/features/agents/tools/catalog/dynamic-tools";
import { createFindingsTools } from "@/features/agents/tools/catalog/findings-tools";
import { createGraphTools } from "@/features/agents/tools/catalog/graph-tools";
import { createIndexTools } from "@/features/agents/tools/catalog/index-tools";
import { createInvariantTools } from "@/features/agents/tools/catalog/invariant-tools";
import { createKnowledgeTools } from "@/features/agents/tools/catalog/knowledge-tools";
import { createPatchTools } from "@/features/agents/tools/catalog/patch-tools";
import { createReportTools } from "@/features/agents/tools/catalog/report-tools";
import { createStaticTools } from "@/features/agents/tools/catalog/static-tools";
import { createTestTools } from "@/features/agents/tools/catalog/test-tools";
import { createValidationTools } from "@/features/agents/tools/catalog/validation-tools";
import { createAuditTools } from "@/features/agents/tools/catalog/audit-tools";
import { attachDefaultToolManifest } from "@/features/agents/tools/manifest";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";

export function createAgentToolCatalog(
  state: AgentToolRuntimeState,
): DeterministicToolSpec[] {
  return [
    ...createIndexTools(state),
    ...createKnowledgeTools(),
    ...createStaticTools(state),
    ...createGraphTools(state),
    ...createBytecodeTools(state),
    ...createDynamicTools(state),
    ...createValidationTools(state),
    ...createFindingsTools(state),
    ...createReportTools(state),
    ...createPatchTools(state),
    ...createInvariantTools(state),
    ...createTestTools(state),
    ...createAuditTools(state),
  ].map(attachDefaultToolManifest);
}

export const AGENT_TOOL_IDS = [
  "rust.index.package",
  "rust.index.read_symbols",
  "index.context.lookup",
  "rust.index.package_overview",
  "rust.knowledge.sui_move.search",
  "rust.knowledge.sui_move.read",
  "rust.static.scan_package",
  "rust.static.inspect_function",
  "rust.static.find_capabilities",
  "rust.static.list_modules",
  "rust.graph.call_graph.read",
  "rust.graph.call_graph",
  "rust.graph.object_lifecycle",
  "rust.graph.cfg",
  "rust.graph.capability_flow",
  "rust.graph.finding_impact",
  "rust.graph.path_query",
  "rust.bytecode.disassemble",
  "rust.bytecode.cfg",
  "rust.bytecode.stack_effects",
  "rust.bytecode.source_map",
  "rust.dynamic.run_test",
  "rust.dynamic.fuzz_function",
  "rust.dynamic.trace_execution",
  "rust.dynamic.state_diff",
  "rust.validation.run_suite",
  "rust.validation.assert_property",
  "rust.findings.emit",
  "rust.findings.triage",
  "rust.findings.attach_trace",
  "rust.findings.attach_graph",
  "rust.findings.attach_bytecode",
  "rust.findings.link_patch",
  "rust.report.synthesize",
  "rust.report.generate",
  "rust.report.export_markdown",
  "rust.patch.suggest",
  "rust.patch.apply_preview",
  "rust.invariant.infer",
  "rust.invariant.check",
  "rust.test.generate_case",
  "rust.audit.create_session",
  "rust.audit.build_index",
  "rust.audit.knowledge_graph",
  "rust.audit.classify",
  "rust.audit.threat_model",
  "rust.audit.function_risk_map",
  "rust.audit.invariants",
  "rust.audit.static_analysis",
  "rust.audit.graph_analysis",
  "rust.audit.bytecode_review",
  "rust.audit.attack_hypotheses",
  "rust.audit.test_plan",
  "rust.audit.dynamic_analysis",
  "rust.audit.invariant_stress",
  "rust.audit.confirm_findings",
  "rust.audit.severity_ranking",
  "rust.audit.remediation",
  "rust.audit.regression_tests",
  "rust.audit.report",
  "rust.audit.trace",
  "rust.audit.fix_verification",
  "rust.audit.run_full",
] as const;
