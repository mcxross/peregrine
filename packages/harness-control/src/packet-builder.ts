import type { AgentContextPacket } from "@peregrine/agent-runtime";
import type { ContextPacketInput } from "./types";

export function buildAgentContextPacket(
  input: ContextPacketInput,
): AgentContextPacket {
  return {
    task: input.task,
    developerIntent: input.developerIntent,
    projectSummary: input.projectSummary,
    securityProfile: input.session.profile.id,
    selectedCode: input.selectedCode ?? [],
    riskSignals: input.riskSignals ?? [],
    relevantGuides: input.guides ?? [],
    currentFindings: input.currentFindings ?? input.session.findings,
    recentToolResults: input.recentToolResults ?? input.session.toolRuns.slice(-8),
    toolCapsules: input.toolCapsules,
    allowedActions: input.allowedActions ?? defaultAllowedActions(),
    approvalPolicy: {
      mode: "localAi",
      networkAccess: "approvalRequired",
      sourceModification: "approvalRequired",
      dependencyModification: "approvalRequired",
      secretAccess: "forbidden",
    },
    outputContract: input.outputContract,
    tokenBudget: input.tokenBudget,
  };
}

export function defaultAllowedActions(): AgentContextPacket["allowedActions"] {
  return [
    {
      actionClass: "readOnly",
      description: "Read compact project context and evidence provided by the harness.",
      requiresApproval: false,
    },
    {
      actionClass: "toolExecution",
      description: "Call registered deterministic Peregrine tools through the harness.",
      requiresApproval: false,
    },
    {
      actionClass: "generatedFileWrite",
      description: "Generate draft artifacts or test plans with preview.",
      requiresApproval: true,
    },
    {
      actionClass: "sourceCodeModification",
      description: "Modify project source files.",
      requiresApproval: true,
    },
    {
      actionClass: "dependencyModification",
      description: "Modify manifests, lockfiles, or dependencies.",
      requiresApproval: true,
    },
    {
      actionClass: "networkAccess",
      description: "Access external networks or transmit project metadata.",
      requiresApproval: true,
    },
  ];
}
