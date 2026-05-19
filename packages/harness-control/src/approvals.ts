import type { ApprovalDecision, ApprovalGate, ApprovalRequest } from "./types";

export class DenyByDefaultApprovalGate implements ApprovalGate {
  async requestApproval(request: ApprovalRequest): Promise<ApprovalDecision> {
    return {
      requestId: request.id,
      status: "denied",
      decidedAt: new Date().toISOString(),
      rationale:
        "No approval gate is connected. Peregrine denies approval-required actions by default.",
    };
  }
}

export class StaticApprovalGate implements ApprovalGate {
  constructor(private readonly status: ApprovalDecision["status"]) {}

  async requestApproval(request: ApprovalRequest): Promise<ApprovalDecision> {
    return {
      requestId: request.id,
      status: this.status,
      decidedAt: new Date().toISOString(),
      rationale: `Static ${this.status} approval gate.`,
    };
  }
}

