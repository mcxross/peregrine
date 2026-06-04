export type AgentStatus =
  | "active"
  | "idle"
  | "running"
  | "blocked"
  | "needsApproval"
  | "failed"
  | "completed";
export type AgentKind = "server";
export type ProviderScope = "cloud" | "local";
export type ExecutionMode = "manual" | "approvalGated" | "background";

export type AgentWorkflowNodeType =
  | "trigger"
  | "agent"
  | "tool"
  | "condition"
  | "model"
  | "memory"
  | "input"
  | "output"
  | "integration";

export type AgentProviderConfig = {
  endpoint?: string;
  modelId: string;
  providerId: string;
};

export type AgentExecutionConfig = {
  mode: ExecutionMode;
  maxSteps: number;
  requireToolApproval: boolean;
  persistMemory: boolean;
};

export type AgentDefinition = {
  id: string;
  kind: AgentKind;
  isClosed?: boolean;
  isPrimary?: boolean;
  roleName?: string;
  serverThreadId?: string;
  name: string;
  description: string;
  systemPrompt: string;
  tools: string[];
  provider: AgentProviderConfig;
  execution: AgentExecutionConfig;
  status: AgentStatus;
  workflowId: string;
  updatedAt: number;
};

export type AgentWorkflowNodeData = {
  label: string;
  description: string;
  nodeType: AgentWorkflowNodeType;
  status: AgentStatus;
  provider?: AgentProviderConfig;
  toolId?: string;
};

export type AgentWorkflowNode = {
  data: AgentWorkflowNodeData;
  id: string;
  position: {
    x: number;
    y: number;
  };
  type?: string;
};

export type AgentWorkflowEdge = {
  animated?: boolean;
  id: string;
  source: string;
  target: string;
  type?: string;
};

export type AgentWorkflow = {
  id: string;
  name: string;
  description: string;
  version: number;
  updatedAt: number;
  nodes: AgentWorkflowNode[];
  edges: AgentWorkflowEdge[];
};

export type AgentExecutionLog = {
  id: string;
  agentId: string;
  workflowId: string;
  nodeId?: string;
  level: "info" | "warning" | "error" | "trace";
  message: string;
  timestamp: number;
};

export type AgentStudioState = {
  agents: AgentDefinition[];
  workflows: AgentWorkflow[];
  logs: AgentExecutionLog[];
  selectedAgentId: string;
  selectedWorkflowId: string;
};

export type AgentToolProjectContext = {
  rootPath: string;
  packagePath: string;
  packageName: string;
  manifestPath: string;
  packageTree: unknown;
};

export type ModelProviderDescriptor = {
  id: string;
  label: string;
  scope: ProviderScope;
  defaultEndpoint?: string;
  defaultModelId?: string;
  supportsTools: boolean;
  supportsLocalModels: boolean;
};
