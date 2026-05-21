import {
  PeregrineAgentRuntime,
  type AgentContextPacket,
  type AgentRole,
  type ToolGateway,
} from "@peregrine/agent-runtime";

import { providerById } from "@/features/agents/model-providers/provider-adapters";
import type {
  AgentDefinition,
  AgentWorkflow,
  AgentWorkflowNode,
} from "@/features/agents/types";

export type AgentRunResult = {
  text: string;
};

export type AgentRunTraceEvent = {
  level: "info" | "warning" | "error" | "trace";
  message: string;
};

export async function runAgentWorkflowWithModel({
  agent,
  onTrace,
  signal,
  workflow,
}: {
  agent: AgentDefinition;
  onTrace?: (event: AgentRunTraceEvent) => void;
  signal?: AbortSignal;
  workflow: AgentWorkflow;
}): Promise<AgentRunResult> {
  const provider = providerById(agent.provider.providerId);
  const model = await provider.resolveLanguageModel(agent.provider);
  const packet = buildWorkflowContextPacket(agent, workflow);
  const prompt = buildAgentPrompt(agent, workflow);
  const runner = new PeregrineAgentRuntime({
    model,
    tools: [],
    toolGateway: noToolsGateway,
    maxSteps: Math.max(1, agent.execution.maxSteps),
  });

  onTrace?.({
    level: "trace",
    message: `Prepared model call for ${agent.provider.providerId}/${agent.provider.modelId}. Endpoint: ${agent.provider.endpoint ?? "provider default"}.`,
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
    timeout: {
      totalMs: 120_000,
    },
    toolChoice: "none",
  });

  onTrace?.({
    level: "info",
    message: "Agent runtime completed through @peregrine/agent-runtime.",
  });

  return {
    text: result.text.trim(),
  };
}

function buildAgentPrompt(agent: AgentDefinition, workflow: AgentWorkflow) {
  const nodes = workflow.nodes.map(formatNode).join("\n");
  const edges = workflow.edges.length
    ? workflow.edges
      .map((edge) => `- ${edge.source} -> ${edge.target}`)
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
    "Available tool identifiers configured for this agent:",
    agent.tools.length ? agent.tools.map((tool) => `- ${tool}`).join("\n") : "- none",
    "",
    "Because this workspace run does not yet expose deterministic tool outputs to the model, focus on analysis planning, requested checks, and expected evidence.",
  ].join("\n");
}

function buildWorkflowContextPacket(
  agent: AgentDefinition,
  workflow: AgentWorkflow,
): AgentContextPacket {
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
      "Do not claim that deterministic repository tools ran unless explicit tool evidence is available.",
      "Return a report with these sections: Run Summary, Reasoning Trace Summary, Findings or Output, Evidence Needed, Next Actions.",
    ].join("\n"),
    projectSummary: {
      id: "local-project",
      name: workflow.name,
      rootPath: "",
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
    currentFindings: [],
    recentToolResults: [],
    allowedActions: [
      {
        actionClass: "readOnly",
        description: "Read the provided workflow context.",
        requiresApproval: false,
      },
      {
        actionClass: "toolExecution",
        description: "Use deterministic tools only when registered in the agent runtime.",
        requiresApproval: agent.execution.requireToolApproval,
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
      requiredEvidence: false,
      description: "Concise workflow report with observable evidence needs and next actions.",
    },
  };
}

const noToolsGateway: ToolGateway = {
  async runTool(request) {
    return {
      status: "failed",
      toolId: request.tool.id,
      toolCallId: request.toolCallId,
      action: request.tool.action,
      summary: "No deterministic tools are registered for this UI workflow run.",
      evidenceRefs: [],
      diagnostics: [
        {
          level: "error",
          source: "agents",
          message: "No deterministic tools are registered for this UI workflow run.",
        },
      ],
    };
  },
};

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
