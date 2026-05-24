import type {
  FindingCandidateSeverity,
  JsonRecord,
  RiskLevel,
} from "@peregrine/agent-runtime";

import type {
  AttackHypothesis,
  AttackHypothesisSet,
  AuditExecutionResult,
  AuditFindingCandidate,
  AuditInvariant,
  AuditKnowledgeGraph,
  AuditReport,
  AuditSessionPacket,
  AuditTestCase,
  AuditTrace,
  AuditTraceArtifactName,
  BytecodeReviewPacket,
  CanonicalArtifact,
  CanonicalFunction,
  CanonicalModule,
  CanonicalProjectIndex,
  CanonicalStruct,
  ConfirmedFindingsSet,
  ContractClassificationReport,
  DynamicEvidencePacket,
  FixVerificationPacket,
  FunctionRiskEntry,
  FunctionRiskMap,
  GraphEvidencePacket,
  GraphEvidenceTrail,
  GraphSummary,
  InvariantRegistry,
  InvariantStressReport,
  ProtocolProfile,
  RegressionTestPacket,
  RemediationPlan,
  SeverityRankedFindingList,
  SeverityScore,
  StaticFindingsSet,
  TestPlanPacket,
  ThreatModelPacket,
} from "./audit-types";
import { AUDIT_TRACE_FILENAMES } from "./audit-types";

type RecordLike = Record<string, unknown>;

export interface CreateAuditSessionPacketInput {
  project: string;
  repoRoot: string;
  commit?: string | null;
  packageManifest: string;
  targetModules?: string[];
  compilerVersion?: string | null;
  dependencyGraph?: unknown;
  enabledTools?: string[];
  auditProfile?: string;
  threatModelProfile?: string;
  timestamp?: string;
  toolVersions?: Record<string, string>;
  policyProfile?: string;
  metadata?: JsonRecord;
}

export interface AuditPacketBuildInput {
  auditSession: AuditSessionPacket;
  packageTree?: unknown;
  movePackage?: unknown;
  indexReport?: unknown;
  packageOverview?: unknown;
  buildOutput?: unknown;
  bytecodeView?: unknown;
  staticReport?: unknown;
  graphs?: unknown;
  dynamicTestOutput?: unknown;
  fuzzOutput?: unknown;
  changedFiles?: string[];
  previousFindings?: AuditFindingCandidate[];
}

export function createAuditSessionPacket(
  input: CreateAuditSessionPacketInput,
): AuditSessionPacket {
  const timestamp = input.timestamp ?? new Date().toISOString();

  return {
    schemaVersion: 1,
    id: stableId("audit", input.repoRoot, input.packageManifest, input.commit ?? "unknown", timestamp),
    project: input.project,
    repoRoot: input.repoRoot,
    commit: input.commit?.trim() || "unknown",
    packageManifest: input.packageManifest,
    targetModules: input.targetModules ?? [],
    compilerVersion: input.compilerVersion ?? null,
    dependencyGraph: input.dependencyGraph,
    selectedChainAdapter: "sui/move",
    enabledTools: input.enabledTools ?? [],
    auditProfile: input.auditProfile ?? "full-sui-move-audit",
    threatModelProfile: input.threatModelProfile ?? "default-smart-contract-threat-model",
    timestamp,
    toolVersions: input.toolVersions ?? {},
    policyProfile: input.policyProfile,
    metadata: input.metadata,
  };
}

export function buildCanonicalProjectIndex(
  input: AuditPacketBuildInput,
): CanonicalProjectIndex {
  const movePackage = firstRecord(input.movePackage)
    ?? activeMovePackage(input.packageTree, input.auditSession.packageManifest)
    ?? {};
  const modules = asArray(recordValue(movePackage, "modules"));
  const surface = asRecord(recordValue(movePackage, "surface"));
  const indexReport = firstRecord(input.indexReport);
  const overview = firstRecord(input.packageOverview);
  const buildOutput = firstRecord(input.buildOutput);
  const bytecodeView = firstRecord(input.bytecodeView);
  const compilerVersion = input.auditSession.compilerVersion
    ?? stringValue(recordValue(indexReport, "compilerVersion"))
    ?? stringValue(pathValue(indexReport, ["indexHealth", "fingerprints", "compilerVersion"]))
    ?? stringValue(pathValue(indexReport, ["index_health", "fingerprints", "compilerVersion"]))
    ?? null;
  const canonicalModulesFromSignatures = modules.map((module, index) =>
    canonicalModule(asRecord(module), index),
  );
  const structsFromSignatures = modules.flatMap((module) =>
    canonicalStructsForModule(asRecord(module)),
  );
  const functionsFromSignatures = modules.flatMap((module) =>
    canonicalFunctionsForModule(asRecord(module)),
  );
  const fallbackFunctions = functionsFromSignatures.length
    ? []
    : canonicalFunctionsFromSurface(surface);
  const fallbackStructs = structsFromSignatures.length
    ? []
    : canonicalStructsFromSurface(surface);
  const canonicalModules = canonicalModulesFromSignatures.length
    ? canonicalModulesFromSignatures
    : canonicalModulesFromSurface(surface, fallbackFunctions, fallbackStructs);
  const structs = structsFromSignatures.length ? structsFromSignatures : fallbackStructs;
  const functions = functionsFromSignatures.length ? functionsFromSignatures : fallbackFunctions;
  const diagnostics = [
    ...stringArray(recordValue(buildOutput, "stderr")).slice(0, 1),
    ...asArray(recordValue(input.staticReport, "diagnostics")).map((item) =>
      stringValue(recordValue(asRecord(item), "message")) ?? JSON.stringify(item),
    ),
  ].filter(Boolean);
  const artifactModules = numberValue(recordValue(bytecodeView, "moduleCount")) ?? 0;
  const artifacts: CanonicalArtifact[] = [
    {
      kind: "moveManifest",
      path: stringValue(recordValue(movePackage, "manifestPath")) ?? input.auditSession.packageManifest,
      summary: "Move package manifest.",
    },
    {
      kind: "compiledBytecode",
      path: stringValue(recordValue(bytecodeView, "buildPath")),
      summary: artifactModules
        ? `Compiled bytecode view contains ${artifactModules} modules.`
        : "Compiled bytecode view was not available.",
    },
  ];

  return {
    schemaVersion: 1,
    auditSessionId: input.auditSession.id,
    repoRoot: input.auditSession.repoRoot,
    packageId: stringValue(recordValue(indexReport, "packageId"))
      ?? stringValue(recordValue(overview, "id"))
      ?? null,
    packageName: stringValue(recordValue(movePackage, "name"))
      ?? stringValue(recordValue(indexReport, "packageName"))
      ?? input.auditSession.project,
    packageManifest: stringValue(recordValue(movePackage, "manifestPath"))
      ?? input.auditSession.packageManifest,
    targetModules: input.auditSession.targetModules.length
      ? input.auditSession.targetModules
      : canonicalModules.map((module) => module.name),
    compilerVersion,
    build: {
      status: commandStatus(buildOutput),
      stdoutExcerpt: excerpt(stringValue(recordValue(buildOutput, "stdout")) ?? "", 1_200),
      stderrExcerpt: excerpt(stringValue(recordValue(buildOutput, "stderr")) ?? "", 1_200),
      diagnostics,
    },
    indexer: indexReport
      ? {
          runId: stringValue(recordValue(indexReport, "runId")),
          packageId: stringValue(recordValue(indexReport, "packageId")),
          dbPath: stringValue(recordValue(indexReport, "dbPath")),
          status: stringValue(recordValue(indexReport, "status")),
          health: recordValue(indexReport, "indexHealth"),
          layers: asArray(recordValue(indexReport, "indexLayers")),
        }
      : undefined,
    modules: canonicalModules,
    structs,
    functions,
    artifacts,
    diagnostics,
    sourcePrecision: functionsFromSignatures.length
      ? "compiler"
      : functions.length
        ? "heuristic"
        : artifactModules
          ? "bytecode"
          : "unknown",
    metadata: {
      overview,
      buildExitStatus: recordValue(buildOutput, "status"),
      bytecodeModuleCount: artifactModules,
      fallbackIndex: functionsFromSignatures.length === 0 && functions.length > 0
        ? "surface-derived"
        : undefined,
    },
  };
}

export function buildAuditKnowledgeGraph(
  input: AuditPacketBuildInput,
  projectIndex: CanonicalProjectIndex,
): AuditKnowledgeGraph {
  const graphs = firstRecord(input.graphs) ?? {};
  const movePackage = firstRecord(input.movePackage)
    ?? activeMovePackage(input.packageTree, projectIndex.packageManifest)
    ?? {};
  const surface = asRecord(recordValue(movePackage, "surface"));
  const callGraph = graphSummary(recordValue(graphs, "callGraph"), "call graph");
  const typeGraph = graphSummary(recordValue(graphs, "typeGraph"), "type graph");
  const stateGraph = graphSummary(recordValue(graphs, "stateAccessGraph"), "storage access graph");
  const lifecycleMaps = asArray(recordValue(surface, "objectLifecycleMaps"));
  const capabilityFindings = asArray(recordValue(surface, "capabilityFindings"));
  const externalCallFindings = asArray(recordValue(surface, "externalCallFindings"));
  const publicEntries = projectIndex.functions.filter((fn) => fn.isTransactionCallable);
  const graphQueries = buildGraphTrails(projectIndex, surface, callGraph, stateGraph);

  return {
    schemaVersion: 1,
    auditSessionId: input.auditSession.id,
    callGraph,
    typeGraph,
    objectLifecycleGraph: {
      nodes: lifecycleMaps.length,
      edges: lifecycleMaps.reduce<number>((total, map) =>
        total + asArray(recordValue(asRecord(map), "stages")).length,
      0),
      highValueNodes: lifecycleMaps
        .map((map) => stringValue(recordValue(asRecord(map), "qualifiedName")))
        .filter(isString),
      evidence: [`Loaded ${lifecycleMaps.length} object lifecycle maps.`],
      raw: lifecycleMaps,
    },
    capabilityGraph: {
      nodes: capabilityFindings.length,
      edges: capabilityFindings.reduce<number>((total, finding) =>
        total + asArray(recordValue(asRecord(finding), "protectedFunctions")).length,
      0),
      highValueNodes: capabilityFindings
        .map((finding) => stringValue(recordValue(asRecord(finding), "qualifiedName")))
        .filter(isString),
      evidence: [`Loaded ${capabilityFindings.length} capability findings.`],
      raw: capabilityFindings,
    },
    assetFlowGraph: assetFlowSummary(projectIndex, surface),
    storageAccessGraph: stateGraph,
    privilegeGraph: privilegeGraphSummary(projectIndex, surface),
    externalInteractionGraph: {
      nodes: externalCallFindings.length,
      edges: externalCallFindings.length,
      highValueNodes: externalCallFindings
        .map((finding) => stringValue(recordValue(asRecord(finding), "target")))
        .filter(isString),
      evidence: [`Loaded ${externalCallFindings.length} external call findings.`],
      raw: externalCallFindings,
    },
    graphQueries,
    unresolvedEdges: [
      unresolvedSummary("callGraph", recordValue(recordValue(graphs, "callGraph"), "unresolvedCalls")),
      unresolvedSummary("typeGraph", recordValue(recordValue(graphs, "typeGraph"), "unresolvedTypes")),
      unresolvedSummary(
        "storageAccessGraph",
        recordValue(recordValue(graphs, "stateAccessGraph"), "unresolvedAccesses"),
      ),
    ].filter((item) => item.count > 0),
    sourcePrecision: callGraph.nodes || typeGraph.nodes || stateGraph.nodes ? "compiler" : "heuristic",
    metadata: {
      publicEntryCount: publicEntries.length,
      surfaceCounts: {
        capabilityCount: numberValue(recordValue(surface, "capabilityCount")) ?? 0,
        sharedObjectCount: numberValue(recordValue(surface, "sharedObjectCount")) ?? 0,
        externalCallCount: numberValue(recordValue(surface, "externalCallCount")) ?? 0,
      },
    },
  };
}

export function buildContractClassification(
  auditSession: AuditSessionPacket,
  projectIndex: CanonicalProjectIndex,
  graph: AuditKnowledgeGraph,
): ContractClassificationReport {
  const text = searchableText(projectIndex);
  const profiles = new Set<string>();
  const addByKeyword = (profile: string, keywords: string[]) => {
    if (keywords.some((keyword) => text.includes(keyword))) profiles.add(profile);
  };

  addByKeyword("token", ["coin", "treasurycap", "mint", "burn", "supply", "token"]);
  addByKeyword("vault", ["vault", "deposit", "withdraw", "share", "receipt"]);
  addByKeyword("staking", ["stake", "unstake", "reward", "validator"]);
  addByKeyword("lending", ["borrow", "repay", "liquidat", "collateral", "loan"]);
  addByKeyword("DEX", ["swap", "pool", "amm", "liquidity", "clob", "order"]);
  addByKeyword("prediction market", ["market", "outcome", "settle", "claim", "prediction"]);
  addByKeyword("bridge", ["bridge", "message", "wormhole", "relayer"]);
  addByKeyword("NFT", ["nft", "kiosk", "collection", "royalty", "display"]);
  addByKeyword("game", ["game", "round", "score", "player"]);
  addByKeyword("escrow", ["escrow", "release", "refund"]);
  addByKeyword("DAO/governance", ["govern", "vote", "proposal", "quorum"]);
  addByKeyword("oracle consumer", ["oracle", "price", "pyth", "feed"]);

  const protocolProfile: ProtocolProfile = {
    assetHolding: text.includes("coin") || text.includes("balance") || text.includes("vault"),
    hasAdmin: text.includes("admin") || graph.capabilityGraph.nodes > 0,
    usesOracle: text.includes("oracle") || text.includes("price") || text.includes("pyth"),
    usesSharedObjects: graph.objectLifecycleGraph.evidence.some((item) =>
      item.toLowerCase().includes("shared"),
    ) || graph.objectLifecycleGraph.highValueNodes.some((node) => node.toLowerCase().includes("shared")),
    usesDynamicFields: text.includes("dynamic_field") || text.includes("dynamic object field"),
    hasUpgradeAuthority: text.includes("upgrade") || text.includes("publisher"),
    hasLiquidation: text.includes("liquidat"),
    hasExternalCalls: graph.externalInteractionGraph.edges > 0,
    hasCapabilityObjects: graph.capabilityGraph.nodes > 0,
  };

  if (protocolProfile.hasAdmin) profiles.add("admin-controlled protocol");
  if (!profiles.size) profiles.add("generic smart contract");

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    profiles: Array.from(profiles),
    protocolProfile,
    confidence:
      projectIndex.functions.length && projectIndex.structs.length
        ? "high"
        : projectIndex.functions.length
          ? "medium"
          : "low",
    evidence: [
      `${projectIndex.functions.length} functions and ${projectIndex.structs.length} structs indexed.`,
      `${graph.capabilityGraph.nodes} capability surfaces and ${graph.externalInteractionGraph.edges} external calls observed.`,
    ],
  };
}

export function buildThreatModel(
  auditSession: AuditSessionPacket,
  projectIndex: CanonicalProjectIndex,
  graph: AuditKnowledgeGraph,
  classification: ContractClassificationReport,
): ThreatModelPacket {
  const entries = projectIndex.functions.filter((fn) => fn.isTransactionCallable);
  const assets = [
    ...projectIndex.structs
      .filter((item) => item.isObjectLike || /coin|balance|vault|receipt|position/i.test(item.qualifiedName))
      .map((item) => item.qualifiedName),
    ...(classification.protocolProfile.assetHolding ? ["coins and balances moved by public entry functions"] : []),
  ];
  const actors = [
    "attacker with no privileges",
    "normal user",
    "object owner",
    ...(classification.protocolProfile.hasAdmin ? ["admin", "attacker with a capability object"] : []),
    ...(classification.protocolProfile.usesOracle ? ["attacker controlling oracle input or timing"] : []),
    ...(classification.protocolProfile.usesSharedObjects ? ["attacker contending on shared objects"] : []),
  ];
  const criticalInvariants = [
    ...(classification.protocolProfile.hasAdmin ? ["only authorized capability holders can change privileged config"] : []),
    ...(classification.protocolProfile.assetHolding ? ["asset movements preserve accounting and ownership"] : []),
    ...(classification.protocolProfile.usesOracle ? ["oracle prices are fresh, for the expected feed, and within confidence bounds"] : []),
    ...(classification.protocolProfile.usesSharedObjects ? ["shared object state cannot be corrupted by public calls"] : []),
    "public entry functions validate required preconditions before mutation",
  ];

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    assetsAtRisk: unique(assets),
    actors: unique(actors),
    privilegedRoles: classification.protocolProfile.hasAdmin
      ? unique(["admin", ...graph.capabilityGraph.highValueNodes])
      : [],
    trustedModules: unique([
      "0x1::",
      "0x2::",
      ...projectIndex.modules.map((module) => module.name),
    ]),
    untrustedInputs: unique([
      "transaction sender",
      "public entry parameters",
      ...(classification.protocolProfile.usesOracle ? ["oracle price data"] : []),
      ...(classification.protocolProfile.usesDynamicFields ? ["dynamic field keys and values"] : []),
    ]),
    entryPoints: entries.map((fn) => fn.qualifiedName),
    attackSurfaces: unique([
      ...entries.map((fn) => `public entry ${fn.qualifiedName}`),
      ...graph.graphQueries.map((trail) => trail.title),
    ]),
    economicAssumptions: classification.protocolProfile.assetHolding
      ? ["balances, shares, receipts, and supplies remain conserved across all asset paths"]
      : [],
    oracleAssumptions: classification.protocolProfile.usesOracle
      ? ["oracle feed identity, timestamp, confidence, and decimals are validated before economic use"]
      : [],
    upgradeAssumptions: classification.protocolProfile.hasUpgradeAuthority
      ? ["upgrade authority is trusted and protected by explicit capability checks"]
      : [],
    criticalInvariants: unique(criticalInvariants),
    evidence: [
      `Profiles: ${classification.profiles.join(", ")}.`,
      `${entries.length} transaction-callable functions define the main public surface.`,
    ],
  };
}

export function buildFunctionRiskMap(
  auditSession: AuditSessionPacket,
  projectIndex: CanonicalProjectIndex,
  graph: AuditKnowledgeGraph,
): FunctionRiskMap {
  const entries = projectIndex.functions.map((fn) => scoreFunctionRisk(fn, graph));
  const summary: Record<RiskLevel, number> = {
    low: 0,
    medium: 0,
    high: 0,
    critical: 0,
  };

  for (const entry of entries) {
    summary[entry.risk] += 1;
  }

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    functions: entries.sort((left, right) =>
      right.score - left.score || left.qualifiedName.localeCompare(right.qualifiedName),
    ),
    summary,
  };
}

export function buildInvariantRegistry(
  auditSession: AuditSessionPacket,
  threatModel: ThreatModelPacket,
  riskMap: FunctionRiskMap,
): InvariantRegistry {
  const invariants: AuditInvariant[] = [];
  const add = (
    invariantClass: string,
    description: string,
    severityIfBroken: FindingCandidateSeverity,
    target?: FunctionRiskEntry,
  ) => {
    invariants.push({
      id: stableId("inv", invariantClass, description, target?.qualifiedName ?? "global"),
      description,
      invariantClass,
      targetModule: target?.moduleName,
      targetFunction: target?.qualifiedName,
      severityIfBroken,
      evidenceSources: [
        ...(target ? target.evidence : []),
        ...threatModel.evidence,
      ],
      validationMethods: ["static graph query", "targeted negative test", "dynamic replay"],
      status: "candidate",
    });
  };

  for (const invariant of threatModel.criticalInvariants) {
    add("threat-model", invariant, "high");
  }

  for (const entry of riskMap.functions.filter((fn) => fn.risk === "critical" || fn.risk === "high").slice(0, 20)) {
    if (entry.tags.includes("admin")) {
      add("access-control", `${entry.qualifiedName} requires the intended admin/capability guard before mutation.`, "high", entry);
    }
    if (entry.tags.includes("asset-flow")) {
      add("balance-conservation", `${entry.qualifiedName} cannot move or mint assets without matching accounting updates.`, "critical", entry);
    }
    if (entry.tags.includes("shared-object")) {
      add("shared-object", `${entry.qualifiedName} preserves shared object state under adversarial callers.`, "high", entry);
    }
  }

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    invariants: dedupeBy(invariants, (item) => item.id),
  };
}

export function buildStaticFindingsSet(
  auditSession: AuditSessionPacket,
  staticReport: unknown,
  riskMap: FunctionRiskMap,
): StaticFindingsSet {
  const findings = asArray(recordValue(staticReport, "findings"));
  const diagnostics = asArray(recordValue(staticReport, "diagnostics"));
  const mapped = findings.map((finding, index) =>
    auditFinding({
      id: stableId("static", stringValue(recordValue(asRecord(finding), "ruleId")) ?? String(index)),
      title: stringValue(recordValue(asRecord(finding), "ruleId")) ?? "Static analysis finding",
      category: stringValue(recordValue(asRecord(finding), "rulesetId")) ?? "static-analysis",
      severity: mapAnalysisSeverity(stringValue(recordValue(asRecord(finding), "severity"))),
      state: "likely",
      confidence: "high",
      affectedSymbols: [
        stringValue(recordValue(asRecord(finding), "file")),
        stringValue(recordValue(asRecord(finding), "ruleId")),
      ].filter(isString),
      evidenceChain: [stringValue(recordValue(asRecord(finding), "message")) ?? "Static analyzer emitted a finding."],
      proofPath: [],
      testCaseIds: [],
      recommendation: "Validate exploitability and add a regression before patching.",
      metadata: { finding },
    }),
  );
  const heuristic = riskMap.functions
    .filter((fn) => (fn.risk === "critical" || fn.risk === "high") && riskyWithoutExplicitGuard(fn))
    .slice(0, 20)
    .map((fn) =>
      auditFinding({
        id: stableId("static-risk", fn.qualifiedName),
        title: `High-risk public surface: ${fn.qualifiedName}`,
        category: "surface-risk",
        severity: fn.risk === "critical" ? "critical" : "high",
        state: "possible",
        confidence: "medium",
        affectedSymbols: [fn.qualifiedName],
        evidenceChain: fn.evidence,
        proofPath: [],
        testCaseIds: [],
        recommendation: "Confirm authorization, state, and asset-flow preconditions before mutation.",
      }),
    );

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    findings: [...mapped, ...heuristic],
    diagnostics: diagnostics
      .map((item) => stringValue(recordValue(asRecord(item), "message")) ?? JSON.stringify(item))
      .slice(0, 50),
    coverage: [
      `${findings.length} bundled static findings.`,
      `${heuristic.length} high-risk surface checks.`,
    ],
  };
}

export function buildGraphEvidencePacket(
  auditSession: AuditSessionPacket,
  graph: AuditKnowledgeGraph,
): GraphEvidencePacket {
  const suspiciousRelationships = graph.graphQueries
    .filter((trail) => trail.severity === "critical" || trail.severity === "high")
    .map((trail) =>
      auditFinding({
        id: stableId("graph", trail.id),
        title: trail.title,
        category: "graph-analysis",
        severity: trail.severity === "critical" ? "critical" : "high",
        state: "possible",
        confidence: trail.paths.length ? "high" : "medium",
        affectedSymbols: trail.paths.flat().filter((item) => item.includes("::")).slice(0, 12),
        evidenceChain: trail.observations,
        proofPath: trail.paths[0] ?? [],
        testCaseIds: [],
        recommendation: "Validate the graph path with targeted tests or bytecode review.",
        metadata: { trail },
      }),
    );

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    trails: graph.graphQueries,
    suspiciousRelationships,
  };
}

export function buildBytecodeReviewPacket(
  auditSession: AuditSessionPacket,
  bytecodeView: unknown,
): BytecodeReviewPacket {
  const view = asRecord(bytecodeView);
  const modules = asArray(recordValue(view, "modules"));
  const functions = modules.flatMap((module) =>
    asArray(recordValue(asRecord(module), "functions")).map((fn) => ({
      module,
      fn: asRecord(fn),
    })),
  );
  const sensitiveInstructions = functions.flatMap(({ module, fn }) =>
    asArray(recordValue(fn, "instructions"))
      .map(asRecord)
      .filter((instruction) => sensitiveOpcode(stringValue(recordValue(instruction, "opcode"))))
      .slice(0, 20)
      .map((instruction) => ({
        moduleName: stringValue(recordValue(asRecord(module), "name")),
        functionName: stringValue(recordValue(fn, "name")),
        opcode: stringValue(recordValue(instruction, "opcode")) ?? "unknown",
        detail: stringValue(recordValue(instruction, "detail")),
        source: recordValue(instruction, "source"),
      })),
  );
  const instructionCount = numberValue(recordValue(view, "instructionCount"))
    ?? functions.reduce((total, item) =>
      total + (numberValue(recordValue(item.fn, "instructionCount")) ?? 0),
    0);

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    reviewed: modules.length > 0,
    modules: numberValue(recordValue(view, "moduleCount")) ?? modules.length,
    functions: numberValue(recordValue(view, "functionCount")) ?? functions.length,
    instructionCount,
    sensitiveInstructions,
    sourceConsistency: functions.slice(0, 50).map(({ fn }) => ({
      functionName: stringValue(recordValue(fn, "name")) ?? "unknown",
      status: asArray(recordValue(fn, "instructions")).some((instruction) =>
        Boolean(recordValue(asRecord(instruction), "source")),
      )
        ? "consistent"
        : "unknown",
      evidence: ["Source-map consistency is inferred from instruction source spans."],
    })),
    diagnostics: modules.length
      ? []
      : ["Bytecode view unavailable; source-level evidence remains primary."],
  };
}

export function buildAttackHypothesisSet(input: {
  auditSession: AuditSessionPacket;
  riskMap: FunctionRiskMap;
  invariants: InvariantRegistry;
  staticFindings: StaticFindingsSet;
  graphEvidence: GraphEvidencePacket;
  bytecodeReview: BytecodeReviewPacket;
}): AttackHypothesisSet {
  const hypotheses: AttackHypothesis[] = [];
  const add = (hypothesis: Omit<AttackHypothesis, "id" | "status">) => {
    hypotheses.push({
      ...hypothesis,
      id: stableId("hyp", hypothesis.claim, hypothesis.targetFunction ?? "global"),
      status: "untested",
    });
  };

  for (const finding of [...input.staticFindings.findings, ...input.graphEvidence.suspiciousRelationships]) {
    add({
      claim: finding.title,
      targetFunction: finding.affectedSymbols.find((symbol) => symbol.includes("::")),
      requiredActor: finding.category.includes("admin") ? "attacker without AdminCap" : "attacker with no privileges",
      requiredState: "protocol initialized with reachable public entry state",
      affectedAsset: finding.assetImpact,
      supportingGraphPath: finding.proofPath,
      suggestedTest: `Construct a negative test for ${finding.title}.`,
      severityIfTrue: finding.severity,
      evidenceRefs: finding.evidenceChain,
    });
  }

  for (const entry of input.riskMap.functions.filter((fn) => fn.risk === "critical" || fn.risk === "high").slice(0, 20)) {
    add({
      claim: `${entry.qualifiedName} may break ${entry.tags.join(", ")} assumptions if preconditions are bypassed.`,
      targetFunction: entry.qualifiedName,
      requiredActor: entry.isEntry ? "attacker with transaction access" : "caller reaching this function through a public path",
      requiredState: "valid object graph accepted by the function signature",
      affectedAsset: entry.tags.includes("asset-flow") ? "coins, balances, receipts, or owned objects" : undefined,
      supportingGraphPath: [entry.qualifiedName],
      suggestedTest: `Exercise ${entry.qualifiedName} with unauthorized sender, wrong state, repeated call, and boundary values.`,
      severityIfTrue: entry.risk === "critical" ? "critical" : "high",
      evidenceRefs: entry.evidence,
    });
  }

  for (const invariant of input.invariants.invariants.slice(0, 20)) {
    add({
      claim: invariant.description,
      targetFunction: invariant.targetFunction,
      requiredActor: "attacker matching the invariant's public surface",
      requiredState: "state satisfying setup preconditions",
      supportingGraphPath: invariant.targetFunction ? [invariant.targetFunction] : [],
      suggestedTest: `Check invariant ${invariant.id} before and after the target action.`,
      severityIfTrue: invariant.severityIfBroken,
      evidenceRefs: invariant.evidenceSources,
    });
  }

  for (const instruction of input.bytecodeReview.sensitiveInstructions.slice(0, 20)) {
    add({
      claim: `${instruction.functionName ?? "unknown function"} contains sensitive bytecode opcode ${instruction.opcode}.`,
      targetFunction: instruction.functionName,
      requiredActor: "caller able to reach the compiled path",
      requiredState: "compiled branch reaches sensitive opcode",
      supportingGraphPath: [instruction.functionName ?? instruction.opcode],
      suggestedTest: "Validate that source-level checks dominate the sensitive bytecode path.",
      severityIfTrue: "medium",
      evidenceRefs: [instruction.detail ?? instruction.opcode],
    });
  }

  return {
    schemaVersion: 1,
    auditSessionId: input.auditSession.id,
    hypotheses: dedupeBy(hypotheses, (item) => item.id).slice(0, 80),
  };
}

export function buildTestPlanPacket(
  auditSession: AuditSessionPacket,
  hypotheses: AttackHypothesisSet,
  invariants: InvariantRegistry,
): TestPlanPacket {
  const tests: AuditTestCase[] = hypotheses.hypotheses.slice(0, 60).map((hypothesis) => ({
    id: stableId("test", hypothesis.id),
    hypothesisId: hypothesis.id,
    invariantId: invariants.invariants.find((invariant) =>
      hypothesis.claim.includes(invariant.description) || invariant.targetFunction === hypothesis.targetFunction,
    )?.id,
    category: categoryForHypothesis(hypothesis),
    setup: hypothesis.requiredState,
    action: hypothesis.suggestedTest,
    expectedResult: "The action aborts or preserves the linked invariant unless the hypothesis is true.",
    tool: hypothesis.severityIfTrue === "critical" ? "sui move test + trace" : "sui move test",
    status: "planned",
    evidence: hypothesis.evidenceRefs,
  }));

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    tests,
  };
}

export function buildDynamicEvidencePacket(
  auditSession: AuditSessionPacket,
  testPlan: TestPlanPacket,
  dynamicOutput: unknown,
): DynamicEvidencePacket {
  const output = asRecord(dynamicOutput);
  const status = commandStatus(output);
  const observed = firstNonEmpty(
    stringValue(recordValue(output, "stderr")),
    stringValue(recordValue(output, "stdout")),
    "Dynamic command was not run.",
  );
  const result: AuditExecutionResult = {
    id: stableId("dynamic", auditSession.id, status, observed),
    target: "package test suite",
    tool: "sui move test",
    status: status === "passed" ? "passed" : status === "failed" ? "failed" : "skipped",
    expected: `${testPlan.tests.length} planned tests should pass after fixes and fail only for reproduced vulnerabilities before fixes.`,
    observed: excerpt(observed, 1_200),
    evidence: [excerpt(observed, 600)],
    confidence: status === "unknown" || status === "skipped" ? "unknown" : "confirmed",
  };

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    testResults: [result],
    simulations: [],
    stateDiffs: status === "failed" ? [result] : [],
    diagnostics: status === "skipped" || status === "unknown"
      ? ["Dynamic tests were unavailable or not run."]
      : [],
  };
}

export function buildInvariantStressReport(
  auditSession: AuditSessionPacket,
  invariants: InvariantRegistry,
  fuzzOutput: unknown,
): InvariantStressReport {
  const output = asRecord(fuzzOutput);
  const status = commandStatus(output);
  const observed = firstNonEmpty(
    stringValue(recordValue(output, "stderr")),
    stringValue(recordValue(output, "stdout")),
    "Fuzz/invariant stress command was not run.",
  );
  const result: AuditExecutionResult = {
    id: stableId("stress", auditSession.id, status, observed),
    target: "critical function and invariant campaign",
    tool: "movy fuzz",
    status: status === "passed" ? "passed" : status === "failed" ? "failed" : "skipped",
    expected: "No invariant counterexample is observed for the configured campaign.",
    observed: excerpt(observed, 1_200),
    evidence: [excerpt(observed, 600)],
    confidence: status === "failed" ? "confirmed" : status === "passed" ? "low" : "unknown",
  };

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    fuzzResults: [result],
    invariantResults: invariants.invariants.slice(0, 25).map((invariant) => ({
      ...result,
      id: stableId("stress-inv", invariant.id, result.id),
      target: invariant.targetFunction ?? invariant.invariantClass,
      expected: invariant.description,
    })),
    seeds: [
      {
        target: "critical surfaces",
        status,
        evidence: [excerpt(observed, 400)],
      },
    ],
    diagnostics: status === "skipped" || status === "unknown"
      ? ["Fuzzing unavailable; invariant stress remains an evidence gap."]
      : [],
  };
}

export function buildConfirmedFindingsSet(input: {
  auditSession: AuditSessionPacket;
  hypotheses: AttackHypothesisSet;
  staticFindings: StaticFindingsSet;
  graphEvidence: GraphEvidencePacket;
  dynamicEvidence: DynamicEvidencePacket;
  invariantStress: InvariantStressReport;
}): ConfirmedFindingsSet {
  const dynamicFailures = [
    ...input.dynamicEvidence.testResults,
    ...input.dynamicEvidence.stateDiffs,
    ...input.invariantStress.fuzzResults,
  ].filter((result) =>
    result.status === "failed"
    && result.confidence === "confirmed"
    && isTargetedExploitProof(result),
  );
  const candidates = [
    ...input.staticFindings.findings,
    ...input.graphEvidence.suspiciousRelationships,
  ];

  const findings: AuditFindingCandidate[] = candidates.map((candidate): AuditFindingCandidate => {
    const relatedFailure = dynamicFailures.find((failure) =>
      candidate.affectedSymbols.some((symbol) => failure.observed.includes(symbol))
      || failure.observed.toLowerCase().includes(candidate.title.toLowerCase().slice(0, 24)),
    );

    if (!relatedFailure) {
      return {
        ...candidate,
        state: candidate.state === "likely" ? ("likely" as const) : ("possible" as const),
      };
    }

    return {
      ...candidate,
      state: "confirmed" as const,
      confidence: "confirmed" as const,
      proofPath: [...candidate.proofPath, relatedFailure.id],
      testCaseIds: [...candidate.testCaseIds, relatedFailure.id],
      evidenceChain: [...candidate.evidenceChain, ...relatedFailure.evidence],
    };
  });

  for (const failure of dynamicFailures) {
    if (findings.some((finding) => finding.testCaseIds.includes(failure.id))) {
      continue;
    }

    findings.push(auditFinding({
      id: stableId("confirmed", failure.id),
      title: `Dynamic validation failure: ${failure.target}`,
      category: "dynamic-analysis",
      severity: "high",
      state: "confirmed",
      confidence: "confirmed",
      affectedSymbols: [failure.target],
      affectedInvariantIds: [],
      evidenceChain: failure.evidence,
      proofPath: [failure.id],
      testCaseIds: [failure.id],
      recommendation: "Minimize the failing input and add a regression test before patching.",
    }));
  }

  return {
    schemaVersion: 1,
    auditSessionId: input.auditSession.id,
    findings: dedupeBy(findings, (item) => item.id),
  };
}

function isTargetedExploitProof(result: AuditExecutionResult) {
  const target = result.target.toLowerCase();
  const evidence = result.evidence.join("\n").toLowerCase();

  if (target === "package test suite" || target === "critical function and invariant campaign") {
    return false;
  }

  return (
    target.includes("hypothesis")
    || target.includes("regression")
    || target.includes("pgr-")
    || evidence.includes("hypothesis")
    || evidence.includes("regression")
    || evidence.includes("pgr-")
    || evidence.includes("unauthorized")
    || evidence.includes("exploit")
  );
}

export function buildSeverityRankedFindingList(
  auditSession: AuditSessionPacket,
  confirmed: ConfirmedFindingsSet,
): SeverityRankedFindingList {
  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    findings: confirmed.findings
      .map((finding) => ({
        ...finding,
        severityScore: severityScore(finding),
      }))
      .sort((left, right) =>
        right.severityScore.total - left.severityScore.total
        || severityRank(left.severity) - severityRank(right.severity)
        || left.title.localeCompare(right.title),
      ),
  };
}

export function buildRemediationPlan(
  auditSession: AuditSessionPacket,
  ranked: SeverityRankedFindingList,
): RemediationPlan {
  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    remediations: ranked.findings
      .filter((finding) => finding.state !== "falsePositive" && finding.state !== "informational")
      .map((finding) => ({
        findingId: finding.id,
        rootCause: rootCauseForFinding(finding),
        affectedFunction: finding.affectedSymbols.find((symbol) => symbol.includes("::")),
        minimalFix: minimalFixForFinding(finding),
        saferRedesign: saferRedesignForFinding(finding),
        testToAdd: `Add a regression that reproduces ${finding.title} before the fix and passes after the fix.`,
        regressionInvariant: finding.affectedInvariantIds[0]
          ?? "The linked security property remains true after the patched action.",
      })),
  };
}

export function buildRegressionTestPacket(
  auditSession: AuditSessionPacket,
  ranked: SeverityRankedFindingList,
  testPlan: TestPlanPacket,
): RegressionTestPacket {
  const tests = ranked.findings
    .filter((finding) => finding.state === "confirmed" || finding.state === "likely")
    .map((finding) => {
      const planned = testPlan.tests.find((test) =>
        finding.testCaseIds.includes(test.id)
        || finding.affectedSymbols.some((symbol) => test.action.includes(symbol)),
      );
      const test: AuditTestCase = planned ?? {
        id: stableId("regression", finding.id),
        category: "regression",
        setup: "Construct the minimum state needed to reach the finding.",
        action: `Reproduce ${finding.title}.`,
        expectedResult: "Before the fix the test demonstrates the issue; after the fix it aborts safely or preserves state.",
        tool: "sui move test",
        status: "planned",
        evidence: finding.evidenceChain,
      };

      return {
        ...test,
        findingId: finding.id,
        draftPath: `tests/security/${finding.id.replace(/[^a-zA-Z0-9_]/g, "_")}.move`,
        beforeFixExpected: "fails or demonstrates vulnerable post-state",
        afterFixExpected: "passes with the issue prevented",
      };
    });

  return {
    schemaVersion: 1,
    auditSessionId: auditSession.id,
    tests,
  };
}

export function buildAuditReport(input: {
  auditSession: AuditSessionPacket;
  projectIndex: CanonicalProjectIndex;
  classification: ContractClassificationReport;
  threatModel: ThreatModelPacket;
  rankedFindings: SeverityRankedFindingList;
  invariants: InvariantRegistry;
  testPlan: TestPlanPacket;
  dynamicEvidence: DynamicEvidencePacket;
  remediationPlan: RemediationPlan;
}): AuditReport {
  const confirmedCount = input.rankedFindings.findings.filter((finding) => finding.state === "confirmed").length;
  const blocked = [
    ...input.dynamicEvidence.diagnostics,
    ...input.projectIndex.diagnostics,
  ].length > 0;
  const markdown = [
    `# Peregrine Audit Report: ${input.auditSession.project}`,
    "",
    "## Executive Summary",
    `Peregrine reviewed ${input.projectIndex.functions.length} functions across ${input.projectIndex.modules.length} modules.`,
    `Profiles: ${input.classification.profiles.join(", ")}.`,
    `Findings: ${input.rankedFindings.findings.length}; confirmed: ${confirmedCount}.`,
    "",
    "## Scope",
    `Repository: ${input.auditSession.repoRoot}`,
    `Commit: ${input.auditSession.commit}`,
    `Manifest: ${input.auditSession.packageManifest}`,
    "",
    "## Methodology",
    "The workflow followed session creation, build/index normalization, graph construction, classification, threat modeling, static and graph analysis, bytecode review, hypothesis generation, targeted tests, dynamic/fuzz evidence, exploitability triage, severity ranking, remediation, and regression planning.",
    "",
    "## Threat Model",
    `Assets at risk: ${input.threatModel.assetsAtRisk.join(", ") || "none identified"}.`,
    `Actors: ${input.threatModel.actors.join(", ")}.`,
    `Entry points: ${input.threatModel.entryPoints.slice(0, 20).join(", ") || "none indexed"}.`,
    "",
    "## Findings",
    input.rankedFindings.findings.length
      ? input.rankedFindings.findings
        .map((finding) =>
          `- [${finding.severity}] ${finding.title} (${finding.state}, ${finding.confidence})`,
        )
        .join("\n")
      : "- No findings were ranked.",
    "",
    "## Invariants Reviewed",
    input.invariants.invariants.length
      ? input.invariants.invariants
        .slice(0, 30)
        .map((invariant) => `- ${invariant.id}: ${invariant.description}`)
        .join("\n")
      : "- No invariants were generated.",
    "",
    "## Tests Generated And Executed",
    `Planned tests: ${input.testPlan.tests.length}.`,
    `Dynamic results: ${input.dynamicEvidence.testResults.map((result) => `${result.tool}:${result.status}`).join(", ") || "none"}.`,
    "",
    "## Recommendations",
    input.remediationPlan.remediations.length
      ? input.remediationPlan.remediations
        .slice(0, 30)
        .map((remediation) => `- ${remediation.findingId}: ${remediation.minimalFix}`)
        .join("\n")
      : "- No remediations were generated.",
  ].join("\n");

  return {
    schemaVersion: 1,
    auditSessionId: input.auditSession.id,
    markdown,
    findingCount: input.rankedFindings.findings.length,
    confirmedFindingCount: confirmedCount,
    evidenceCompleteness: blocked ? "partial" : "complete",
  };
}

export function buildFixVerificationPacket(input: {
  auditSession: AuditSessionPacket;
  changedFiles?: string[];
  rankedFindings: SeverityRankedFindingList;
  dynamicEvidence?: DynamicEvidencePacket;
}): FixVerificationPacket {
  const changedFiles = input.changedFiles ?? [];
  const rerunStages = changedFiles.length
    ? (["buildNormalize", "semanticGraphs", "staticAnalysis", "graphAnalysis", "bytecodeReview", "dynamicAnalysis", "invariantStress"] as const)
    : ([] as const);
  const hasFailure = input.dynamicEvidence?.testResults.some((result) => result.status === "failed");

  return {
    schemaVersion: 1,
    auditSessionId: input.auditSession.id,
    changedFiles,
    rerunStages: [...rerunStages],
    findingStatuses: input.rankedFindings.findings.map((finding) => ({
      findingId: finding.id,
      previousState: "open",
      nextState: hasFailure
        ? "needsReview"
        : changedFiles.length
          ? "partiallyFixed"
          : "needsReview",
      evidence: changedFiles.length
        ? [`Changed files: ${changedFiles.join(", ")}`]
        : ["No changed files were supplied for fix verification."],
    })),
    diagnostics: changedFiles.length
      ? []
      : ["Fix verification requires changed files or a before/after run diff."],
  };
}

export function buildAuditTrace(input: {
  auditSession: AuditSessionPacket;
  packets: Partial<Record<AuditTraceArtifactName, unknown>>;
  stageRuns?: AuditTrace["stageRuns"];
  findingSource?: SeverityRankedFindingList | ConfirmedFindingsSet;
  generatedAt?: string;
}): AuditTrace {
  return {
    schemaVersion: 1,
    auditSessionId: input.auditSession.id,
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    artifacts: Object.entries(input.packets).map(([name, packet]) => ({
      name: name as AuditTraceArtifactName,
      filename: AUDIT_TRACE_FILENAMES[name as AuditTraceArtifactName],
      summary: summaryForPacket(packet),
    })),
    stageRuns: input.stageRuns ?? [],
    findings: "findings" in (input.findingSource ?? {})
      ? ((input.findingSource as SeverityRankedFindingList | ConfirmedFindingsSet).findings as AuditFindingCandidate[])
      : [],
  };
}

function canonicalModule(module: RecordLike, index: number): CanonicalModule {
  const name = stringValue(recordValue(module, "name")) ?? `module_${index}`;

  return {
    id: stableId("module", stringValue(recordValue(module, "address")) ?? "", name),
    name,
    address: stringValue(recordValue(module, "address")) ?? null,
    filePath: stringValue(recordValue(module, "filePath")) ?? null,
    attributes: stringArray(recordValue(module, "attributes")),
  };
}

function canonicalStructsForModule(module: RecordLike): CanonicalStruct[] {
  const moduleName = stringValue(recordValue(module, "name")) ?? "unknown";
  const filePath = stringValue(recordValue(module, "filePath")) ?? null;

  return asArray(recordValue(module, "structs")).map((value, index) => {
    const moveStruct = asRecord(value);
    const name = stringValue(recordValue(moveStruct, "name")) ?? `struct_${index}`;
    const qualifiedName = `${moduleName}::${name}`;
    const abilities = stringArray(recordValue(moveStruct, "abilities"));
    const fields = asArray(recordValue(moveStruct, "fields")).map((field) => ({
      name: stringValue(recordValue(asRecord(field), "name")) ?? "",
      typeName: stringValue(recordValue(asRecord(field), "typeName")) ?? "",
    }));

    return {
      id: stableId("struct", qualifiedName),
      moduleName,
      name,
      qualifiedName,
      abilities,
      fields,
      filePath,
      isCapabilityLike: /cap|authority|treasury/i.test(qualifiedName)
        || fields.some((field) => /cap|authority|treasury/i.test(field.typeName)),
      isObjectLike: abilities.includes("key") || fields.some((field) => field.typeName.includes("UID")),
    };
  });
}

function canonicalFunctionsForModule(module: RecordLike): CanonicalFunction[] {
  const moduleName = stringValue(recordValue(module, "name")) ?? "unknown";
  const filePath = stringValue(recordValue(module, "filePath")) ?? null;

  return asArray(recordValue(module, "functions")).map((value, index) => {
    const fn = asRecord(value);
    const name = stringValue(recordValue(fn, "name")) ?? `function_${index}`;
    const signature = stringValue(recordValue(fn, "signature"));

    return {
      id: stableId("function", moduleName, name, signature ?? ""),
      moduleName,
      name,
      qualifiedName: `${moduleName}::${name}`,
      visibility: stringValue(recordValue(fn, "visibility")) ?? "unknown",
      isEntry: booleanValue(recordValue(fn, "isEntry")),
      isTransactionCallable: booleanValue(recordValue(fn, "isTransactionCallable")),
      parameters: parseParameters(signature),
      returns: parseReturns(signature),
      attributes: stringArray(recordValue(fn, "attributes")),
      filePath,
      signature,
    };
  });
}

function canonicalFunctionsFromSurface(surface: RecordLike): CanonicalFunction[] {
  const refs = new Map<string, RecordLike>();
  const addRef = (value: unknown) => {
    const ref = asRecord(value);
    const qualifiedName = stringValue(recordValue(ref, "qualifiedName"));
    const moduleName = stringValue(recordValue(ref, "moduleName"));
    const functionName = stringValue(recordValue(ref, "functionName"));

    if (!qualifiedName && (!moduleName || !functionName)) {
      return;
    }

    refs.set(qualifiedName ?? `${moduleName}::${functionName}`, ref);
  };

  for (const map of asArray(recordValue(surface, "objectLifecycleMaps"))) {
    const lifecycle = asRecord(map);
    for (const ref of asArray(recordValue(lifecycle, "touchedBy"))) {
      addRef(ref);
    }
    for (const stage of asArray(recordValue(lifecycle, "stages"))) {
      for (const ref of asArray(recordValue(asRecord(stage), "functions"))) {
        addRef(ref);
      }
    }
    for (const risk of asArray(recordValue(lifecycle, "risks"))) {
      for (const ref of asArray(recordValue(asRecord(risk), "functions"))) {
        addRef(ref);
      }
    }
  }

  for (const finding of asArray(recordValue(surface, "adminControlFindings"))) {
    addRef(finding);
  }
  for (const finding of asArray(recordValue(surface, "externalCallFindings"))) {
    addRef({
      moduleName: stringValue(recordValue(asRecord(finding), "callerModule")),
      functionName: stringValue(recordValue(asRecord(finding), "callerFunction")),
      qualifiedName: [
        stringValue(recordValue(asRecord(finding), "callerModule")),
        stringValue(recordValue(asRecord(finding), "callerFunction")),
      ].filter(Boolean).join("::"),
    });
  }
  for (const relationship of asArray(recordValue(surface, "publicPackageRelationships"))) {
    addRef({
      moduleName: stringValue(recordValue(asRecord(relationship), "sourceModule")),
      functionName: stringValue(recordValue(asRecord(relationship), "sourceFunction")),
      qualifiedName: [
        stringValue(recordValue(asRecord(relationship), "sourceModule")),
        stringValue(recordValue(asRecord(relationship), "sourceFunction")),
      ].filter(Boolean).join("::"),
    });
  }

  return Array.from(refs.entries()).map(([qualifiedName, ref]) => {
    const [fallbackModuleName, fallbackFunctionName] = qualifiedName.split("::");
    const moduleName = stringValue(recordValue(ref, "moduleName")) ?? fallbackModuleName ?? "unknown";
    const name = stringValue(recordValue(ref, "functionName")) ?? fallbackFunctionName ?? "unknown";
    const visibility = stringValue(recordValue(ref, "visibility")) ?? "public";
    const isEntry = booleanValue(recordValue(ref, "isEntry"));
    const isTransactionCallable = booleanValue(recordValue(ref, "isTransactionCallable")) || isEntry || visibility === "public";

    return {
      id: stableId("function", moduleName, name, "surface"),
      moduleName,
      name,
      qualifiedName: `${moduleName}::${name}`,
      visibility,
      isEntry,
      isTransactionCallable,
      parameters: [],
      returns: [],
      attributes: [],
      filePath: stringValue(recordValue(ref, "filePath")) ?? null,
      signature: null,
    };
  });
}

function canonicalStructsFromSurface(surface: RecordLike): CanonicalStruct[] {
  const refs = new Map<string, Partial<CanonicalStruct>>();
  const addStruct = (qualifiedName: string | undefined, options: Partial<CanonicalStruct> = {}) => {
    if (!qualifiedName) {
      return;
    }

    refs.set(qualifiedName, {
      ...refs.get(qualifiedName),
      ...options,
      qualifiedName,
    });
  };

  for (const name of stringArray(recordValue(surface, "capabilityStructs"))) {
    addStruct(name, { isCapabilityLike: true });
  }
  for (const name of stringArray(recordValue(surface, "sharedObjectStructs"))) {
    addStruct(name, { isObjectLike: true });
  }
  for (const finding of asArray(recordValue(surface, "capabilityFindings"))) {
    addStruct(stringValue(recordValue(asRecord(finding), "qualifiedName")), {
      isCapabilityLike: true,
      moduleName: stringValue(recordValue(asRecord(finding), "moduleName")),
      name: stringValue(recordValue(asRecord(finding), "typeName")),
    });
  }
  for (const finding of asArray(recordValue(surface, "objectOwnershipFindings"))) {
    addStruct(stringValue(recordValue(asRecord(finding), "qualifiedName")), {
      isObjectLike: true,
      moduleName: stringValue(recordValue(asRecord(finding), "moduleName")),
      name: stringValue(recordValue(asRecord(finding), "typeName")),
    });
  }
  for (const map of asArray(recordValue(surface, "objectLifecycleMaps"))) {
    const lifecycle = asRecord(map);
    addStruct(stringValue(recordValue(lifecycle, "qualifiedName")), {
      isCapabilityLike: booleanValue(recordValue(lifecycle, "isCapabilityLike")),
      isObjectLike: true,
      moduleName: stringValue(recordValue(lifecycle, "moduleName")),
      name: stringValue(recordValue(lifecycle, "typeName")),
      filePath: stringValue(recordValue(lifecycle, "filePath")),
      abilities: stringArray(recordValue(lifecycle, "abilities")),
    });
  }

  return Array.from(refs.entries()).map(([qualifiedName, value], index) => {
    const [moduleName = "unknown", name = `struct_${index}`] = qualifiedName.split("::");

    return {
      id: stableId("struct", qualifiedName),
      moduleName: value.moduleName ?? moduleName,
      name: value.name ?? name,
      qualifiedName,
      abilities: value.abilities ?? [],
      fields: value.fields ?? [],
      filePath: value.filePath ?? null,
      isCapabilityLike: Boolean(value.isCapabilityLike) || /cap|authority|treasury/i.test(qualifiedName),
      isObjectLike: Boolean(value.isObjectLike),
    };
  });
}

function canonicalModulesFromSurface(
  surface: RecordLike,
  functions: CanonicalFunction[],
  structs: CanonicalStruct[],
): CanonicalModule[] {
  const moduleNames = new Set<string>();
  for (const fn of functions) moduleNames.add(fn.moduleName);
  for (const struct of structs) moduleNames.add(struct.moduleName);
  for (const finding of asArray(recordValue(surface, "externalCallFindings"))) {
    const callerModule = stringValue(recordValue(asRecord(finding), "callerModule"));
    if (callerModule) moduleNames.add(callerModule);
  }

  return Array.from(moduleNames).sort().map((name) => ({
    id: stableId("module", "", name),
    name,
    address: null,
    filePath: null,
    attributes: [],
  }));
}

function graphSummary(value: unknown, label: string): GraphSummary {
  const graph = asRecord(value);
  const nodes = asArray(recordValue(graph, "nodes"));
  const edges = asArray(recordValue(graph, "edges"));

  return {
    nodes: nodes.length,
    edges: edges.length,
    highValueNodes: nodes
      .map((node) =>
        stringValue(recordValue(asRecord(node), "qualifiedName"))
        ?? stringValue(recordValue(asRecord(node), "id"))
        ?? stringValue(node),
      )
      .filter(isString)
      .filter((item) => /entry|withdraw|deposit|transfer|admin|cap|mint|burn|delete|oracle|price/i.test(item))
      .slice(0, 40),
    evidence: [`Loaded ${label} with ${nodes.length} nodes and ${edges.length} edges.`],
    raw: graph,
  };
}

function assetFlowSummary(projectIndex: CanonicalProjectIndex, surface: RecordLike): GraphSummary {
  const assetFunctions = projectIndex.functions.filter((fn) =>
    /coin|balance|transfer|withdraw|deposit|mint|burn|split|join|pay|redeem/i.test(fn.qualifiedName + " " + (fn.signature ?? "")),
  );

  return {
    nodes: assetFunctions.length,
    edges: assetFunctions.length,
    highValueNodes: assetFunctions.map((fn) => fn.qualifiedName).slice(0, 40),
    evidence: [
      `${assetFunctions.length} functions mention asset movement or accounting terms.`,
      `${numberValue(recordValue(surface, "entryFunctionCount")) ?? 0} entry functions are transaction-callable.`,
    ],
  };
}

function privilegeGraphSummary(projectIndex: CanonicalProjectIndex, surface: RecordLike): GraphSummary {
  const capabilityFindings = asArray(recordValue(surface, "capabilityFindings"));
  const adminFindings = asArray(recordValue(surface, "adminControlFindings"));
  const adminFunctions = projectIndex.functions.filter((fn) =>
    /admin|owner|config|fee|pause|upgrade|cap/i.test(fn.qualifiedName + " " + (fn.signature ?? "")),
  );

  return {
    nodes: capabilityFindings.length + adminFunctions.length,
    edges: adminFindings.length + capabilityFindings.length,
    highValueNodes: unique([
      ...adminFunctions.map((fn) => fn.qualifiedName),
      ...capabilityFindings
        .map((finding) => stringValue(recordValue(asRecord(finding), "qualifiedName")))
        .filter(isString),
    ]).slice(0, 40),
    evidence: [
      `${capabilityFindings.length} capability findings and ${adminFindings.length} admin-control findings.`,
    ],
    raw: { capabilityFindings, adminFindings },
  };
}

function buildGraphTrails(
  projectIndex: CanonicalProjectIndex,
  surface: RecordLike,
  callGraph: GraphSummary,
  stateGraph: GraphSummary,
): GraphEvidenceTrail[] {
  const trails: GraphEvidenceTrail[] = [];
  const publicEntries = projectIndex.functions.filter((fn) => fn.isTransactionCallable);
  const assetEntries = publicEntries.filter((fn) =>
    /withdraw|deposit|transfer|mint|burn|redeem|claim|settle|split|join/i.test(fn.qualifiedName),
  );
  const adminEntries = publicEntries.filter((fn) =>
    /admin|config|fee|pause|upgrade|owner|cap/i.test(fn.qualifiedName + " " + (fn.signature ?? "")),
  );
  const sharedMutators = asArray(recordValue(surface, "objectLifecycleMaps"))
    .flatMap((map) => asArray(recordValue(asRecord(map), "touchedBy")))
    .map(asRecord)
    .filter((fn) => booleanValue(recordValue(fn, "isTransactionCallable")))
    .map((fn) => stringValue(recordValue(fn, "qualifiedName")))
    .filter(isString);

  if (assetEntries.length) {
    trails.push({
      id: "public-entry-to-asset-movement",
      title: "Public entry path reaches asset movement",
      query: "public entry -> transfer/mint/burn/withdraw/deposit",
      severity: "critical",
      paths: assetEntries.map((fn) => [fn.qualifiedName]),
      observations: assetEntries.map((fn) => `${fn.qualifiedName} is transaction-callable and asset-named.`),
      evidenceSources: ["canonical index", ...callGraph.evidence],
    });
  }

  if (adminEntries.length) {
    trails.push({
      id: "public-entry-to-privileged-state",
      title: "Public entry path reaches privileged state or config mutation",
      query: "public entry -> admin/config/capability surface",
      severity: "high",
      paths: adminEntries.map((fn) => [fn.qualifiedName]),
      observations: adminEntries.map((fn) => `${fn.qualifiedName} appears to touch privileged state.`),
      evidenceSources: ["canonical index", ...stateGraph.evidence],
    });
  }

  if (sharedMutators.length) {
    trails.push({
      id: "public-shared-object-mutation",
      title: "Public path can mutate shared or lifecycle-sensitive objects",
      query: "public entry -> shared object mutation",
      severity: "high",
      paths: sharedMutators.map((qualifiedName) => [qualifiedName]),
      observations: sharedMutators.map((qualifiedName) => `${qualifiedName} touches lifecycle-sensitive object state.`),
      evidenceSources: ["object lifecycle map"],
    });
  }

  return trails;
}

function scoreFunctionRisk(fn: CanonicalFunction, graph: AuditKnowledgeGraph): FunctionRiskEntry {
  let score = 0;
  const reasons: string[] = [];
  const tags: string[] = [];
  const text = `${fn.qualifiedName} ${fn.signature ?? ""}`.toLowerCase();
  const add = (points: number, reason: string, tag: string) => {
    score += points;
    reasons.push(reason);
    tags.push(tag);
  };

  if (fn.isEntry) add(20, "entry function", "entry");
  if (fn.isTransactionCallable && !fn.isEntry) add(14, "public transaction-callable function", "public");
  if (/withdraw|redeem|claim|settle|transfer/.test(text)) add(35, "moves or releases assets/objects", "asset-flow");
  if (/mint|burn|supply|treasurycap/.test(text)) add(30, "changes mint/burn/supply state", "asset-flow");
  if (/admin|config|fee|pause|upgrade|owner|cap/.test(text)) add(30, "touches admin, config, ownership, or capabilities", "admin");
  if (/shared|&mut/.test(text) || graph.objectLifecycleGraph.highValueNodes.some((node) => text.includes(node.toLowerCase()))) {
    add(20, "mutates shared or lifecycle-sensitive state", "shared-object");
  }
  if (/oracle|price|feed|pyth|clock|timestamp/.test(text)) add(18, "depends on price, time, or oracle-like input", "oracle");
  if (/dynamic_field|table|bag/.test(text)) add(16, "uses dynamic storage or collections", "dynamic-field");
  if (/delete|destroy|unwrap|unpack/.test(text)) add(18, "can delete or consume objects", "lifecycle");
  if (graph.externalInteractionGraph.highValueNodes.some((node) => text.includes(lastSegment(node).toLowerCase()))) {
    add(12, "related to external package interaction", "external-call");
  }

  const risk: RiskLevel =
    score >= 80 ? "critical" : score >= 55 ? "high" : score >= 30 ? "medium" : "low";

  return {
    functionId: fn.id,
    qualifiedName: fn.qualifiedName,
    moduleName: fn.moduleName,
    functionName: fn.name,
    isEntry: fn.isEntry,
    isTransactionCallable: fn.isTransactionCallable,
    risk,
    score,
    reasons: unique(reasons),
    tags: unique(tags),
    evidence: unique([
      fn.signature ?? fn.qualifiedName,
      ...reasons,
    ]),
  };
}

function auditFinding(input: Partial<AuditFindingCandidate> & Pick<AuditFindingCandidate, "id" | "title" | "category" | "severity" | "state" | "confidence" | "affectedSymbols" | "evidenceChain" | "proofPath" | "testCaseIds">): AuditFindingCandidate {
  return {
    affectedInvariantIds: [],
    ...input,
  };
}

function severityScore(finding: AuditFindingCandidate): SeverityScore {
  const severityBase = { critical: 10, high: 8, medium: 5, low: 2, info: 1 }[finding.severity];
  const confirmed = finding.state === "confirmed" ? 2 : finding.state === "likely" ? 1 : 0;
  const asset = /asset|coin|balance|withdraw|mint|burn|transfer/i.test(`${finding.title} ${finding.affectedSymbols.join(" ")}`) ? 10 : severityBase;
  const admin = /admin|owner|cap|upgrade/i.test(`${finding.title} ${finding.affectedSymbols.join(" ")}`) ? 9 : severityBase;

  return {
    assetImpact: asset,
    privilegeRequired: admin,
    attackComplexity: finding.state === "confirmed" ? 8 : 5,
    affectedUsers: severityBase,
    repeatability: confirmed ? 9 : 5,
    protocolStateImpact: Math.max(asset, admin, severityBase),
    recoverability: finding.severity === "critical" ? 9 : severityBase,
    detectability: finding.confidence === "confirmed" ? 8 : 4,
    economicImpact: asset,
    total: asset + admin + severityBase * 4 + confirmed * 5,
  };
}

function rootCauseForFinding(finding: AuditFindingCandidate) {
  if (/admin|cap|owner/i.test(finding.title)) return "Privileged state is reachable without enough proven authorization evidence.";
  if (/asset|withdraw|mint|burn|transfer/i.test(finding.title)) return "Asset movement path needs stronger accounting and authorization evidence.";
  if (/shared/i.test(finding.title)) return "Shared object mutation path needs explicit caller/state precondition checks.";
  return "Security-sensitive behavior lacks complete validation evidence.";
}

function minimalFixForFinding(finding: AuditFindingCandidate) {
  if (/admin|cap|owner/i.test(finding.title)) return "Require the intended capability or owner check before any privileged state mutation.";
  if (/asset|withdraw|mint|burn|transfer/i.test(finding.title)) return "Check authorization and accounting invariants before moving, minting, burning, or deleting assets.";
  if (/oracle|price/i.test(finding.title)) return "Validate feed identity, freshness, confidence, and decimals before using price data.";
  return "Add precondition checks before mutation and cover the path with a regression test.";
}

function saferRedesignForFinding(finding: AuditFindingCandidate) {
  if (/cap/i.test(finding.title)) return "Centralize privileged effects behind a small internal function that always requires the capability type.";
  if (/asset|balance/i.test(finding.title)) return "Model accounting as a single state transition that updates balances and moves assets atomically.";
  return "Reduce the public surface and expose narrow entry wrappers around checked internal transitions.";
}

function riskyWithoutExplicitGuard(entry: FunctionRiskEntry) {
  const text = `${entry.qualifiedName} ${entry.evidence.join(" ")}`.toLowerCase();
  return (
    entry.tags.some((tag) => ["admin", "asset-flow", "shared-object"].includes(tag))
    && !/admincap|treasurycap|ownercap|assert|abort|has_access|authorized/.test(text)
  );
}

function categoryForHypothesis(hypothesis: AttackHypothesis): AuditTestCase["category"] {
  const text = hypothesis.claim.toLowerCase();
  if (/admin|cap|owner|auth/.test(text)) return "authorization";
  if (/state|phase|settle|unlock|expiry/.test(text)) return "stateMachine";
  if (/invariant|conservation|preserve/.test(text)) return "invariant";
  if (/fuzz|boundary|round/.test(text)) return "fuzz";
  return "negative";
}

function mapAnalysisSeverity(severity?: string): FindingCandidateSeverity {
  switch (severity?.toLowerCase()) {
    case "error":
      return "high";
    case "warning":
      return "medium";
    case "info":
      return "low";
    default:
      return "medium";
  }
}

function commandStatus(output: RecordLike | undefined): CanonicalProjectIndex["build"]["status"] {
  if (!output || !Object.keys(output).length) return "unknown";
  const status = recordValue(output, "status");
  if (status === null || status === undefined) return "unknown";
  return status === 0 ? "passed" : "failed";
}

function parseParameters(signature?: string | null) {
  const inside = signature?.match(/\((.*)\)/s)?.[1];
  if (!inside?.trim()) return [];
  return inside.split(",").map((part) => part.trim()).filter(Boolean);
}

function parseReturns(signature?: string | null) {
  const match = signature?.match(/\)\s*:\s*(.+)$/s);
  if (!match) return [];
  return match[1].split(",").map((part) => part.trim()).filter(Boolean);
}

function sensitiveOpcode(opcode?: string | null) {
  return Boolean(opcode && /Call|Pack|Unpack|MutBorrow|Write|MoveFrom|MoveTo|Abort|Branch|FreezeRef|Destroy|BorrowField/i.test(opcode));
}

function activeMovePackage(packageTree: unknown, manifestPath: string) {
  const tree = asRecord(packageTree);
  const packages = asArray(recordValue(tree, "movePackages")).map(asRecord);
  return packages.find((candidate) =>
    stringValue(recordValue(candidate, "manifestPath")) === manifestPath,
  ) ?? packages[0];
}

function searchableText(projectIndex: CanonicalProjectIndex) {
  return [
    projectIndex.packageName,
    ...projectIndex.modules.map((item) => item.name),
    ...projectIndex.structs.map((item) => `${item.qualifiedName} ${item.fields.map((field) => field.typeName).join(" ")}`),
    ...projectIndex.functions.map((item) => `${item.qualifiedName} ${item.signature ?? ""}`),
  ].join(" ").toLowerCase();
}

function unresolvedSummary(graph: string, value: unknown) {
  const examples = asArray(value);
  return {
    graph,
    count: examples.length,
    examples: examples.slice(0, 10),
  };
}

function summaryForPacket(packet: unknown) {
  const record = asRecord(packet);
  if ("markdown" in record) return "Human-readable audit report.";
  if ("findings" in record) return `${asArray(record.findings).length} finding records.`;
  if ("hypotheses" in record) return `${asArray(record.hypotheses).length} attack hypotheses.`;
  if ("tests" in record) return `${asArray(record.tests).length} test records.`;
  if ("functions" in record) return `${asArray(record.functions).length} function records.`;
  return "Audit packet.";
}

function severityRank(severity: FindingCandidateSeverity) {
  return { critical: 0, high: 1, medium: 2, low: 3, info: 4 }[severity];
}

function firstRecord(value: unknown) {
  return value && typeof value === "object" && !Array.isArray(value) ? value as RecordLike : undefined;
}

function asRecord(value: unknown): RecordLike {
  return value && typeof value === "object" && !Array.isArray(value) ? value as RecordLike : {};
}

function recordValue(value: unknown, key: string) {
  return asRecord(value)[key];
}

function pathValue(value: unknown, path: string[]) {
  let current = value;
  for (const segment of path) {
    current = recordValue(current, segment);
  }
  return current;
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function booleanValue(value: unknown): boolean {
  return value === true;
}

function stringArray(value: unknown): string[] {
  if (typeof value === "string") return [value];
  return asArray(value).filter(isString);
}

function isString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function unique<T>(items: T[]) {
  return Array.from(new Set(items));
}

function dedupeBy<T>(items: T[], key: (item: T) => string) {
  const seen = new Set<string>();
  const result: T[] = [];
  for (const item of items) {
    const id = key(item);
    if (seen.has(id)) continue;
    seen.add(id);
    result.push(item);
  }
  return result;
}

function excerpt(value: string, maxLength: number) {
  const compact = value.replace(/\s+/g, " ").trim();
  return compact.length > maxLength ? `${compact.slice(0, maxLength)}...` : compact;
}

function firstNonEmpty(...values: Array<string | undefined>) {
  return values.find((value) => value?.trim()) ?? "";
}

function lastSegment(value: string) {
  return value.split("::").at(-1) ?? value;
}

function stableId(prefix: string, ...parts: string[]) {
  const input = parts.join("\u0000");
  let hash = 2166136261;
  for (let index = 0; index < input.length; index += 1) {
    hash ^= input.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return `${prefix}_${(hash >>> 0).toString(16)}`;
}
