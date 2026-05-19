import type {
  AgentDefinition,
  AgentExecutionConfig,
  AgentWorkflow,
  AgentWorkflowNodeType,
} from "@/features/agents/types";

const defaultExecution: AgentExecutionConfig = {
  mode: "approvalGated",
  maxSteps: 12,
  requireToolApproval: true,
  persistMemory: false,
};

const now = 1_779_062_400_000;

export const defaultAgents: AgentDefinition[] = [
  {
    id: "agent-code-review",
    kind: "default",
    name: "Code Review Agent",
    description: "Reviews Move modules for correctness, maintainability, and risky diffs.",
    systemPrompt:
      "Review the provided repository context. Focus on concrete issues, evidence, and validation steps.",
    tools: ["index.context.lookup", "tool.run.tests", "report.findings"],
    provider: {
      providerId: "ai-gateway",
      modelId: "openai/gpt-5.2",
    },
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-code-review",
    updatedAt: now,
  },
  {
    id: "agent-security-analysis",
    kind: "default",
    name: "Security Analysis Agent",
    description: "Coordinates security review, tool evidence, and finding drafts.",
    systemPrompt:
      "Assess smart contract risk from bounded context packets and deterministic tool evidence.",
    tools: ["index.context.lookup", "static.signals.read", "report.findings"],
    provider: {
      providerId: "ai-gateway",
      modelId: "anthropic/claude-sonnet-4-5",
    },
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-security-analysis",
    updatedAt: now,
  },
  {
    id: "agent-indexing",
    kind: "default",
    name: "Indexing Agent",
    description: "Monitors project indexing and requests targeted context refreshes.",
    systemPrompt:
      "Inspect index health and request bounded refreshes without making security conclusions.",
    tools: ["index.status.read", "index.context.lookup", "index.refresh"],
    provider: {
      providerId: "ollama",
      modelId: "llama3.2",
      endpoint: "http://127.0.0.1:11434",
    },
    execution: {
      ...defaultExecution,
      mode: "manual",
      maxSteps: 8,
      requireToolApproval: false,
    },
    status: "idle",
    workflowId: "workflow-indexing",
    updatedAt: now,
  },
  {
    id: "agent-documentation",
    kind: "default",
    name: "Documentation Agent",
    description: "Turns verified findings and context into audit-ready documentation.",
    systemPrompt:
      "Prepare concise documentation from evidence. Do not invent unsupported claims.",
    tools: ["index.context.lookup", "report.export.markdown"],
    provider: {
      providerId: "ai-gateway",
      modelId: "google/gemini-3-pro",
    },
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-documentation",
    updatedAt: now,
  },
  {
    id: "agent-test-generation",
    kind: "default",
    name: "Test Generation Agent",
    description: "Drafts adversarial test plans and validation commands.",
    systemPrompt:
      "Generate test plans and patches only when supported by code context and expected checks.",
    tools: ["index.context.lookup", "test.plan.create", "tool.run.tests"],
    provider: {
      providerId: "ai-gateway",
      modelId: "openai/gpt-5.2",
    },
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-test-generation",
    updatedAt: now,
  },
];

export const defaultWorkflows: AgentWorkflow[] = defaultAgents.map((agent, index) =>
  createAgentWorkflow({
    id: agent.workflowId,
    agentName: agent.name,
    description: agent.description,
    offset: index * 16,
    providerId: agent.provider.providerId,
    modelId: agent.provider.modelId,
  }),
);

export function createAgentWorkflow({
  agentName,
  description,
  id,
  modelId,
  offset = 0,
  providerId,
}: {
  agentName: string;
  description: string;
  id: string;
  modelId: string;
  offset?: number;
  providerId: string;
}) {
  const node = (
    nodeType: AgentWorkflowNodeType,
    label: string,
    x: number,
    y: number,
    nodeDescription = "",
  ) => ({
    id: `${id}-${nodeType}`,
    type: "agentWorkflow",
    position: { x: x + offset, y },
    data: {
      label,
      description: nodeDescription,
      nodeType,
      status: "idle" as const,
      provider:
        nodeType === "agent" || nodeType === "model"
          ? { providerId, modelId }
          : undefined,
    },
  });

  const nodes = [
    node("trigger", "Manual trigger", 40, 90, "Starts the workflow."),
    node("input", "Context packet", 230, 90, "Loads bounded project context."),
    node("agent", agentName, 440, 90, description),
    node("tool", "Deterministic tools", 665, 20, "Runs allowed Peregrine tools."),
    node("condition", "Evidence gate", 665, 160, "Checks evidence completeness."),
    node("output", "Report event", 900, 90, "Stores trace and output."),
  ];

  return {
    id,
    name: agentName,
    description,
    version: 1,
    updatedAt: now,
    nodes,
    edges: [
      edge(id, "trigger", "input"),
      edge(id, "input", "agent"),
      edge(id, "agent", "tool"),
      edge(id, "agent", "condition"),
      edge(id, "tool", "output"),
      edge(id, "condition", "output"),
    ],
  };
}

function edge(workflowId: string, source: AgentWorkflowNodeType, target: AgentWorkflowNodeType) {
  return {
    id: `${workflowId}-${source}-${target}`,
    source: `${workflowId}-${source}`,
    target: `${workflowId}-${target}`,
    animated: source === "agent",
    type: "smoothstep",
  };
}

