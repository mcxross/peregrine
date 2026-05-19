import type {
  AgentActionRequest,
  AgentContextPacket,
  AgentDiagnostic,
  AgentGenerateRequest,
  AgentGenerateResult,
  AgentRuntimeToolResult,
  AllowedAction,
  ApprovalPolicySnapshot,
  DeterministicToolSpec,
  EvidenceCandidate,
  EvidenceKind,
  EvidenceRef,
  FindingRef,
  GuideRef,
  JsonRecord,
  OutputContract,
  ProjectSummary,
  RiskLevel,
  ToolGateway,
  ToolRunSummary,
} from "@peregrine/agent-runtime";
import type { LanguageModel } from "ai";

export type SessionStatus =
  | "created"
  | "indexing"
  | "ready"
  | "running"
  | "waitingForApproval"
  | "completed"
  | "failed";

export type PolicyDisposition =
  | "allowed"
  | "approvalRequired"
  | "forbidden";

export interface SecurityProfile {
  id: string;
  title: string;
  chain?: string;
  guideIds?: string[];
  metadata?: JsonRecord;
}

export interface AssessmentSession {
  id: string;
  projectPath: string;
  targetChain?: string;
  profile: SecurityProfile;
  status: SessionStatus;
  createdAt: string;
  updatedAt: string;
  findings: FindingRef[];
  toolRuns: ToolRunSummary[];
  approvals: ApprovalDecision[];
  evidenceRefs: EvidenceRef[];
  metadata?: JsonRecord;
}

export interface CreateSessionRequest {
  projectPath: string;
  profile: SecurityProfile;
  targetChain?: string;
  metadata?: JsonRecord;
}

export interface ApprovalEvaluation {
  disposition: PolicyDisposition;
  reason: string;
  risk: RiskLevel;
  diagnostics?: AgentDiagnostic[];
}

export interface ApprovalRequest {
  id: string;
  sessionId?: string;
  taskId: string;
  toolId?: string;
  action: AgentActionRequest;
  reason: string;
  filesAffected: string[];
  networkDomains: string[];
  diffPreview?: string;
  expectedChecks: string[];
  risk: RiskLevel;
  createdAt: string;
}

export interface ApprovalDecision {
  requestId: string;
  status: "approved" | "denied";
  decidedAt: string;
  decidedBy?: string;
  rationale?: string;
}

export interface ApprovalGate {
  requestApproval(request: ApprovalRequest): Promise<ApprovalDecision>;
}

export interface ApprovalPolicy {
  evaluateAction(
    action: AgentActionRequest,
    context: {
      sessionId?: string;
      taskId: string;
      toolId?: string;
    },
  ): ApprovalEvaluation;
  snapshot(): ApprovalPolicySnapshot;
}

export interface EvidenceRecord {
  id: string;
  kind: EvidenceKind;
  source: string;
  summary: string;
  rawPath?: string;
  contentHash: string;
  createdAt: string;
  metadata?: JsonRecord;
}

export interface EvidenceStore {
  record(candidate: EvidenceCandidate): Promise<EvidenceRecord>;
  get(id: string): EvidenceRecord | undefined;
  list(): EvidenceRecord[];
}

export interface ToolRegistry {
  register(tool: DeterministicToolSpec): void;
  get(toolId: string): DeterministicToolSpec | undefined;
  list(): DeterministicToolSpec[];
}

export interface SessionStore {
  create(request: CreateSessionRequest): AssessmentSession;
  get(sessionId: string): AssessmentSession | undefined;
  update(session: AssessmentSession): void;
  list(): AssessmentSession[];
}

export interface HarnessToolRuntimeConfig {
  policy: ApprovalPolicy;
  approvalGate: ApprovalGate;
  evidenceStore: EvidenceStore;
  now?: () => Date;
  onToolRun?: (
    toolRun: ToolRunSummary,
    context: { sessionId?: string; taskId: string },
  ) => void;
  onApprovalDecision?: (
    request: ApprovalRequest,
    decision: ApprovalDecision,
  ) => void;
}

export interface ContextPacketInput {
  session: AssessmentSession;
  task: AgentContextPacket["task"];
  developerIntent: string;
  projectSummary: ProjectSummary;
  selectedCode?: AgentContextPacket["selectedCode"];
  riskSignals?: AgentContextPacket["riskSignals"];
  guides?: GuideRef[];
  currentFindings?: FindingRef[];
  recentToolResults?: ToolRunSummary[];
  allowedActions?: AllowedAction[];
  outputContract: OutputContract;
  tokenBudget?: AgentContextPacket["tokenBudget"];
}

export interface PeregrineHarnessConfig {
  model: LanguageModel;
  policy?: ApprovalPolicy;
  approvalGate?: ApprovalGate;
  evidenceStore?: EvidenceStore;
  sessionStore?: SessionStore;
  toolRegistry?: ToolRegistry;
  toolRuntime?: ToolGateway;
  maxAgentSteps?: number;
}

export interface RunAgentTaskRequest
  extends Omit<AgentGenerateRequest, "sessionId" | "packet"> {
  sessionId: string;
  packet: AgentContextPacket;
}

export interface RunAgentTaskResult extends AgentGenerateResult {
  evidenceRef: EvidenceRef;
}

export interface RunToolRequest<Input = unknown> {
  sessionId: string;
  taskId: string;
  toolId: string;
  toolCallId?: string;
  input: Input;
  metadata?: JsonRecord;
  abortSignal?: AbortSignal;
}

export type RunToolResult<Output = unknown> = AgentRuntimeToolResult<Output>;
