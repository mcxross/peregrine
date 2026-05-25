import {
  PeregrineAgentRuntime,
  SUI_MOVE_SECURITY_GUIDE,
  createAiSdkToolName,
  shouldAttachSuiMoveSecurityKnowledge,
  type AgentContextPacket,
  type AgentRuntimeToolResult,
  type AgentRole,
  type ToolCapsule,
  type ToolRunSummary,
} from "@peregrine/agent-runtime";
import type { ToolRouteDecision } from "@peregrine/harness-control";

import {
  createAgentToolRuntimeState,
  createAgentWorkspaceRuntime,
  type AgentToolProjectContext,
} from "./tools";
import { providerById } from "./model-providers/provider-adapters";
import type {
  AgentDefinition,
  AuditReportExport,
  AgentWorkflow,
  AgentWorkflowNode,
} from "./types";

export type AgentRunResult = {
  auditReportExport?: AuditReportExport | null;
  text: string;
  toolRuns: ToolRunSummary[];
};

export type AgentRunTraceEvent = {
  level: "info" | "warning" | "error" | "trace";
  message: string;
};

export type AgentRunStreamEvent =
  | {
      capsules: ToolCapsule[];
      decisions: ToolRouteDecision[];
      type: "route-plan";
    }
  | { type: "text-delta"; text: string }
  | { type: "reasoning-delta"; text: string }
  | {
      input?: unknown;
      title?: string;
      toolCallId: string;
      toolName: string;
      type: "tool-call";
    }
  | {
      output?: unknown;
      summary: string;
      title?: string;
      toolCallId: string;
      toolName: string;
      type: "tool-result";
    }
  | {
      message: string;
      title?: string;
      toolCallId?: string;
      toolName?: string;
      type: "tool-error";
    }
  | {
      approvalId: string;
      toolCallId: string;
      toolName: string;
      type: "tool-approval-request";
    }
  | { toolCallId: string; toolName: string; type: "tool-output-denied" }
  | { type: "step-start" }
  | { finishReason?: string; type: "step-finish" }
  | { finishReason?: string; type: "finish" }
  | { reason?: string; type: "abort" }
  | { message: string; type: "error" };

export async function runAgentWorkflowWithModel({
  agent,
  onTrace,
  onStream,
  projectContext,
  signal,
  workflow,
}: {
  agent: AgentDefinition;
  onTrace?: (event: AgentRunTraceEvent) => void;
  onStream?: (event: AgentRunStreamEvent) => void;
  projectContext?: AgentToolProjectContext | null;
  signal?: AbortSignal;
  workflow: AgentWorkflow;
}): Promise<AgentRunResult> {
  const provider = providerById(agent.provider.providerId);
  const model = await provider.resolveLanguageModel(agent.provider);
  const toolState = createAgentToolRuntimeState(
    projectContext ?? {
      rootPath: "",
      packagePath: ".",
      packageName: workflow.name,
      manifestPath: "",
      packageTree: null,
    },
  );
  const toolRuns: ToolRunSummary[] = [];
  const role = agentRoleForWorkflow(agent, workflow);
  const workspaceRuntime = createAgentWorkspaceRuntime({
    state: toolState,
    activeToolIds: agent.tools,
    objective: agent.description || workflow.description,
    role,
    requireToolApproval: agent.execution.requireToolApproval,
    onToolRun: (toolRun) => {
      toolRuns.push(toolRun);
      onTrace?.({
        level: toolRun.status === "failed" || toolRun.status === "denied" ? "warning" : "trace",
        message: `Tool ${toolRun.toolId}: ${toolRun.summary}`,
      });
    },
  });
  const packet = {
    ...buildWorkflowContextPacket(agent, workflow, toolState, projectContext),
    toolCapsules: workspaceRuntime.toolCapsules,
  };
  const prompt = buildAgentPrompt(agent, workflow, workspaceRuntime.toolCapsules);
  const runner = new PeregrineAgentRuntime({
    model,
    tools: workspaceRuntime.tools,
    toolGateway: workspaceRuntime.toolRuntime,
    maxSteps: Math.max(1, agent.execution.maxSteps),
  });

  onTrace?.({
    level: "trace",
    message: `Prepared model call for ${agent.provider.providerId}/${agent.provider.modelId}. Endpoint: ${agent.provider.endpoint ?? "provider default"}.`,
  });
  onTrace?.({
    level: "trace",
    message: `Registered ${workspaceRuntime.tools.length} deterministic tools for ${agent.name}.`,
  });
  onTrace?.({
    level: "trace",
    message: `Tool router selected ${workspaceRuntime.routePlan.tools.length} tools and skipped ${
      workspaceRuntime.routePlan.decisions.filter((decision) => !decision.selected).length
    }.`,
  });
  onStream?.({
    type: "route-plan",
    capsules: workspaceRuntime.toolCapsules,
    decisions: workspaceRuntime.routePlan.decisions,
  });
  onTrace?.({
    level: "trace",
    message: `Agent context packet sent: ${formatTraceText(JSON.stringify(packet), 1_200)}`,
  });
  onTrace?.({
    level: "trace",
    message: `User prompt sent: ${formatTraceText(prompt, 1_200)}`,
  });

  const stream = await runner.stream({
    packet,
    abortSignal: signal,
    prompt,
    activeToolIds: agent.tools,
    timeout: {
      totalMs: 120_000,
    },
  });
  let text = "";
  let streamAbortReason: string | undefined;
  let streamError: Error | null = null;

  for await (const part of stream.result.fullStream) {
    switch (part.type) {
      case "text-delta":
        text += part.text;
        onStream?.({ type: "text-delta", text: part.text });
        break;
      case "reasoning-delta":
        onStream?.({ type: "reasoning-delta", text: part.text });
        break;
      case "tool-call":
        onStream?.({
          type: "tool-call",
          input: part.input,
          title: part.title,
          toolCallId: part.toolCallId,
          toolName: part.toolName,
        });
        break;
      case "tool-result":
        onStream?.({
          type: "tool-result",
          output: part.output,
          summary: summarizeStreamOutput(part.output),
          title: part.title,
          toolCallId: part.toolCallId,
          toolName: part.toolName,
        });
        break;
      case "tool-error":
        onStream?.({
          type: "tool-error",
          message: errorMessage(part.error),
          title: part.title,
          toolCallId: part.toolCallId,
          toolName: part.toolName,
        });
        break;
      case "tool-approval-request":
        onStream?.({
          type: "tool-approval-request",
          approvalId: part.approvalId,
          toolCallId: part.toolCall.toolCallId,
          toolName: part.toolCall.toolName,
        });
        break;
      case "tool-output-denied":
        onStream?.({
          type: "tool-output-denied",
          toolCallId: part.toolCallId,
          toolName: part.toolName,
        });
        break;
      case "start-step":
        onStream?.({ type: "step-start" });
        break;
      case "finish-step":
        onStream?.({ type: "step-finish", finishReason: part.finishReason });
        break;
      case "finish":
        onStream?.({ type: "finish", finishReason: part.finishReason });
        break;
      case "abort":
        streamAbortReason = part.reason;
        onStream?.({ type: "abort", reason: part.reason });
        break;
      case "error":
        streamError = new Error(errorMessage(part.error));
        onStream?.({ type: "error", message: streamError.message });
        break;
      default:
        break;
    }
  }

  if (streamError) {
    throw streamError;
  }

  if (streamAbortReason !== undefined || signal?.aborted) {
    throw createAbortError(streamAbortReason);
  }

  onTrace?.({
    level: "info",
    message: "Agent runtime completed through @peregrine/agent-runtime.",
  });

  return {
    text: text.trim(),
    toolRuns,
  };
}

export async function runFullAuditWorkflowDeterministic({
  agent,
  onTrace,
  onStream,
  projectContext,
  signal,
  workflow,
}: {
  agent: AgentDefinition;
  onTrace?: (event: AgentRunTraceEvent) => void;
  onStream?: (event: AgentRunStreamEvent) => void;
  projectContext?: AgentToolProjectContext | null;
  signal?: AbortSignal;
  workflow: AgentWorkflow;
}): Promise<AgentRunResult> {
  const toolState = createAgentToolRuntimeState(
    projectContext ?? {
      rootPath: "",
      packagePath: ".",
      packageName: workflow.name,
      manifestPath: "",
      packageTree: null,
    },
  );
  const toolRuns: ToolRunSummary[] = [];
  const workspaceRuntime = createAgentWorkspaceRuntime({
    state: toolState,
    activeToolIds: FULL_AUDIT_STAGE_TOOL_IDS,
    objective: "Run the complete Peregrine audit workflow and emit the audit trace.",
    role: "securityReview",
    requireToolApproval: agent.execution.requireToolApproval,
    onToolRun: (toolRun) => {
      toolRuns.push(toolRun);
      onTrace?.({
        level: toolRun.status === "failed" || toolRun.status === "denied" ? "warning" : "trace",
        message: `Tool ${toolRun.toolId}: ${toolRun.summary}`,
      });
    },
  });
  const toolsById = new Map(workspaceRuntime.tools.map((tool) => [tool.id, tool]));
  const results: AgentRuntimeToolResult[] = [];

  onTrace?.({
    level: "trace",
    message: "Prepared deterministic full audit run. The model is bypassed while stages execute.",
  });
  onStream?.({
    type: "route-plan",
    capsules: workspaceRuntime.toolCapsules,
    decisions: workspaceRuntime.routePlan.decisions,
  });
  const input = projectContext
    ? {
        rootPath: projectContext.rootPath,
        packagePath: projectContext.packagePath,
      }
    : {};
  onStream?.({
    type: "text-delta",
    text: `Running full audit workflow for ${projectContext?.packageName ?? workflow.name}.\n\n`,
  });

  for (const [index, toolId] of FULL_AUDIT_STAGE_TOOL_IDS.entries()) {
    if (toolId === "rust.audit.run_full") {
      continue;
    }

    if (signal?.aborted) {
      throw createAbortError();
    }

    const tool = toolsById.get(toolId);
    const label = auditToolLabel(toolId);
    const toolCallId = `direct_${index}_${Date.now().toString(36)}`;

    if (!tool) {
      const message = `${toolId} is unavailable in the current tool route.`;
      onTrace?.({ level: "error", message });
      throw new Error(message);
    }

    onStream?.({
      type: "tool-call",
      input,
      title: tool.title ?? label,
      toolCallId,
      toolName: tool.id,
    });
    onStream?.({
      type: "text-delta",
      text: `- ${label}: running\n`,
    });

    const result = await workspaceRuntime.toolRuntime.runTool({
      tool,
      input,
      toolCallId,
      context: {
        taskId: "audit-full",
        abortSignal: signal,
        metadata: {
          source: "agents-ui-full-audit",
        },
      },
    });

    results.push(result);

    if (result.status === "failed" || result.status === "denied" || result.status === "timedOut") {
      onStream?.({
        type: "tool-error",
        message: result.summary,
        title: tool.title ?? label,
        toolCallId,
        toolName: tool.id,
      });
      onStream?.({
        type: "text-delta",
        text: `  failed: ${result.summary}\n`,
      });
      onTrace?.({
        level: "warning",
        message: `Deterministic audit stopped at ${toolId}: ${result.summary}`,
      });
      break;
    }

    onStream?.({
      type: "tool-result",
      output: result,
      summary: result.summary,
      title: tool.title ?? label,
      toolCallId,
      toolName: tool.id,
    });
    onStream?.({
      type: "text-delta",
      text: `  done: ${result.summary}\n`,
    });
  }

  const failed = results.find((result) =>
    result.status === "failed" || result.status === "denied" || result.status === "timedOut",
  );
  const digest = deterministicAuditDigest(toolState, results, failed?.summary);

  onStream?.({
    type: "text-delta",
    text: `\n${digest}`,
  });

  onTrace?.({
    level: failed ? "warning" : "info",
    message: failed
      ? `Deterministic full audit stopped early: ${failed.summary}`
      : "Deterministic full audit completed and emitted audit trace.",
  });

  return {
    auditReportExport: buildAuditReportExport(toolState, projectContext, failed?.summary),
    text: "",
    toolRuns,
  };
}

function buildAuditReportExport(
  toolState: ReturnType<typeof createAgentToolRuntimeState>,
  projectContext: AgentToolProjectContext | null | undefined,
  failure?: string,
): AuditReportExport | null {
  const report = toolState.audit.packets.auditReport;

  if (!report?.markdown) {
    return null;
  }

  const auditSession = toolState.audit.packets.auditSession;
  const trace = toolState.audit.packets.auditTrace;
  const projectIndex = toolState.audit.packets.projectIndex;
  const packageName =
    projectIndex?.packageName
    ?? projectContext?.packageName
    ?? auditSession?.project
    ?? "move-package";
  const projectName = auditSession?.project ?? packageName;
  const generatedAt = trace?.generatedAt ?? new Date().toISOString();
  const fileStem = safeExportFileName(`${projectName}-audit-report-${generatedAt.slice(0, 10)}`);
  const markdown = [
    report.markdown.trim(),
    "",
    "---",
    "",
    "## Export Metadata",
    `Audit session: ${report.auditSessionId}`,
    `Generated at: ${generatedAt}`,
    `Evidence completeness: ${report.evidenceCompleteness}`,
    failure ? `Workflow stop reason: ${failure}` : "Workflow status: completed",
  ].join("\n");

  return {
    auditSessionId: report.auditSessionId,
    defaultFileName: `${fileStem || "peregrine-audit-report"}.md`,
    generatedAt,
    markdown,
    packageName,
    projectName,
    reportJson: JSON.stringify(report, null, 2),
    traceJson: trace ? JSON.stringify(trace, null, 2) : undefined,
  };
}

function safeExportFileName(value: string) {
  return value
    .trim()
    .replace(/[^a-z0-9._-]+/gi, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 120)
    .toLowerCase();
}

const FULL_AUDIT_STAGE_TOOL_IDS = [
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
];

function auditToolLabel(toolId: string) {
  const labels: Record<string, string> = {
    "rust.audit.create_session": "Create audit session",
    "rust.audit.build_index": "Build canonical index",
    "rust.audit.knowledge_graph": "Build knowledge graph",
    "rust.audit.classify": "Classify contract",
    "rust.audit.threat_model": "Generate threat model",
    "rust.audit.function_risk_map": "Rank function risk",
    "rust.audit.invariants": "Extract invariants",
    "rust.audit.static_analysis": "Run static analysis",
    "rust.audit.graph_analysis": "Run graph analysis",
    "rust.audit.bytecode_review": "Review bytecode",
    "rust.audit.attack_hypotheses": "Generate attack hypotheses",
    "rust.audit.test_plan": "Generate targeted tests",
    "rust.audit.dynamic_analysis": "Run dynamic analysis",
    "rust.audit.invariant_stress": "Run invariant stress",
    "rust.audit.confirm_findings": "Confirm exploitability",
    "rust.audit.severity_ranking": "Rank severity",
    "rust.audit.remediation": "Generate remediation",
    "rust.audit.regression_tests": "Draft regression tests",
    "rust.audit.report": "Generate audit report",
    "rust.audit.trace": "Export audit trace",
  };

  return labels[toolId] ?? toolId;
}

function deterministicAuditDigest(
  toolState: ReturnType<typeof createAgentToolRuntimeState>,
  results: AgentRuntimeToolResult[],
  failure?: string,
) {
  const packets = toolState.audit.packets;
  const trace = packets.auditTrace;
  const report = packets.auditReport;
  const projectIndex = packets.projectIndex;
  const classification = packets.classification;
  const threatModel = packets.threatModel;
  const riskMap = packets.functionRiskMap;
  const invariants = packets.invariants;
  const hypotheses = packets.attackHypotheses;
  const tests = packets.testPlan;
  const confirmed = packets.confirmedFindings;
  const dynamic = packets.dynamicResults;
  const stress = packets.invariantStress;
  const successfulStages = results.filter((result) => result.status === "succeeded").length;
  const topFindings = (packets.severityRanking?.findings ?? confirmed?.findings ?? []).slice(0, 5);
  const lines = [
    failure ? "Full audit workflow stopped early." : "Full audit workflow completed.",
    "",
    `Stages completed: ${successfulStages}/${FULL_AUDIT_STAGE_TOOL_IDS.length}`,
    `Trace artifacts: ${trace?.artifacts.length ?? 0}`,
    `Package: ${projectIndex?.packageName ?? packets.auditSession?.project ?? "unknown"}`,
    `Modules/functions indexed: ${projectIndex?.modules.length ?? 0}/${projectIndex?.functions.length ?? 0}`,
    `Profiles: ${classification?.profiles.join(", ") || "none"}`,
    `Entry points in threat model: ${threatModel?.entryPoints.length ?? 0}`,
    `High/Critical risk functions: ${(riskMap?.summary.high ?? 0) + (riskMap?.summary.critical ?? 0)}`,
    `Invariants: ${invariants?.invariants.length ?? 0}`,
    `Hypotheses: ${hypotheses?.hypotheses.length ?? 0}`,
    `Targeted tests: ${tests?.tests.length ?? 0}`,
    `Dynamic results: ${dynamic?.testResults.length ?? 0}`,
    `Fuzz/stress results: ${stress?.fuzzResults.length ?? 0}`,
    `Findings: ${report?.findingCount ?? confirmed?.findings.length ?? 0}`,
    `Confirmed findings: ${report?.confirmedFindingCount ?? confirmed?.findings.filter((finding) => finding.state === "confirmed").length ?? 0}`,
    `Evidence completeness: ${report?.evidenceCompleteness ?? (failure ? "blocked" : "partial")}`,
  ];

  if (failure) {
    lines.push("", `Stop reason: ${failure}`);
  }

  if (topFindings.length) {
    lines.push("", "Top findings:");
    for (const finding of topFindings) {
      lines.push(`- [${finding.severity}] ${finding.title} (${finding.state})`);
    }
  } else {
    lines.push("", "Top findings: none emitted by the current evidence set.");
  }

  lines.push("", "Review the Evidence & Findings section in run details for packet-level cards.");

  return lines.join("\n");
}

function summarizeStreamOutput(output: unknown) {
  if (
    output
    && typeof output === "object"
    && "summary" in output
    && typeof (output as { summary?: unknown }).summary === "string"
  ) {
    return (output as { summary: string }).summary;
  }

  return formatTraceText(safeStringify(output ?? null), 360);
}

function errorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return formatTraceText(safeStringify(error ?? "Unknown error"), 360);
}

function createAbortError(reason?: string) {
  if (typeof DOMException !== "undefined") {
    return new DOMException(reason ?? "Aborted", "AbortError");
  }

  const error = new Error(reason ?? "Aborted");
  error.name = "AbortError";
  return error;
}

function safeStringify(value: unknown) {
  try {
    const json = JSON.stringify(value);
    return json ?? String(value);
  } catch {
    return String(value);
  }
}

function buildAgentPrompt(
  agent: AgentDefinition,
  workflow: AgentWorkflow,
  tools: ToolCapsule[],
) {
  const nodes = workflow.nodes.map(formatNode).join("\n");
  const edges = workflow.edges.length
    ? workflow.edges
      .map((edge) => `- ${edge.source} -> ${edge.target}`)
      .join("\n")
    : "- none";
  const toolLines = tools.length
    ? tools
      .map(
        (tool) =>
          [
            `- callable: ${tool.callableName ?? createAiSdkToolName(tool.id)}`,
            `  Peregrine id: ${tool.id}`,
            `  description: ${tool.description}`,
            `  category: ${tool.category}`,
            `  risk: ${tool.risk}`,
            `  use: ${tool.whenToUse.join("; ")}`,
            `  avoid: ${tool.whenNotToUse.join("; ")}`,
          ].join("\n"),
      )
      .join("\n")
    : "- none";

  return [
    `Run the "${agent.name}" workflow and produce the agent's response.`,
    "",
    "Workflow nodes:",
    nodes || "- none",
    "",
    "Workflow edges:",
    edges,
    "",
    "Deterministic Peregrine tools available in this run:",
    toolLines,
    "",
    "When invoking a tool, use the callable name exactly. The Peregrine id is only for evidence lineage and UI display.",
    "First establish Package Intent: identify what the package appears to implement, its main assets, actors, entrypoints, capabilities, and trust boundaries.",
    "Choose specialized security tools only after the package intent is stated or explicitly blocked by missing evidence.",
    "Use the configured tools when a claim can be checked deterministically.",
    "Do not claim a check passed unless a tool result supports it.",
    "Return a report with these sections: Run Summary, Package Intent, Security Tool Plan, Reasoning Trace Summary, Findings or Output, Evidence Needed, Next Actions.",
  ].join("\n");
}

function buildWorkflowContextPacket(
  agent: AgentDefinition,
  workflow: AgentWorkflow,
  toolState: ReturnType<typeof createAgentToolRuntimeState>,
  projectContext?: AgentToolProjectContext | null,
): AgentContextPacket {
  const rootPath = projectContext?.rootPath ?? "";
  const packageName = projectContext?.packageName ?? workflow.name;
  const role = agentRoleForWorkflow(agent, workflow);
  const chain = "sui";

  return {
    task: {
      id: workflow.id,
      role,
      title: workflow.name,
      objective: agent.description || workflow.description,
    },
    developerIntent: [
      agent.systemPrompt,
      "Run the configured Peregrine Agents workflow.",
      "Use the workflow graph as the task boundary.",
      "First establish Package Intent before selecting specialized security checks.",
      "Call deterministic Peregrine tools when evidence is required.",
      "Use callable tool names exactly; dotted Peregrine IDs are lineage identifiers, not callable names.",
      "Return a report with these sections: Run Summary, Package Intent, Security Tool Plan, Reasoning Trace Summary, Findings or Output, Evidence Needed, Next Actions.",
    ].join("\n"),
    projectSummary: {
      id: rootPath || "local-project",
      name: packageName,
      rootPath,
      chain,
      modules: workflow.nodes.map((node) => ({
        id: node.id,
        name: node.data.label,
        summary: node.data.description || node.data.nodeType,
      })),
    },
    securityProfile: "local-agent-workflow",
    selectedCode: [],
    riskSignals: [],
    relevantGuides: shouldAttachSuiMoveSecurityKnowledge(role, chain)
      ? [SUI_MOVE_SECURITY_GUIDE]
      : [],
    currentFindings: toolState.session.triageFindings().map((finding) => ({
      id: finding.id,
      title: finding.title,
      severity: finding.severity,
      status: finding.status,
      location: finding.location,
      evidenceRefs: finding.evidenceRefs.map((ref) => ({
        id: ref,
        kind: "diagnostic" as const,
        summary: finding.message,
        source: finding.id,
      })),
    })),
    recentToolResults: [],
    allowedActions: [
      {
        actionClass: "readOnly",
        description: "Read project, index, graph, and bytecode context.",
        requiresApproval: false,
      },
      {
        actionClass: "toolExecution",
        description: "Run registered deterministic Peregrine tools.",
        requiresApproval: agent.execution.requireToolApproval,
      },
      {
        actionClass: "generatedFileWrite",
        description: "Generate draft reports or test templates.",
        requiresApproval: true,
      },
    ],
    approvalPolicy: {
      mode: providerById(agent.provider.providerId).scope === "local" ? "localAi" : "cloudAiRedacted",
      networkAccess: "approvalRequired",
      sourceModification: "approvalRequired",
      dependencyModification: "approvalRequired",
      secretAccess: "forbidden",
    },
    outputContract: {
      format: "markdown",
      requiredEvidence: true,
      description:
        "Concise workflow report that establishes package intent before evidence-backed security findings.",
    },
  };
}

function agentRoleForWorkflow(agent: AgentDefinition, workflow: AgentWorkflow): AgentRole {
  const text = `${agent.name} ${agent.description} ${workflow.name} ${workflow.description}`.toLowerCase();

  if (text.includes("test")) {
    return "testGeneration";
  }

  if (text.includes("fuzz")) {
    return "fuzzCampaign";
  }

  if (text.includes("formal") || text.includes("spec")) {
    return "formalSpec";
  }

  if (text.includes("patch")) {
    return "patch";
  }

  if (text.includes("document") || text.includes("report")) {
    return "report";
  }

  if (text.includes("triage")) {
    return "triage";
  }

  if (text.includes("ci")) {
    return "ci";
  }

  return "securityReview";
}

function formatNode(node: AgentWorkflowNode) {
  return [
    `- ${node.id}`,
    `  type: ${node.data.nodeType}`,
    `  label: ${node.data.label}`,
    `  description: ${node.data.description}`,
    node.data.toolId ? `  tool: ${node.data.toolId}` : null,
    node.data.provider
      ? `  provider: ${node.data.provider.providerId}/${node.data.provider.modelId}`
      : null,
  ]
    .filter(Boolean)
    .join("\n");
}

function formatTraceText(text: string, maxLength: number) {
  const compact = text.replace(/\s+/g, " ").trim();

  if (compact.length <= maxLength) {
    return compact;
  }

  return `${compact.slice(0, maxLength)}...`;
}
