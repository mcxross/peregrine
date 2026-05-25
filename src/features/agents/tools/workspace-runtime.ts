import type { ToolRunSummary } from "@peregrine/agent-runtime";
import type { AgentRole, ToolCapsule } from "@peregrine/agent-runtime";
import {
  DefaultApprovalPolicy,
  HarnessToolRuntime,
  InMemoryEvidenceStore,
  routeTools,
} from "@peregrine/harness-control";
import type { ApprovalGate } from "@peregrine/harness-control";
import type { ToolRoutePlan } from "@peregrine/harness-control";

import { createAgentToolRegistry, resolveAgentTools } from "@/features/agents/tools/registry";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";

export type AgentWorkspaceRuntime = {
  registry: ReturnType<typeof createAgentToolRegistry>;
  routePlan: ToolRoutePlan;
  toolRuntime: HarnessToolRuntime;
  tools: ReturnType<typeof resolveAgentTools>;
  toolCapsules: ToolCapsule[];
};

export type CreateAgentWorkspaceRuntimeOptions = {
  state: AgentToolRuntimeState;
  activeToolIds?: string[];
  objective?: string;
  requireToolApproval?: boolean;
  role?: AgentRole;
  target?: string;
  onToolRun?: (toolRun: ToolRunSummary) => void;
};

class WorkspaceApprovalGate implements ApprovalGate {
  constructor(private readonly requireToolApproval: boolean) {}

  async requestApproval(request: Parameters<ApprovalGate["requestApproval"]>[0]) {
    const destructiveActions = new Set([
      "sourceCodeModification",
      "dependencyModification",
      "networkAccess",
      "packagePublishing",
    ]);

    if (this.requireToolApproval && destructiveActions.has(request.action.actionClass)) {
      return {
        requestId: request.id,
        status: "denied" as const,
        decidedAt: new Date().toISOString(),
        rationale:
          "Destructive or network actions require explicit human approval in this workspace run.",
      };
    }

    return {
      requestId: request.id,
      status: "approved" as const,
      decidedAt: new Date().toISOString(),
      rationale: "Workspace run auto-approved non-destructive tool execution.",
    };
  }
}

export function createAgentWorkspaceRuntime(
  options: CreateAgentWorkspaceRuntimeOptions,
): AgentWorkspaceRuntime {
  const registry = createAgentToolRegistry(options.state);
  const evidenceStore = new InMemoryEvidenceStore();
  const policy = new DefaultApprovalPolicy({
    toolExecution: "allowed",
    readOnly: "allowed",
    generatedFileWrite: options.requireToolApproval ? "approvalRequired" : "allowed",
    sourceModification: "approvalRequired",
    dependencyModification: "approvalRequired",
    networkAccess: "approvalRequired",
  });
  const approvalGate = new WorkspaceApprovalGate(Boolean(options.requireToolApproval));
  const toolRuntime = new HarnessToolRuntime({
    policy,
    approvalGate,
    evidenceStore,
    onToolRun: (toolRun) => {
      options.onToolRun?.(toolRun);
    },
  });

  const activeToolIds = expandIntentDiscoveryTools(options.activeToolIds, options.role);
  const routePlan = routeTools(resolveAgentTools(options.state, activeToolIds), {
    activeToolIds,
    objective: options.objective,
    role: options.role,
    target: options.target,
  });

  return {
    registry,
    routePlan,
    toolRuntime,
    tools: routePlan.tools,
    toolCapsules: routePlan.capsules,
  };
}

function expandIntentDiscoveryTools(
  activeToolIds: string[] | undefined,
  role: AgentRole | undefined,
) {
  if (!activeToolIds?.length || !needsPackageIntent(role)) {
    return activeToolIds;
  }

  return Array.from(
    new Set([
      "rust.index.package",
      "rust.index.package_overview",
      "workspace.permissions.describe",
      "cc.tools.list",
      "cc.glob",
      "cc.grep",
      "cc.read",
      "cc.task.create",
      "cc.task.list",
      "cc.task.get",
      "cc.task.update",
      "cc.todo.write",
      ...activeToolIds,
    ]),
  );
}

function needsPackageIntent(role: AgentRole | undefined) {
  return role !== "ci";
}
