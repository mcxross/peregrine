import type {
  DeterministicToolSpec,
  ToolManifest,
  ToolPrerequisite,
  RiskLevel,
} from "@peregrine/agent-runtime";

export function attachDefaultToolManifest(
  tool: DeterministicToolSpec,
): DeterministicToolSpec {
  const category = categoryForTool(tool.id);
  const manifest: ToolManifest = {
    id: tool.id,
    version: tool.version ?? "1",
    chain: tool.id.startsWith("rust.") || tool.id.startsWith("index.") ? "sui" : undefined,
    category,
    description: tool.description,
    whenToUse: whenToUse(tool.id, category),
    whenNotToUse: whenNotToUse(tool.id, category),
    prerequisites: prerequisitesForTool(tool.id),
    inputSchema: tool.inputSchema,
    outputSchema: tool.outputSchema,
    cost: {
      risk: tool.action.risk,
      expectedLatencyMs: expectedLatencyMs(tool.id),
      outputBudgetTokens: outputBudgetTokens(tool.id),
      tokenBudgetHint: outputBudgetTokens(tool.id),
    },
    actionClass: tool.action.actionClass,
    sideEffects: [
      {
        actionClass: tool.action.actionClass,
        description: tool.action.reason,
        requiresApproval:
          tool.action.actionClass !== "readOnly"
          && (tool.action.risk === "medium"
            || tool.action.risk === "high"
            || tool.action.risk === "critical"),
      },
    ],
    timeoutMs: timeoutMs(tool.id),
    reducerId: reducerForTool(tool.id),
  };

  return {
    ...tool,
    manifest,
  };
}

function categoryForTool(toolId: string) {
  if (toolId.includes(".index.") || toolId.startsWith("index.")) return "index";
  if (toolId.includes(".static.")) return "staticAnalysis";
  if (toolId.includes(".graph.")) return "graph";
  if (toolId.includes(".bytecode.")) return "bytecode";
  if (toolId.includes(".dynamic.")) return "dynamicAnalysis";
  if (toolId.includes(".validation.")) return "validation";
  if (toolId.includes(".findings.")) return "finding";
  if (toolId.includes(".patch.")) return "patch";
  if (toolId.includes(".report.")) return "report";
  if (toolId.includes(".invariant.")) return "invariant";
  if (toolId.includes(".test.")) return "validation";
  return "utility";
}

function reducerForTool(toolId: string) {
  if (toolId.includes(".static.")) return "staticAnalysis";
  if (toolId.includes(".graph.")) return "graph";
  if (toolId.includes(".bytecode.")) return "bytecode";
  if (toolId.includes(".fuzz")) return "fuzz";
  if (toolId.includes("assert_property") || toolId.includes("formal")) return "prover";
  if (toolId.includes(".dynamic.") || toolId.includes(".validation.")) return "command";
  return "generic";
}

function prerequisitesForTool(toolId: string): ToolPrerequisite[] {
  const prerequisites: ToolPrerequisite[] = [];

  if (
    toolId.startsWith("index.context.")
    || toolId.includes(".index.read_symbols")
    || toolId.includes(".index.package_overview")
    || toolId.includes(".static.inspect_function")
    || toolId.includes(".graph.call_graph")
    || toolId.includes(".graph.finding_impact")
    || toolId.includes(".graph.path_query")
    || toolId.includes(".dynamic.trace_execution")
    || toolId.includes(".invariant.")
  ) {
    prerequisites.push({
      toolId: "rust.index.package",
      reason: "Indexed SQLite context must exist before index-backed lookup.",
      required: true,
    });
  }

  if (toolId.includes(".findings.triage")) {
    prerequisites.push({
      toolId: "rust.static.scan_package",
      reason: "Static scan produces the first deterministic finding set.",
      required: false,
    });
  }

  if (toolId.includes(".bytecode.")) {
    prerequisites.push({
      toolId: "rust.validation.run_suite",
      reason: "Bytecode inspection requires a recent successful package build.",
      required: false,
    });
  }

  if (toolId.includes(".patch.") || toolId.includes(".test.generate")) {
    prerequisites.push({
      toolId: "rust.findings.emit",
      reason: "Patch and regression test drafts should be tied to a concrete finding.",
      required: false,
    });
  }

  return prerequisites;
}

function whenToUse(toolId: string, category: string) {
  if (toolId === "rust.index.package") {
    return [
      "Use first to build the package index before intent discovery or code-specific security claims.",
    ];
  }
  if (toolId === "rust.index.package_overview") {
    return [
      "Use immediately after indexing to establish package intent, module shape, and analysis priorities.",
    ];
  }
  if (category === "index") {
    return ["Use to discover symbols or load bounded context before making code-specific claims."];
  }
  if (category === "staticAnalysis") {
    return ["Use for deterministic source-level findings, diagnostics, and precise locations."];
  }
  if (category === "graph") {
    return ["Use to reason about call paths, object lifecycle, capability flow, or impact radius."];
  }
  if (category === "bytecode") {
    return ["Use when source-level evidence is incomplete or compiled behavior must be checked."];
  }
  if (category === "dynamicAnalysis" || category === "validation") {
    return ["Use to validate high-impact claims with tests, fuzzing, or formal checks."];
  }
  if (category === "finding") {
    return ["Use to record, triage, or attach evidence to structured findings."];
  }
  if (category === "patch") {
    return ["Use after a finding has evidence and needs a concrete mitigation plan."];
  }
  if (category === "report") {
    return ["Use after evidence has been collected and findings have been triaged."];
  }

  return [`Use when the task specifically requires ${toolId}.`];
}

function whenNotToUse(toolId: string, category: string) {
  const generic = [
    "Do not call when fresh equivalent evidence is already present in the context packet.",
    "Do not call just to fill time; call only when it resolves missing evidence or uncertainty.",
  ];

  if (toolId.includes(".fuzz")) {
    return [
      ...generic,
      "Do not treat a no-crash fuzz result as proof of safety.",
      "Do not call without enough budget for the target risk.",
    ];
  }

  if (category === "bytecode") {
    return [
      ...generic,
      "Do not call before the package has been built or when source-level evidence is sufficient.",
    ];
  }

  if (category === "patch") {
    return [
      ...generic,
      "Do not propose patches for hypotheses that lack deterministic evidence or a validation plan.",
    ];
  }

  return generic;
}

function expectedLatencyMs(toolId: string) {
  if (toolId.includes(".fuzz")) return 30_000;
  if (toolId.includes("assert_property")) return 45_000;
  if (toolId.includes(".validation.run_suite")) return 60_000;
  if (toolId.includes(".bytecode.")) return 4_000;
  if (toolId.includes(".index.package")) return 8_000;
  if (toolId.includes(".static.scan")) return 3_000;
  return 1_000;
}

function outputBudgetTokens(toolId: string) {
  if (toolId.includes(".bytecode.")) return 900;
  if (toolId.includes(".graph.")) return 800;
  if (toolId.includes(".fuzz") || toolId.includes("assert_property")) return 700;
  if (toolId.includes(".static.")) return 700;
  return 500;
}

function timeoutMs(toolId: string) {
  const expected = expectedLatencyMs(toolId);
  return Math.max(10_000, expected * timeoutMultiplier(toolId));
}

function timeoutMultiplier(toolId: string): number {
  if (toolId.includes(".fuzz") || toolId.includes("assert_property")) return 2;
  if (toolId.includes(".validation.run_suite")) return 2;
  return 3;
}

export function routeRisk(tool: DeterministicToolSpec): RiskLevel {
  return tool.manifest?.cost.risk ?? tool.action.risk;
}
