import type { AgentActionRequest } from "@peregrine/agent-runtime";

export function readOnlyAction(
  reason: string,
  risk: AgentActionRequest["risk"] = "low",
): AgentActionRequest {
  return {
    actionClass: "readOnly",
    reason,
    risk,
  };
}

export function toolExecutionAction(
  reason: string,
  risk: AgentActionRequest["risk"] = "low",
): AgentActionRequest {
  return {
    actionClass: "toolExecution",
    reason,
    risk,
  };
}

export function generatedFileAction(reason: string): AgentActionRequest {
  return {
    actionClass: "generatedFileWrite",
    reason,
    risk: "medium",
  };
}
