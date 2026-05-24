import type {
  AgentRole,
  DeterministicToolSpec,
  ToolCapsule,
  ToolManifest,
} from "@peregrine/agent-runtime";
import { createAiSdkToolName } from "@peregrine/agent-runtime";

export type ToolRouteStage = 1 | 2 | 3 | 4;

export interface ToolRouteRequest {
  role?: AgentRole;
  objective?: string;
  maxStage?: ToolRouteStage;
  activeToolIds?: string[];
  completedToolIds?: string[];
  evidenceToolIds?: string[];
  target?: string;
  tokenBudget?: number;
}

export interface ToolRouteDecision {
  toolId: string;
  selected: boolean;
  reason: string;
}

export interface ToolRoutePlan {
  tools: DeterministicToolSpec[];
  capsules: ToolCapsule[];
  decisions: ToolRouteDecision[];
}

const ROLE_STAGE: Partial<Record<AgentRole, ToolRouteStage>> = {
  securityReview: 2,
  testGeneration: 3,
  fuzzCampaign: 3,
  formalSpec: 3,
  patch: 4,
  report: 4,
  explainer: 2,
  triage: 4,
  ci: 3,
};

export function routeTools(
  tools: DeterministicToolSpec[],
  request: ToolRouteRequest = {},
): ToolRoutePlan {
  const allowed = request.activeToolIds?.length
    ? new Set(request.activeToolIds)
    : undefined;
  const completed = new Set(request.completedToolIds ?? []);
  const evidence = new Set(request.evidenceToolIds ?? []);
  const maxStage = request.maxStage ?? stageForRole(request.role);
  const decisions: ToolRouteDecision[] = [];
  const selected: DeterministicToolSpec[] = [];

  for (const tool of tools) {
    const manifest = requireManifest(tool);

    if (allowed && !allowed.has(tool.id)) {
      decisions.push(decision(tool.id, false, "Tool is not enabled for this agent."));
      continue;
    }

    const stage = stageForManifest(manifest);
    if (stage > maxStage) {
      decisions.push(
        decision(tool.id, false, `Tool is stage ${stage}; current route allows stage ${maxStage}.`),
      );
      continue;
    }

    const missingPrerequisite = manifest.prerequisites.find(
      (prerequisite) =>
        prerequisite.required
        && !completed.has(prerequisite.toolId)
        && !selected.some((candidate) => candidate.id === prerequisite.toolId)
        && !tools.some((candidate) => candidate.id === prerequisite.toolId),
    );
    if (missingPrerequisite) {
      decisions.push(
        decision(
          tool.id,
          false,
          `Missing prerequisite ${missingPrerequisite.toolId}: ${missingPrerequisite.reason}`,
        ),
      );
      continue;
    }

    if (evidence.has(tool.id) && tool.action.actionClass !== "readOnly") {
      decisions.push(
        decision(tool.id, false, "Fresh equivalent evidence already exists for this tool."),
      );
      continue;
    }

    if (
      request.tokenBudget
      && manifest.cost.outputBudgetTokens
      && manifest.cost.outputBudgetTokens > request.tokenBudget
    ) {
      decisions.push(
        decision(tool.id, false, "Tool output budget exceeds the current context budget."),
      );
      continue;
    }

    if (requiresConcreteTarget(tool) && !request.target) {
      decisions.push(decision(tool.id, false, "Tool requires a concrete function or module target."));
      continue;
    }

    selected.push(tool);
    decisions.push(decision(tool.id, true, `Selected stage ${stage} ${manifest.category} tool.`));
  }

  return {
    tools: selected,
    capsules: selected.map(toolCapsule),
    decisions,
  };
}

export function toolCapsule(tool: DeterministicToolSpec): ToolCapsule {
  const manifest = requireManifest(tool);

  return {
    callableName: createAiSdkToolName(tool.id),
    id: tool.id,
    title: tool.title,
    description: manifest.description || tool.description,
    category: manifest.category,
    actionClass: manifest.actionClass,
    risk: manifest.cost.risk,
    whenToUse: manifest.whenToUse,
    whenNotToUse: manifest.whenNotToUse,
    prerequisites: manifest.prerequisites,
    inputSchema: manifest.inputSchema,
    outputBudgetTokens: manifest.cost.outputBudgetTokens,
  };
}

export function requireManifest(tool: DeterministicToolSpec): ToolManifest {
  return (
    tool.manifest ?? {
      id: tool.id,
      version: tool.version ?? "1",
      category: inferCategory(tool.id),
      description: tool.description,
      whenToUse: [tool.description],
      whenNotToUse: ["Do not call when fresh equivalent evidence is already available."],
      prerequisites: [],
      inputSchema: tool.inputSchema,
      outputSchema: tool.outputSchema,
      cost: {
        risk: tool.action.risk,
        outputBudgetTokens: 700,
      },
      actionClass: tool.action.actionClass,
      sideEffects: [
        {
          actionClass: tool.action.actionClass,
          description: tool.action.reason,
          requiresApproval: tool.action.risk === "high" || tool.action.risk === "critical",
        },
      ],
      reducerId: inferReducerId(tool.id),
    }
  );
}

function stageForRole(role?: AgentRole): ToolRouteStage {
  return (role && ROLE_STAGE[role]) || 2;
}

function stageForManifest(manifest: ToolManifest): ToolRouteStage {
  switch (manifest.category) {
    case "index":
    case "knowledge":
    case "staticAnalysis":
      return 1;
    case "context":
    case "graph":
    case "audit":
      return 2;
    case "bytecode":
    case "dynamicAnalysis":
    case "validation":
    case "invariant":
      return 3;
    case "finding":
    case "patch":
    case "report":
      return 4;
    default:
      return 2;
  }
}

function inferCategory(toolId: string) {
  if (toolId.includes(".index.") || toolId.startsWith("index.")) return "index";
  if (toolId.includes(".knowledge.")) return "knowledge";
  if (toolId.includes(".static.")) return "staticAnalysis";
  if (toolId.includes(".graph.")) return "graph";
  if (toolId.includes(".bytecode.")) return "bytecode";
  if (toolId.includes(".audit.")) return "audit";
  if (toolId.includes(".dynamic.")) return "dynamicAnalysis";
  if (toolId.includes(".validation.")) return "validation";
  if (toolId.includes(".findings.")) return "finding";
  if (toolId.includes(".patch.")) return "patch";
  if (toolId.includes(".report.")) return "report";
  if (toolId.includes(".invariant.")) return "invariant";
  if (toolId.includes(".test.")) return "validation";
  return "utility";
}

function inferReducerId(toolId: string) {
  if (toolId.includes(".static.")) return "staticAnalysis";
  if (toolId.includes(".graph.")) return "graph";
  if (toolId.includes(".bytecode.")) return "bytecode";
  if (toolId.includes(".audit.")) return "audit";
  if (toolId.includes(".fuzz")) return "fuzz";
  if (toolId.includes("assert_property") || toolId.includes("formal")) return "prover";
  if (toolId.includes(".dynamic.") || toolId.includes(".validation.")) return "command";
  return "generic";
}

function requiresConcreteTarget(tool: DeterministicToolSpec) {
  const schema = tool.inputSchema as { required?: unknown };
  const required = Array.isArray(schema.required) ? schema.required : [];
  return required.includes("functionId") || required.includes("moduleName");
}

function decision(
  toolId: string,
  selected: boolean,
  reason: string,
): ToolRouteDecision {
  return { toolId, selected, reason };
}
