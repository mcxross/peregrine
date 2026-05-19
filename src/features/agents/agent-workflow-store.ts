import type {
  AgentDefinition,
  AgentExecutionLog,
  AgentStudioState,
  AgentWorkflow,
  AgentWorkflowNodeType,
} from "@/features/agents/types";
import {
  createAgentWorkflow,
  defaultAgents,
  defaultWorkflows,
} from "@/features/agents/default-agents";
import type { ProjectMetadata } from "@/features/empty-project/filesystem-tree";

const STORAGE_KEY = "peregrine.agents.studio.v1";

export function loadAgentStudioState(): AgentStudioState {
  if (typeof window === "undefined") {
    return createInitialState();
  }

  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);

    if (!stored) {
      return createInitialState();
    }

    return normalizeAgentStudioState(JSON.parse(stored) as Partial<AgentStudioState>);
  } catch {
    return createInitialState();
  }
}

export function loadAgentStudioStateFromProjectMetadata(
  metadata: ProjectMetadata,
): AgentStudioState {
  if (!metadata.agents || typeof metadata.agents !== "object") {
    return loadAgentStudioState();
  }

  return normalizeAgentStudioState(metadata.agents as Partial<AgentStudioState>);
}

export function agentStudioStateToProjectMetadata(
  metadata: ProjectMetadata,
  state: AgentStudioState,
): ProjectMetadata {
  return {
    ...metadata,
    agents: state,
  };
}

export function saveAgentStudioState(state: AgentStudioState) {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

export function createCustomAgent(): {
  agent: AgentDefinition;
  workflow: AgentWorkflow;
} {
  const id = createClientId("agent");
  const workflowId = createClientId("workflow");
  const name = "Custom Agent";
  const workflow = createAgentWorkflow({
    id: workflowId,
    agentName: name,
    description: "Custom visual workflow.",
    modelId: "llama3.2",
    providerId: "ollama",
  });

  return {
    agent: {
      id,
      kind: "custom",
      name,
      description: "User-defined agent workflow.",
      systemPrompt:
        "You are a Peregrine agent. Use structured context, deterministic tools, and evidence-backed output.",
      tools: ["index.context.lookup"],
      provider: {
        providerId: "ollama",
        modelId: "llama3.2",
        endpoint: "http://127.0.0.1:11434",
      },
      execution: {
        mode: "approvalGated",
        maxSteps: 10,
        requireToolApproval: true,
        persistMemory: false,
      },
      status: "idle",
      workflowId,
      updatedAt: Date.now(),
    },
    workflow,
  };
}

export function duplicateAgent(
  agent: AgentDefinition,
  workflow: AgentWorkflow,
): { agent: AgentDefinition; workflow: AgentWorkflow } {
  const agentId = createClientId("agent");
  const workflowId = createClientId("workflow");
  const nextName = `${agent.name} Copy`;

  return {
    agent: {
      ...agent,
      id: agentId,
      kind: "custom",
      name: nextName,
      workflowId,
      status: "idle",
      updatedAt: Date.now(),
    },
    workflow: {
      ...workflow,
      id: workflowId,
      name: nextName,
      version: workflow.version + 1,
      updatedAt: Date.now(),
      nodes: workflow.nodes.map((node) => ({
        ...node,
        id: node.id.replace(workflow.id, workflowId),
        data: {
          ...node.data,
          label: node.data.nodeType === "agent" ? nextName : node.data.label,
          status: "idle",
        },
      })),
      edges: workflow.edges.map((edge) => ({
        ...edge,
        id: edge.id.replace(workflow.id, workflowId),
        source: edge.source.replace(workflow.id, workflowId),
        target: edge.target.replace(workflow.id, workflowId),
      })),
    },
  };
}

export function createWorkflowNode(
  nodeType: AgentWorkflowNodeType,
  position: { x: number; y: number },
) {
  const labels: Record<AgentWorkflowNodeType, string> = {
    trigger: "Trigger",
    agent: "Agent",
    tool: "Tool",
    condition: "Condition",
    model: "Model call",
    memory: "Memory",
    input: "Input",
    output: "Output",
    integration: "Integration",
  };

  return {
    id: createClientId(`node-${nodeType}`),
    type: "agentWorkflow",
    position,
    data: {
      label: labels[nodeType],
      description: "",
      nodeType,
      status: "idle" as const,
    },
  };
}

export function createExecutionLog(
  log: Omit<AgentExecutionLog, "id" | "timestamp">,
): AgentExecutionLog {
  return {
    ...log,
    id: createClientId("log"),
    timestamp: Date.now(),
  };
}

function createInitialState(): AgentStudioState {
  return {
    agents: clone(defaultAgents),
    workflows: clone(defaultWorkflows),
    logs: [],
    selectedAgentId: defaultAgents[0].id,
    selectedWorkflowId: defaultAgents[0].workflowId,
  };
}

function normalizeAgentStudioState(parsed: Partial<AgentStudioState>): AgentStudioState {
  const agents = mergeDefaultAgents(parsed.agents ?? []);
  const workflows = mergeDefaultWorkflows(parsed.workflows ?? []);
  const selectedAgentId = agents.some((agent) => agent.id === parsed.selectedAgentId)
    ? parsed.selectedAgentId
    : agents[0]?.id;
  const selectedWorkflowId = workflows.some((workflow) => workflow.id === parsed.selectedWorkflowId)
    ? parsed.selectedWorkflowId
    : agents.find((agent) => agent.id === selectedAgentId)?.workflowId ?? workflows[0]?.id;

  return {
    agents,
    workflows,
    logs: parsed.logs ?? [],
    selectedAgentId: selectedAgentId ?? defaultAgents[0].id,
    selectedWorkflowId: selectedWorkflowId ?? defaultWorkflows[0].id,
  };
}

function mergeDefaultAgents(agents: AgentDefinition[]) {
  const customAgents = agents.filter((agent) => agent.kind === "custom").map(clone);

  return [
    ...defaultAgents.map((defaultAgent) => {
      const storedAgent = agents.find((agent) => agent.id === defaultAgent.id);

      return {
        ...defaultAgent,
        provider: storedAgent?.provider ?? defaultAgent.provider,
        execution: storedAgent?.execution ?? defaultAgent.execution,
        systemPrompt: storedAgent?.systemPrompt ?? defaultAgent.systemPrompt,
        tools: storedAgent?.tools ?? defaultAgent.tools,
        status: storedAgent?.status ?? defaultAgent.status,
        updatedAt: storedAgent?.updatedAt ?? defaultAgent.updatedAt,
      };
    }),
    ...customAgents,
  ];
}

function mergeDefaultWorkflows(workflows: AgentWorkflow[]) {
  const customWorkflows = workflows.filter(
    (workflow) => !defaultWorkflows.some((defaultWorkflow) => defaultWorkflow.id === workflow.id),
  ).map(clone);
  const mergedDefaultWorkflows = defaultWorkflows.map((defaultWorkflow) => {
    const storedWorkflow = workflows.find((workflow) => workflow.id === defaultWorkflow.id);

    return clone(storedWorkflow ?? defaultWorkflow);
  });

  return [...mergedDefaultWorkflows, ...customWorkflows];
}

function createClientId(prefix: string) {
  if ("randomUUID" in crypto) {
    return `${prefix}_${crypto.randomUUID().replace(/-/g, "")}`;
  }

  return `${prefix}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2)}`;
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
