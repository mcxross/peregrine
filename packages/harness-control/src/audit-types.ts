import type {
  EvidenceRef,
  FindingCandidate,
  FindingCandidateSeverity,
  JsonRecord,
  RiskLevel,
} from "@peregrine/agent-runtime";

export const AUDIT_TRACE_FILENAMES = {
  auditSession: "audit-session.json",
  projectIndex: "project-index.json",
  knowledgeGraph: "knowledge-graph.json",
  classification: "classification.json",
  threatModel: "threat-model.json",
  functionRiskMap: "function-risk-map.json",
  invariants: "invariants.json",
  staticFindings: "static-findings.json",
  graphEvidence: "graph-evidence.json",
  bytecodeReview: "bytecode-review.json",
  attackHypotheses: "attack-hypotheses.json",
  testPlan: "test-plan.json",
  dynamicResults: "dynamic-results.json",
  invariantStress: "invariant-stress.json",
  confirmedFindings: "confirmed-findings.json",
  severityRanking: "severity-ranking.json",
  remediationPlan: "remediation-plan.json",
  regressionTests: "regression-tests.json",
  auditReport: "audit-report.json",
  auditTrace: "audit-trace.json",
  fixVerification: "fix-verification.json",
} as const;

export type AuditTraceArtifactName = keyof typeof AUDIT_TRACE_FILENAMES;

export type AuditStageId =
  | "auditSession"
  | "buildNormalize"
  | "semanticGraphs"
  | "classification"
  | "threatModel"
  | "functionRiskMap"
  | "invariants"
  | "staticAnalysis"
  | "graphAnalysis"
  | "bytecodeReview"
  | "attackHypotheses"
  | "targetedTests"
  | "dynamicAnalysis"
  | "invariantStress"
  | "exploitConfirmation"
  | "severityRanking"
  | "remediation"
  | "regressionTests"
  | "auditReport"
  | "auditTrace"
  | "fixVerification";

export const AUDIT_STAGE_SEQUENCE: AuditStageId[] = [
  "auditSession",
  "buildNormalize",
  "semanticGraphs",
  "classification",
  "threatModel",
  "functionRiskMap",
  "invariants",
  "staticAnalysis",
  "graphAnalysis",
  "bytecodeReview",
  "attackHypotheses",
  "targetedTests",
  "dynamicAnalysis",
  "invariantStress",
  "exploitConfirmation",
  "severityRanking",
  "remediation",
  "regressionTests",
  "auditReport",
  "auditTrace",
  "fixVerification",
];

export type AuditStageStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "skipped";

export type AuditFindingState =
  | "confirmed"
  | "likely"
  | "possible"
  | "falsePositive"
  | "informational"
  | "needsHumanReview";

export type AuditFixState =
  | "open"
  | "fixed"
  | "partiallyFixed"
  | "regressionAdded"
  | "riskAccepted"
  | "falsePositive"
  | "needsReview";

export interface AuditStageRun {
  id: string;
  stageId: AuditStageId;
  status: AuditStageStatus;
  startedAt: string;
  completedAt?: string;
  summary: string;
  artifactName?: AuditTraceArtifactName;
  filename?: string;
  evidenceRef?: EvidenceRef;
  diagnostics?: Array<{
    level: "info" | "warning" | "error";
    message: string;
    source: string;
  }>;
  metadata?: JsonRecord;
}

export interface AuditSessionPacket {
  schemaVersion: 1;
  id: string;
  project: string;
  repoRoot: string;
  commit: string;
  packageManifest: string;
  targetModules: string[];
  compilerVersion?: string | null;
  dependencyGraph?: unknown;
  selectedChainAdapter: "sui/move";
  enabledTools: string[];
  auditProfile: string;
  threatModelProfile: string;
  timestamp: string;
  toolVersions: Record<string, string>;
  policyProfile?: string;
  metadata?: JsonRecord;
}

export interface CanonicalProjectIndex {
  schemaVersion: 1;
  auditSessionId: string;
  repoRoot: string;
  packageId?: string | null;
  packageName: string;
  packageManifest: string;
  targetModules: string[];
  compilerVersion?: string | null;
  build: {
    status: "passed" | "failed" | "skipped" | "unknown";
    stdoutExcerpt?: string;
    stderrExcerpt?: string;
    diagnostics: string[];
  };
  indexer?: {
    runId?: string;
    packageId?: string;
    dbPath?: string;
    status?: string;
    health?: unknown;
    layers?: unknown[];
  };
  modules: CanonicalModule[];
  structs: CanonicalStruct[];
  functions: CanonicalFunction[];
  artifacts: CanonicalArtifact[];
  diagnostics: string[];
  sourcePrecision: "compiler" | "sourceMap" | "bytecode" | "heuristic" | "unknown";
  metadata?: JsonRecord;
}

export interface CanonicalModule {
  id: string;
  name: string;
  address?: string | null;
  filePath?: string | null;
  attributes: string[];
}

export interface CanonicalStruct {
  id: string;
  moduleName: string;
  name: string;
  qualifiedName: string;
  abilities: string[];
  fields: Array<{ name: string; typeName: string }>;
  filePath?: string | null;
  isCapabilityLike: boolean;
  isObjectLike: boolean;
}

export interface CanonicalFunction {
  id: string;
  moduleName: string;
  name: string;
  qualifiedName: string;
  visibility: string;
  isEntry: boolean;
  isTransactionCallable: boolean;
  parameters: string[];
  returns: string[];
  attributes: string[];
  filePath?: string | null;
  signature?: string | null;
}

export interface CanonicalArtifact {
  kind: string;
  path?: string | null;
  summary: string;
  metadata?: JsonRecord;
}

export interface AuditKnowledgeGraph {
  schemaVersion: 1;
  auditSessionId: string;
  callGraph: GraphSummary;
  typeGraph: GraphSummary;
  objectLifecycleGraph: GraphSummary;
  capabilityGraph: GraphSummary;
  assetFlowGraph: GraphSummary;
  storageAccessGraph: GraphSummary;
  privilegeGraph: GraphSummary;
  externalInteractionGraph: GraphSummary;
  graphQueries: GraphEvidenceTrail[];
  unresolvedEdges: Array<{ graph: string; count: number; examples: unknown[] }>;
  sourcePrecision: "compiler" | "sourceMap" | "bytecode" | "heuristic" | "unknown";
  metadata?: JsonRecord;
}

export interface GraphSummary {
  nodes: number;
  edges: number;
  highValueNodes: string[];
  evidence: string[];
  raw?: unknown;
}

export interface GraphEvidenceTrail {
  id: string;
  title: string;
  query: string;
  severity: RiskLevel;
  paths: string[][];
  observations: string[];
  evidenceSources: string[];
}

export interface ContractClassificationReport {
  schemaVersion: 1;
  auditSessionId: string;
  profiles: string[];
  protocolProfile: ProtocolProfile;
  confidence: "high" | "medium" | "low";
  evidence: string[];
}

export interface ProtocolProfile {
  assetHolding: boolean;
  hasAdmin: boolean;
  usesOracle: boolean;
  usesSharedObjects: boolean;
  usesDynamicFields: boolean;
  hasUpgradeAuthority: boolean;
  hasLiquidation: boolean;
  hasExternalCalls: boolean;
  hasCapabilityObjects: boolean;
}

export interface ThreatModelPacket {
  schemaVersion: 1;
  auditSessionId: string;
  assetsAtRisk: string[];
  actors: string[];
  privilegedRoles: string[];
  trustedModules: string[];
  untrustedInputs: string[];
  entryPoints: string[];
  attackSurfaces: string[];
  economicAssumptions: string[];
  oracleAssumptions: string[];
  upgradeAssumptions: string[];
  criticalInvariants: string[];
  evidence: string[];
}

export interface FunctionRiskMap {
  schemaVersion: 1;
  auditSessionId: string;
  functions: FunctionRiskEntry[];
  summary: Record<RiskLevel, number>;
}

export interface FunctionRiskEntry {
  functionId: string;
  qualifiedName: string;
  moduleName: string;
  functionName: string;
  isEntry: boolean;
  isTransactionCallable: boolean;
  risk: RiskLevel;
  score: number;
  reasons: string[];
  tags: string[];
  evidence: string[];
}

export interface InvariantRegistry {
  schemaVersion: 1;
  auditSessionId: string;
  invariants: AuditInvariant[];
}

export interface AuditInvariant {
  id: string;
  description: string;
  invariantClass: string;
  targetModule?: string;
  targetFunction?: string;
  severityIfBroken: FindingCandidateSeverity;
  evidenceSources: string[];
  validationMethods: string[];
  status: "candidate" | "checked" | "violated" | "needsHumanReview";
}

export interface StaticFindingsSet {
  schemaVersion: 1;
  auditSessionId: string;
  findings: AuditFindingCandidate[];
  diagnostics: string[];
  coverage: string[];
}

export interface GraphEvidencePacket {
  schemaVersion: 1;
  auditSessionId: string;
  trails: GraphEvidenceTrail[];
  suspiciousRelationships: AuditFindingCandidate[];
}

export interface BytecodeReviewPacket {
  schemaVersion: 1;
  auditSessionId: string;
  reviewed: boolean;
  modules: number;
  functions: number;
  instructionCount: number;
  sensitiveInstructions: Array<{
    moduleName?: string;
    functionName?: string;
    opcode: string;
    detail?: string;
    source?: unknown;
  }>;
  sourceConsistency: Array<{
    functionName: string;
    status: "consistent" | "mismatch" | "unknown";
    evidence: string[];
  }>;
  diagnostics: string[];
}

export interface AttackHypothesisSet {
  schemaVersion: 1;
  auditSessionId: string;
  hypotheses: AttackHypothesis[];
}

export interface AttackHypothesis {
  id: string;
  claim: string;
  targetFunction?: string;
  requiredActor: string;
  requiredState: string;
  affectedAsset?: string;
  supportingGraphPath: string[];
  suggestedTest: string;
  severityIfTrue: FindingCandidateSeverity;
  evidenceRefs: string[];
  status: "untested" | "supported" | "rejected" | "needsHumanReview";
}

export interface TestPlanPacket {
  schemaVersion: 1;
  auditSessionId: string;
  tests: AuditTestCase[];
}

export interface AuditTestCase {
  id: string;
  hypothesisId?: string;
  invariantId?: string;
  category:
    | "unit"
    | "negative"
    | "authorization"
    | "stateMachine"
    | "invariant"
    | "property"
    | "fuzz"
    | "scenario"
    | "regression";
  setup: string;
  action: string;
  expectedResult: string;
  tool: string;
  status: "planned" | "generated" | "executed" | "failed" | "passed" | "blocked";
  evidence: string[];
}

export interface DynamicEvidencePacket {
  schemaVersion: 1;
  auditSessionId: string;
  testResults: AuditExecutionResult[];
  simulations: AuditExecutionResult[];
  stateDiffs: AuditExecutionResult[];
  diagnostics: string[];
}

export interface InvariantStressReport {
  schemaVersion: 1;
  auditSessionId: string;
  fuzzResults: AuditExecutionResult[];
  invariantResults: AuditExecutionResult[];
  seeds: Array<{ target: string; seed?: number; status: string; evidence: string[] }>;
  diagnostics: string[];
}

export interface AuditExecutionResult {
  id: string;
  target: string;
  tool: string;
  status: "passed" | "failed" | "skipped" | "blocked" | "unknown";
  expected: string;
  observed: string;
  evidence: string[];
  confidence: "confirmed" | "high" | "medium" | "low" | "unknown";
}

export interface ConfirmedFindingsSet {
  schemaVersion: 1;
  auditSessionId: string;
  findings: AuditFindingCandidate[];
}

export interface AuditFindingCandidate {
  id: string;
  title: string;
  category: string;
  state: AuditFindingState;
  severity: FindingCandidateSeverity;
  confidence: "confirmed" | "high" | "medium" | "low" | "unknown";
  affectedSymbols: string[];
  affectedInvariantIds: string[];
  evidenceChain: string[];
  proofPath: string[];
  testCaseIds: string[];
  assetImpact?: string;
  recommendation?: string;
  metadata?: JsonRecord;
}

export interface SeverityRankedFindingList {
  schemaVersion: 1;
  auditSessionId: string;
  findings: Array<
    AuditFindingCandidate & {
      severityScore: SeverityScore;
    }
  >;
}

export interface SeverityScore {
  assetImpact: number;
  privilegeRequired: number;
  attackComplexity: number;
  affectedUsers: number;
  repeatability: number;
  protocolStateImpact: number;
  recoverability: number;
  detectability: number;
  economicImpact: number;
  total: number;
}

export interface RemediationPlan {
  schemaVersion: 1;
  auditSessionId: string;
  remediations: AuditRemediation[];
}

export interface AuditRemediation {
  findingId: string;
  rootCause: string;
  affectedFunction?: string;
  minimalFix: string;
  saferRedesign: string;
  testToAdd: string;
  regressionInvariant: string;
}

export interface RegressionTestPacket {
  schemaVersion: 1;
  auditSessionId: string;
  tests: Array<
    AuditTestCase & {
      findingId: string;
      draftPath: string;
      beforeFixExpected: string;
      afterFixExpected: string;
    }
  >;
}

export interface AuditReport {
  schemaVersion: 1;
  auditSessionId: string;
  markdown: string;
  findingCount: number;
  confirmedFindingCount: number;
  evidenceCompleteness: "complete" | "partial" | "blocked";
}

export interface FixVerificationPacket {
  schemaVersion: 1;
  auditSessionId: string;
  changedFiles: string[];
  rerunStages: AuditStageId[];
  findingStatuses: Array<{
    findingId: string;
    previousState: AuditFixState;
    nextState: AuditFixState;
    evidence: string[];
  }>;
  diagnostics: string[];
}

export interface AuditTrace {
  schemaVersion: 1;
  auditSessionId: string;
  generatedAt: string;
  artifacts: Array<{
    name: AuditTraceArtifactName;
    filename: string;
    evidenceRef?: EvidenceRef;
    summary: string;
  }>;
  stageRuns: AuditStageRun[];
  findings: AuditFindingCandidate[];
  metadata?: JsonRecord;
}

export interface AuditPacketBundle {
  auditSession?: AuditSessionPacket;
  projectIndex?: CanonicalProjectIndex;
  knowledgeGraph?: AuditKnowledgeGraph;
  classification?: ContractClassificationReport;
  threatModel?: ThreatModelPacket;
  functionRiskMap?: FunctionRiskMap;
  invariants?: InvariantRegistry;
  staticFindings?: StaticFindingsSet;
  graphEvidence?: GraphEvidencePacket;
  bytecodeReview?: BytecodeReviewPacket;
  attackHypotheses?: AttackHypothesisSet;
  testPlan?: TestPlanPacket;
  dynamicResults?: DynamicEvidencePacket;
  invariantStress?: InvariantStressReport;
  confirmedFindings?: ConfirmedFindingsSet;
  severityRanking?: SeverityRankedFindingList;
  remediationPlan?: RemediationPlan;
  regressionTests?: RegressionTestPacket;
  auditReport?: AuditReport;
  fixVerification?: FixVerificationPacket;
  auditTrace?: AuditTrace;
}

export function toFindingCandidate(finding: AuditFindingCandidate): FindingCandidate {
  return {
    id: finding.id,
    title: finding.title,
    category: finding.category,
    severity: finding.severity,
    confidence: finding.confidence,
    status: findingStateToCandidateStatus(finding.state),
    affectedSymbols: finding.affectedSymbols,
    evidenceRefs: finding.evidenceChain,
    validationPlan: {
      commands: ["sui move test", "peregrine audit --verify"],
      expectedEvidence: ["Validation produces exploit proof, mitigation proof, accepted risk, or a documented false positive."],
      required: finding.state !== "informational",
    },
    metadata: {
      ...finding.metadata,
      affectedInvariantIds: finding.affectedInvariantIds,
      proofPath: finding.proofPath,
      testCaseIds: finding.testCaseIds,
      assetImpact: finding.assetImpact,
    },
    patchRecommendation: finding.recommendation
      ? {
          summary: finding.recommendation,
          affectedLocations: [],
          minimalChange: finding.recommendation,
          regressionTests: finding.testCaseIds,
          verificationCommands: ["sui move test"],
        }
      : undefined,
  };
}

function findingStateToCandidateStatus(state: AuditFindingState): FindingCandidate["status"] {
  switch (state) {
    case "confirmed":
      return "confirmed";
    case "likely":
      return "likely";
    case "possible":
      return "possible";
    case "falsePositive":
      return "falsePositive";
    case "informational":
      return "informational";
    case "needsHumanReview":
      return "needsHumanReview";
    default:
      return "needsValidation";
  }
}
