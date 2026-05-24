import type {
  AgentDiagnostic,
  DeterministicToolSpec,
  EvidenceConfidence,
  FindingCandidate,
  SecurityEvidenceItem,
  ToolRunStatus,
} from "@peregrine/agent-runtime";

import { createId } from "./ids";

export interface ToolEvidenceCompileRequest {
  tool: DeterministicToolSpec;
  output: unknown;
  status: ToolRunStatus;
  summary: string;
  toolRunId: string;
}

export interface ToolEvidenceCompileResult {
  summary: string;
  modelOutput: {
    summary: string;
    evidence: SecurityEvidenceItem[];
    findingCandidates: FindingCandidate[];
    compact: unknown;
  };
  evidence: SecurityEvidenceItem[];
  findingCandidates: FindingCandidate[];
  diagnostics: AgentDiagnostic[];
}

type JsonRecord = Record<string, unknown>;

export function compileToolEvidence(
  request: ToolEvidenceCompileRequest,
): ToolEvidenceCompileResult {
  const reducerId = request.tool.manifest?.reducerId ?? inferReducerId(request.tool.id);

  switch (reducerId) {
    case "staticAnalysis":
      return reduceStaticAnalysis(request);
    case "graph":
      return reduceGraph(request);
    case "bytecode":
      return reduceBytecode(request);
    case "fuzz":
      return reduceFuzz(request);
    case "prover":
      return reduceProver(request);
    case "command":
      return reduceCommand(request);
    default:
      return reduceGeneric(request);
  }
}

function reduceStaticAnalysis(
  request: ToolEvidenceCompileRequest,
): ToolEvidenceCompileResult {
  const report = unwrapRecord(request.output, "report") ?? asRecord(request.output);
  const findings = asArray(report?.findings);
  const diagnostics = asArray(report?.diagnostics);
  const severityCounts = countBy(findings, (finding) =>
    stringValue(asRecord(finding)?.severity) ?? "unknown",
  );
  const ruleCounts = countBy(findings, (finding) =>
    stringValue(asRecord(finding)?.ruleId ?? asRecord(finding)?.rule_id) ?? "unknown",
  );
  const topFindings = findings.slice(0, 12).map(compactStaticFinding);
  const evidence = topFindings.map((finding) =>
    evidenceItem(request, {
      kind: "staticFinding",
      claim: finding.ruleId
        ? `Static rule ${finding.ruleId} reported a security-relevant signal.`
        : "Static analysis reported a security-relevant signal.",
      observation: finding.message ?? request.summary,
      confidence: finding.file ? "high" : "medium",
      sourcePrecision: finding.file ? "compiler" : "heuristic",
      location: finding.file
        ? {
            file: finding.file,
            startLine: finding.span?.startLine,
            endLine: finding.span?.endLine,
          }
        : undefined,
      symbolRefs: finding.ruleId ? [finding.ruleId] : [],
      followUp: "Inspect the affected function and validate exploitability before patching.",
      metadata: finding,
    }),
  );
  const findingCandidates = evidence.map((item, index) => {
    const finding = topFindings[index];

    return findingCandidate({
      title: finding.ruleId ?? "Static analysis finding",
      category: finding.rulesetId ?? "staticAnalysis",
      severity: mapAnalysisSeverity(finding.severity),
      confidence: item.confidence,
      status: "likely",
      evidenceRefs: [item.id],
      affectedSymbols: finding.ruleId ? [finding.ruleId] : [],
      validationCommands: ["sui move test", "peregrine analyze"],
      expectedEvidence: ["Static rule no longer reports the issue after the patch."],
      metadata: finding,
    });
  });
  const compact = {
    reducer: "staticAnalysis",
    findingCount: findings.length,
    diagnosticCount: diagnostics.length,
    severityCounts,
    ruleCounts,
    topFindings,
    diagnostics: diagnostics.slice(0, 8),
  };

  return compiled(
    `Static analysis produced ${findings.length} findings and ${diagnostics.length} diagnostics.`,
    compact,
    evidence,
    findingCandidates,
  );
}

function reduceGraph(request: ToolEvidenceCompileRequest): ToolEvidenceCompileResult {
  const value = asRecord(request.output);
  const graph = graphLike(value);
  const lifecycleMaps = asArray(value?.objectLifecycleMaps);
  const ownershipFindings = asArray(value?.objectOwnershipFindings);
  const capabilityFindings = asArray(value?.capabilityFindings);
  const risks = lifecycleMaps.flatMap((map) =>
    asArray(asRecord(map)?.risks).map((risk) => ({
      map: asRecord(map),
      risk: asRecord(risk),
    })),
  );
  const compact = {
    reducer: "graph",
    nodeCount: graph.nodes.length,
    edgeCount: graph.edges.length,
    unresolvedCount: graph.unresolved.length,
    confidenceCounts: countConfidence(value),
    lifecycleMapCount: lifecycleMaps.length,
    ownershipFindingCount: ownershipFindings.length,
    capabilityFindingCount: capabilityFindings.length,
    topRisks: risks.slice(0, 10).map(({ map, risk }) => ({
      object: stringValue(map?.qualifiedName ?? map?.typeName),
      kind: stringValue(risk?.kind),
      severity: stringValue(risk?.severity),
      message: stringValue(risk?.message),
      evidence: asArray(risk?.evidence).slice(0, 4),
    })),
    trimmed: booleanValue(value?.trimmed),
    trimReasons: asArray(value?.trimReasons),
  };
  const evidence = [
    evidenceItem(request, {
      kind: "graphSignal",
      claim: "Graph analysis produced structural security evidence.",
      observation:
        graph.nodes.length || graph.edges.length
          ? `Graph contains ${graph.nodes.length} nodes and ${graph.edges.length} edges.`
          : request.summary,
      confidence: graph.unresolved.length ? "medium" : "high",
      sourcePrecision: "heuristic",
      symbolRefs: [],
      followUp: graph.unresolved.length
        ? "Review unresolved graph edges before treating the result as complete."
        : undefined,
      metadata: compact,
    }),
  ];
  const findingCandidates = risks.slice(0, 10).map(({ map, risk }) =>
    findingCandidate({
      title: stringValue(risk?.kind) ?? "Object lifecycle risk",
      category: "objectLifecycle",
      severity: mapSecuritySeverity(stringValue(risk?.severity)),
      confidence: "medium",
      status: "likely",
      evidenceRefs: [evidence[0].id],
      affectedSymbols: [stringValue(map?.qualifiedName) ?? stringValue(map?.typeName) ?? ""].filter(
        Boolean,
      ),
      validationCommands: ["sui move test", "peregrine call-graph"],
      expectedEvidence: ["Graph path is guarded, removed, or covered by a regression test."],
      metadata: { map, risk },
    }),
  );

  return compiled(
    `Graph evidence contains ${graph.nodes.length} nodes, ${graph.edges.length} edges, and ${risks.length} risk signals.`,
    compact,
    evidence,
    findingCandidates,
  );
}

function reduceBytecode(request: ToolEvidenceCompileRequest): ToolEvidenceCompileResult {
  const value = asRecord(request.output);
  const modules = asArray(value?.modules);
  const functions = modules.flatMap((module) => asArray(asRecord(module)?.functions));
  const instructionCount = functions.reduce<number>(
    (total, fn) => total + numberValue(asRecord(fn)?.instructionCount),
    0,
  );
  const sensitiveInstructions = functions.flatMap((fn) =>
    asArray(asRecord(fn)?.instructions)
      .filter((instruction) => sensitiveOpcode(stringValue(asRecord(instruction)?.opcode)))
      .slice(0, 12)
      .map((instruction) => ({
        functionName: stringValue(asRecord(fn)?.name ?? asRecord(fn)?.functionName),
        opcode: stringValue(asRecord(instruction)?.opcode),
        detail: stringValue(asRecord(instruction)?.detail),
        source: asRecord(instruction)?.source,
      })),
  );
  const sourceMappedCount = functions.reduce<number>(
    (total, fn) =>
      total
      + asArray(asRecord(fn)?.instructions).filter((instruction) =>
        Boolean(asRecord(instruction)?.source),
      ).length,
    0,
  );
  const compact = {
    reducer: "bytecode",
    moduleCount: numberValue(value?.package && asRecord(value.package)?.moduleCount) || modules.length,
    functionCount:
      numberValue(value?.package && asRecord(value.package)?.functionCount) || functions.length,
    instructionCount,
    sourceMappedInstructionCount: sourceMappedCount,
    sensitiveInstructions: sensitiveInstructions.slice(0, 20),
  };
  const evidence = [
    evidenceItem(request, {
      kind: "bytecodeSignal",
      claim: "Bytecode analysis produced compiled-code evidence.",
      observation:
        sensitiveInstructions.length > 0
          ? `Found ${sensitiveInstructions.length} sensitive bytecode instructions.`
          : `Reviewed ${instructionCount} bytecode instructions without high-signal opcode extraction.`,
      confidence: sourceMappedCount > 0 ? "high" : "medium",
      sourcePrecision: sourceMappedCount > 0 ? "sourceMap" : "bytecode",
      symbolRefs: sensitiveInstructions
        .map((instruction) => instruction.functionName)
        .filter((name): name is string => Boolean(name)),
      followUp: "Compare bytecode signals against source-level expectations before reporting.",
      metadata: compact,
    }),
  ];

  return compiled(
    `Bytecode evidence covers ${functions.length} functions and ${instructionCount} instructions.`,
    compact,
    evidence,
    [],
  );
}

function reduceFuzz(request: ToolEvidenceCompileRequest): ToolEvidenceCompileResult {
  const value = asRecord(request.output);
  const stdout = stringValue(value?.stdout) ?? "";
  const manifest = asRecord(value?.manifest ?? parseManifestFromStdout(stdout));
  const crashCount =
    numberValue(manifest?.crashEntries)
    || parseCount(stdout, /Crash entries:\s*(\d+)/i)
    || 0;
  const targetCount =
    numberValue(manifest?.publicFunctionCount)
    || asArray(manifest?.targetFunctions).length;
  const compact = {
    reducer: "fuzz",
    status: request.status,
    seed: numberValue(manifest?.seed),
    timeLimitSeconds: numberValue(manifest?.timeLimitSeconds),
    targetCount,
    crashCount,
    queueEntries: numberValue(manifest?.queueEntries),
    targetFunctions: asArray(manifest?.targetFunctions).slice(0, 24),
    stdoutExcerpt: excerpt(stdout, 1_000),
  };
  const evidence = [
    evidenceItem(request, {
      kind: crashCount > 0 ? "fuzzCounterexample" : "toolOutput",
      claim:
        crashCount > 0
          ? "Fuzzing found at least one crash or counterexample."
          : "No fuzz crash was observed during the configured campaign.",
      observation:
        crashCount > 0
          ? `Movy reported ${crashCount} crash entries.`
          : `Movy reported no crash entries across ${targetCount} public targets for the configured budget.`,
      confidence: crashCount > 0 ? "confirmed" : "low",
      sourcePrecision: "heuristic",
      symbolRefs: asArray(manifest?.targetFunctions)
        .map((target) => stringValue(target))
        .filter((target): target is string => Boolean(target))
        .slice(0, 24),
      followUp:
        crashCount > 0
          ? "Minimize the crashing input and add a regression test."
          : "Do not treat a no-crash fuzz run as proof of safety; increase budget or add targeted invariants for critical paths.",
      metadata: compact,
    }),
  ];
  const findingCandidates =
    crashCount > 0
      ? [
          findingCandidate({
            title: "Fuzz counterexample",
            category: "dynamicAnalysis",
            severity: "high",
            confidence: "confirmed",
            status: "confirmed",
            evidenceRefs: [evidence[0].id],
            affectedSymbols: evidence[0].symbolRefs,
            validationCommands: ["sui move test", "peregrine fuzz --seed <failing-seed>"],
            expectedEvidence: ["Crash is reproducible before the patch and absent after the patch."],
            metadata: compact,
          }),
        ]
      : [];

  return compiled(
    crashCount > 0
      ? `Fuzzing found ${crashCount} crash entries.`
      : "Fuzzing completed without observed crashes; this is not a safety proof.",
    compact,
    evidence,
    findingCandidates,
  );
}

function reduceProver(request: ToolEvidenceCompileRequest): ToolEvidenceCompileResult {
  const value = asRecord(request.output);
  const failed = commandFailed(value, request.status);
  const stdout = stringValue(value?.stdout) ?? "";
  const stderr = stringValue(value?.stderr) ?? "";
  const compact = {
    reducer: "prover",
    status: failed ? "failed" : "provedOrCompleted",
    exitStatus: value?.status,
    stdoutExcerpt: excerpt(stdout, 1_000),
    stderrExcerpt: excerpt(stderr, 1_000),
  };
  const evidence = [
    evidenceItem(request, {
      kind: "proverResult",
      claim: failed
        ? "Formal verification failed or timed out."
        : "Formal verification completed for the selected target.",
      observation: failed
        ? firstNonEmptyLine(stderr, stdout) ?? request.summary
        : "The prover command completed successfully for the configured target.",
      confidence: failed ? "confirmed" : "high",
      sourcePrecision: "compiler",
      symbolRefs: [],
      followUp: failed
        ? "Inspect the failed obligation or counterexample trace and decide whether the spec or implementation is wrong."
        : "Only claim properties that were actually encoded in the prover target.",
      metadata: compact,
    }),
  ];
  const findingCandidates = failed
    ? [
        findingCandidate({
          title: "Formal verification failure",
          category: "formalVerification",
          severity: "medium",
          confidence: "confirmed",
          status: "confirmed",
          evidenceRefs: [evidence[0].id],
          affectedSymbols: [],
          validationCommands: ["peregrine verify"],
          expectedEvidence: ["The same property verifies after the patch or spec correction."],
          metadata: compact,
        }),
      ]
    : [];

  return compiled(request.summary, compact, evidence, findingCandidates);
}

function reduceCommand(request: ToolEvidenceCompileRequest): ToolEvidenceCompileResult {
  const value = asRecord(request.output);
  const failed = commandFailed(value, request.status);
  const label = request.tool.id.includes("test") ? "test" : "command";
  const compact = {
    reducer: "command",
    status: failed ? "failed" : "passed",
    exitStatus: value?.status,
    stdoutExcerpt: excerpt(stringValue(value?.stdout) ?? "", 1_000),
    stderrExcerpt: excerpt(stringValue(value?.stderr) ?? "", 1_000),
  };
  const evidence = [
    evidenceItem(request, {
      kind: request.tool.id.includes("test") ? "testResult" : "toolOutput",
      claim: failed ? `The ${label} failed.` : `The ${label} completed successfully.`,
      observation: firstNonEmptyLine(stringValue(value?.stderr), stringValue(value?.stdout))
        ?? request.summary,
      confidence: "confirmed",
      sourcePrecision: "compiler",
      symbolRefs: [],
      followUp: failed ? "Fix or explain the failing command before release." : undefined,
      metadata: compact,
    }),
  ];

  return compiled(request.summary, compact, evidence, []);
}

function reduceGeneric(request: ToolEvidenceCompileRequest): ToolEvidenceCompileResult {
  const compact = {
    reducer: "generic",
    output: boundedJson(request.output, request.tool.manifest?.cost.outputBudgetTokens ?? 600),
  };
  const evidence = [
    evidenceItem(request, {
      kind: request.status === "failed" ? "toolFailure" : "toolOutput",
      claim: `Tool ${request.tool.id} ${request.status}.`,
      observation: request.summary,
      confidence: request.status === "succeeded" ? "medium" : "confirmed",
      sourcePrecision: "summary",
      symbolRefs: [],
      metadata: compact,
    }),
  ];

  return compiled(request.summary, compact, evidence, []);
}

function compiled(
  summary: string,
  compact: unknown,
  evidence: SecurityEvidenceItem[],
  findingCandidates: FindingCandidate[],
  diagnostics: AgentDiagnostic[] = [],
): ToolEvidenceCompileResult {
  return {
    summary,
    modelOutput: {
      summary,
      evidence,
      findingCandidates,
      compact,
    },
    evidence,
    findingCandidates,
    diagnostics,
  };
}

function evidenceItem(
  request: ToolEvidenceCompileRequest,
  item: Omit<SecurityEvidenceItem, "id" | "toolRunId">,
): SecurityEvidenceItem {
  return {
    ...item,
    id: createId("evidence_item"),
    toolRunId: request.toolRunId,
  };
}

function findingCandidate(input: {
  title: string;
  category: string;
  severity: FindingCandidate["severity"];
  confidence: EvidenceConfidence;
  status: FindingCandidate["status"];
  evidenceRefs: string[];
  affectedSymbols: string[];
  validationCommands: string[];
  expectedEvidence: string[];
  metadata?: JsonRecord;
}): FindingCandidate {
  return {
    id: createId("finding"),
    title: input.title,
    category: input.category,
    severity: input.severity,
    confidence: input.confidence,
    status: input.status,
    affectedSymbols: input.affectedSymbols,
    evidenceRefs: input.evidenceRefs,
    validationPlan: {
      commands: input.validationCommands,
      expectedEvidence: input.expectedEvidence,
      required: input.status !== "hypothesis",
    },
    metadata: input.metadata,
  };
}

function inferReducerId(toolId: string) {
  if (toolId.includes(".static.")) return "staticAnalysis";
  if (toolId.includes(".graph.")) return "graph";
  if (toolId.includes(".bytecode.")) return "bytecode";
  if (toolId.includes(".fuzz")) return "fuzz";
  if (toolId.includes("assert_property") || toolId.includes("formal")) return "prover";
  if (toolId.includes(".dynamic.run_test") || toolId.includes(".validation.")) return "command";

  return "generic";
}

function compactStaticFinding(value: unknown) {
  const finding = asRecord(value);
  const span = asRecord(finding?.span);

  return {
    ruleId: stringValue(finding?.ruleId ?? finding?.rule_id),
    rulesetId: stringValue(finding?.rulesetId ?? finding?.ruleset_id),
    severity: stringValue(finding?.severity),
    message: stringValue(finding?.message),
    file: stringValue(finding?.file),
    span: span
      ? {
          startLine: numberValue(span.startLine ?? span.start_line) || undefined,
          endLine: numberValue(span.endLine ?? span.end_line) || undefined,
        }
      : undefined,
  };
}

function graphLike(value: JsonRecord | undefined) {
  const nodes = asArray(value?.nodes);
  const edges = asArray(value?.edges);
  const unresolved = [
    ...asArray(value?.unresolvedCalls),
    ...asArray(value?.unresolvedTypes),
    ...asArray(value?.unresolvedAccesses),
  ];

  return { nodes, edges, unresolved };
}

function countConfidence(value: unknown) {
  const counts: Record<string, number> = {};
  walk(value, (node) => {
    const confidence = stringValue(asRecord(node)?.confidence);
    if (confidence) {
      counts[confidence] = (counts[confidence] ?? 0) + 1;
    }
  });
  return counts;
}

function walk(value: unknown, visit: (value: unknown) => void) {
  visit(value);
  if (Array.isArray(value)) {
    for (const item of value) walk(item, visit);
  } else if (value && typeof value === "object") {
    for (const item of Object.values(value)) walk(item, visit);
  }
}

function countBy(values: unknown[], key: (value: unknown) => string) {
  const counts: Record<string, number> = {};
  for (const value of values) {
    const countKey = key(value);
    counts[countKey] = (counts[countKey] ?? 0) + 1;
  }
  return counts;
}

function commandFailed(value: JsonRecord | undefined, status: ToolRunStatus) {
  return status !== "succeeded" || (typeof value?.status === "number" && value.status !== 0);
}

function sensitiveOpcode(opcode?: string | null) {
  return Boolean(
    opcode
      && /Call|BorrowGlobalMut|MoveFrom|MoveTo|WriteField|BorrowFieldMut|Pack|Unpack|Abort|Assert/i.test(
        opcode,
      ),
  );
}

function mapAnalysisSeverity(value?: string | null): FindingCandidate["severity"] {
  if (value === "error") return "high";
  if (value === "warning") return "medium";
  if (value === "info") return "info";
  return "low";
}

function mapSecuritySeverity(value?: string | null): FindingCandidate["severity"] {
  if (value === "critical" || value === "high" || value === "medium" || value === "low") {
    return value;
  }
  return "info";
}

function unwrapRecord(output: unknown, key: string) {
  const record = asRecord(output);
  return asRecord(record?.[key]);
}

function asRecord(value: unknown): JsonRecord | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonRecord)
    : undefined;
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function booleanValue(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function numberValue(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function parseCount(text: string, pattern: RegExp) {
  const match = text.match(pattern);
  return match?.[1] ? Number.parseInt(match[1], 10) : 0;
}

function parseManifestFromStdout(stdout?: string) {
  if (!stdout) {
    return undefined;
  }

  return {
    crashEntries: parseCount(stdout, /Crash entries:\s*(\d+)/i),
    queueEntries: parseCount(stdout, /Queue entries:\s*(\d+)/i),
    publicFunctionCount: parseCount(stdout, /Public targets:\s*(\d+)/i),
    seed: parseCount(stdout, /Seed:\s*(\d+)/i),
    timeLimitSeconds: parseCount(stdout, /Time limit:\s*(\d+)s/i),
  };
}

function firstNonEmptyLine(...values: Array<string | undefined>) {
  return values
    .flatMap((value) => value?.split(/\r?\n/) ?? [])
    .map((line) => line.trim())
    .find(Boolean);
}

function excerpt(value: string, maxLength: number) {
  const compact = value.replace(/\s+/g, " ").trim();
  return compact.length > maxLength ? `${compact.slice(0, maxLength)}...` : compact;
}

function boundedJson(value: unknown, tokenBudget: number) {
  const maxLength = Math.max(200, tokenBudget * 4);
  try {
    return excerpt(JSON.stringify(value), maxLength);
  } catch {
    return excerpt(String(value), maxLength);
  }
}
