import {
  PeregrineAgentRuntime,
  type AgentContextPacket,
  type AgentRole,
  type ToolRunSummary,
} from "@peregrine/agent-runtime";

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

export async function runAgentWorkflowWithModel({
  agent,
  onTrace,
  projectContext,
  signal,
  workflow,
}: {
  agent: AgentDefinition;
  onTrace?: (event: AgentRunTraceEvent) => void;
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
  const workspaceRuntime = createAgentWorkspaceRuntime({
    state: toolState,
    activeToolIds: agent.tools,
    requireToolApproval: agent.execution.requireToolApproval,
    onToolRun: (toolRun) => {
      toolRuns.push(toolRun);
      onTrace?.({
        level: toolRun.status === "failed" || toolRun.status === "denied" ? "warning" : "trace",
        message: `Tool ${toolRun.toolId}: ${toolRun.summary}`,
      });
    },
  });
  const packet = buildWorkflowContextPacket(agent, workflow, toolState, projectContext);
  const prompt = buildAgentPrompt(agent, workflow, workspaceRuntime.tools);
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
    message: `Agent context packet sent: ${formatTraceText(JSON.stringify(packet), 1_200)}`,
  });
  onTrace?.({
    level: "trace",
    message: `User prompt sent: ${formatTraceText(prompt, 1_200)}`,
  });

  const result = await runner.generate({
    packet,
    abortSignal: signal,
    prompt,
    activeToolIds: agent.tools,
    timeout: {
      totalMs: 120_000,
    },
  });

  onTrace?.({
    level: "info",
    message: "Agent runtime completed through @peregrine/agent-runtime.",
  });

  return {
    text: result.text.trim(),
    toolRuns,
  };
}

function buildAgentPrompt(
  agent: AgentDefinition,
  workflow: AgentWorkflow,
  tools: Array<{ id: string; description: string }>,
) {
  const nodes = workflow.nodes.map(formatNode).join("\n");
  const edges = workflow.edges.length
    ? workflow.edges
      .map((edge) => `- ${edge.source} -> ${edge.target}`)
      .join("\n")
    : "- none";
  const toolLines = tools.length
    ? tools.map((tool) => `- ${tool.id}: ${tool.description}`).join("\n")
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
    "Use the configured tools when a claim can be checked deterministically.",
    "Do not claim a check passed unless a tool result supports it.",
    "Return a report with these sections: Run Summary, Reasoning Trace Summary, Findings or Output, Evidence Needed, Next Actions.",
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
      "Call deterministic Peregrine tools when evidence is required.",
      "Return a report with these sections: Run Summary, Reasoning Trace Summary, Findings or Output, Evidence Needed, Next Actions.",
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
      description: "Concise workflow report backed by deterministic tool evidence.",
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
