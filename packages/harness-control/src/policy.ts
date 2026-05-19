import type {
  ActionClass,
  AgentActionRequest,
  ApprovalPolicySnapshot,
  RiskLevel,
} from "@peregrine/agent-runtime";

import type {
  ApprovalEvaluation,
  ApprovalPolicy,
  PolicyDisposition,
} from "./types";

export interface DefaultApprovalPolicyConfig {
  aiMode?: ApprovalPolicySnapshot["mode"];
  readOnly?: PolicyDisposition;
  toolExecution?: PolicyDisposition;
  generatedFileWrite?: PolicyDisposition;
  sourceModification?: PolicyDisposition;
  dependencyModification?: PolicyDisposition;
  networkAccess?: PolicyDisposition;
  packagePublishing?: Extract<PolicyDisposition, "approvalRequired" | "forbidden">;
}

const DEFAULT_CONFIG: Required<DefaultApprovalPolicyConfig> = {
  aiMode: "localAi",
  readOnly: "allowed",
  toolExecution: "allowed",
  generatedFileWrite: "approvalRequired",
  sourceModification: "approvalRequired",
  dependencyModification: "approvalRequired",
  networkAccess: "approvalRequired",
  packagePublishing: "forbidden",
};

export class DefaultApprovalPolicy implements ApprovalPolicy {
  private readonly config: Required<DefaultApprovalPolicyConfig>;

  constructor(config: DefaultApprovalPolicyConfig = {}) {
    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
    };
  }

  evaluateAction(action: AgentActionRequest): ApprovalEvaluation {
    if (action.actionClass === "secretAccess") {
      return forbid(action, "Secret access is forbidden by default.");
    }

    const disposition = dispositionFor(action.actionClass, this.config);

    if (disposition === "forbidden") {
      return {
        disposition,
        reason: `${action.actionClass} is forbidden by policy.`,
        risk: elevateRisk(action.risk, "high"),
      };
    }

    if (disposition === "approvalRequired") {
      return {
        disposition,
        reason: `${action.actionClass} requires explicit approval.`,
        risk: action.risk,
      };
    }

    return {
      disposition,
      reason: `${action.actionClass} is allowed by policy.`,
      risk: action.risk,
    };
  }

  snapshot(): ApprovalPolicySnapshot {
    return {
      mode: this.config.aiMode,
      networkAccess: toSnapshotDisposition(this.config.networkAccess),
      sourceModification: toSnapshotDisposition(this.config.sourceModification),
      dependencyModification: toSnapshotDisposition(
        this.config.dependencyModification,
      ),
      secretAccess: "forbidden",
    };
  }
}

function dispositionFor(
  actionClass: ActionClass,
  config: Required<DefaultApprovalPolicyConfig>,
): PolicyDisposition {
  switch (actionClass) {
    case "readOnly":
      return config.readOnly;
    case "toolExecution":
      return config.toolExecution;
    case "generatedFileWrite":
      return config.generatedFileWrite;
    case "sourceCodeModification":
      return config.sourceModification;
    case "dependencyModification":
      return config.dependencyModification;
    case "packagePublishing":
      return config.packagePublishing;
    case "networkAccess":
      return config.networkAccess;
    case "secretAccess":
      return "forbidden";
  }
}

function forbid(
  action: AgentActionRequest,
  reason: string,
): ApprovalEvaluation {
  return {
    disposition: "forbidden",
    reason,
    risk: elevateRisk(action.risk, "critical"),
  };
}

function toSnapshotDisposition(
  disposition: PolicyDisposition,
): "forbidden" | "approvalRequired" | "allowed" {
  return disposition;
}

function elevateRisk(current: RiskLevel, minimum: RiskLevel): RiskLevel {
  const order: RiskLevel[] = ["low", "medium", "high", "critical"];

  return order.indexOf(current) >= order.indexOf(minimum) ? current : minimum;
}

