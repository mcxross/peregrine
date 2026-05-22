import type {
  AgentDefinition,
  AgentExecutionConfig,
  AgentWorkflow,
  AgentWorkflowNodeType,
} from "@/features/agents/types";

const defaultExecution: AgentExecutionConfig = {
  mode: "approvalGated",
  maxSteps: 12,
  requireToolApproval: true,
  persistMemory: false,
};

const defaultProvider = {
  providerId: "ollama",
  modelId: "",
  endpoint: "http://127.0.0.1:11434",
};

const now = 1_779_062_400_000;

export const defaultAgents: AgentDefinition[] = [
  {
    id: "agent-orchestrator",
    kind: "default",
    name: "Orchestrator Agent",
    description: "Plans runs, coordinates agents, and synthesizes results.",
    systemPrompt:
      "Plan Peregrine security workflows. Coordinate specialist agents and summarize only evidence-backed results.",
    tools: [
      "rust.index.package",
      "rust.static.scan_package",
      "rust.graph.object_lifecycle",
      "rust.bytecode.disassemble",
      "rust.dynamic.run_test",
      "rust.validation.run_suite",
      "rust.findings.triage",
      "rust.report.synthesize",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "active",
    workflowId: "workflow-orchestrator",
    updatedAt: now,
  },
  {
    id: "agent-static-analysis",
    kind: "default",
    name: "Static Analysis Agent",
    description: "Runs source-level analyzers and detects vulnerability patterns.",
    systemPrompt:
      "Run source-level Peregrine analysis. Ground every finding in static tool output and concrete code locations.",
    tools: [
      "rust.static.scan_package",
      "rust.static.inspect_function",
      "rust.static.find_capabilities",
      "rust.graph.call_graph.read",
      "rust.index.read_symbols",
      "rust.findings.emit",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "active",
    workflowId: "workflow-static-analysis",
    updatedAt: now,
  },
  {
    id: "agent-dynamic-analysis",
    kind: "default",
    name: "Dynamic Analysis Agent",
    description: "Executes tests, fuzzing, traces, and simulations to validate findings.",
    systemPrompt:
      "Execute targeted validation. Prefer reproducible tests, traces, and state diffs over speculative conclusions.",
    tools: [
      "rust.dynamic.run_test",
      "rust.dynamic.trace_execution",
      "rust.dynamic.fuzz_function",
      "rust.dynamic.state_diff",
      "rust.test.generate_case",
      "rust.findings.attach_trace",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-dynamic-analysis",
    updatedAt: now,
  },
  {
    id: "agent-graph-reasoning",
    kind: "default",
    name: "Graph Reasoning Agent",
    description: "Builds and interprets lifecycle, CFG, call, and capability graphs.",
    systemPrompt:
      "Interpret Peregrine graph output. Explain object lifecycle, control flow, call graph, and capability flow evidence.",
    tools: [
      "rust.graph.object_lifecycle",
      "rust.graph.cfg",
      "rust.graph.call_graph",
      "rust.graph.capability_flow",
      "rust.graph.finding_impact",
      "rust.graph.path_query",
      "rust.findings.attach_graph",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "active",
    workflowId: "workflow-graph-reasoning",
    updatedAt: now,
  },
  {
    id: "agent-bytecode",
    kind: "default",
    name: "Bytecode Agent",
    description: "Inspects compiled Move bytecode and control flow.",
    systemPrompt:
      "Inspect compiled Move bytecode. Use bytecode control flow, source maps, and stack effects as evidence.",
    tools: [
      "rust.bytecode.disassemble",
      "rust.bytecode.cfg",
      "rust.bytecode.stack_effects",
      "rust.bytecode.source_map",
      "rust.findings.attach_bytecode",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-bytecode",
    updatedAt: now,
  },
  {
    id: "agent-invariant",
    kind: "default",
    name: "Invariant Agent",
    description: "Infers and checks invariants across modules and objects.",
    systemPrompt:
      "Infer candidate invariants from code, object state, and graph evidence. Mark unsupported invariants as hypotheses.",
    tools: [
      "rust.invariant.infer",
      "rust.invariant.check",
      "rust.validation.assert_property",
      "rust.findings.emit",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-invariant",
    updatedAt: now,
  },
  {
    id: "agent-patch",
    kind: "default",
    name: "Patch Agent",
    description: "Proposes minimal, safe code changes to fix issues.",
    systemPrompt:
      "Propose minimal Move changes only after findings have evidence. Preserve behavior outside the issue scope.",
    tools: [
      "rust.patch.suggest",
      "rust.patch.apply_preview",
      "rust.findings.link_patch",
    ],
    provider: defaultProvider,
    execution: {
      ...defaultExecution,
      requireToolApproval: true,
    },
    status: "idle",
    workflowId: "workflow-patch",
    updatedAt: now,
  },
  {
    id: "agent-test-generation",
    kind: "default",
    name: "Test Generation Agent",
    description: "Generates regression tests, scenarios, and validation suites.",
    systemPrompt:
      "Generate regression tests and validation scenarios tied to concrete findings, code paths, and expected behavior.",
    tools: [
      "rust.test.generate_case",
      "rust.dynamic.run_test",
      "rust.dynamic.fuzz_function",
      "rust.validation.run_suite",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-test-generation",
    updatedAt: now,
  },
  {
    id: "agent-report",
    kind: "default",
    name: "Report Agent",
    description: "Produces human-readable audit reports and summaries.",
    systemPrompt:
      "Produce concise audit reports. Separate confirmed findings, evidence, residual risk, and next actions.",
    tools: [
      "rust.report.generate",
      "rust.report.export_markdown",
    ],
    provider: defaultProvider,
    execution: defaultExecution,
    status: "idle",
    workflowId: "workflow-report",
    updatedAt: now,
  },
];

export const defaultWorkflows: AgentWorkflow[] = defaultAgents.map((agent, index) =>
  createAgentWorkflow({
    id: agent.workflowId,
    agentName: agent.name,
    description: agent.description,
    offset: index * 16,
    providerId: agent.provider.providerId,
    modelId: agent.provider.modelId,
  }),
);

export function createAgentWorkflow({
  agentName,
  description,
  id,
  modelId,
  offset = 0,
  providerId,
}: {
  agentName: string;
  description: string;
  id: string;
  modelId: string;
  offset?: number;
  providerId: string;
}) {
  const node = (
    nodeType: AgentWorkflowNodeType,
    label: string,
    x: number,
    y: number,
    nodeDescription = "",
  ) => ({
    id: `${id}-${nodeType}`,
    type: "agentWorkflow",
    position: { x: x + offset, y },
    data: {
      label,
      description: nodeDescription,
      nodeType,
      status: "idle" as const,
      provider:
        nodeType === "agent" || nodeType === "model"
          ? { providerId, modelId }
          : undefined,
    },
  });

  const nodes = [
    node("trigger", "Manual trigger", 40, 90, "Starts the workflow."),
    node("input", "Evidence packet", 230, 90, "Loads bounded project context and prior tool output."),
    node("agent", agentName, 440, 90, description),
    node("tool", "Rust tool gateway", 665, 20, "Runs allowed Peregrine tools."),
    node("condition", "Evidence gate", 665, 160, "Checks evidence completeness."),
    node("output", "Run summary", 900, 90, "Stores trace, evidence, and output."),
  ];

  return {
    id,
    name: agentName,
    description,
    version: 1,
    updatedAt: now,
    nodes,
    edges: [
      edge(id, "trigger", "input"),
      edge(id, "input", "agent"),
      edge(id, "agent", "tool"),
      edge(id, "agent", "condition"),
      edge(id, "tool", "output"),
      edge(id, "condition", "output"),
    ],
  };
}

function edge(workflowId: string, source: AgentWorkflowNodeType, target: AgentWorkflowNodeType) {
  return {
    id: `${workflowId}-${source}-${target}`,
    source: `${workflowId}-${source}`,
    target: `${workflowId}-${target}`,
    animated: source === "agent",
    type: "smoothstep",
  };
}
