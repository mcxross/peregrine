import {
  PeregrineAgentRuntime,
  createAiSdkToolName,
  type AgentContextPacket,
  type AgentRole,
  type ToolCapsule,
  type ToolRunSummary,
} from "@peregrine/agent-runtime";
import type { ToolRouteDecision } from "@peregrine/harness-control";

import {
  createAgentToolRuntimeState,
  createAgentWorkspaceRuntime,
  type AgentToolProjectContext,
} from "@/features/agents/tools";
import { providerById } from "@/features/agents/model-providers/provider-adapters";
import type {
  AgentDefinition,
  AgentWorkflow,
  AgentWorkflowNode,
} from "@/features/agents/types";

export type AgentRunResult = {
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

  return {
    task: {
      id: workflow.id,
      role: agentRoleForWorkflow(agent, workflow),
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
      chain: "sui",
      modules: workflow.nodes.map((node) => ({
        id: node.id,
        name: node.data.label,
        summary: node.data.description || node.data.nodeType,
      })),
    },
    securityProfile: "local-agent-workflow",
    selectedCode: [],
    riskSignals: [],
    relevantGuides: [],
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
