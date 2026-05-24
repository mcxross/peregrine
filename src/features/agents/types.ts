import type { Edge, Node } from "@xyflow/react";

export type AgentStatus =
  | "active"
  | "idle"
  | "running"
  | "blocked"
  | "needsApproval"
  | "failed"
  | "completed";
export type AgentKind = "default" | "custom";
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

export type AgentWorkflowNode = Node<AgentWorkflowNodeData>;
export type AgentWorkflowEdge = Edge;

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

export type AuditReportExport = {
  auditSessionId: string;
  defaultFileName: string;
  generatedAt: string;
  markdown: string;
  packageName: string;
  projectName: string;
  reportJson: string;
  traceJson?: string;
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
