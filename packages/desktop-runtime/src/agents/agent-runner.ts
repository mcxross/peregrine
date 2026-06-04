import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  RequestId,
  ServerNotification,
  ServerRequest,
} from "@peregrine/app-server-protocol";
import type {
  ModelListResponse,
  ModelProviderListResponse,
  Thread,
} from "@peregrine/app-server-protocol/v2";

import type {
  AgentDefinition,
  AgentToolProjectContext,
  AgentWorkflow,
} from "./types";

export const AGENT_SERVER_EVENT = "agent-server-event";
export const AGENT_SERVER_REQUEST = "agent-server-request";
export const AGENT_SERVER_DISCONNECTED = "agent-server-disconnected";

const SETTINGS_KEY = "peregrine.agents.appServer.v1";

export type AgentServerTarget =
  | { mode: "embedded"; endpoint?: undefined }
  | { mode: "localDaemon"; endpoint?: string }
  | { mode: "remote"; endpoint: string };

export type AgentServerSettings = {
  target: AgentServerTarget;
};

export type AgentRunResult = {
  text: string;
  toolRuns: [];
};

export type AgentRunTraceEvent = {
  level: "info" | "warning" | "error" | "trace";
  message: string;
};

export type AgentRunStreamEvent =
  | { type: "text-delta"; text: string }
  | { type: "reasoning-delta"; text: string }
  | { type: "status"; level: AgentRunTraceEvent["level"]; message: string; title: string }
  | { type: "server-request"; request: ServerRequest }
  | { type: "thread-started"; isPrimary?: boolean; thread: Thread }
  | { type: "thread-closed"; threadId: string }
  | { type: "finish"; finishReason?: string }
  | { type: "abort"; reason?: string }
  | { type: "error"; message: string };

export type AgentServerRequestResolution =
  | { type: "resolve"; result: unknown }
  | { type: "reject"; message: string; code?: number };

export type AgentServerEventEnvelope = {
  sessionId: string;
  runId: string;
  event:
    | { type: "lagged"; skipped: number }
    | { type: "notification"; notification: ServerNotification }
    | { type: "disconnected"; message: string };
};

export type AgentServerRequestEnvelope = {
  sessionId: string;
  runId: string;
  request: ServerRequest;
};

export type AgentServerDisconnectedEnvelope = {
  sessionId: string;
  runId: string;
  message: string;
};

type AgentServerStartResponse = {
  sessionId: string;
  threadId: string;
  thread: Thread;
  turnId: string;
  model: string;
  modelProvider: string;
};

type AgentServerModelListResponse = {
  models: ModelListResponse;
  providers: ModelProviderListResponse;
};

export function loadAgentServerSettings(): AgentServerSettings {
  if (typeof window === "undefined") {
    return defaultAgentServerSettings();
  }

  try {
    const stored = window.localStorage.getItem(SETTINGS_KEY);

    if (!stored) {
      return defaultAgentServerSettings();
    }

    return normalizeAgentServerSettings(JSON.parse(stored) as Partial<AgentServerSettings>);
  } catch {
    return defaultAgentServerSettings();
  }
}

export function saveAgentServerSettings(settings: AgentServerSettings) {
  window.localStorage.setItem(
    SETTINGS_KEY,
    JSON.stringify(normalizeAgentServerSettings(settings)),
  );
}

export async function listAgentServerModels({
  cwd,
  target,
}: {
  cwd?: string;
  target?: AgentServerTarget;
} = {}) {
  return invoke<AgentServerModelListResponse>("agent_server_model_list", {
    request: {
      cwd,
      target: target ?? loadAgentServerSettings().target,
    },
  });
}

export function resolveAgentServerRequest(
  sessionId: string,
  requestId: RequestId,
  result: unknown,
) {
  return invoke<void>("agent_server_request_resolve", {
    request: { sessionId, requestId, result },
  });
}

export function rejectAgentServerRequest(
  sessionId: string,
  requestId: RequestId,
  message: string,
  code?: number,
) {
  return invoke<void>("agent_server_request_reject", {
    request: { sessionId, requestId, message, code },
  });
}

export function interruptAgentRun(sessionId: string, turnId?: string) {
  return invoke<void>("agent_server_turn_interrupt", {
    request: { sessionId, turnId },
  });
}

export function stopAgentRun(sessionId: string) {
  return invoke<void>("agent_server_stop", {
    request: { sessionId },
  });
}

export async function sendAgentTurn({
  prompt,
  sessionId,
}: {
  prompt: string;
  sessionId: string;
}) {
  return invoke<{ threadId: string; turnId: string }>("agent_server_turn_send", {
    request: { sessionId, prompt },
  });
}

export async function runAgentWorkflowWithAppServer({
  agent,
  onServerRequest,
  onStream,
  onTrace,
  projectContext,
  signal,
  target,
  workflow,
}: {
  agent: AgentDefinition;
  onServerRequest?: (request: ServerRequest) => AgentServerRequestResolution | Promise<AgentServerRequestResolution>;
  onStream?: (event: AgentRunStreamEvent) => void;
  onTrace?: (event: AgentRunTraceEvent) => void;
  projectContext?: AgentToolProjectContext | null;
  signal?: AbortSignal;
  target?: AgentServerTarget;
  workflow: AgentWorkflow;
}): Promise<AgentRunResult> {
  const sessionId = createRunSessionId(agent.id);
  let responseText = "";
  let reasoningText = "";
  let settled = false;
  let startResponse: AgentServerStartResponse | null = null;
  const unlisten: UnlistenFn[] = [];

  const finish = new Promise<AgentRunResult>((resolve, reject) => {
    const settleResolve = (result: AgentRunResult) => {
      if (!settled) {
        settled = true;
        resolve(result);
      }
    };
    const settleReject = (error: unknown) => {
      if (!settled) {
        settled = true;
        reject(error);
      }
    };

    const abort = () => {
      void interruptAgentRun(sessionId, startResponse?.turnId).catch(() => undefined);
      void stopAgentRun(sessionId).catch(() => undefined);
      onStream?.({ type: "abort", reason: "Run stopped by user." });
      settleReject(new DOMException("Run stopped by user.", "AbortError"));
    };

    if (signal?.aborted) {
      abort();
      return;
    }

    signal?.addEventListener("abort", abort, { once: true });

    void Promise.all([
      listen<AgentServerEventEnvelope>(AGENT_SERVER_EVENT, (event) => {
        if (event.payload.sessionId !== sessionId) {
          return;
        }
        handleAgentServerEvent({
          envelope: event.payload,
          onStream,
          onTrace,
          settleReject,
          settleResolve,
          updateReasoning: (delta) => {
            reasoningText += delta;
          },
          updateResponse: (delta) => {
            responseText += delta;
          },
        });
      }),
      listen<AgentServerRequestEnvelope>(AGENT_SERVER_REQUEST, (event) => {
        if (event.payload.sessionId !== sessionId) {
          return;
        }
        const { request } = event.payload;
        onStream?.({ type: "server-request", request });
        void resolveServerRequestFromUi(sessionId, request, onServerRequest)
          .catch((error) => {
            onTrace?.({
              level: "error",
              message: `Server request handling failed: ${formatError(error)}`,
            });
          });
      }),
      listen<AgentServerDisconnectedEnvelope>(AGENT_SERVER_DISCONNECTED, (event) => {
        if (event.payload.sessionId !== sessionId) {
          return;
        }
        settleReject(new Error(event.payload.message));
      }),
    ])
      .then((listeners) => {
        unlisten.push(...listeners);
      })
      .then(async () => {
        const prompt = buildAgentPrompt(agent, workflow, projectContext ?? null);
        onTrace?.({
          level: "trace",
          message: `Starting app-server run for ${agent.name}.`,
        });
        startResponse = await invoke<AgentServerStartResponse>("agent_server_start", {
          request: {
            sessionId,
            agentName: agent.name,
            agentRole: agent.isPrimary ? undefined : agent.roleName,
            agentInstructions: agent.systemPrompt,
            workflowName: workflow.name,
            prompt,
            cwd: projectContext?.rootPath,
            workspaceRoots: projectContext?.rootPath ? [projectContext.rootPath] : [],
            target: target ?? loadAgentServerSettings().target,
          },
        });
        onStream?.({ type: "thread-started", isPrimary: true, thread: startResponse.thread });
        onTrace?.({
          level: "info",
          message: `Connected to app-server model ${startResponse.modelProvider}/${startResponse.model}.`,
        });
      })
      .catch(settleReject);
  });

  try {
    const result = await finish;
    return {
      ...result,
      text: result.text || responseText,
      toolRuns: [],
    };
  } finally {
    for (const stopListening of unlisten) {
      stopListening();
    }
    await stopAgentRun(sessionId).catch(() => undefined);
    void reasoningText;
  }
}

async function resolveServerRequestFromUi(
  sessionId: string,
  request: ServerRequest,
  onServerRequest?: (request: ServerRequest) => AgentServerRequestResolution | Promise<AgentServerRequestResolution>,
) {
  const resolution = onServerRequest
    ? await onServerRequest(request)
    : { type: "reject" as const, message: "No desktop request handler is registered." };

  if (resolution.type === "resolve") {
    await resolveAgentServerRequest(sessionId, request.id, resolution.result);
    return;
  }

  await rejectAgentServerRequest(sessionId, request.id, resolution.message, resolution.code);
}

function handleAgentServerEvent({
  envelope,
  onStream,
  onTrace,
  settleReject,
  settleResolve,
  updateReasoning,
  updateResponse,
}: {
  envelope: AgentServerEventEnvelope;
  onStream?: (event: AgentRunStreamEvent) => void;
  onTrace?: (event: AgentRunTraceEvent) => void;
  settleReject: (error: unknown) => void;
  settleResolve: (result: AgentRunResult) => void;
  updateReasoning: (delta: string) => void;
  updateResponse: (delta: string) => void;
}) {
  const { event } = envelope;

  if (event.type === "lagged") {
    onTrace?.({
      level: "warning",
      message: `App-server stream skipped ${event.skipped} best-effort events.`,
    });
    return;
  }

  if (event.type === "disconnected") {
    settleReject(new Error(event.message));
    return;
  }

  const notification = event.notification;
  for (const streamEvent of mapAgentServerNotificationToRunEvents(notification)) {
    onStream?.(streamEvent);
  }

  switch (notification.method) {
    case "item/agentMessage/delta":
      updateResponse(notification.params.delta);
      break;
    case "item/reasoning/summaryTextDelta":
    case "item/reasoning/textDelta":
      updateReasoning(notification.params.delta);
      break;
    case "warning":
      onTrace?.({ level: "warning", message: notification.params.message });
      break;
    case "error":
      settleReject(new Error(notification.params.error.message));
      break;
    case "turn/completed":
      settleResolve({ text: "", toolRuns: [] });
      break;
    default:
      break;
  }
}

export function mapAgentServerNotificationToRunEvents(
  notification: ServerNotification,
): AgentRunStreamEvent[] {
  switch (notification.method) {
    case "item/agentMessage/delta":
      return [{ type: "text-delta", text: notification.params.delta }];
    case "thread/started":
      return [{ type: "thread-started", thread: notification.params.thread }];
    case "thread/closed":
      return [{ type: "thread-closed", threadId: notification.params.threadId }];
    case "item/reasoning/summaryTextDelta":
    case "item/reasoning/textDelta":
      return [{ type: "reasoning-delta", text: notification.params.delta }];
    case "item/commandExecution/outputDelta":
      return [{
        type: "status",
        level: "trace",
        title: "Command output",
        message: notification.params.delta,
      }];
    case "item/started":
      return [{
        type: "status",
        level: "trace",
        title: "Item started",
        message: describeThreadItem(notification.params.item),
      }];
    case "item/completed":
      return [{
        type: "status",
        level: "trace",
        title: "Item completed",
        message: describeThreadItem(notification.params.item),
      }];
    case "warning":
      return [{
        type: "status",
        level: "warning",
        title: "Warning",
        message: notification.params.message,
      }];
    case "error":
      return [{
        type: "error",
        message: notification.params.error.message,
      }];
    case "turn/completed":
      return [{ type: "finish", finishReason: notification.params.turn.status }];
    default:
      return [];
  }
}

function describeThreadItem(item: { type: string } & Record<string, unknown>) {
  switch (item.type) {
    case "agentMessage":
      return "Assistant response";
    case "commandExecution":
      return typeof item.command === "string" ? item.command : "Command execution";
    case "fileChange":
      return "File change";
    case "mcpToolCall":
      return typeof item.tool === "string" ? `MCP tool ${item.tool}` : "MCP tool call";
    case "dynamicToolCall":
      return typeof item.tool === "string" ? `Dynamic tool ${item.tool}` : "Dynamic tool call";
    case "reasoning":
      return "Reasoning";
    case "plan":
      return "Plan";
    default:
      return item.type;
  }
}

function buildAgentPrompt(
  agent: AgentDefinition,
  workflow: AgentWorkflow,
  projectContext: AgentToolProjectContext | null,
) {
  const projectText = projectContext
    ? [
        `Project root: ${projectContext.rootPath}`,
        `Package: ${projectContext.packageName}`,
        `Package path: ${projectContext.packagePath}`,
        `Manifest: ${projectContext.manifestPath}`,
      ].join("\n")
    : "No Move package is currently open.";

  return [
    `Run the ${agent.name} desktop workflow.`,
    agent.roleName ? `App-server role: ${agent.roleName}` : "",
    "",
    `Workflow: ${workflow.name}`,
    workflow.description ? `Workflow description: ${workflow.description}` : "",
    "",
    "Project context:",
    projectText,
    "",
    "Agent instructions:",
    agent.systemPrompt,
    "",
    "Return a concise, evidence-aware response. The TypeScript deterministic audit tools are not available in this desktop path; use the app-server capabilities and ask for approvals when required.",
  ]
    .filter(Boolean)
    .join("\n");
}

function normalizeAgentServerSettings(
  settings: Partial<AgentServerSettings>,
): AgentServerSettings {
  const target = settings.target;

  if (target?.mode === "remote" && target.endpoint?.trim()) {
    return { target: { mode: "remote", endpoint: target.endpoint.trim() } };
  }

  if (target?.mode === "localDaemon") {
    return {
      target: {
        mode: "localDaemon",
        endpoint: target.endpoint?.trim() || undefined,
      },
    };
  }

  return defaultAgentServerSettings();
}

function defaultAgentServerSettings(): AgentServerSettings {
  return {
    target: { mode: "embedded" },
  };
}

function createRunSessionId(agentId: string) {
  const suffix = Math.random().toString(36).slice(2, 8);
  return `${agentId}-${Date.now().toString(36)}-${suffix}`;
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
