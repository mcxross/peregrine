import type {
  AgentExecutionLog,
  AgentStudioState,
  AgentWorkflowNodeType,
} from "./types";
import type { ProjectMetadata } from "../project/filesystem-tree";

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
    agents: persistedAgentStudioState(state),
  };
}

export function saveAgentStudioState(state: AgentStudioState) {
  window.localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify(persistedAgentStudioState(state)),
  );
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
    agents: [],
    workflows: [],
    logs: [],
    selectedAgentId: "",
    selectedWorkflowId: "",
  };
}

function normalizeAgentStudioState(parsed: Partial<AgentStudioState>): AgentStudioState {
  return {
    agents: [],
    workflows: [],
    logs: Array.isArray(parsed.logs) ? parsed.logs : [],
    selectedAgentId: typeof parsed.selectedAgentId === "string" ? parsed.selectedAgentId : "",
    selectedWorkflowId: typeof parsed.selectedWorkflowId === "string" ? parsed.selectedWorkflowId : "",
  };
}

function persistedAgentStudioState(state: AgentStudioState): AgentStudioState {
  return {
    agents: [],
    workflows: [],
    logs: state.logs,
    selectedAgentId: state.selectedAgentId,
    selectedWorkflowId: state.selectedWorkflowId,
  };
}

function createClientId(prefix: string) {
  if ("randomUUID" in crypto) {
    return `${prefix}_${crypto.randomUUID().replace(/-/g, "")}`;
  }

  return `${prefix}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2)}`;
}
