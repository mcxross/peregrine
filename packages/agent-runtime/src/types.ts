import type {
  GenerateTextResult,
  JSONValue,
  LanguageModel,
  ModelMessage,
  StopCondition,
  StreamTextResult,
  Tool,
  ToolChoice,
} from "ai";

export type JsonRecord = Record<string, JSONValue | undefined>;

export type JsonSchemaDefinition = {
  readonly [key: string]: unknown;
};

export type AgentRole =
  | "securityReview"
  | "testGeneration"
  | "fuzzCampaign"
  | "formalSpec"
  | "patch"
  | "report"
  | "explainer"
  | "triage"
  | "ci";

export type ActionClass =
  | "readOnly"
  | "toolExecution"
  | "generatedFileWrite"
  | "sourceCodeModification"
  | "dependencyModification"
  | "packagePublishing"
  | "networkAccess"
  | "secretAccess";

export type RiskLevel = "low" | "medium" | "high" | "critical";

export type EvidenceKind =
  | "toolOutput"
  | "toolFailure"
  | "codeLocation"
  | "testResult"
  | "fuzzCounterexample"
  | "proverResult"
  | "dependencyDiff"
  | "humanApproval"
  | "acceptedRisk"
  | "agentOutput"
  | "diagnostic";

export interface EvidenceRef {
  id: string;
  kind: EvidenceKind;
  summary: string;
  source?: string;
}

export interface EvidenceCandidate {
  kind: EvidenceKind;
  summary: string;
  source: string;
  raw?: unknown;
  rawPath?: string;
  contentHash?: string;
}

export interface AgentDiagnostic {
  level: "info" | "warning" | "error";
  source: string;
  message: string;
  resolution?: string;
}

export interface AllowedAction {
  actionClass: ActionClass;
  description: string;
  target?: string;
  requiresApproval: boolean;
}

export interface ApprovalPolicySnapshot {
  mode: "noAi" | "localAi" | "cloudAiRedacted" | "cloudAiFullContext";
  networkAccess: "forbidden" | "approvalRequired" | "allowed";
  sourceModification: "forbidden" | "approvalRequired" | "allowed";
  dependencyModification: "forbidden" | "approvalRequired" | "allowed";
  secretAccess: "forbidden";
}

export interface OutputContract {
  format: "json" | "markdown" | "text";
  schema?: JsonSchemaDefinition;
  requiredEvidence: boolean;
  description: string;
}

export interface AgentTask {
  id: string;
  role: AgentRole;
  title: string;
  objective: string;
  target?: string;
}

export interface ProjectSummary {
  id: string;
  name: string;
  rootPath: string;
  chain?: string;
  modules?: Array<{
    id: string;
    name: string;
    path?: string;
    summary?: string;
  }>;
  diagnostics?: AgentDiagnostic[];
  metadata?: JsonRecord;
}

export interface GuideRef {
  id: string;
  title: string;
  summary: string;
  content?: string;
  source?: string;
}

export interface FindingRef {
  id: string;
  title: string;
  severity: "critical" | "high" | "medium" | "low" | "info";
  status: "open" | "fixed" | "accepted" | "falsePositive" | "needsReview";
  location?: string;
  evidenceRefs: EvidenceRef[];
}

export interface ToolRunSummary {
  id: string;
  toolId: string;
  status: "succeeded" | "failed" | "denied" | "requiresApproval";
  summary: string;
  evidenceRefs: EvidenceRef[];
  diagnostics?: AgentDiagnostic[];
}

export interface AgentContextPacket {
  task: AgentTask;
  developerIntent: string;
  projectSummary: ProjectSummary;
  securityProfile: string;
  selectedCode: Array<{
    id: string;
    label: string;
    path?: string;
    excerpt?: string;
    evidenceRefs?: EvidenceRef[];
  }>;
  riskSignals: Array<{
    id: string;
    label: string;
    summary: string;
    evidenceRefs: EvidenceRef[];
  }>;
  relevantGuides: GuideRef[];
  currentFindings: FindingRef[];
  recentToolResults: ToolRunSummary[];
  allowedActions: AllowedAction[];
  approvalPolicy: ApprovalPolicySnapshot;
  outputContract: OutputContract;
  tokenBudget?: {
    estimatedTokens: number;
    budget: number;
    trimmed: boolean;
    trimReasons: string[];
  };
}

export interface AgentActionRequest {
  actionClass: ActionClass;
  reason: string;
  risk: RiskLevel;
  toolId?: string;
  files?: string[];
  networkDomains?: string[];
  diffPreview?: string;
  expectedChecks?: string[];
  metadata?: JsonRecord;
}

export interface DeterministicToolExecutionContext {
  sessionId?: string;
  taskId: string;
  toolCallId: string;
  action: AgentActionRequest;
  abortSignal?: AbortSignal;
  messages?: ModelMessage[];
  metadata?: JsonRecord;
}

export interface DeterministicToolExecutionResult<Output = unknown> {
  status?: "succeeded" | "failed";
  output?: Output;
  summary?: string;
  evidence?: EvidenceCandidate[];
  diagnostics?: AgentDiagnostic[];
}

export interface DeterministicToolSpec<Input = unknown, Output = unknown> {
  id: string;
  title?: string;
  version?: string;
  description: string;
  inputSchema: JsonSchemaDefinition;
  outputSchema?: JsonSchemaDefinition;
  action: AgentActionRequest;
  examples?: Array<{ input: Input }>;
  execute: (
    input: Input,
    context: DeterministicToolExecutionContext,
  ) =>
    | Output
    | DeterministicToolExecutionResult<Output>
    | Promise<Output | DeterministicToolExecutionResult<Output>>;
}

export interface AgentRuntimeToolResult<Output = unknown> {
  status: "succeeded" | "failed" | "denied" | "requiresApproval";
  toolId: string;
  toolCallId: string;
  action: AgentActionRequest;
  summary: string;
  output?: Output;
  evidenceRefs: EvidenceRef[];
  diagnostics: AgentDiagnostic[];
}

export interface ToolGatewayRequest<Input = unknown, Output = unknown> {
  tool: DeterministicToolSpec<Input, Output>;
  input: Input;
  toolCallId: string;
  context: Omit<DeterministicToolExecutionContext, "action" | "toolCallId">;
}

export interface ToolGateway {
  runTool<Input = unknown, Output = unknown>(
    request: ToolGatewayRequest<Input, Output>,
  ): Promise<AgentRuntimeToolResult<Output>>;
}

export interface AiSdkToolSet {
  tools: Record<string, Tool>;
  toolNamesById: ReadonlyMap<string, string>;
  toolIdsByName: ReadonlyMap<string, string>;
}

export interface AgentRuntimeConfig {
  model: LanguageModel;
  tools: DeterministicToolSpec[];
  toolGateway: ToolGateway;
  maxSteps?: number;
}

export interface AgentRunOptions {
  sessionId?: string;
  activeToolIds?: string[];
  maxSteps?: number;
  toolChoice?: ToolChoice<Record<string, Tool>>;
  stopWhen?:
    | StopCondition<Record<string, Tool>>
    | Array<StopCondition<Record<string, Tool>>>;
  timeout?: number | { totalMs?: number; stepMs?: number; chunkMs?: number };
  abortSignal?: AbortSignal;
  metadata?: JsonRecord;
}

export interface AgentGenerateRequest extends AgentRunOptions {
  packet: AgentContextPacket;
  prompt?: string;
  messages?: ModelMessage[];
}

export interface AgentStreamRequest extends AgentRunOptions {
  packet: AgentContextPacket;
  prompt?: string;
  messages?: ModelMessage[];
}

export interface AgentGenerateResult {
  packet: AgentContextPacket;
  text: string;
  result: GenerateTextResult<Record<string, Tool>, never>;
}

export interface AgentStreamResult {
  packet: AgentContextPacket;
  result: StreamTextResult<Record<string, Tool>, never>;
}
