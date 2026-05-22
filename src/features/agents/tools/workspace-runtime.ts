import type { ToolRunSummary } from "@peregrine/agent-runtime";
import {
  DefaultApprovalPolicy,
  HarnessToolRuntime,
  InMemoryEvidenceStore,
} from "@peregrine/harness-control";
import type { ApprovalGate } from "@peregrine/harness-control";

import { createAgentToolRegistry, resolveAgentTools } from "@/features/agents/tools/registry";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";

export type AgentWorkspaceRuntime = {
  registry: ReturnType<typeof createAgentToolRegistry>;
  toolRuntime: HarnessToolRuntime;
  tools: ReturnType<typeof resolveAgentTools>;
};

export type CreateAgentWorkspaceRuntimeOptions = {
  state: AgentToolRuntimeState;
  activeToolIds?: string[];
  requireToolApproval?: boolean;
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

  return {
    registry,
    toolRuntime,
    tools: resolveAgentTools(options.state, options.activeToolIds),
  };
}
