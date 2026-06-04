import type { Thread } from "@peregrine/app-server-protocol/v2";

import type {
  AgentDefinition,
  AgentExecutionConfig,
  AgentStatus,
  AgentStudioState,
  AgentWorkflow,
  AgentWorkflowNodeType,
} from "./types";

const PRIMARY_AGENT_ID = "main";

const appServerExecution: AgentExecutionConfig = {
  mode: "approvalGated",
  maxSteps: 12,
  requireToolApproval: true,
  persistMemory: false,
};

const appServerProvider = {
  providerId: "app-server",
  modelId: "",
};

export function ensurePrimaryAgentThreadState(state: AgentStudioState): AgentStudioState {
  const primary = state.agents.find((agent) => agent.isPrimary)
    ?? state.agents.find((agent) => agent.id === PRIMARY_AGENT_ID);
  const agents = primary
    ? state.agents.map((agent) =>
      agent.id === primary.id ? { ...primaryAgentDefinition(), ...agent, isPrimary: true } : agent,
    )
    : [primaryAgentDefinition(), ...state.agents];
  const workflows = agents.flatMap((agent, index) => {
    const existing = state.workflows.find((workflow) => workflow.id === agent.workflowId);
    return existing ? [existing] : [createAgentWorkflowForThread(agent, index)];
  });
  const selectedAgentId = agents.some((agent) => agent.id === state.selectedAgentId)
    ? state.selectedAgentId
    : PRIMARY_AGENT_ID;
  const selectedWorkflowId = workflows.some((workflow) => workflow.id === state.selectedWorkflowId)
    ? state.selectedWorkflowId
    : agents.find((agent) => agent.id === selectedAgentId)?.workflowId ?? workflows[0]?.id ?? "";

  return {
    ...state,
    agents,
    workflows,
    selectedAgentId,
    selectedWorkflowId,
  };
}

export function syncAgentStudioStateWithServerThread(
  state: AgentStudioState,
  thread: Thread,
  options: { isPrimary?: boolean } = {},
): AgentStudioState {
  const baseState = ensurePrimaryAgentThreadState(state);
  const isPrimary = options.isPrimary ?? thread.agentRole === null;
  const existing = isPrimary
    ? baseState.agents.find((agent) => agent.isPrimary)
    : baseState.agents.find((agent) => agent.serverThreadId === thread.id || agent.id === thread.id);
  const threadAgent = serverThreadToDefinition(thread, existing?.status, isPrimary);
  const agents = existing
    ? baseState.agents.map((agent) => agent.id === existing.id ? { ...threadAgent, id: existing.id } : agent)
    : [...baseState.agents, threadAgent];
  const workflows = agents.flatMap((agent, index) => {
    const existingWorkflow = baseState.workflows.find((workflow) => workflow.id === agent.workflowId);
    return existingWorkflow ? [existingWorkflow] : [createAgentWorkflowForThread(agent, index)];
  });
  const selectedAgentId = agents.some((agent) => agent.id === baseState.selectedAgentId)
    ? baseState.selectedAgentId
    : threadAgent.id;
  const selectedWorkflowId = workflows.some((workflow) => workflow.id === baseState.selectedWorkflowId)
    ? baseState.selectedWorkflowId
    : agents.find((agent) => agent.id === selectedAgentId)?.workflowId ?? threadAgent.workflowId;

  return {
    ...baseState,
    agents,
    workflows,
    selectedAgentId,
    selectedWorkflowId,
  };
}

export function markAgentThreadClosed(
  state: AgentStudioState,
  threadId: string,
): AgentStudioState {
  return {
    ...state,
    agents: state.agents.map((agent) =>
      agent.serverThreadId === threadId || agent.id === threadId
        ? {
          ...agent,
          isClosed: true,
          status: agent.status === "running" ? "completed" : agent.status,
          updatedAt: Date.now(),
        }
        : agent,
    ),
  };
}

export function formatAgentThreadName({
  agentNickname,
  agentRole,
  isPrimary,
}: {
  agentNickname?: string | null;
  agentRole?: string | null;
  isPrimary: boolean;
}) {
  if (isPrimary) {
    return "Main [default]";
  }

  const nickname = agentNickname?.trim();
  const role = agentRole?.trim();

  if (nickname && role) {
    return `${nickname} [${role}]`;
  }
  if (nickname) {
    return nickname;
  }
  if (role) {
    return `[${role}]`;
  }

  return "Agent";
}

function primaryAgentDefinition(): AgentDefinition {
  const workflowId = workflowIdForThread(PRIMARY_AGENT_ID);

  return {
    id: PRIMARY_AGENT_ID,
    kind: "server",
    isPrimary: true,
    name: formatAgentThreadName({ isPrimary: true }),
    description: "Primary app-server thread.",
    systemPrompt: "",
    tools: [],
    provider: appServerProvider,
    execution: appServerExecution,
    status: "active",
    workflowId,
    updatedAt: Date.now(),
  };
}

function serverThreadToDefinition(
  thread: Thread,
  storedStatus: AgentStatus | undefined,
  isPrimary: boolean,
): AgentDefinition {
  const id = isPrimary ? PRIMARY_AGENT_ID : thread.id;
  const status = storedStatus === "running"
    ? "running"
    : thread.status.type === "notLoaded"
      ? "completed"
      : isPrimary
        ? "active"
        : "idle";

  return {
    id,
    kind: "server",
    isPrimary,
    isClosed: thread.status.type === "notLoaded",
    serverThreadId: thread.id,
    roleName: thread.agentRole ?? undefined,
    name: formatAgentThreadName({
      agentNickname: thread.agentNickname,
      agentRole: thread.agentRole,
      isPrimary,
    }),
    description: isPrimary
      ? "Primary app-server thread."
      : `App-server thread ${thread.id}.`,
    systemPrompt: thread.preview,
    tools: [],
    provider: appServerProvider,
    execution: appServerExecution,
    status,
    workflowId: workflowIdForThread(id),
    updatedAt: Date.now(),
  };
}

function createAgentWorkflowForThread(
  agent: AgentDefinition,
  index: number,
): AgentWorkflow {
  const id = agent.workflowId;
  const offset = index * 16;
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
          ? appServerProvider
          : undefined,
    },
  });

  return {
    id,
    name: agent.name,
    description: agent.description,
    version: 1,
    updatedAt: Date.now(),
    nodes: [
      node("trigger", "Manual trigger", 40, 90, "Starts an app-server thread turn."),
      node("input", "Project context", 230, 90, "Loads bounded desktop project context."),
      node("agent", agent.name, 440, 90, agent.description),
      node("tool", "App-server tools", 665, 20, "Uses tools exposed by the Rust app server."),
      node("condition", "Approval gate", 665, 160, "Waits for app-server approval or input requests."),
      node("output", "Run stream", 900, 90, "Streams app-server notifications into the desktop UI."),
    ],
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

function workflowIdForThread(threadId: string) {
  return `workflow-agent-${encodeURIComponent(threadId)}`;
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
