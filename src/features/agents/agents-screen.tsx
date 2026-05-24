import React from "react";
import type {
  AgentRole,
  FindingCandidate,
  SecurityEvidenceItem,
  ToolCapsule,
  ToolRunSummary,
} from "@peregrine/agent-runtime";
import { routeTools, type ToolRouteDecision } from "@peregrine/harness-control";
import {
  Activity,
  AlertTriangle,
  BarChart3,
  Binary,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleDot,
  Clock3,
  FileText,
  Hammer,
  MessageSquareText,
  MoreHorizontal,
  Network,
  Play,
  Plus,
  ShieldCheck,
  Square,
  Terminal,
  Workflow,
  Wrench,
  X,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import {
  agentStudioStateToProjectMetadata,
  createCustomAgent,
  createExecutionLog,
  duplicateAgent,
  loadAgentStudioState,
  loadAgentStudioStateFromProjectMetadata,
  saveAgentStudioState,
} from "@/features/agents/agent-workflow-store";
import { AgentWorkflowCanvas } from "@/features/agents/agent-workflow-canvas";
import {
  loadProviderModelOptions,
  modelProviderAdapters,
  providerById,
} from "@/features/agents/model-providers/provider-adapters";
import type {
  AgentDefinition,
  AgentExecutionLog,
  AgentProviderConfig,
  AgentStatus,
  AgentStudioState,
  AgentWorkflow,
  AuditReportExport,
} from "@/features/agents/types";
import {
  displayMovePackageName,
  loadProjectMetadata,
  saveProjectMetadata,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import {
  createAgentToolRuntimeState,
  resolveAgentTools,
  type AgentToolProjectContext,
} from "@/features/agents/tools";
import type { AgentRunStreamEvent } from "@/features/agents/agent-runner";
import { cn } from "@/lib/utils";

type AgentCategory = "Core" | "Analysis" | "Action" | "Output" | "Custom";
type AgentFilter = "all" | AgentCategory;
type MainTab = "agents" | "runs" | "activity" | "details";
type InspectorTab = "overview" | "tools" | "permissions" | "runs";

type AgentUiMetadata = {
  capabilities: string[];
  category: AgentCategory;
};

type RecommendedWorkflow = {
  description: string;
  id: string;
  steps: number;
  title: string;
};

type ActivityRow = {
  agent: string;
  event: string;
  level: AgentExecutionLog["level"];
  timestamp: string;
  tool: string;
};

type RunSnapshot = {
  activeTool: string;
  durationLabel: string;
  evidenceArtifacts: string[];
  issueCount: number;
  latestError?: string;
  startedLabel: string;
  status: "Idle" | "Running" | "Completed" | "Blocked" | "Stopped";
  steps: Array<{
    label: string;
    state: "active" | "blocked" | "done" | "pending";
  }>;
  warningCount: number;
};

type AgentRunDetailStatus = "idle" | "running" | "completed" | "blocked" | "stopped";

type AgentRunDetailEvent = {
  id: string;
  kind: "status" | "model" | "reasoning" | "tool" | "error" | "trace";
  level: AgentExecutionLog["level"];
  message: string;
  timestamp: number;
  title: string;
};

type AgentRunDetail = {
  agentId: string;
  completedAt?: number;
  displayName: string;
  evidence: SecurityEvidenceItem[];
  events: AgentRunDetailEvent[];
  findingCandidates: FindingCandidate[];
  id: string;
  reasoningText: string;
  responseText: string;
  routeDecisions: ToolRouteDecision[];
  startedAt: number;
  status: AgentRunDetailStatus;
  toolCapsules: ToolCapsule[];
  toolRuns: ToolRunSummary[];
  workflowId: string;
  workflowName: string;
};

type HarnessRoutePreview = {
  capsules: ToolCapsule[];
  decisions: ToolRouteDecision[];
};

const AGENT_FILTERS: Array<{ label: string; value: AgentFilter }> = [
  { label: "Primary", value: "Core" },
  { label: "Specialists", value: "Analysis" },
  { label: "Actions", value: "Action" },
  { label: "Reports", value: "Output" },
  { label: "Custom", value: "Custom" },
  { label: "All", value: "all" },
];

const RECOMMENDED_WORKFLOWS: RecommendedWorkflow[] = [
  {
    id: "full-package-audit",
    title: "Full Audit Workflow",
    description: "Evidence-gated audit run from immutable session through report and trace export.",
    steps: 21,
  },
];

const AUDIT_PHASES = [
  "Session",
  "Index",
  "Graphs",
  "Threats",
  "Tests",
  "Confirm",
  "Report",
  "Regress",
];

const AGENT_METADATA: Record<string, AgentUiMetadata> = {
  "agent-orchestrator": {
    category: "Core",
    capabilities: ["Full audit", "Stage ordering", "Evidence gates", "Trace export"],
  },
  "agent-intake": {
    category: "Analysis",
    capabilities: ["Audit session", "Scope", "Tool checks", "Build readiness"],
  },
  "agent-indexer": {
    category: "Analysis",
    capabilities: ["Canonical index", "Compiler facts", "Symbol map"],
  },
  "agent-static-analysis": {
    category: "Analysis",
    capabilities: ["Source scan", "Pattern detection", "Capability lookup"],
  },
  "agent-dynamic-analysis": {
    category: "Analysis",
    capabilities: ["Tests", "Fuzzing", "Traces", "State diffs"],
  },
  "agent-graph-reasoning": {
    category: "Analysis",
    capabilities: ["Object lifecycle", "Call graph", "CFG", "Capability flow"],
  },
  "agent-bytecode": {
    category: "Analysis",
    capabilities: ["Disassembly", "Bytecode CFG", "Stack effects", "Source maps"],
  },
  "agent-invariant": {
    category: "Analysis",
    capabilities: ["Invariant inference", "Property checks", "Object state"],
  },
  "agent-threat-model": {
    category: "Analysis",
    capabilities: ["Classification", "Threat model", "Risk map", "Invariants"],
  },
  "agent-attack-planner": {
    category: "Analysis",
    capabilities: ["Attack hypotheses", "Validation strategy", "Evidence paths"],
  },
  "agent-patch": {
    category: "Action",
    capabilities: ["Patch proposal", "Change preview", "Finding links"],
  },
  "agent-triage": {
    category: "Analysis",
    capabilities: ["Exploitability", "Severity scoring", "Finding states"],
  },
  "agent-remediation": {
    category: "Action",
    capabilities: ["Fix guidance", "Regression planning", "Safer redesigns"],
  },
  "agent-test-generation": {
    category: "Action",
    capabilities: ["Regression cases", "Validation suites", "Scenario generation"],
  },
  "agent-report": {
    category: "Output",
    capabilities: ["Audit report", "Finding summary", "Markdown export"],
  },
  "agent-fix-verification": {
    category: "Action",
    capabilities: ["Changed files", "Affected reruns", "Status updates"],
  },
};

const TOOL_FAMILY_LABELS: Record<string, string> = {
  "rust.audit": "Audit workflow",
  "rust.bytecode": "Bytecode",
  "rust.dynamic": "Dynamic analysis",
  "rust.findings": "Findings",
  "rust.graph": "Graphing",
  "rust.index": "Index",
  "rust.invariant": "Invariants",
  "rust.patch": "Patch",
  "rust.report": "Reports",
  "rust.static": "Static analysis",
  "rust.test": "Tests",
  "rust.validation": "Validation",
};

const PACKAGE_INTENT_TOOL_IDS = new Set([
  "rust.index.package",
  "rust.index.package_overview",
]);

export function AgentsScreen({
  activeMovePackage,
  onAuditReportExportReady,
  packageTree,
  projectRootPath,
}: {
  activeMovePackage?: MovePackage | null;
  onAuditReportExportReady?: (report: AuditReportExport | null) => void;
  packageTree?: PackageTree | null;
  projectRootPath?: string;
}) {
  const projectContext = React.useMemo<AgentToolProjectContext | null>(() => {
    if (!projectRootPath || !activeMovePackage) {
      return null;
    }

    return {
      rootPath: projectRootPath,
      packagePath: activeMovePackage.path || ".",
      packageName: activeMovePackage.name,
      manifestPath: activeMovePackage.manifestPath,
      packageTree: packageTree ?? null,
    };
  }, [activeMovePackage, packageTree, projectRootPath]);
  const [state, setState] = React.useState<AgentStudioState>(() => loadAgentStudioState());
  const [isInspectorOpen, setIsInspectorOpen] = React.useState(true);
  const [activeMainTab, setActiveMainTab] = React.useState<MainTab>("agents");
  const [agentFilter, setAgentFilter] = React.useState<AgentFilter>("Core");
  const [inspectorTab, setInspectorTab] = React.useState<InspectorTab>("overview");
  const [activeRunName, setActiveRunName] = React.useState("");
  const [runDetailsByAgentId, setRunDetailsByAgentId] = React.useState<Record<string, AgentRunDetail>>({});
  const activeRunControllerRef = React.useRef<AbortController | null>(null);
  const [isProjectStateLoaded, setIsProjectStateLoaded] = React.useState(false);

  const selectedAgent =
    state.agents.find((agent) => agent.id === state.selectedAgentId) ?? state.agents[0];
  const selectedWorkflow =
    state.workflows.find((workflow) => workflow.id === state.selectedWorkflowId)
    ?? state.workflows.find((workflow) => workflow.id === selectedAgent?.workflowId)
    ?? state.workflows[0];
  const visibleAgents = state.agents.filter((agent) =>
    agentFilter === "all" ? true : agentMetadata(agent).category === agentFilter,
  );
  const isRunInProgress = state.agents.some((agent) => agent.status === "running");
  const selectedRunSnapshot = createRunSnapshot({
    agent: selectedAgent,
    logs: state.logs,
    workflow: selectedWorkflow,
  });
  const selectedRunDetail = runDetailsByAgentId[selectedAgent.id];
  const selectedHarnessPreview = React.useMemo(
    () =>
      createHarnessRoutePreview({
        agent: selectedAgent,
        projectContext,
        workflow: selectedWorkflow,
      }),
    [projectContext, selectedAgent, selectedWorkflow],
  );

  React.useEffect(() => {
    let cancelled = false;

    setIsProjectStateLoaded(false);

    if (!projectRootPath) {
      setState(loadAgentStudioState());
      setIsProjectStateLoaded(true);
      return;
    }

    void loadProjectMetadata(projectRootPath)
      .then((metadata) => {
        if (!cancelled) {
          setState(loadAgentStudioStateFromProjectMetadata(metadata));
          setIsProjectStateLoaded(true);
        }
      })
      .catch((error) => {
        console.warn("Could not load project Agents metadata; using local fallback.", error);

        if (!cancelled) {
          setState(loadAgentStudioState());
          setIsProjectStateLoaded(true);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [projectRootPath]);

  React.useEffect(() => {
    if (!isProjectStateLoaded) {
      return;
    }

    if (!projectRootPath) {
      saveAgentStudioState(state);
      return;
    }

    const timeout = window.setTimeout(() => {
      void loadProjectMetadata(projectRootPath)
        .then((metadata) =>
          saveProjectMetadata(
            projectRootPath,
            agentStudioStateToProjectMetadata(metadata, state),
          ),
        )
        .catch((error) => {
          console.warn("Could not save project Agents metadata.", error);
        });
    }, 250);

    return () => window.clearTimeout(timeout);
  }, [isProjectStateLoaded, projectRootPath, state]);

  React.useEffect(() => {
    if (
      agentFilter !== "Core"
      || selectedAgent.kind === "custom"
      || agentMetadata(selectedAgent).category === "Core"
    ) {
      return;
    }

    const orchestrator = state.agents.find((agent) => agent.id === "agent-orchestrator");

    if (!orchestrator) {
      return;
    }

    setState((current) => ({
      ...current,
      selectedAgentId: orchestrator.id,
      selectedWorkflowId: orchestrator.workflowId,
    }));
  }, [agentFilter, selectedAgent, state.agents]);

  if (!selectedAgent || !selectedWorkflow) {
    return null;
  }

  return (
    <div
      className={cn(
        "grid h-full min-h-0 min-w-0 overflow-hidden bg-[var(--app-window)] text-foreground transition-[grid-template-columns] duration-200",
        isInspectorOpen
          ? "grid-cols-[minmax(0,1fr)_clamp(340px,26vw,390px)]"
          : "grid-cols-[minmax(0,1fr)_44px]",
      )}
    >
      <main className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border-r border-[color:var(--app-border)]">
        <PageHeader
          agentFilter={agentFilter}
          isRunInProgress={isRunInProgress}
          onCreateAgent={createAgent}
          onFilterChange={setAgentFilter}
          onRunWorkflow={() => void runWorkflow()}
          onStopRun={stopWorkflowRun}
        />

        <ScrollArea className="min-h-0 min-w-0 overflow-hidden">
          <div className="min-w-0 space-y-4 p-4 pb-6">
            {activeMainTab !== "details" ? (
              <>
                <SummaryCards
                  lastRunLabel={lastRunLabel(state.logs)}
                />

                <RecommendedWorkflows
                  disabled={isRunInProgress}
                  onRun={(workflow) => void runRecommendedWorkflow(workflow)}
                  workflows={RECOMMENDED_WORKFLOWS}
                />
              </>
            ) : null}

            <Tabs
              className="min-h-0 min-w-0 gap-3"
              onValueChange={(value) => setActiveMainTab(value as MainTab)}
              value={activeMainTab}
            >
              <div className="flex items-center justify-between border-b border-[color:var(--app-border)]">
                <TabsList className="h-9 rounded-none p-0" variant="line">
                  <TabsTrigger className="h-9 px-3 text-xs" value="agents">
                    Agents
                  </TabsTrigger>
                  <TabsTrigger className="h-9 px-3 text-xs" value="runs">
                    Runs
                  </TabsTrigger>
                  <TabsTrigger className="h-9 px-3 text-xs" value="activity">
                    Activity
                  </TabsTrigger>
                  <TabsTrigger className="h-9 px-3 text-xs" value="details">
                    Details
                  </TabsTrigger>
                </TabsList>
                <div className="hidden items-center gap-2 text-[11px] text-muted-foreground md:flex">
                  <CircleDot className="size-3 text-emerald-300" />
                  Orchestrated audit surface
                </div>
              </div>

              <TabsContent className="mt-0" value="agents">
                <AgentsTable
                  agents={visibleAgents}
                  logs={state.logs}
                  onDeleteAgent={deleteAgent}
                  onDuplicateAgent={duplicateSelectedAgent}
                  onOpenAgentDetails={openAgentDetails}
                  onSelectAgent={selectAgent}
                  selectedAgentId={selectedAgent.id}
                />
              </TabsContent>

              <TabsContent className="mt-0" value="runs">
                <div className="grid gap-3 xl:grid-cols-[minmax(0,1.15fr)_minmax(320px,0.85fr)]">
                  <RunStatusCard
                    activeAgent={selectedAgent.name}
                    onOpenDetails={() => openAgentDetails(selectedAgent)}
                    run={selectedRunSnapshot}
                    workflowName={activeRunName || selectedWorkflow.name}
                  />
                  <WorkflowPreviewCard
                    onRunNode={runNode}
                    onWorkflowChange={updateWorkflow}
                    workflow={selectedWorkflow}
                  />
                </div>
              </TabsContent>

              <TabsContent className="mt-0" value="activity">
                <ActivityTable
                  rows={activityRows(state.logs, state.agents)}
                />
              </TabsContent>

              <TabsContent className="mt-0" value="details">
                <AgentDetailScreen
                  agent={selectedAgent}
                  isRunInProgress={isRunInProgress}
                  onRunWorkflow={() => void runWorkflow()}
                  onStopRun={stopWorkflowRun}
                  plannedRoute={selectedHarnessPreview}
                  run={selectedRunDetail}
                  workflow={selectedWorkflow}
                />
              </TabsContent>
            </Tabs>
          </div>
        </ScrollArea>
      </main>

      <AgentInspector
        activeTab={inspectorTab}
        agent={selectedAgent}
        isOpen={isInspectorOpen}
        logs={state.logs}
        onOpenDetails={() => openAgentDetails(selectedAgent)}
        run={selectedRunSnapshot}
        onRunWorkflow={() => void runWorkflow()}
        onTabChange={setInspectorTab}
        onToggleOpen={() => setIsInspectorOpen((current) => !current)}
        onUpdateAgent={(patch) => updateAgent(selectedAgent.id, patch)}
        workflow={selectedWorkflow}
      />
    </div>
  );

  function createAgent() {
    const created = createCustomAgent();

    setActiveMainTab("agents");
    setInspectorTab("overview");
    setIsInspectorOpen(true);
    setState((current) => ({
      ...current,
      agents: [...current.agents, created.agent],
      workflows: [...current.workflows, created.workflow],
      selectedAgentId: created.agent.id,
      selectedWorkflowId: created.workflow.id,
    }));
  }

  function deleteAgent(agent: AgentDefinition) {
    if (agent.kind !== "custom") {
      return;
    }

    setState((current) => {
      const agents = current.agents.filter((candidate) => candidate.id !== agent.id);
      const workflows = current.workflows.filter((workflow) => workflow.id !== agent.workflowId);
      const nextSelected = agents[0];

      return {
        ...current,
        agents,
        workflows,
        selectedAgentId: nextSelected?.id ?? current.selectedAgentId,
        selectedWorkflowId: nextSelected?.workflowId ?? current.selectedWorkflowId,
      };
    });
  }

  function duplicateSelectedAgent(agent: AgentDefinition) {
    const workflow = state.workflows.find((candidate) => candidate.id === agent.workflowId);

    if (!workflow) {
      return;
    }

    const duplicated = duplicateAgent(agent, workflow);
    setActiveMainTab("agents");
    setInspectorTab("overview");
    setIsInspectorOpen(true);
    setState((current) => ({
      ...current,
      agents: [...current.agents, duplicated.agent],
      workflows: [...current.workflows, duplicated.workflow],
      selectedAgentId: duplicated.agent.id,
      selectedWorkflowId: duplicated.workflow.id,
    }));
  }

  function selectAgent(agent: AgentDefinition) {
    setInspectorTab("overview");
    setIsInspectorOpen(true);
    setState((current) => ({
      ...current,
      selectedAgentId: agent.id,
      selectedWorkflowId: agent.workflowId,
    }));
  }

  function openAgentDetails(agent: AgentDefinition) {
    setActiveMainTab("details");
    setInspectorTab("runs");
    setIsInspectorOpen(true);
    setState((current) => ({
      ...current,
      selectedAgentId: agent.id,
      selectedWorkflowId: agent.workflowId,
    }));
  }

  function updateAgent(agentId: string, patch: Partial<AgentDefinition>) {
    const providerPatch = patch.provider;

    setState((current) => ({
      ...current,
      agents: current.agents.map((agent) =>
        agent.id === agentId
          ? { ...agent, ...patch, updatedAt: Date.now() }
          : agent,
      ),
      workflows: providerPatch
        ? current.workflows.map((workflow) =>
            workflow.id === current.agents.find((agent) => agent.id === agentId)?.workflowId
              ? syncWorkflowProvider(workflow, providerPatch)
              : workflow,
          )
        : current.workflows,
    }));
  }

  function updateWorkflow(workflow: AgentWorkflow) {
    setState((current) => ({
      ...current,
      workflows: current.workflows.map((candidate) =>
        candidate.id === workflow.id ? workflow : candidate,
      ),
    }));
  }

  function runNode(nodeId: string) {
    const workflow = {
      ...selectedWorkflow,
      nodes: selectedWorkflow.nodes.map((node) =>
        node.id === nodeId
          ? { ...node, data: { ...node.data, status: "completed" as const } }
          : node,
      ),
    };

    setState((current) => ({
      ...current,
      workflows: current.workflows.map((candidate) =>
        candidate.id === workflow.id ? workflow : candidate,
      ),
      logs: [
        ...current.logs,
        createExecutionLog({
          agentId: selectedAgent.id,
          workflowId: selectedWorkflow.id,
          nodeId,
          level: "trace",
          message: `Node completed: ${nodeId}`,
        }),
      ].slice(-120),
    }));
  }

  async function runWorkflow() {
    if (selectedAgent.tools.includes("rust.audit.run_full")) {
      await runFullAuditWorkflowFor(selectedAgent, selectedWorkflow, selectedWorkflow.name);
      return;
    }

    await runWorkflowFor(selectedAgent, selectedWorkflow, selectedWorkflow.name);
  }

  async function runRecommendedWorkflow(recommendedWorkflow: RecommendedWorkflow) {
    const orchestrator =
      state.agents.find((agent) => agent.id === "agent-orchestrator") ?? selectedAgent;
    const orchestratorWorkflow =
      state.workflows.find((workflow) => workflow.id === orchestrator.workflowId)
      ?? selectedWorkflow;

    if (recommendedWorkflow.id === "full-package-audit") {
      await runFullAuditWorkflowFor(orchestrator, orchestratorWorkflow, recommendedWorkflow.title);
      return;
    }

    await runWorkflowFor(orchestrator, orchestratorWorkflow, recommendedWorkflow.title);
  }

  function startRunDetail(
    runAgent: AgentDefinition,
    runWorkflowState: AgentWorkflow,
    displayName: string,
    status: AgentRunDetailStatus,
    initialEvent: Omit<AgentRunDetailEvent, "id" | "timestamp">,
  ) {
    const timestamp = Date.now();

    setActiveMainTab("details");
    setInspectorTab("runs");
    setIsInspectorOpen(true);
    setActiveRunName(displayName);
    setRunDetailsByAgentId((current) => ({
      ...current,
      [runAgent.id]: {
        agentId: runAgent.id,
        displayName,
        evidence: [],
        events: [createRunDetailEvent(initialEvent, timestamp)],
        findingCandidates: [],
        id: createRunDetailId(runAgent.id),
        reasoningText: "",
        responseText: "",
        routeDecisions: [],
        startedAt: timestamp,
        status,
        toolCapsules: [],
        toolRuns: [],
        workflowId: runWorkflowState.id,
        workflowName: runWorkflowState.name,
      },
    }));
  }

  function appendRunDetailEvent(
    agentId: string,
    event: Omit<AgentRunDetailEvent, "id" | "timestamp">,
  ) {
    const timestamp = Date.now();

    setRunDetailsByAgentId((current) => {
      const detail = current[agentId];

      if (!detail) {
        return current;
      }

      return {
        ...current,
        [agentId]: {
          ...detail,
          events: [
            ...detail.events,
            createRunDetailEvent(event, timestamp),
          ].slice(-160),
        },
      };
    });
  }

  function appendRunDetailText(
    agentId: string,
    field: "reasoningText" | "responseText",
    text: string,
  ) {
    setRunDetailsByAgentId((current) => {
      const detail = current[agentId];

      if (!detail) {
        return current;
      }

      return {
        ...current,
        [agentId]: {
          ...detail,
          [field]: `${detail[field]}${text}`,
        },
      };
    });
  }

  function updateRunDetailRoute(
    agentId: string,
    capsules: ToolCapsule[],
    decisions: ToolRouteDecision[],
  ) {
    setRunDetailsByAgentId((current) => {
      const detail = current[agentId];

      if (!detail) {
        return current;
      }

      return {
        ...current,
        [agentId]: {
          ...detail,
          routeDecisions: decisions,
          toolCapsules: capsules,
        },
      };
    });
  }

  function appendRunDetailEvidence(
    agentId: string,
    evidence: SecurityEvidenceItem[],
    findingCandidates: FindingCandidate[],
  ) {
    if (!evidence.length && !findingCandidates.length) {
      return;
    }

    setRunDetailsByAgentId((current) => {
      const detail = current[agentId];

      if (!detail) {
        return current;
      }

      return {
        ...current,
        [agentId]: {
          ...detail,
          evidence: mergeById(detail.evidence, evidence),
          findingCandidates: mergeById(detail.findingCandidates, findingCandidates),
        },
      };
    });
  }

  function attachRunDetailToolRuns(agentId: string, toolRuns: ToolRunSummary[]) {
    setRunDetailsByAgentId((current) => {
      const detail = current[agentId];

      if (!detail) {
        return current;
      }

      return {
        ...current,
        [agentId]: {
          ...detail,
          toolRuns: mergeById(detail.toolRuns, toolRuns),
        },
      };
    });
  }

  function finishRunDetail(
    agentId: string,
    status: AgentRunDetailStatus,
    event: Omit<AgentRunDetailEvent, "id" | "timestamp">,
  ) {
    const timestamp = Date.now();

    setRunDetailsByAgentId((current) => {
      const detail = current[agentId];

      if (!detail) {
        return current;
      }

      return {
        ...current,
        [agentId]: {
          ...detail,
          completedAt: timestamp,
          events: [
            ...detail.events,
            createRunDetailEvent(event, timestamp),
          ].slice(-160),
          status,
        },
      };
    });
  }

  function recordRunStreamEvent(agentId: string, event: AgentRunStreamEvent) {
    switch (event.type) {
      case "route-plan": {
        const selectedCount = event.decisions.filter((decision) => decision.selected).length;
        const skippedCount = event.decisions.length - selectedCount;

        updateRunDetailRoute(agentId, event.capsules, event.decisions);
        appendRunDetailEvent(agentId, {
          kind: "tool",
          level: "trace",
          title: "Tool router",
          message: `Selected ${selectedCount} active tool capsules; skipped ${skippedCount}.`,
        });
        break;
      }
      case "text-delta":
        appendRunDetailText(agentId, "responseText", event.text);
        break;
      case "reasoning-delta":
        appendRunDetailText(agentId, "reasoningText", event.text);
        break;
      case "tool-call":
        appendRunDetailEvent(agentId, {
          kind: "tool",
          level: "trace",
          title: event.title ?? displayStreamToolName(event.toolName),
          message: event.input === undefined
            ? `${displayStreamToolName(event.toolName)} called.`
            : `${displayStreamToolName(event.toolName)} called.\nInput: ${formatDetailPayload(event.input, 520)}`,
        });
        break;
      case "tool-result":
        {
          const extracted = extractToolResultEvidence(event.output);

          appendRunDetailEvidence(
            agentId,
            extracted.evidence,
            extracted.findingCandidates,
          );
        }
        appendRunDetailEvent(agentId, {
          kind: "tool",
          level: "info",
          title: event.title ?? displayStreamToolName(event.toolName),
          message: event.summary || `${displayStreamToolName(event.toolName)} returned output.`,
        });
        break;
      case "tool-error":
        appendRunDetailEvent(agentId, {
          kind: "error",
          level: "error",
          title: event.title ?? (event.toolName ? displayStreamToolName(event.toolName) : "Tool error"),
          message: event.message,
        });
        break;
      case "tool-approval-request":
        appendRunDetailEvent(agentId, {
          kind: "tool",
          level: "warning",
          title: "Tool approval requested",
          message: `${displayStreamToolName(event.toolName)} requested approval (${event.approvalId}).`,
        });
        break;
      case "tool-output-denied":
        appendRunDetailEvent(agentId, {
          kind: "tool",
          level: "warning",
          title: "Tool output denied",
          message: `${displayStreamToolName(event.toolName)} output was denied.`,
        });
        break;
      case "step-start":
        appendRunDetailEvent(agentId, {
          kind: "model",
          level: "trace",
          title: "Model step started",
          message: "The model is evaluating the next step.",
        });
        break;
      case "step-finish":
        appendRunDetailEvent(agentId, {
          kind: "model",
          level: "trace",
          title: "Model step finished",
          message: `Finish reason: ${event.finishReason ?? "unknown"}.`,
        });
        break;
      case "finish":
        appendRunDetailEvent(agentId, {
          kind: "model",
          level: "info",
          title: "Stream finished",
          message: `Finish reason: ${event.finishReason ?? "unknown"}.`,
        });
        break;
      case "abort":
        appendRunDetailEvent(agentId, {
          kind: "status",
          level: "warning",
          title: "Run stopped",
          message: event.reason ?? "The run was stopped before completion.",
        });
        break;
      case "error":
        appendRunDetailEvent(agentId, {
          kind: "error",
          level: "error",
          title: "Stream error",
          message: event.message,
        });
        break;
      default:
        break;
    }
  }

  async function runWorkflowFor(
    runAgent: AgentDefinition,
    runWorkflowState: AgentWorkflow,
    displayName: string,
  ) {
    if (activeRunControllerRef.current) {
      return;
    }

    if (!runAgent.provider.modelId.trim()) {
      const message = `No model selected for ${providerById(runAgent.provider.providerId).label}. Refresh the model list or select an installed model before running ${displayName}.`;

      startRunDetail(runAgent, runWorkflowState, displayName, "blocked", {
        kind: "error",
        level: "error",
        message,
        title: "Model unavailable",
      });
      setState((current) => ({
        ...current,
        selectedAgentId: runAgent.id,
        selectedWorkflowId: runWorkflowState.id,
        logs: [
          ...current.logs,
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: "error",
            message,
          }),
        ].slice(-120),
      }));
      return;
    }

    const controller = new AbortController();
    const previousStatus = runAgent.status;
    activeRunControllerRef.current = controller;
    startRunDetail(runAgent, runWorkflowState, displayName, "running", {
      kind: "status",
      level: "trace",
      message: projectContext
        ? `${runAgent.name} started ${displayName} against ${displayMovePackageName(projectContext.packageName)}.`
        : `${runAgent.name} started ${displayName} without an open Move package.`,
      title: "Run started",
    });

    setState((current) => ({
      ...current,
      selectedAgentId: runAgent.id,
      selectedWorkflowId: runWorkflowState.id,
      agents: current.agents.map((agent) =>
        agent.id === runAgent.id ? { ...agent, status: "running" } : agent,
      ),
      workflows: current.workflows.map((workflow) =>
        workflow.id === runWorkflowState.id ? markWorkflowStatus(workflow, "running") : workflow,
      ),
      logs: [
        ...current.logs,
        createExecutionLog({
          agentId: runAgent.id,
          workflowId: runWorkflowState.id,
          level: "trace",
          message: projectContext
            ? `${displayName} started. ${runAgent.name} is coordinating ${runAgent.tools.length} tools against ${displayMovePackageName(projectContext.packageName)}.`
            : `${displayName} started without an open Move package. Tool calls that need project context will fail until a project is loaded.`,
        }),
      ].slice(-120),
    }));

    try {
      const { runAgentWorkflowWithModel } = await import("@/features/agents/agent-runner");
      const result = await runAgentWorkflowWithModel({
        agent: runAgent,
        onTrace: (event) => {
          appendRunLog(runAgent.id, runWorkflowState.id, event);
          appendRunDetailEvent(runAgent.id, {
            kind: event.level === "trace" ? "trace" : "status",
            level: event.level,
            message: event.message,
            title: event.level === "trace" ? "Runtime trace" : "Runtime update",
          });
        },
        onStream: (event) => {
          recordRunStreamEvent(runAgent.id, event);
        },
        projectContext,
        signal: controller.signal,
        workflow: runWorkflowState,
      });

      attachRunDetailToolRuns(runAgent.id, result.toolRuns);
      finishRunDetail(runAgent.id, "completed", {
        kind: "status",
        level: "info",
        message: result.text
          ? `${displayName} completed with a streamed response.`
          : `${displayName} completed without model text.`,
        title: "Run completed",
      });

      setState((current) => ({
        ...current,
        agents: current.agents.map((agent) =>
          agent.id === runAgent.id
            ? {
                ...agent,
                status: previousStatus === "active" ? "active" : "idle",
              }
            : agent,
        ),
        workflows: current.workflows.map((workflow) =>
          workflow.id === runWorkflowState.id ? markWorkflowStatus(workflow, "completed") : workflow,
        ),
        logs: [
          ...current.logs,
          ...result.toolRuns.map((toolRun) =>
            createExecutionLog({
              agentId: runAgent.id,
              workflowId: runWorkflowState.id,
              level: toolRun.status === "failed" || toolRun.status === "denied" ? "warning" : "trace",
              message: `Tool ${toolRun.toolId} (${toolRun.status}): ${toolRun.summary}`,
            }),
          ),
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: "info",
            message: `Agent report:\n${result.text || "(empty response)"}`,
          }),
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: "info",
            message: `${displayName} completed.`,
          }),
        ].slice(-120),
      }));
    } catch (error) {
      const aborted = isAbortError(error);

      finishRunDetail(runAgent.id, aborted ? "stopped" : "blocked", {
        kind: aborted ? "status" : "error",
        level: aborted ? "warning" : "error",
        message: aborted
          ? `${displayName} stopped before the model completed.`
          : `Model call failed: ${error instanceof Error ? error.message : String(error)}`,
        title: aborted ? "Run stopped" : "Run failed",
      });

      setState((current) => ({
        ...current,
        agents: current.agents.map((agent) =>
          agent.id === runAgent.id
            ? { ...agent, status: previousStatus === "active" ? "active" : "idle" }
            : agent,
        ),
        workflows: current.workflows.map((workflow) =>
          workflow.id === runWorkflowState.id
            ? markWorkflowStatus(workflow, aborted ? "idle" : "blocked")
            : workflow,
        ),
        logs: [
          ...current.logs,
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: aborted ? "warning" : "error",
            message: aborted
              ? `${displayName} stopped before the model completed.`
              : `Model call failed: ${error instanceof Error ? error.message : String(error)}`,
          }),
        ].slice(-120),
      }));
    } finally {
      if (activeRunControllerRef.current === controller) {
        activeRunControllerRef.current = null;
      }
    }
  }

  async function runFullAuditWorkflowFor(
    runAgent: AgentDefinition,
    runWorkflowState: AgentWorkflow,
    displayName: string,
  ) {
    if (activeRunControllerRef.current) {
      return;
    }

    const controller = new AbortController();
    const previousStatus = runAgent.status;
    activeRunControllerRef.current = controller;
    onAuditReportExportReady?.(null);
    startRunDetail(runAgent, runWorkflowState, displayName, "running", {
      kind: "status",
      level: "trace",
      message: projectContext
        ? `Started deterministic full audit against ${displayMovePackageName(projectContext.packageName)}.`
        : "Started deterministic full audit without an open Move package.",
      title: "Run started",
    });

    setState((current) => ({
      ...current,
      selectedAgentId: runAgent.id,
      selectedWorkflowId: runWorkflowState.id,
      agents: current.agents.map((agent) =>
        agent.id === runAgent.id ? { ...agent, status: "running" } : agent,
      ),
      workflows: current.workflows.map((workflow) =>
        workflow.id === runWorkflowState.id ? markWorkflowStatus(workflow, "running") : workflow,
      ),
      logs: [
        ...current.logs,
        createExecutionLog({
          agentId: runAgent.id,
          workflowId: runWorkflowState.id,
          level: "trace",
          message: projectContext
            ? `${displayName} started through rust.audit.run_full against ${displayMovePackageName(projectContext.packageName)}.`
            : `${displayName} started without an open Move package. Project-dependent stages may fail.`,
        }),
      ].slice(-120),
    }));

    try {
      const { runFullAuditWorkflowDeterministic } = await import("@/features/agents/agent-runner");
      const result = await runFullAuditWorkflowDeterministic({
        agent: runAgent,
        onTrace: (event) => {
          appendRunLog(runAgent.id, runWorkflowState.id, event);
          appendRunDetailEvent(runAgent.id, {
            kind: event.level === "trace" ? "trace" : "status",
            level: event.level,
            message: event.message,
            title: event.level === "trace" ? "Runtime trace" : "Runtime update",
          });
        },
        onStream: (event) => {
          recordRunStreamEvent(runAgent.id, event);
        },
        projectContext,
        signal: controller.signal,
        workflow: runWorkflowState,
      });

      if (result.text) {
        appendRunDetailText(runAgent.id, "responseText", result.text);
      }
      if (result.auditReportExport) {
        onAuditReportExportReady?.(result.auditReportExport);
      }
      attachRunDetailToolRuns(runAgent.id, result.toolRuns);
      finishRunDetail(runAgent.id, "completed", {
        kind: "status",
        level: "info",
        message: `${displayName} completed through the deterministic audit workflow.`,
        title: "Run completed",
      });

      setState((current) => ({
        ...current,
        agents: current.agents.map((agent) =>
          agent.id === runAgent.id
            ? {
                ...agent,
                status: previousStatus === "active" ? "active" : "idle",
              }
            : agent,
        ),
        workflows: current.workflows.map((workflow) =>
          workflow.id === runWorkflowState.id ? markWorkflowStatus(workflow, "completed") : workflow,
        ),
        logs: [
          ...current.logs,
          ...result.toolRuns.map((toolRun) =>
            createExecutionLog({
              agentId: runAgent.id,
              workflowId: runWorkflowState.id,
              level: toolRun.status === "failed" || toolRun.status === "denied" ? "warning" : "trace",
              message: `Tool ${toolRun.toolId} (${toolRun.status}): ${toolRun.summary}`,
            }),
          ),
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: "info",
            message: `Agent report:\n${result.text || "Full audit workflow streamed stage output."}`,
          }),
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: "info",
            message: `${displayName} completed.`,
          }),
        ].slice(-120),
      }));
    } catch (error) {
      const aborted = isAbortError(error);

      finishRunDetail(runAgent.id, aborted ? "stopped" : "blocked", {
        kind: aborted ? "status" : "error",
        level: aborted ? "warning" : "error",
        message: aborted
          ? `${displayName} stopped before the audit completed.`
          : `Full audit failed: ${error instanceof Error ? error.message : String(error)}`,
        title: aborted ? "Run stopped" : "Run failed",
      });

      setState((current) => ({
        ...current,
        agents: current.agents.map((agent) =>
          agent.id === runAgent.id
            ? { ...agent, status: previousStatus === "active" ? "active" : "idle" }
            : agent,
        ),
        workflows: current.workflows.map((workflow) =>
          workflow.id === runWorkflowState.id
            ? markWorkflowStatus(workflow, aborted ? "idle" : "blocked")
            : workflow,
        ),
        logs: [
          ...current.logs,
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: aborted ? "warning" : "error",
            message: aborted
              ? `${displayName} stopped before the audit completed.`
              : `Full audit failed: ${error instanceof Error ? error.message : String(error)}`,
          }),
        ].slice(-120),
      }));
    } finally {
      if (activeRunControllerRef.current === controller) {
        activeRunControllerRef.current = null;
      }
    }
  }

  function appendRunLog(
    agentId: string,
    workflowId: string,
    event: Pick<AgentExecutionLog, "level" | "message">,
  ) {
    setState((current) => ({
      ...current,
      logs: [
        ...current.logs,
        createExecutionLog({
          agentId,
          workflowId,
          level: event.level,
          message: event.message,
        }),
      ].slice(-120),
    }));
  }

  function stopWorkflowRun() {
    if (activeRunControllerRef.current) {
      activeRunControllerRef.current.abort();
      return;
    }
  }
}

function PageHeader({
  agentFilter,
  isRunInProgress,
  onCreateAgent,
  onFilterChange,
  onRunWorkflow,
  onStopRun,
}: {
  agentFilter: AgentFilter;
  isRunInProgress: boolean;
  onCreateAgent: () => void;
  onFilterChange: (filter: AgentFilter) => void;
  onRunWorkflow: () => void;
  onStopRun: () => void;
}) {
  return (
    <header className="border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] px-4 py-3">
      <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
        <div className="min-w-0">
          <h1 className="text-base font-semibold tracking-normal">Audit Harness</h1>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <label className="relative">
            <span className="sr-only">Filter agents</span>
            <select
              className="h-8 appearance-none rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] pl-3 pr-8 text-xs text-foreground outline-none transition hover:bg-[var(--app-subtle)]"
              onChange={(event) => onFilterChange(event.target.value as AgentFilter)}
              value={agentFilter}
            >
              {AGENT_FILTERS.map((filter) => (
                <option key={filter.value} value={filter.value}>
                  {filter.label}
                </option>
              ))}
            </select>
            <ChevronDown className="pointer-events-none absolute right-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          </label>
          <Button className="h-8 gap-1.5 text-xs" onClick={onCreateAgent} type="button" variant="outline">
            <Plus className="size-3.5" />
            New Agent
          </Button>
          {isRunInProgress ? (
            <Button className="h-8 gap-1.5 text-xs" onClick={onStopRun} type="button" variant="outline">
              <Square className="size-3.5" />
              Stop
            </Button>
          ) : null}
          <Button
            className="h-8 gap-1.5 text-xs"
            disabled={isRunInProgress}
            onClick={onRunWorkflow}
            type="button"
          >
            <Play className="size-3.5" />
            Run Workflow
          </Button>
        </div>
      </div>
    </header>
  );
}

function SummaryCards({
  lastRunLabel,
}: {
  lastRunLabel: string;
}) {
  const cards = [
    { icon: Workflow, label: "Workflow", value: "Full audit" },
    { icon: Network, label: "Phases", value: String(AUDIT_PHASES.length) },
    { icon: FileText, label: "Trace", value: "21 artifacts" },
    { icon: Clock3, label: "Last Run", value: lastRunLabel },
  ];

  return (
    <section className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,180px),1fr))] gap-2">
      {cards.map((card) => (
        <div
          className="grid min-h-16 grid-cols-[auto_minmax(0,1fr)] items-center gap-3 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2.5"
          key={card.label}
        >
          <span className="grid size-8 place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] text-muted-foreground">
            <card.icon className="size-3.5" />
          </span>
          <span className="min-w-0">
            <span className="block text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              {card.label}
            </span>
            <span className="mt-0.5 block truncate text-sm font-semibold text-foreground">
              {card.value}
            </span>
          </span>
        </div>
      ))}
    </section>
  );
}

function RecommendedWorkflows({
  disabled,
  onRun,
  workflows,
}: {
  disabled: boolean;
  onRun: (workflow: RecommendedWorkflow) => void;
  workflows: RecommendedWorkflow[];
}) {
  const primaryWorkflow = workflows[0];

  if (!primaryWorkflow) {
    return null;
  }

  return (
    <section className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-3">
      <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
        <div className="min-w-0">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <h2 className="min-w-0 truncate text-sm font-semibold">{primaryWorkflow.title}</h2>
            <Badge className="rounded px-1.5 py-0.5 text-[10px]" variant="secondary">
              {primaryWorkflow.steps} stages
            </Badge>
          </div>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">
            {primaryWorkflow.description}
          </p>
        </div>
        <Button
          className="h-9 gap-1.5 text-xs"
          disabled={disabled}
          onClick={() => onRun(primaryWorkflow)}
          type="button"
        >
          <Play className="size-3.5" />
          Run Full Audit
        </Button>
      </div>
      <div className="mt-3 grid grid-cols-[repeat(auto-fit,minmax(min(100%,92px),1fr))] gap-1.5">
        {AUDIT_PHASES.map((phase, index) => (
          <div
            className="grid min-h-11 min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-1.5"
            key={phase}
          >
            <span className="grid size-5 place-items-center rounded border border-[color:var(--app-border)] text-[10px] font-semibold tabular-nums text-muted-foreground">
              {index + 1}
            </span>
            <span className="min-w-0 truncate text-[11px] font-medium text-muted-foreground">
              {phase}
            </span>
          </div>
        ))}
      </div>
    </section>
  );
}

function AgentsTable({
  agents,
  logs,
  onDeleteAgent,
  onDuplicateAgent,
  onOpenAgentDetails,
  onSelectAgent,
  selectedAgentId,
}: {
  agents: AgentDefinition[];
  logs: AgentExecutionLog[];
  onDeleteAgent: (agent: AgentDefinition) => void;
  onDuplicateAgent: (agent: AgentDefinition) => void;
  onOpenAgentDetails: (agent: AgentDefinition) => void;
  onSelectAgent: (agent: AgentDefinition) => void;
  selectedAgentId: string;
}) {
  return (
    <section className="overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
      <div className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-2.5">
        <h2 className="text-sm font-semibold">Audit Agents</h2>
        <span className="text-[11px] text-muted-foreground">{agents.length} shown</span>
      </div>
      <div className="overflow-x-auto">
        <div className="min-w-[860px]">
          <div className="grid grid-cols-[36px_minmax(250px,1.7fr)_112px_96px_92px_74px_36px] items-center gap-3 border-b border-[color:var(--app-border)] px-3 py-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            <span />
            <span>Agent</span>
            <span>Category</span>
            <span>Status</span>
            <span>Last Run</span>
            <span>Tools</span>
            <span />
          </div>
          {agents.length ? (
            agents.map((agent) => (
              <AgentRow
                agent={agent}
                key={agent.id}
                lastRun={agentLastRunLabel(logs, agent)}
                onDeleteAgent={onDeleteAgent}
                onDuplicateAgent={onDuplicateAgent}
                onOpenAgentDetails={onOpenAgentDetails}
                onSelectAgent={onSelectAgent}
                selected={selectedAgentId === agent.id}
              />
            ))
          ) : (
            <div className="px-4 py-8 text-center text-xs text-muted-foreground">
              No agents match this filter.
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function AgentRow({
  agent,
  lastRun,
  onDeleteAgent,
  onDuplicateAgent,
  onOpenAgentDetails,
  onSelectAgent,
  selected,
}: {
  agent: AgentDefinition;
  lastRun: string;
  onDeleteAgent: (agent: AgentDefinition) => void;
  onDuplicateAgent: (agent: AgentDefinition) => void;
  onOpenAgentDetails: (agent: AgentDefinition) => void;
  onSelectAgent: (agent: AgentDefinition) => void;
  selected: boolean;
}) {
  const metadata = agentMetadata(agent);

  return (
    <div
      className={cn(
        "grid grid-cols-[minmax(0,1fr)_36px] items-center border-b border-[color:var(--app-border)] text-xs transition last:border-b-0",
        selected
          ? "border-l-2 border-l-primary bg-[var(--app-subtle)]"
          : "border-l-2 border-l-transparent hover:bg-[var(--app-subtle)]",
      )}
    >
      <button
        className="grid min-h-[52px] w-full grid-cols-[36px_minmax(250px,1.7fr)_112px_96px_92px_74px] items-center gap-3 px-3 py-2.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
        onClick={() => onOpenAgentDetails(agent)}
        type="button"
      >
        <AgentIcon agent={agent} />
        <div className="min-w-0">
          <div className="truncate font-semibold text-foreground">{agent.name}</div>
          <div className="mt-0.5 truncate text-[11px] text-muted-foreground">
            {agent.description}
          </div>
        </div>
        <CategoryBadge category={metadata.category} />
        <StatusBadge status={agent.status} />
        <span className="text-[11px] text-muted-foreground">{lastRun}</span>
        <span className="tabular-nums text-[11px] text-muted-foreground">{agent.tools.length}</span>
      </button>
      <div className="flex justify-end pr-3">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              aria-label={`Open actions for ${agent.name}`}
              className="size-7 text-muted-foreground hover:text-foreground"
              size="icon-xs"
              title="Agent actions"
              type="button"
              variant="ghost"
            >
              <MoreHorizontal className="size-3.5" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="border-[color:var(--app-border)] bg-[var(--app-panel)]">
            <DropdownMenuItem className="text-xs" onSelect={() => onSelectAgent(agent)}>
              Open settings
            </DropdownMenuItem>
            <DropdownMenuItem className="text-xs" onSelect={() => onOpenAgentDetails(agent)}>
              Open run details
            </DropdownMenuItem>
            <DropdownMenuItem className="text-xs" onSelect={() => onDuplicateAgent(agent)}>
              Duplicate
            </DropdownMenuItem>
            {agent.kind === "custom" ? (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  className="text-xs"
                  onSelect={() => onDeleteAgent(agent)}
                  variant="destructive"
                >
                  Delete
                </DropdownMenuItem>
              </>
            ) : null}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}

function AgentDetailScreen({
  agent,
  isRunInProgress,
  onRunWorkflow,
  onStopRun,
  plannedRoute,
  run,
  workflow,
}: {
  agent: AgentDefinition;
  isRunInProgress: boolean;
  onRunWorkflow: () => void;
  onStopRun: () => void;
  plannedRoute: HarnessRoutePreview;
  run?: AgentRunDetail;
  workflow: AgentWorkflow;
}) {
  const provider = providerById(agent.provider.providerId);
  const agentIsRunning = agent.status === "running" || run?.status === "running";
  const responseText = run?.responseText.trim();
  const reasoningText = run?.reasoningText.trim();
  const routeCapsules = run?.toolCapsules.length ? run.toolCapsules : plannedRoute.capsules;
  const routeDecisions = run?.routeDecisions.length ? run.routeDecisions : plannedRoute.decisions;
  const skippedToolCount = routeDecisions.filter((decision) => !decision.selected).length;
  const duration = run
    ? durationLabel((run.completedAt ?? Date.now()) - run.startedAt)
    : "Not run";
  const responseTitle = agent.tools.includes("rust.audit.run_full")
    ? "Audit Output"
    : "Model Response";

  return (
    <section className="min-w-0 space-y-3">
      <div className="grid gap-3 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-start">
        <div className="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] gap-3">
          <AgentIcon agent={agent} />
          <div className="min-w-0">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <h2 className="min-w-0 truncate text-base font-semibold">{agent.name}</h2>
              <StatusBadge status={agent.status} />
              {run ? <RunDetailStatusBadge status={run.status} /> : null}
            </div>
            <p className="mt-1 min-w-0 text-xs leading-5 text-muted-foreground">
              {agent.description}
            </p>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2 lg:justify-end">
          {agentIsRunning ? (
            <Button className="h-8 gap-1.5 text-xs" onClick={onStopRun} type="button" variant="outline">
              <Square className="size-3.5" />
              Stop
            </Button>
          ) : null}
          <Button
            className="h-8 gap-1.5 text-xs"
            disabled={isRunInProgress}
            onClick={onRunWorkflow}
            type="button"
          >
            <Play className="size-3.5" />
            Run
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,160px),1fr))] gap-2">
        <MetricTile label="Workflow" value={run?.displayName ?? workflow.name} />
        <MetricTile label="Provider" value={provider.label} />
        <MetricTile label="Model" value={agent.provider.modelId || "No model"} />
        <MetricTile label="Duration" value={duration} />
        <MetricTile label="Routed" value={String(routeCapsules.length)} />
        <MetricTile label="Skipped" value={String(skippedToolCount)} />
        <MetricTile label="Evidence" value={String(run?.evidence.length ?? 0)} />
        <MetricTile label="Findings" value={String(run?.findingCandidates.length ?? 0)} />
      </div>

      <AuditResultsPanel
        evidence={run?.evidence ?? []}
        findingCandidates={run?.findingCandidates ?? []}
      />

      <HarnessCorrelationPanel
        capsules={routeCapsules}
        decisions={routeDecisions}
        evidence={run?.evidence ?? []}
        findingCandidates={run?.findingCandidates ?? []}
        toolRuns={run?.toolRuns ?? []}
      />

      <div className="grid min-w-0 gap-3 xl:grid-cols-[minmax(0,1.2fr)_minmax(300px,0.8fr)]">
        <section className="min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
          <header className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-2.5">
            <div className="flex min-w-0 items-center gap-2">
              <MessageSquareText className="size-4 text-muted-foreground" />
              <h3 className="truncate text-sm font-semibold">{responseTitle}</h3>
            </div>
            {agentIsRunning ? (
              <span className="shrink-0 text-[11px] text-sky-300">Streaming</span>
            ) : null}
          </header>
          <div className="grid min-h-[320px] content-start gap-3 p-3">
            {responseText ? (
              <pre className="min-h-[240px] whitespace-pre-wrap break-words rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-3 text-xs leading-5 text-foreground">
                {responseText}
              </pre>
            ) : (
              <div className="grid min-h-[240px] place-items-center rounded-md border border-dashed border-[color:var(--app-border)] bg-[var(--app-elevated)] p-6 text-center text-xs leading-5 text-muted-foreground">
                {agentIsRunning
                  ? "Waiting for streamed model text."
                  : "No model response has been captured for this agent."}
              </div>
            )}

            {reasoningText ? (
              <section className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-3">
                <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                  <Terminal className="size-3.5" />
                  Reasoning Stream
                </div>
                <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words text-[11px] leading-4 text-muted-foreground">
                  {reasoningText}
                </pre>
              </section>
            ) : null}
          </div>
        </section>

        <section className="min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
          <header className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-2.5">
            <div className="flex min-w-0 items-center gap-2">
              <Terminal className="size-4 text-muted-foreground" />
              <h3 className="truncate text-sm font-semibold">Run Activity</h3>
            </div>
            <span className="shrink-0 text-[11px] text-muted-foreground">
              {run?.events.length ?? 0} events
            </span>
          </header>
          <div className="max-h-[520px] overflow-auto p-3">
            {run?.events.length ? (
              <div className="grid gap-2">
                {run.events.map((event) => (
                  <div
                    className="grid min-w-0 grid-cols-[24px_minmax(0,1fr)] gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-2"
                    key={event.id}
                  >
                    <RunDetailEventIcon event={event} />
                    <div className="min-w-0">
                      <div className="flex min-w-0 items-center justify-between gap-2">
                        <span className={cn("min-w-0 truncate text-xs font-semibold", logLevelClass(event.level))}>
                          {event.title}
                        </span>
                        <span className="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                          {timeLabel(event.timestamp)}
                        </span>
                      </div>
                      <p className="mt-1 whitespace-pre-wrap break-words text-[11px] leading-4 text-muted-foreground">
                        {event.message}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="grid min-h-[260px] place-items-center rounded-md border border-dashed border-[color:var(--app-border)] bg-[var(--app-elevated)] p-6 text-center text-xs leading-5 text-muted-foreground">
                Open or run this agent to see runtime activity.
              </div>
            )}
          </div>
        </section>
      </div>
    </section>
  );
}

function AuditResultsPanel({
  evidence,
  findingCandidates,
}: {
  evidence: SecurityEvidenceItem[];
  findingCandidates: FindingCandidate[];
}) {
  return (
    <section className="min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
      <header className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <ShieldCheck className="size-4 text-muted-foreground" />
          <h3 className="truncate text-sm font-semibold">Evidence & Findings</h3>
        </div>
        <div className="flex shrink-0 items-center gap-1.5 text-[11px] text-muted-foreground">
          <span>{evidence.length} evidence</span>
          <span className="text-muted-foreground/50">/</span>
          <span>{findingCandidates.length} findings</span>
        </div>
      </header>

      <div className="grid min-w-0 gap-3 p-3 xl:grid-cols-2">
        <ResultColumn
          count={evidence.length}
          emptyLabel="No evidence packets have been reduced for this run."
          title="Evidence Cards"
        >
          {evidence.slice(0, 12).map((item) => (
            <EvidenceDetailCard item={item} key={item.id} />
          ))}
        </ResultColumn>

        <ResultColumn
          count={findingCandidates.length}
          emptyLabel="No finding candidates were emitted by this run."
          title="Finding Cards"
        >
          {findingCandidates.slice(0, 12).map((finding) => (
            <FindingDetailCard finding={finding} key={finding.id} />
          ))}
        </ResultColumn>
      </div>
    </section>
  );
}

function ResultColumn({
  children,
  count,
  emptyLabel,
  title,
}: {
  children: React.ReactNode;
  count: number;
  emptyLabel: string;
  title: string;
}) {
  const hasContent = React.Children.count(children) > 0;

  return (
    <section className="min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-2.5">
      <div className="mb-2 flex items-center justify-between gap-2">
        <h4 className="truncate text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
          {title}
        </h4>
        <Badge className="rounded px-1.5 py-0.5 text-[10px]" variant="secondary">
          {count}
        </Badge>
      </div>
      {hasContent ? (
        <div className="grid max-h-[440px] gap-2 overflow-auto pr-1">{children}</div>
      ) : (
        <div className="grid min-h-28 place-items-center rounded-md border border-dashed border-[color:var(--app-border)] px-3 py-4 text-center text-[11px] leading-4 text-muted-foreground">
          {emptyLabel}
        </div>
      )}
    </section>
  );
}

function EvidenceDetailCard({ item }: { item: SecurityEvidenceItem }) {
  const title = evidenceCardTitle(item);

  return (
    <details className="group rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 py-2">
      <summary className="grid min-h-10 cursor-pointer list-none grid-cols-[minmax(0,1fr)_auto] items-start gap-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background">
        <span className="min-w-0">
          <span className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 truncate text-xs font-semibold">{title}</span>
            <span className={cn("shrink-0 text-[10px] font-semibold", confidenceClass(item.confidence))}>
              {item.confidence}
            </span>
          </span>
          <span className="mt-1 block line-clamp-2 text-[11px] leading-4 text-muted-foreground">
            {item.claim}
          </span>
        </span>
        <ChevronRight className="mt-0.5 size-3.5 shrink-0 text-muted-foreground transition group-open:rotate-90" />
      </summary>
      <div className="mt-2 grid gap-2 border-t border-[color:var(--app-border)] pt-2 text-[11px] leading-4 text-muted-foreground">
        <DetailPair label="Observation" value={item.observation} />
        <DetailPair label="Precision" value={item.sourcePrecision} />
        {item.symbolRefs.length ? (
          <DetailPair label="Symbols" value={item.symbolRefs.slice(0, 8).join(", ")} />
        ) : null}
        {item.followUp ? <DetailPair label="Follow-up" value={item.followUp} /> : null}
      </div>
    </details>
  );
}

function evidenceCardTitle(item: SecurityEvidenceItem) {
  const packetType = typeof item.metadata?.packetType === "string"
    ? item.metadata.packetType
    : undefined;

  return packetType ?? item.kind;
}

function FindingDetailCard({ finding }: { finding: FindingCandidate }) {
  const confirmed = finding.status === "confirmed";

  return (
    <details className="group rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 py-2">
      <summary className="grid min-h-10 cursor-pointer list-none grid-cols-[minmax(0,1fr)_auto] items-start gap-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background">
        <span className="min-w-0">
          <span className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 truncate text-xs font-semibold" title={finding.title}>
              {finding.title}
            </span>
            <FindingSeverityBadge confirmed={confirmed} severity={finding.severity} />
          </span>
          <span className="mt-1 block truncate text-[11px] text-muted-foreground">
            {findingStatusLabel(finding.status)} - {finding.confidence}
          </span>
        </span>
        <ChevronRight className="mt-0.5 size-3.5 shrink-0 text-muted-foreground transition group-open:rotate-90" />
      </summary>
      <div className="mt-2 grid gap-2 border-t border-[color:var(--app-border)] pt-2 text-[11px] leading-4 text-muted-foreground">
        <DetailPair label="Category" value={finding.category} />
        <DetailPair label="Status meaning" value={findingStatusExplanation(finding)} />
        <DetailPair label="Why severity" value={findingSeverityRationale(finding)} />
        <DetailPair label="Impact if true" value={findingImpact(finding)} />
        {finding.affectedSymbols.length ? (
          <DetailPair label="Affected" value={finding.affectedSymbols.slice(0, 8).join(", ")} />
        ) : null}
        {finding.exploitScenario ? (
          <DetailPair label="Scenario" value={finding.exploitScenario} />
        ) : null}
        <DetailPair label="Mitigation" value={findingMitigation(finding)} />
        <DetailPair
          label="Validation"
          value={findingValidationText(finding)}
        />
        {finding.evidenceRefs.length ? (
          <DetailPair label="Evidence summary" value={finding.evidenceRefs.slice(0, 4).join("; ")} />
        ) : null}
      </div>
    </details>
  );
}

function FindingSeverityBadge({
  confirmed,
  severity,
}: {
  confirmed: boolean;
  severity: FindingCandidate["severity"];
}) {
  return (
    <Badge
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px]",
        severity === "critical" && "bg-red-500/20 text-red-200",
        severity === "high" && "bg-red-500/15 text-red-300",
        severity === "medium" && "bg-amber-500/15 text-amber-300",
        severity === "low" && "bg-sky-500/15 text-sky-300",
        severity === "info" && "bg-muted text-muted-foreground",
      )}
      variant="secondary"
    >
      {confirmed ? severity : `${severity} candidate`}
    </Badge>
  );
}

function findingStatusLabel(status: FindingCandidate["status"]) {
  if (status === "hypothesis" || status === "possible") {
    return "candidate";
  }
  if (status === "needsValidation") {
    return "needs validation";
  }
  if (status === "needsHumanReview") {
    return "needs review";
  }

  return status;
}

function findingStatusExplanation(finding: FindingCandidate) {
  if (finding.status === "confirmed") {
    return "Confirmed means the harness found deterministic evidence tied to this finding. Treat it as an active issue unless a human review proves the evidence is invalid.";
  }
  if (finding.status === "likely") {
    return "Likely means multiple evidence sources point at the issue. Keep it open and prioritize validation or remediation.";
  }
  if (finding.status === "possible" || finding.status === "hypothesis") {
    return "This is an evidence-backed candidate from static or graph analysis. It stays open until targeted validation proves exploitability, mitigation, accepted risk, or a concrete false positive.";
  }
  if (finding.status === "needsHumanReview" || finding.status === "needsValidation") {
    return "The harness found a security-relevant signal and needs human review or a targeted validation run before final disposition.";
  }

  return `Current finding state: ${finding.status}.`;
}

function findingSeverityRationale(finding: FindingCandidate) {
  const evidenceText = finding.evidenceRefs.join(" ").toLowerCase();
  const traits = [
    /public|entry|transaction-callable/.test(evidenceText) ? "reachable public or transaction-callable surface" : "",
    /mint|burn|supply|coin|balance|withdraw|deposit|asset/.test(evidenceText) ? "asset or supply movement" : "",
    /shared|lifecycle|mutat/.test(evidenceText) ? "shared or lifecycle-sensitive state mutation" : "",
    /oracle|price|clock|time/.test(evidenceText) ? "oracle, price, or time dependency" : "",
    /external package|external call|dependency/.test(evidenceText) ? "external dependency interaction" : "",
    /admin|cap|authority|owner/.test(evidenceText) ? "authorization or capability boundary" : "",
  ].filter(Boolean);
  const basis = traits.length
    ? traits.join(", ")
    : "the affected symbol, static rule, and available evidence chain";
  const prefix = finding.status === "confirmed"
    ? "Severity is assigned from observed evidence involving"
    : "Candidate severity is assigned because the evidence shows";

  return `${prefix} ${basis}. Confidence is ${finding.confidence}; this should drive validation priority, not dismissal.`;
}

function findingImpact(finding: FindingCandidate) {
  const assetImpact = typeof finding.metadata?.assetImpact === "string"
    ? finding.metadata.assetImpact
    : undefined;

  if (assetImpact) {
    return assetImpact;
  }
  if (finding.category === "surface-risk") {
    return "Potential unauthorized state mutation, asset movement, accounting corruption, stale price use, or dependency-triggered behavior on a reachable public surface.";
  }
  if (finding.category.includes("complexity")) {
    return "Complex code paths increase review risk and can hide missing checks, but complexity alone is not an exploit.";
  }
  if (finding.title.toLowerCase().includes("unchecked")) {
    return "Unchecked results can hide failed operations or skipped validation, depending on the affected call.";
  }

  return "Impact depends on whether the evidence chain is reachable by the stated actor and can change assets, privileges, or protocol state.";
}

function findingMitigation(finding: FindingCandidate) {
  if (finding.patchRecommendation?.minimalChange) {
    return finding.patchRecommendation.minimalChange;
  }

  const title = finding.title.toLowerCase();
  if (finding.category === "surface-risk" || title.includes("high-risk public surface")) {
    return "Require explicit capability or owner authorization, validate state, amount, oracle freshness, and bounds before mutation or transfer, and add a negative regression for unauthorized or malformed calls.";
  }
  if (title.includes("function_complexity")) {
    return "Split the function into smaller helpers around authorization, accounting, and effects; add focused tests for each branch that mutates state or moves assets.";
  }
  if (title.includes("module_complexity")) {
    return "Separate protocol concerns into smaller modules or helper APIs and add module-level invariants for critical accounting and authorization boundaries.";
  }
  if (title.includes("unchecked_return")) {
    return "Use, assert, or explicitly document the returned value. If the return encodes success, failure, amount, or object state, abort on unexpected values.";
  }

  return "Define the intended invariant, add the smallest guard before mutation, and attach a regression test that fails before the fix and passes after it.";
}

function findingValidationText(finding: FindingCandidate) {
  const commands = finding.validationPlan.commands.length
            ? finding.validationPlan.commands.join(", ")
            : "No validation command attached";
  const expected = finding.validationPlan.expectedEvidence.length
    ? ` Expected: ${finding.validationPlan.expectedEvidence.join(" ")}`
    : "";

  return `${commands}.${expected || " Keep this candidate open until validation produces a proof, mitigation, accepted risk, or a documented false positive."}`;
}

function DetailPair({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="grid gap-0.5">
      <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground/80">
        {label}
      </span>
      <span className="break-words text-muted-foreground">{value}</span>
    </div>
  );
}

function HarnessCorrelationPanel({
  capsules,
  decisions,
  evidence,
  findingCandidates,
  toolRuns,
}: {
  capsules: ToolCapsule[];
  decisions: ToolRouteDecision[];
  evidence: SecurityEvidenceItem[];
  findingCandidates: FindingCandidate[];
  toolRuns: ToolRunSummary[];
}) {
  const selectedCount = decisions.filter((decision) => decision.selected).length;
  const skippedCount = decisions.length - selectedCount;
  const categories = categoryCounts(capsules);
  const latestRuns = toolRuns.slice(-5).reverse();
  const intentCapsules = capsules.filter((capsule) => PACKAGE_INTENT_TOOL_IDS.has(capsule.id));
  const intentRuns = toolRuns.filter((toolRun) => PACKAGE_INTENT_TOOL_IDS.has(toolRun.toolId));

  return (
    <section className="min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
      <header className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <Network className="size-4 text-muted-foreground" />
          <h3 className="truncate text-sm font-semibold">Harness Correlation</h3>
        </div>
        <div className="flex shrink-0 items-center gap-1.5 text-[11px] text-muted-foreground">
          <span>{selectedCount} routed</span>
          <span className="text-muted-foreground/50">/</span>
          <span>{skippedCount} skipped</span>
        </div>
      </header>

      <div className="grid gap-3 p-3 xl:grid-cols-4">
        <HarnessColumn
          emptyLabel="Intent discovery tools are not routed for this agent."
          icon={<FileText className="size-3.5" />}
          title="Package Intent"
        >
          {intentCapsules.length || intentRuns.length ? (
            <div className="grid gap-2">
              <div className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2">
                <div className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                  Intent Gate
                </div>
                <p className="mt-1 text-[11px] leading-4 text-muted-foreground">
                  {intentRuns.some((run) => run.status === "succeeded")
                    ? "Package intent evidence has started to land."
                    : "Run index and overview before specialized checks."}
                </p>
              </div>
              <div className="grid gap-1.5">
                {(intentRuns.length ? intentRuns : intentCapsules).slice(0, 4).map((item) => {
                  const id = "toolId" in item ? item.toolId : item.id;
                  const status = "status" in item ? item.status : "routed";

                  return (
                    <div
                      className="grid min-w-0 gap-1 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2"
                      key={`${id}-${status}`}
                    >
                      <span className="min-w-0 truncate font-mono text-[11px] text-foreground" title={id}>
                        {"callableName" in item ? item.callableName ?? id : id}
                      </span>
                      <span className="min-w-0 truncate text-[10px] text-muted-foreground">
                        {status}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          ) : null}
        </HarnessColumn>

        <HarnessColumn
          emptyLabel="No active capsules routed."
          icon={<Wrench className="size-3.5" />}
          title="Tool Router"
        >
          {capsules.length ? (
            <div className="grid gap-2">
              <div className="flex flex-wrap gap-1.5">
                {categories.map(([category, count]) => (
                  <Badge className="rounded px-2 py-1 text-[10px]" key={category} variant="secondary">
                    {category}: {count}
                  </Badge>
                ))}
              </div>
              <div className="grid gap-1.5">
                {capsules.slice(0, 6).map((capsule) => (
                  <div
                    className="grid min-w-0 gap-1 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2"
                    key={capsule.id}
                  >
                    <div className="flex min-w-0 items-center justify-between gap-2">
                      <span
                        className="min-w-0 truncate font-mono text-[11px] text-foreground"
                        title={capsule.callableName ?? capsule.id}
                      >
                        {capsule.callableName ?? capsule.id}
                      </span>
                      <Badge className="rounded px-1.5 py-0.5 text-[10px]" variant="secondary">
                        {capsule.risk}
                      </Badge>
                    </div>
                    <p className="min-w-0 truncate font-mono text-[10px] text-muted-foreground" title={capsule.id}>
                      {capsule.id}
                    </p>
                    <p className="min-w-0 truncate text-[11px] text-muted-foreground">
                      {capsule.category}
                      {capsule.outputBudgetTokens ? ` - ${capsule.outputBudgetTokens} token cap` : ""}
                    </p>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </HarnessColumn>

        <HarnessColumn
          emptyLabel="No reduced evidence captured."
          icon={<Terminal className="size-3.5" />}
          title="Evidence Compiler"
        >
          {evidence.length ? (
            <div className="grid gap-1.5">
              {evidence.slice(0, 6).map((item) => (
                <div
                  className="grid min-w-0 gap-1 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2"
                  key={item.id}
                >
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="min-w-0 truncate text-[11px] font-semibold">{item.kind}</span>
                    <span className={cn("shrink-0 text-[10px] font-semibold", confidenceClass(item.confidence))}>
                      {item.confidence}
                    </span>
                  </div>
                  <p className="line-clamp-2 min-w-0 text-[11px] leading-4 text-muted-foreground">
                    {item.claim}
                  </p>
                </div>
              ))}
            </div>
          ) : null}
        </HarnessColumn>

        <HarnessColumn
          emptyLabel="No finding candidates emitted."
          icon={<ShieldCheck className="size-3.5" />}
          title="Finding Engine"
        >
          {findingCandidates.length ? (
            <div className="grid gap-1.5">
              {findingCandidates.slice(0, 6).map((finding) => (
                <div
                  className="grid min-w-0 gap-1 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2"
                  key={finding.id}
                >
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="min-w-0 truncate text-[11px] font-semibold" title={finding.title}>
                      {finding.title}
                    </span>
                    <SeverityBadge severity={finding.severity} />
                  </div>
                  <p className="min-w-0 truncate text-[11px] text-muted-foreground">
                    {finding.status} - {finding.confidence}
                  </p>
                </div>
              ))}
            </div>
          ) : latestRuns.length ? (
            <div className="grid gap-1.5">
              {latestRuns.map((toolRun) => (
                <div
                  className="grid min-w-0 gap-1 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2"
                  key={toolRun.id}
                >
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="min-w-0 truncate font-mono text-[11px]" title={toolRun.toolId}>
                      {toolRun.toolId}
                    </span>
                    <span className="shrink-0 text-[10px] text-muted-foreground">
                      {toolRun.status}
                    </span>
                  </div>
                  <p className="line-clamp-2 min-w-0 text-[11px] leading-4 text-muted-foreground">
                    {toolRun.summary}
                  </p>
                </div>
              ))}
            </div>
          ) : null}
        </HarnessColumn>
      </div>
    </section>
  );
}

function HarnessColumn({
  children,
  emptyLabel,
  icon,
  title,
}: {
  children: React.ReactNode;
  emptyLabel: string;
  icon: React.ReactNode;
  title: string;
}) {
  const hasContent = React.Children.count(children) > 0;

  return (
    <section className="min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-2.5">
      <div className="mb-2 flex min-w-0 items-center gap-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {icon}
        <span className="truncate">{title}</span>
      </div>
      {hasContent ? (
        children
      ) : (
        <div className="grid min-h-24 place-items-center rounded-md border border-dashed border-[color:var(--app-border)] px-3 py-4 text-center text-[11px] leading-4 text-muted-foreground">
          {emptyLabel}
        </div>
      )}
    </section>
  );
}

function AgentInspector({
  activeTab,
  agent,
  isOpen,
  logs,
  onOpenDetails,
  onRunWorkflow,
  onTabChange,
  onToggleOpen,
  onUpdateAgent,
  run,
  workflow,
}: {
  activeTab: InspectorTab;
  agent: AgentDefinition;
  isOpen: boolean;
  logs: AgentExecutionLog[];
  onOpenDetails: () => void;
  onRunWorkflow: () => void;
  onTabChange: (tab: InspectorTab) => void;
  onToggleOpen: () => void;
  onUpdateAgent: (patch: Partial<AgentDefinition>) => void;
  run: RunSnapshot;
  workflow: AgentWorkflow;
}) {
  const metadata = agentMetadata(agent);
  const provider = providerById(agent.provider.providerId);
  const [modelList, setModelList] = React.useState<{
    error?: string;
    modelIds: string[];
    source: string;
    status: "idle" | "loading" | "ready";
  }>({
    modelIds: [],
    source: "",
    status: "idle",
  });
  const canEditIdentity = agent.kind === "custom";
  const report = latestAgentReport(logs);
  const selectedModelValue = modelList.modelIds.includes(agent.provider.modelId)
    ? agent.provider.modelId
    : modelList.modelIds[0] ?? "";

  const refreshModelList = React.useCallback(() => {
    let cancelled = false;

    setModelList({
      error: undefined,
      modelIds: [],
      source: provider.label,
      status: "loading",
    });

    void loadProviderModelOptions(agent.provider)
      .then((result) => {
        if (!cancelled) {
          setModelList({
            error: result.error,
            modelIds: result.modelIds,
            source: result.source,
            status: "ready",
          });
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setModelList({
            error: error instanceof Error ? error.message : String(error),
            modelIds: [],
            source: provider.label,
            status: "ready",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [agent.provider, provider.label]);

  React.useEffect(() => refreshModelList(), [refreshModelList]);

  React.useEffect(() => {
    if (
      modelList.status === "ready"
      && modelList.modelIds.length
      && !modelList.modelIds.includes(agent.provider.modelId)
    ) {
      onUpdateAgent({
        provider: {
          ...agent.provider,
          modelId: modelList.modelIds[0],
        },
      });
    }
  }, [agent.provider, modelList.modelIds, modelList.status, onUpdateAgent]);

  if (!isOpen) {
    return (
      <aside className="min-w-0 overflow-hidden border-l border-[color:var(--app-border)] bg-[var(--app-panel)]">
        <button
          aria-label="Open agent settings"
          className="flex h-full w-full items-start justify-center px-2 py-4 text-muted-foreground transition hover:text-foreground"
          onClick={onToggleOpen}
          title="Open agent settings"
          type="button"
        >
          <span className="text-[10px] font-semibold uppercase tracking-wide [writing-mode:vertical-rl]">
            Inspector
          </span>
        </button>
      </aside>
    );
  }

  return (
    <aside className="grid min-h-0 min-w-0 max-w-full grid-rows-[auto_minmax(0,1fr)] overflow-hidden border-l border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <header className="border-b border-[color:var(--app-border)] px-4 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              <h2 className="truncate text-sm font-semibold">{agent.name}</h2>
              <StatusBadge status={agent.status} />
            </div>
          </div>
          <Button
            aria-label="Close agent settings"
            className="size-7 text-muted-foreground hover:text-foreground"
            onClick={onToggleOpen}
            size="icon-xs"
            title="Close agent settings"
            type="button"
            variant="ghost"
          >
            <X className="size-3.5" />
          </Button>
        </div>
      </header>

      <Tabs
        className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] gap-0"
        onValueChange={(value) => onTabChange(value as InspectorTab)}
        value={activeTab}
      >
        <div className="border-b border-[color:var(--app-border)] px-4 py-3">
          <TabsList className="grid h-8 w-full min-w-0 grid-cols-[repeat(4,minmax(0,1fr))] overflow-hidden rounded-md bg-[var(--app-surface)] p-0.5">
            <TabsTrigger className="h-full min-w-0 flex-none overflow-hidden text-ellipsis px-1 text-[11px]" value="overview">
              Overview
            </TabsTrigger>
            <TabsTrigger className="h-full min-w-0 flex-none overflow-hidden text-ellipsis px-1 text-[11px]" value="tools">
              Tools
            </TabsTrigger>
            <TabsTrigger className="h-full min-w-0 flex-none overflow-hidden text-ellipsis px-1 text-[11px]" value="permissions">
              Access
            </TabsTrigger>
            <TabsTrigger className="h-full min-w-0 flex-none overflow-hidden text-ellipsis px-1 text-[11px]" value="runs">
              Runs
            </TabsTrigger>
          </TabsList>
        </div>

        <ScrollArea className="min-h-0 min-w-0 max-w-full overflow-hidden">
          <div className="w-full min-w-0 max-w-full space-y-4 overflow-x-hidden px-4 pb-4 pr-5 pt-3">
            <TabsContent className="mt-0 min-w-0 space-y-4" value="overview">
              {canEditIdentity ? (
                <SettingsBlock title="Identity">
                  <LabelledField label="Name">
                    <Input
                      className="h-8 text-xs"
                      onChange={(event) => onUpdateAgent({ name: event.target.value })}
                      value={agent.name}
                    />
                  </LabelledField>
                  <LabelledField label="Description">
                    <textarea
                      className="min-h-16 w-full resize-none rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2 text-xs leading-5 outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                      onChange={(event) => onUpdateAgent({ description: event.target.value })}
                      value={agent.description}
                    />
                  </LabelledField>
                </SettingsBlock>
              ) : null}

              <SettingsBlock title="Description">
                <p className="text-xs leading-5 text-muted-foreground">{agent.description}</p>
                <div className="flex flex-wrap gap-1.5">
                  {metadata.capabilities.map((capability) => (
                    <Badge className="rounded px-2 py-1 text-[10px]" key={capability} variant="secondary">
                      {capability}
                    </Badge>
                  ))}
                </div>
              </SettingsBlock>

              <SettingsBlock title="Model">
                <LabelledField label="Provider">
                  <select
                    className="h-8 w-full min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-xs outline-none"
                    onChange={(event) => {
                      const nextProvider = providerById(event.target.value);
                      onUpdateAgent({
                        provider: {
                          providerId: nextProvider.id,
                          modelId: nextProvider.defaultModelId ?? "",
                          endpoint: nextProvider.defaultEndpoint,
                        },
                      });
                    }}
                    value={agent.provider.providerId}
                  >
                    {modelProviderAdapters.map((adapter) => (
                      <option key={adapter.id} value={adapter.id}>
                        {adapter.label} ({adapter.scope})
                      </option>
                    ))}
                  </select>
                </LabelledField>
                <LabelledField label="Model">
                  {modelList.modelIds.length ? (
                    <select
                      className="h-8 w-full min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-xs outline-none"
                      onChange={(event) =>
                        onUpdateAgent({
                          provider: {
                            ...agent.provider,
                            modelId: event.target.value,
                          },
                        })
                      }
                      value={selectedModelValue}
                    >
                      {modelList.modelIds.map((modelId) => (
                        <option key={modelId} value={modelId}>
                          {modelId}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <Input
                      className="h-8 text-xs"
                      disabled={provider.id === "ollama"}
                      onChange={(event) =>
                        onUpdateAgent({
                          provider: {
                            ...agent.provider,
                            modelId: event.target.value,
                          },
                        })
                      }
                      placeholder={provider.id === "ollama" ? "No Ollama models found" : "Enter AI Gateway model ID"}
                      value={agent.provider.modelId}
                    />
                  )}
                </LabelledField>
                <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2">
                  <p className="min-w-0 truncate text-[10px] leading-4 text-muted-foreground" title={modelList.source}>
                    {modelList.status === "loading"
                      ? "Inspecting models..."
                      : modelList.modelIds.length
                        ? `${modelList.modelIds.length} models`
                        : modelList.error
                          ? `Model scan failed: ${modelList.error}`
                          : "No models found"}
                  </p>
                  <Button
                    className="h-6 shrink-0 px-2 text-[10px]"
                    onClick={() => {
                      refreshModelList();
                    }}
                    type="button"
                    variant="outline"
                  >
                    Refresh
                  </Button>
                </div>
                {provider.id === "ollama" ? (
                  <LabelledField label="Endpoint">
                    <Input
                      className="h-8 min-w-0 truncate text-xs"
                      onChange={(event) =>
                        onUpdateAgent({
                          provider: {
                            ...agent.provider,
                            endpoint: event.target.value,
                          },
                        })
                      }
                      value={agent.provider.endpoint ?? provider.defaultEndpoint ?? ""}
                    />
                  </LabelledField>
                ) : null}
              </SettingsBlock>

              <PermissionsList compact />

              <LastRunCard
                onOpenDetails={onOpenDetails}
                run={run}
                report={report}
                workflowName={workflow.name}
              />
            </TabsContent>

            <TabsContent className="mt-0 min-w-0 space-y-4" value="tools">
              <ToolGroups tools={agent.tools} />
            </TabsContent>

            <TabsContent className="mt-0 min-w-0 space-y-4" value="permissions">
              <SettingsBlock title="Execution Policy">
                <LabelledField label="Mode">
                  <select
                    className="h-8 w-full min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-xs outline-none"
                    onChange={(event) =>
                      onUpdateAgent({
                        execution: {
                          ...agent.execution,
                          mode: event.target.value as AgentDefinition["execution"]["mode"],
                        },
                      })
                    }
                    value={agent.execution.mode}
                  >
                    <option value="manual">Manual</option>
                    <option value="approvalGated">Approval gated</option>
                    <option value="background">Background</option>
                  </select>
                </LabelledField>
                <LabelledField label="Max steps">
                  <Input
                    className="h-8 text-xs"
                    min={1}
                    onChange={(event) =>
                      onUpdateAgent({
                        execution: {
                          ...agent.execution,
                          maxSteps: Number(event.target.value),
                        },
                      })
                    }
                    type="number"
                    value={agent.execution.maxSteps}
                  />
                </LabelledField>
                <ToggleRow
                  checked={agent.execution.requireToolApproval}
                  label="Tool approvals"
                  onChange={(checked) =>
                    onUpdateAgent({
                      execution: {
                        ...agent.execution,
                        requireToolApproval: checked,
                      },
                    })
                  }
                />
                <ToggleRow
                  checked={agent.execution.persistMemory}
                  label="Memory"
                  onChange={(checked) =>
                    onUpdateAgent({
                      execution: {
                        ...agent.execution,
                        persistMemory: checked,
                      },
                    })
                  }
                />
              </SettingsBlock>
              <PermissionsList />
            </TabsContent>

            <TabsContent className="mt-0 min-w-0 space-y-4" value="runs">
              <RunStatusCard
                activeAgent={agent.name}
                compact
                onOpenDetails={onOpenDetails}
                run={run}
                workflowName={workflow.name}
              />
              <Button className="h-8 w-full gap-1.5 text-xs" onClick={onRunWorkflow} type="button">
                <Play className="size-3.5" />
                Run Agent Workflow
              </Button>
            </TabsContent>
          </div>
        </ScrollArea>
      </Tabs>
    </aside>
  );
}

function RunStatusCard({
  activeAgent,
  compact = false,
  onOpenDetails,
  run,
  workflowName,
}: {
  activeAgent: string;
  compact?: boolean;
  onOpenDetails?: () => void;
  run: RunSnapshot;
  workflowName: string;
}) {
  return (
    <section className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <h3 className="truncate text-sm font-semibold">{workflowName}</h3>
            <RunStatusBadge status={run.status} />
          </div>
          <div className="mt-2 grid gap-1 text-[11px] text-muted-foreground">
            <span>Active Agent: {activeAgent}</span>
            <span>Active Tool: {run.activeTool}</span>
          </div>
        </div>
        <Button
          className="h-7 px-2 text-[11px]"
          disabled={!onOpenDetails}
          onClick={onOpenDetails}
          type="button"
          variant="outline"
        >
          Open Run
        </Button>
      </div>

      <div className="mt-4 grid gap-2">
        {run.steps.map((step) => (
          <div className="grid grid-cols-[18px_minmax(0,1fr)] items-center gap-2 text-xs" key={step.label}>
            <ProgressGlyph state={step.state} />
            <span className={cn("min-w-0 truncate", step.state === "pending" && "text-muted-foreground")}>
              {step.label}
            </span>
          </div>
        ))}
      </div>

      <div className={cn("mt-4 grid gap-2", compact ? "grid-cols-2" : "grid-cols-4")}>
        {[
          ["Events", String(run.steps.filter((step) => step.state !== "pending").length)],
          ["Issues", String(run.issueCount)],
          ["Warnings", String(run.warningCount)],
          ["Evidence", String(run.evidenceArtifacts.length)],
        ].map(([label, value]) => (
          <div
            className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-2"
            key={label}
          >
            <div className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              {label}
            </div>
            <div className="mt-0.5 text-sm font-semibold tabular-nums">{value}</div>
          </div>
        ))}
      </div>

      <div className="mt-3 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-2">
        <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
          Evidence produced
        </div>
        {run.evidenceArtifacts.length ? (
          <div className="flex flex-wrap gap-1.5">
            {run.evidenceArtifacts.map((item) => (
              <Badge className="max-w-full rounded px-2 py-1 text-[10px]" key={item} variant="secondary">
                {item}
              </Badge>
            ))}
          </div>
        ) : (
          <p className="text-[11px] leading-4 text-muted-foreground">
            No evidence artifacts recorded for this workflow run.
          </p>
        )}
        {run.latestError ? (
          <p className="mt-2 rounded border border-red-500/20 bg-red-500/10 p-2 text-[11px] leading-4 text-red-200">
            {run.latestError}
          </p>
        ) : null}
      </div>
    </section>
  );
}

function WorkflowPreviewCard({
  onRunNode,
  onWorkflowChange,
  workflow,
}: {
  onRunNode: (nodeId: string) => void;
  onWorkflowChange: (workflow: AgentWorkflow) => void;
  workflow: AgentWorkflow;
}) {
  return (
    <section className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-3">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold">Workflow Preview</h3>
        <Dialog>
          <DialogTrigger asChild>
            <Button className="h-7 px-2 text-[11px]" type="button" variant="outline">
              Open Editor
            </Button>
          </DialogTrigger>
          <DialogContent className="h-[min(760px,calc(100vh-3rem))] max-w-[min(1180px,calc(100vw-3rem))] grid-rows-[auto_minmax(0,1fr)] p-0">
            <DialogHeader className="border-b border-[color:var(--app-border)] px-4 py-3">
              <DialogTitle className="text-sm">{workflow.name}</DialogTitle>
              <DialogDescription className="text-xs">
                Selected agent workflow graph.
              </DialogDescription>
            </DialogHeader>
            <div className="min-h-0">
              <AgentWorkflowCanvas
                onRunNode={onRunNode}
                onWorkflowChange={onWorkflowChange}
                workflow={workflow}
              />
            </div>
          </DialogContent>
        </Dialog>
      </div>
      <div className="mt-4 flex flex-wrap items-center gap-2">
        {workflow.nodes.map((node, index) => (
          <React.Fragment key={node.id}>
            <span className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2 py-1.5 text-[11px] text-foreground">
              {node.data.label}
            </span>
            {index < workflow.nodes.length - 1 ? (
              <ChevronRight className="size-3.5 text-muted-foreground" />
            ) : null}
          </React.Fragment>
        ))}
      </div>
    </section>
  );
}

function ActivityTable({ rows }: { rows: ActivityRow[] }) {
  return (
    <section className="overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
      <div className="overflow-x-auto">
        <div className="min-w-[760px]">
          <div className="grid grid-cols-[84px_190px_86px_minmax(0,1fr)_220px] gap-3 border-b border-[color:var(--app-border)] px-3 py-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            <span>Time</span>
            <span>Agent</span>
            <span>Level</span>
            <span>Event</span>
            <span>Tool Used</span>
          </div>
          {rows.length ? (
            rows.map((row, index) => (
              <div
                className="grid grid-cols-[84px_190px_86px_minmax(0,1fr)_220px] gap-3 border-b border-[color:var(--app-border)] px-3 py-2 text-xs last:border-b-0 hover:bg-[var(--app-subtle)]"
                key={`${row.timestamp}-${row.agent}-${index}`}
              >
                <span className="tabular-nums text-muted-foreground">{row.timestamp}</span>
                <span className="truncate font-medium">{row.agent}</span>
                <span className={cn("truncate font-semibold uppercase", logLevelClass(row.level))}>
                  {row.level}
                </span>
                <span className="truncate text-muted-foreground">{row.event}</span>
                <span className="truncate font-mono text-[11px] text-muted-foreground">{row.tool}</span>
              </div>
            ))
          ) : (
            <div className="px-4 py-8 text-center text-xs text-muted-foreground">
              No agent activity recorded yet.
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function LastRunCard({
  onOpenDetails,
  report,
  run,
  workflowName,
}: {
  onOpenDetails?: () => void;
  report?: string;
  run: RunSnapshot;
  workflowName: string;
}) {
  return (
    <SettingsBlock title="Last Run">
      <div className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-3">
        <div className="grid grid-cols-2 gap-2 text-xs">
          <InfoPair label="Status" value={run.status} />
          <InfoPair label="Duration" value={run.durationLabel} />
          <InfoPair label="Started" value={run.startedLabel} />
          <InfoPair label="Workflow" value={workflowName} />
        </div>
        {run.latestError ? (
          <p className="mt-3 rounded border border-red-500/20 bg-red-500/10 p-2 text-[11px] leading-4 text-red-200">
            {run.latestError}
          </p>
        ) : null}
        {report ? (
          <pre className="mt-3 max-h-28 overflow-auto whitespace-pre-wrap break-words rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-2 text-[11px] leading-4 text-muted-foreground">
            {report}
          </pre>
        ) : null}
        <Button
          className="mt-3 h-8 w-full text-xs"
          disabled={!onOpenDetails}
          onClick={onOpenDetails}
          type="button"
          variant="outline"
        >
          View Run Details
        </Button>
      </div>
    </SettingsBlock>
  );
}

function ToolGroups({ tools }: { tools: string[] }) {
  const grouped = groupTools(tools);

  return (
    <>
      {grouped.map(([family, familyTools]) => (
        <SettingsBlock key={family} title={family}>
          <div className="grid gap-1.5">
            {familyTools.map((tool) => (
              <div
                className="min-w-0 break-all rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 py-2 font-mono text-[11px] text-muted-foreground"
                key={tool}
                title={tool}
              >
                {tool}
              </div>
            ))}
          </div>
        </SettingsBlock>
      ))}
    </>
  );
}

function PermissionsList({ compact = false }: { compact?: boolean }) {
  const rows = [
    ["Read", "Allow"],
    ["Write", "Ask"],
    ["Tools", "Allow"],
    ["Network", "Deny"],
    ["Long runs", "Ask"],
  ];

  return (
    <SettingsBlock title="Access">
      <div className="grid min-w-0 gap-1.5">
        {rows.map(([label, value]) => (
          <div
            className={cn(
              "grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 text-xs",
              compact ? "py-1.5" : "py-2",
            )}
            key={label}
          >
            <span className="min-w-0 truncate text-muted-foreground">{label}</span>
            <PermissionValue value={value} />
          </div>
        ))}
      </div>
    </SettingsBlock>
  );
}

function SettingsBlock({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="w-full min-w-0 space-y-2.5">
      <h3 className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {title}
      </h3>
      <div className="w-full min-w-0 space-y-2.5 overflow-hidden">{children}</div>
      <Separator />
    </section>
  );
}

function LabelledField({
  children,
  label,
}: {
  children: React.ReactNode;
  label: string;
}) {
  return (
    <label className="grid min-w-0 gap-1.5 text-[11px] font-medium text-muted-foreground">
      <span>{label}</span>
      {children}
    </label>
  );
}

function ToggleRow({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex h-8 min-w-0 items-center justify-between gap-3 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-xs">
      <span className="min-w-0 truncate">{label}</span>
      <input
        checked={checked}
        className="size-4 accent-primary"
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
    </label>
  );
}

function InfoPair({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="min-w-0">
      <div className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="mt-0.5 truncate text-xs font-medium">{value}</div>
    </div>
  );
}

function MetricTile({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="min-w-0 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2">
      <div className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="mt-1 truncate text-xs font-semibold" title={value}>
        {value}
      </div>
    </div>
  );
}

function PermissionValue({ value }: { value: string }) {
  return (
    <span
      className={cn(
        "min-w-0 max-w-full truncate text-right text-[11px] font-medium",
        value === "Allow" && "text-emerald-300",
        value === "Deny" && "text-red-300",
        value === "Ask" && "text-amber-300",
      )}
      title={permissionTitle(value)}
    >
      {value}
    </span>
  );
}

function RunDetailStatusBadge({ status }: { status: AgentRunDetailStatus }) {
  return (
    <Badge
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px]",
        status === "idle" && "bg-muted text-muted-foreground",
        status === "running" && "bg-sky-500/15 text-sky-300",
        status === "completed" && "bg-emerald-500/15 text-emerald-300",
        status === "blocked" && "bg-red-500/15 text-red-300",
        status === "stopped" && "bg-amber-500/15 text-amber-300",
      )}
      variant="secondary"
    >
      {detailStatusLabel(status)}
    </Badge>
  );
}

function RunDetailEventIcon({ event }: { event: AgentRunDetailEvent }) {
  const iconClass = cn(
    "size-3.5",
    event.level === "error" && "text-red-300",
    event.level === "warning" && "text-amber-300",
    event.level === "trace" && "text-sky-300",
    event.level === "info" && "text-emerald-300",
  );

  return (
    <span className="mt-0.5 grid size-6 place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
      {event.kind === "tool" ? <Wrench className={iconClass} /> : null}
      {event.kind === "model" || event.kind === "reasoning" ? <MessageSquareText className={iconClass} /> : null}
      {event.kind === "error" ? <AlertTriangle className={iconClass} /> : null}
      {event.kind === "status" ? <CheckCircle2 className={iconClass} /> : null}
      {event.kind === "trace" ? <Terminal className={iconClass} /> : null}
    </span>
  );
}

function permissionTitle(value: string) {
  if (value === "Allow") {
    return "Allowed";
  }
  if (value === "Deny") {
    return "Denied";
  }
  if (value === "Ask") {
    return "Requires approval";
  }

  return value;
}

function ProgressGlyph({ state }: { state: string }) {
  if (state === "done") {
    return <CheckCircle2 className="size-3.5 text-emerald-300" />;
  }

  if (state === "active") {
    return <CircleDot className="size-3.5 text-sky-300" />;
  }

  if (state === "blocked") {
    return <X className="size-3.5 text-red-300" />;
  }

  return <span className="ml-0.5 size-2.5 rounded-full border border-muted-foreground/45" />;
}

function AgentIcon({ agent }: { agent: AgentDefinition }) {
  const Icon = agentIcon(agent);

  return (
    <span className="grid size-8 place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] text-muted-foreground">
      <Icon className="size-4" />
    </span>
  );
}

function CategoryBadge({ category }: { category: AgentCategory }) {
  return (
    <Badge className="rounded px-1.5 py-0.5 text-[10px]" variant="secondary">
      {categoryLabel(category)}
    </Badge>
  );
}

function StatusBadge({ status }: { status: AgentStatus }) {
  const display = statusDisplay(status);

  return (
    <Badge
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px] font-semibold",
        display === "Idle" && "bg-muted text-muted-foreground",
        display === "Active" && "bg-emerald-500/15 text-emerald-300",
        display === "Running" && "bg-sky-500/15 text-sky-300",
        display === "Blocked" && "bg-red-500/15 text-red-300",
        display === "Needs Approval" && "bg-amber-500/15 text-amber-300",
      )}
      variant="secondary"
    >
      {display}
    </Badge>
  );
}

function RunStatusBadge({ status }: { status: RunSnapshot["status"] }) {
  return (
    <Badge
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px]",
        status === "Idle" && "bg-muted text-muted-foreground",
        status === "Running" && "bg-sky-500/15 text-sky-300",
        status === "Completed" && "bg-emerald-500/15 text-emerald-300",
        status === "Blocked" && "bg-red-500/15 text-red-300",
        status === "Stopped" && "bg-amber-500/15 text-amber-300",
      )}
      variant="secondary"
    >
      {status}
    </Badge>
  );
}

function SeverityBadge({ severity }: { severity: FindingCandidate["severity"] }) {
  return (
    <Badge
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px]",
        severity === "critical" && "bg-red-500/20 text-red-200",
        severity === "high" && "bg-red-500/15 text-red-300",
        severity === "medium" && "bg-amber-500/15 text-amber-300",
        severity === "low" && "bg-sky-500/15 text-sky-300",
        severity === "info" && "bg-muted text-muted-foreground",
      )}
      variant="secondary"
    >
      {severity}
    </Badge>
  );
}

function confidenceClass(confidence: SecurityEvidenceItem["confidence"]) {
  if (confidence === "confirmed" || confidence === "high") {
    return "text-emerald-300";
  }
  if (confidence === "medium") {
    return "text-sky-300";
  }
  if (confidence === "low") {
    return "text-amber-300";
  }

  return "text-muted-foreground";
}

function markWorkflowStatus(workflow: AgentWorkflow, status: AgentStatus): AgentWorkflow {
  return {
    ...workflow,
    nodes: workflow.nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        status,
      },
    })),
    updatedAt: Date.now(),
  };
}

function syncWorkflowProvider(
  workflow: AgentWorkflow,
  provider: AgentProviderConfig,
): AgentWorkflow {
  return {
    ...workflow,
    nodes: workflow.nodes.map((node) =>
      node.data.nodeType === "agent" || node.data.nodeType === "model"
        ? {
            ...node,
            data: {
              ...node.data,
              provider,
            },
          }
        : node,
    ),
    updatedAt: Date.now(),
    version: workflow.version + 1,
  };
}

function agentMetadata(agent: AgentDefinition): AgentUiMetadata {
  if (agent.kind === "custom") {
    return {
      category: "Custom",
      capabilities: ["Custom workflow", "Configured tools", "Approval policy"],
    };
  }

  return AGENT_METADATA[agent.id] ?? {
    category: categoryForAgent(agent.name),
    capabilities: ["Tool operation", "Evidence review"],
  };
}

function categoryForAgent(name: string): AgentCategory {
  const normalized = name.toLowerCase();

  if (normalized.includes("orchestrator")) {
    return "Core";
  }
  if (normalized.includes("patch") || normalized.includes("test")) {
    return "Action";
  }
  if (normalized.includes("report") || normalized.includes("documentation")) {
    return "Output";
  }

  return "Analysis";
}

function categoryLabel(category: AgentCategory) {
  if (category === "Core") {
    return "Primary";
  }
  if (category === "Analysis") {
    return "Specialist";
  }
  if (category === "Action") {
    return "Action";
  }
  if (category === "Output") {
    return "Report";
  }

  return category;
}

function agentIcon(agent: AgentDefinition) {
  const normalized = agent.name.toLowerCase();

  if (normalized.includes("orchestrator")) {
    return Workflow;
  }
  if (normalized.includes("static")) {
    return ShieldCheck;
  }
  if (normalized.includes("dynamic")) {
    return Activity;
  }
  if (normalized.includes("graph")) {
    return Network;
  }
  if (normalized.includes("bytecode")) {
    return Binary;
  }
  if (normalized.includes("invariant")) {
    return CheckCircle2;
  }
  if (normalized.includes("patch")) {
    return Hammer;
  }
  if (normalized.includes("test")) {
    return BarChart3;
  }
  if (normalized.includes("report") || normalized.includes("document")) {
    return FileText;
  }

  return Bot;
}

function statusDisplay(status: AgentStatus) {
  if (status === "active" || status === "completed") {
    return "Active";
  }
  if (status === "running") {
    return "Running";
  }
  if (status === "blocked" || status === "failed") {
    return "Blocked";
  }
  if (status === "needsApproval") {
    return "Needs Approval";
  }

  return "Idle";
}

function groupTools(tools: string[]): Array<[string, string[]]> {
  const groups = new Map<string, string[]>();

  for (const tool of tools) {
    const familyKey = Object.keys(TOOL_FAMILY_LABELS).find((prefix) => tool.startsWith(prefix));
    const family = familyKey ? TOOL_FAMILY_LABELS[familyKey] : "Other";
    groups.set(family, [...(groups.get(family) ?? []), tool]);
  }

  return Array.from(groups.entries());
}

function activityRows(logs: AgentExecutionLog[], agents: AgentDefinition[]): ActivityRow[] {
  return logs.slice(-12).reverse().map((log) => {
    const agent = agents.find((candidate) => candidate.id === log.agentId);
    const tool = inferToolFromMessage(log.message) ?? "none";

    return {
      timestamp: new Date(log.timestamp).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      }),
      agent: agent?.name ?? "Agent",
      event: compactEvent(log.message),
      level: log.level,
      tool,
    };
  });
}

function inferToolFromMessage(message: string) {
  return message.match(/rust\.[a-z_.]+/)?.[0];
}

function compactEvent(message: string) {
  const compact = message.replace(/^Agent report:\n/, "Produced agent report").replace(/\s+/g, " ").trim();

  return compact.length > 92 ? `${compact.slice(0, 89)}...` : compact;
}

function lastRunLabel(logs: AgentExecutionLog[]) {
  if (!logs.length) {
    return "Never";
  }

  const latest = logs.reduce((max, log) => Math.max(max, log.timestamp), 0);
  return relativeTimeLabel(latest);
}

function agentLastRunLabel(logs: AgentExecutionLog[], agent: AgentDefinition) {
  const latest = logs
    .filter((log) => log.agentId === agent.id)
    .reduce((max, log) => Math.max(max, log.timestamp), 0);

  return latest ? relativeTimeLabel(latest) : "Never";
}

function createRunSnapshot({
  agent,
  logs,
  workflow,
}: {
  agent: AgentDefinition;
  logs: AgentExecutionLog[];
  workflow: AgentWorkflow;
}): RunSnapshot {
  const workflowLogs = logs.filter((log) => log.workflowId === workflow.id);
  const latestLog = workflowLogs[workflowLogs.length - 1];
  const startedLog = [...workflowLogs].reverse().find((log) => /\bstarted\b/i.test(log.message));
  const completedLog = [...workflowLogs].reverse().find((log) => /\bcompleted\b/i.test(log.message));
  const errorLog = [...workflowLogs].reverse().find((log) => log.level === "error");
  const warningCount = workflowLogs.filter((log) => log.level === "warning").length;
  const issueCount = workflowLogs.reduce((total, log) => total + countIssuesInText(log.message), 0);
  const evidenceArtifacts = extractEvidenceArtifacts(workflowLogs);
  const status = runStatusFromState(agent.status, workflowLogs, workflow);
  const startedAt = startedLog?.timestamp ?? workflowLogs[0]?.timestamp;
  const endedAt = completedLog?.timestamp ?? errorLog?.timestamp ?? latestLog?.timestamp;

  return {
    activeTool: latestLog ? inferToolFromMessage(latestLog.message) ?? "none" : "none",
    durationLabel: startedAt && endedAt && endedAt >= startedAt
      ? durationLabel(endedAt - startedAt)
      : "Not recorded",
    evidenceArtifacts,
    issueCount,
    latestError: errorLog ? compactEvent(errorLog.message) : undefined,
    startedLabel: startedAt ? relativeTimeLabel(startedAt) : "Never",
    status,
    steps: workflow.nodes.map((node) => ({
      label: node.data.label,
      state: stepStateFromStatus(node.data.status),
    })),
    warningCount,
  };
}

function runStatusFromState(
  agentStatus: AgentStatus,
  logs: AgentExecutionLog[],
  workflow: AgentWorkflow,
): RunSnapshot["status"] {
  if (agentStatus === "running") {
    return "Running";
  }
  if (agentStatus === "blocked" || agentStatus === "failed") {
    return "Blocked";
  }

  const latestLog = logs[logs.length - 1];

  if (!latestLog && workflow.nodes.every((node) => node.data.status === "idle")) {
    return "Idle";
  }
  if (latestLog?.level === "error" || workflow.nodes.some((node) => node.data.status === "blocked" || node.data.status === "failed")) {
    return "Blocked";
  }
  if (latestLog?.level === "warning" && /stopped/i.test(latestLog.message)) {
    return "Stopped";
  }
  if (logs.some((log) => /\bcompleted\b/i.test(log.message)) || workflow.nodes.some((node) => node.data.status === "completed")) {
    return "Completed";
  }

  return "Idle";
}

function stepStateFromStatus(status: AgentStatus): RunSnapshot["steps"][number]["state"] {
  if (status === "running") {
    return "active";
  }
  if (status === "completed" || status === "active") {
    return "done";
  }
  if (status === "blocked" || status === "failed") {
    return "blocked";
  }

  return "pending";
}

function countIssuesInText(text: string) {
  const issueMatch = text.match(/\b(\d+)\s+(?:issues?|findings?)\b/i);

  return issueMatch ? Number(issueMatch[1]) : 0;
}

function extractEvidenceArtifacts(logs: AgentExecutionLog[]) {
  const artifacts = new Set<string>();
  const artifactPattern = /\b[\w.-]+\.(?:json|graph|dot|md|txt|trace)\b/g;

  for (const log of logs) {
    for (const match of log.message.matchAll(artifactPattern)) {
      artifacts.add(match[0]);
    }
  }

  return Array.from(artifacts);
}

function createHarnessRoutePreview({
  agent,
  projectContext,
  workflow,
}: {
  agent: AgentDefinition;
  projectContext: AgentToolProjectContext | null;
  workflow: AgentWorkflow;
}): HarnessRoutePreview {
  const toolState = createAgentToolRuntimeState(
    projectContext ?? {
      rootPath: "",
      packagePath: ".",
      packageName: workflow.name,
      manifestPath: "",
      packageTree: null,
    },
  );
  const routePlan = routeTools(resolveAgentTools(toolState, agent.tools), {
    activeToolIds: agent.tools,
    objective: agent.description || workflow.description,
    role: agentRoleForHarnessPreview(agent, workflow),
  });

  return {
    capsules: routePlan.capsules,
    decisions: routePlan.decisions,
  };
}

function agentRoleForHarnessPreview(
  agent: AgentDefinition,
  workflow: AgentWorkflow,
): AgentRole {
  const text = `${agent.name} ${agent.description} ${workflow.name} ${workflow.description}`.toLowerCase();

  if (text.includes("test")) {
    return "testGeneration";
  }
  if (text.includes("fuzz")) {
    return "fuzzCampaign";
  }
  if (text.includes("formal") || text.includes("spec")) {
    return "formalSpec";
  }
  if (text.includes("patch")) {
    return "patch";
  }
  if (text.includes("document") || text.includes("report")) {
    return "report";
  }
  if (text.includes("triage")) {
    return "triage";
  }
  if (text.includes("ci")) {
    return "ci";
  }

  return "securityReview";
}

function categoryCounts(capsules: ToolCapsule[]) {
  const counts = new Map<string, number>();

  for (const capsule of capsules) {
    counts.set(capsule.category, (counts.get(capsule.category) ?? 0) + 1);
  }

  return Array.from(counts.entries());
}

function mergeById<T extends { id: string }>(existing: T[], incoming: T[]) {
  const merged = new Map(existing.map((item) => [item.id, item] as const));

  for (const item of incoming) {
    merged.set(item.id, item);
  }

  return Array.from(merged.values());
}

function extractToolResultEvidence(output: unknown): {
  evidence: SecurityEvidenceItem[];
  findingCandidates: FindingCandidate[];
} {
  const record = asRecord(output);

  return {
    evidence: asSecurityEvidenceItems(record?.evidence ?? record?.securityEvidence),
    findingCandidates: asFindingCandidates(record?.findingCandidates),
  };
}

function asSecurityEvidenceItems(value: unknown): SecurityEvidenceItem[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter((item): item is SecurityEvidenceItem => {
    const record = asRecord(item);
    return Boolean(
      record
        && typeof record.id === "string"
        && typeof record.kind === "string"
        && typeof record.claim === "string"
        && typeof record.observation === "string"
        && typeof record.confidence === "string",
    );
  });
}

function asFindingCandidates(value: unknown): FindingCandidate[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter((item): item is FindingCandidate => {
    const record = asRecord(item);
    return Boolean(
      record
        && typeof record.id === "string"
        && typeof record.title === "string"
        && typeof record.severity === "string"
        && typeof record.confidence === "string"
        && typeof record.status === "string",
    );
  });
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function createRunDetailId(agentId: string) {
  return `${agentId}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function createRunDetailEvent(
  event: Omit<AgentRunDetailEvent, "id" | "timestamp">,
  timestamp: number,
): AgentRunDetailEvent {
  return {
    ...event,
    id: `event-${timestamp}-${Math.random().toString(36).slice(2, 8)}`,
    timestamp,
  };
}

function detailStatusLabel(status: AgentRunDetailStatus) {
  if (status === "running") {
    return "Running";
  }
  if (status === "completed") {
    return "Completed";
  }
  if (status === "blocked") {
    return "Blocked";
  }
  if (status === "stopped") {
    return "Stopped";
  }

  return "Idle";
}

function displayStreamToolName(toolName: string) {
  return toolName.replace(/_/g, ".");
}

function formatDetailPayload(payload: unknown, maxLength: number) {
  if (payload === undefined) {
    return "";
  }

  if (typeof payload === "string") {
    return formatTraceText(payload, maxLength);
  }

  try {
    return formatTraceText(JSON.stringify(payload), maxLength);
  } catch {
    return formatTraceText(String(payload), maxLength);
  }
}

function formatTraceText(text: string, maxLength: number) {
  const compact = text.replace(/\s+/g, " ").trim();

  if (compact.length <= maxLength) {
    return compact;
  }

  return `${compact.slice(0, maxLength)}...`;
}

function timeLabel(timestamp: number) {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function relativeTimeLabel(timestamp: number) {
  const minutes = Math.max(1, Math.round((Date.now() - timestamp) / 60_000));

  if (minutes < 60) {
    return `${minutes}m ago`;
  }

  const hours = Math.round(minutes / 60);

  if (hours < 24) {
    return `${hours}h ago`;
  }

  return `${Math.round(hours / 24)}d ago`;
}

function durationLabel(durationMs: number) {
  const seconds = Math.max(1, Math.round(durationMs / 1000));

  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;

  return remainingSeconds ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`;
}

function logLevelClass(level: AgentExecutionLog["level"]) {
  if (level === "error") {
    return "text-red-300";
  }
  if (level === "warning") {
    return "text-amber-300";
  }
  if (level === "trace") {
    return "text-sky-300";
  }

  return "text-emerald-300";
}

function latestAgentReport(logs: AgentExecutionLog[]) {
  const reportLog = [...logs].reverse().find((log) => log.message.startsWith("Agent report:\n"));

  return reportLog?.message.replace(/^Agent report:\n/, "").trim();
}

function isAbortError(error: unknown) {
  return (
    error instanceof DOMException && error.name === "AbortError"
  ) || (
    typeof error === "object"
    && error !== null
    && "name" in error
    && (error as { name?: unknown }).name === "AbortError"
  );
}
