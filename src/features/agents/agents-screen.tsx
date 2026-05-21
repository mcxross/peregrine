import React from "react";
import {
  Bot,
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Copy,
  Database,
  FileText,
  Gauge,
  GitBranch,
  Hammer,
  ListRestart,
  Pause,
  Play,
  Plus,
  ShieldCheck,
  Square,
  Trash2,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { AgentWorkflowCanvas } from "@/features/agents/agent-workflow-canvas";
import {
  agentStudioStateToProjectMetadata,
  createCustomAgent,
  createExecutionLog,
  duplicateAgent,
  loadAgentStudioState,
  loadAgentStudioStateFromProjectMetadata,
  saveAgentStudioState,
} from "@/features/agents/agent-workflow-store";
import {
  modelProviderAdapters,
  providerById,
  providerModelOptions,
} from "@/features/agents/model-providers/provider-adapters";
import type {
  AgentDefinition,
  AgentExecutionLog,
  AgentProviderConfig,
  AgentStatus,
  AgentStudioState,
  AgentWorkflow,
} from "@/features/agents/types";
import {
  loadProjectMetadata,
  saveProjectMetadata,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

export function AgentsScreen({
  projectRootPath,
}: {
  projectRootPath?: string;
}) {
  const [state, setState] = React.useState<AgentStudioState>(() => loadAgentStudioState());
  const [isLibraryPanelOpen, setIsLibraryPanelOpen] = React.useState(true);
  const [isConfigPanelOpen, setIsConfigPanelOpen] = React.useState(true);
  const [tracePanelHeight, setTracePanelHeight] = React.useState(260);
  const activeRunControllerRef = React.useRef<AbortController | null>(null);
  const [isProjectStateLoaded, setIsProjectStateLoaded] = React.useState(false);
  const selectedAgent = state.agents.find((agent) => agent.id === state.selectedAgentId) ?? state.agents[0];
  const selectedWorkflow =
    state.workflows.find((workflow) => workflow.id === state.selectedWorkflowId)
    ?? state.workflows.find((workflow) => workflow.id === selectedAgent?.workflowId)
    ?? state.workflows[0];

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

  if (!selectedAgent || !selectedWorkflow) {
    return null;
  }

  const agentLogs = state.logs
    .filter((log) => log.agentId === selectedAgent.id || log.workflowId === selectedWorkflow.id)
    .slice(-14)
    .reverse();

  return (
    <div
      className={cn(
        "grid h-full min-h-0 overflow-hidden bg-[var(--app-window)] text-foreground transition-[grid-template-columns] duration-200",
        isLibraryPanelOpen && isConfigPanelOpen && "grid-cols-[300px_minmax(0,1fr)_380px]",
        isLibraryPanelOpen && !isConfigPanelOpen && "grid-cols-[300px_minmax(0,1fr)_48px]",
        !isLibraryPanelOpen && isConfigPanelOpen && "grid-cols-[48px_minmax(0,1fr)_380px]",
        !isLibraryPanelOpen && !isConfigPanelOpen && "grid-cols-[48px_minmax(0,1fr)_48px]",
      )}
    >
      <AgentLibrary
        agents={state.agents}
        isOpen={isLibraryPanelOpen}
        onCreateAgent={() => {
          const created = createCustomAgent();
          setState((current) => ({
            ...current,
            agents: [...current.agents, created.agent],
            workflows: [...current.workflows, created.workflow],
            selectedAgentId: created.agent.id,
            selectedWorkflowId: created.workflow.id,
          }));
        }}
        onDeleteAgent={(agent) => {
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
        }}
        onDuplicateAgent={(agent) => {
          const workflow = state.workflows.find((candidate) => candidate.id === agent.workflowId);

          if (!workflow) {
            return;
          }

          const duplicated = duplicateAgent(agent, workflow);
          setState((current) => ({
            ...current,
            agents: [...current.agents, duplicated.agent],
            workflows: [...current.workflows, duplicated.workflow],
            selectedAgentId: duplicated.agent.id,
            selectedWorkflowId: duplicated.workflow.id,
          }));
        }}
        onSelectAgent={(agent) =>
          setState((current) => ({
            ...current,
            selectedAgentId: agent.id,
            selectedWorkflowId: agent.workflowId,
          }))
        }
        onToggleOpen={() => setIsLibraryPanelOpen((current) => !current)}
        selectedAgentId={selectedAgent.id}
        workflows={state.workflows}
      />

      <section
        className="grid min-h-0 overflow-hidden border-r border-[color:var(--app-border)]"
        style={{
          gridTemplateRows: `auto minmax(0, 1fr) ${tracePanelHeight}px`,
        }}
      >
        <WorkflowToolbar
          agent={selectedAgent}
          onPause={() => updateAgentStatus("idle")}
          onReset={() => resetWorkflowRun()}
          onRun={() => runWorkflow()}
          onStop={() => stopWorkflowRun()}
          workflow={selectedWorkflow}
        />
        <AgentWorkflowCanvas
          onRunNode={(nodeId) => runNode(nodeId)}
          onWorkflowChange={(workflow) => updateWorkflow(workflow)}
          workflow={selectedWorkflow}
        />
        <TracePanel
          height={tracePanelHeight}
          logs={agentLogs}
          onResize={setTracePanelHeight}
          report={latestAgentReport(agentLogs)}
        />
      </section>

      <AgentConfigPanel
        agent={selectedAgent}
        isOpen={isConfigPanelOpen}
        onToggleOpen={() => setIsConfigPanelOpen((current) => !current)}
        onUpdateAgent={(patch) => updateAgent(selectedAgent.id, patch)}
        workflow={selectedWorkflow}
      />
    </div>
  );

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

  function updateAgentStatus(status: AgentStatus) {
    updateAgent(selectedAgent.id, { status });
    appendLog({
      agentId: selectedAgent.id,
      workflowId: selectedWorkflow.id,
      level: status === "failed" ? "error" : "info",
      message: `Agent ${status}.`,
    });
  }

  async function runWorkflow() {
    if (activeRunControllerRef.current) {
      return;
    }

    const runAgent = selectedAgent;
    const runWorkflowState = selectedWorkflow;
    const controller = new AbortController();
    activeRunControllerRef.current = controller;

    updateAgent(selectedAgent.id, { status: "running" });
    const runningWorkflow = markWorkflowStatus(selectedWorkflow, "running");
    updateWorkflow(runningWorkflow);
    appendLog({
      agentId: selectedAgent.id,
      workflowId: selectedWorkflow.id,
      level: "trace",
      message: `Workflow started with ${selectedWorkflow.nodes.length} nodes. Calling ${selectedAgent.provider.providerId}/${selectedAgent.provider.modelId}.`,
    });

    try {
      const { runAgentWorkflowWithModel } = await import("@/features/agents/agent-runner");
      const result = await runAgentWorkflowWithModel({
        agent: runAgent,
        onTrace: (event) => {
          appendRunLog(runAgent.id, runWorkflowState.id, event);
        },
        signal: controller.signal,
        workflow: runWorkflowState,
      });

      setState((current) => ({
        ...current,
        agents: current.agents.map((agent) =>
          agent.id === runAgent.id ? { ...agent, status: "completed" } : agent,
        ),
        workflows: current.workflows.map((workflow) =>
          workflow.id === runWorkflowState.id ? markWorkflowStatus(workflow, "completed") : workflow,
        ),
        logs: [
          ...current.logs,
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
            message: "Workflow completed.",
          }),
        ].slice(-120),
      }));
    } catch (error) {
      const aborted = isAbortError(error);
      setState((current) => ({
        ...current,
        agents: current.agents.map((agent) =>
          agent.id === runAgent.id ? { ...agent, status: aborted ? "idle" : "failed" } : agent,
        ),
        workflows: current.workflows.map((workflow) =>
          workflow.id === runWorkflowState.id
            ? markWorkflowStatus(workflow, aborted ? "idle" : "failed")
            : workflow,
        ),
        logs: [
          ...current.logs,
          createExecutionLog({
            agentId: runAgent.id,
            workflowId: runWorkflowState.id,
            level: aborted ? "warning" : "error",
            message: aborted
              ? "Workflow run stopped before the model completed."
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

  function runNode(nodeId: string) {
    const workflow = {
      ...selectedWorkflow,
      nodes: selectedWorkflow.nodes.map((node) =>
        node.id === nodeId
          ? { ...node, data: { ...node.data, status: "completed" as const } }
          : node,
      ),
    };

    updateWorkflow(workflow);
    appendLog({
      agentId: selectedAgent.id,
      workflowId: selectedWorkflow.id,
      nodeId,
      level: "trace",
      message: `Node completed: ${nodeId}`,
    });
  }

  function resetWorkflowRun() {
    activeRunControllerRef.current?.abort();
    updateAgent(selectedAgent.id, { status: "idle" });
    updateWorkflow(markWorkflowStatus(selectedWorkflow, "idle"));
    appendLog({
      agentId: selectedAgent.id,
      workflowId: selectedWorkflow.id,
      level: "info",
      message: "Workflow reset.",
    });
  }

  function appendLog(log: Omit<AgentExecutionLog, "id" | "timestamp">) {
    setState((current) => ({
      ...current,
      logs: [...current.logs, createExecutionLog(log)].slice(-120),
    }));
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

    updateAgentStatus("failed");
  }
}

function AgentLibrary({
  agents,
  isOpen,
  onCreateAgent,
  onDeleteAgent,
  onDuplicateAgent,
  onSelectAgent,
  onToggleOpen,
  selectedAgentId,
  workflows,
}: {
  agents: AgentDefinition[];
  isOpen: boolean;
  onCreateAgent: () => void;
  onDeleteAgent: (agent: AgentDefinition) => void;
  onDuplicateAgent: (agent: AgentDefinition) => void;
  onSelectAgent: (agent: AgentDefinition) => void;
  onToggleOpen: () => void;
  selectedAgentId: string;
  workflows: AgentWorkflow[];
}) {
  const defaultAgents = agents.filter((agent) => agent.kind === "default");
  const customAgents = agents.filter((agent) => agent.kind === "custom");

  return (
    <aside className="grid min-h-0 grid-rows-[auto_1fr] border-r border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <header className="border-b border-[color:var(--app-border)] px-3 py-3">
        <div
          className={cn(
            "flex items-center gap-2",
            isOpen ? "justify-between" : "justify-center",
          )}
        >
          <div className="min-w-0">
            <h1 className={cn("truncate text-sm font-semibold", !isOpen && "sr-only")}>
              Agents
            </h1>
            <p className={cn("mt-0.5 truncate text-[11px] text-muted-foreground", !isOpen && "sr-only")}>
              {agents.length} agents / {workflows.length} workflows
            </p>
          </div>
          {isOpen ? (
            <div className="flex items-center gap-1.5">
              <IconButton label="Collapse agents library" onClick={onToggleOpen}>
                <ChevronLeft className="size-3.5" />
              </IconButton>
              <Button className="h-8 gap-1.5 text-xs" onClick={onCreateAgent} size="sm" type="button">
                <Plus className="size-3.5" />
                New
              </Button>
            </div>
          ) : (
            <IconButton label="Expand agents library" onClick={onToggleOpen}>
              <ChevronRight className="size-3.5" />
            </IconButton>
          )}
        </div>
      </header>
      {isOpen ? (
      <ScrollArea className="min-h-0">
        <div className="space-y-4 p-3">
          <AgentGroup
            agents={defaultAgents}
            label="Default agents"
            onDeleteAgent={onDeleteAgent}
            onDuplicateAgent={onDuplicateAgent}
            onSelectAgent={onSelectAgent}
            selectedAgentId={selectedAgentId}
          />
          <AgentGroup
            agents={customAgents}
            emptyLabel="No custom agents"
            label="User-defined agents"
            onDeleteAgent={onDeleteAgent}
            onDuplicateAgent={onDuplicateAgent}
            onSelectAgent={onSelectAgent}
            selectedAgentId={selectedAgentId}
          />
        </div>
      </ScrollArea>
      ) : (
        <button
          aria-label="Expand agents library"
          className="flex min-h-0 items-start justify-center px-2 py-4 text-muted-foreground transition hover:text-foreground"
          onClick={onToggleOpen}
          title="Expand agents library"
          type="button"
        >
          <span className="text-[10px] font-semibold uppercase tracking-wide [writing-mode:vertical-rl]">
            Agents
          </span>
        </button>
      )}
    </aside>
  );
}

function AgentGroup({
  agents,
  emptyLabel,
  label,
  onDeleteAgent,
  onDuplicateAgent,
  onSelectAgent,
  selectedAgentId,
}: {
  agents: AgentDefinition[];
  emptyLabel?: string;
  label: string;
  onDeleteAgent: (agent: AgentDefinition) => void;
  onDuplicateAgent: (agent: AgentDefinition) => void;
  onSelectAgent: (agent: AgentDefinition) => void;
  selectedAgentId: string;
}) {
  return (
    <section>
      <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </h2>
      <div className="space-y-1.5">
        {agents.length ? (
          agents.map((agent) => (
            <button
              className={cn(
                "grid w-full min-w-0 gap-2 rounded-md border p-2 text-left transition",
                selectedAgentId === agent.id
                  ? "border-primary/45 bg-[var(--app-subtle)]"
                  : "border-[color:var(--app-border)] bg-[var(--app-surface)] hover:bg-[var(--app-subtle)]",
              )}
              key={agent.id}
              onClick={() => onSelectAgent(agent)}
              type="button"
            >
              <span className="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2">
                <AgentIcon agent={agent} />
                <span className="min-w-0">
                  <span className="block truncate text-xs font-semibold text-foreground">
                    {agent.name}
                  </span>
                  <span className="block truncate text-[11px] text-muted-foreground">
                    {providerById(agent.provider.providerId).label} / {agent.provider.modelId}
                  </span>
                </span>
                <StatusBadge status={agent.status} />
              </span>
              <span className="line-clamp-2 text-[11px] leading-4 text-muted-foreground">
                {agent.description}
              </span>
              <span className="flex items-center gap-1">
                <Button
                  className="h-6 px-2 text-[10px]"
                  onClick={(event) => {
                    event.stopPropagation();
                    onDuplicateAgent(agent);
                  }}
                  type="button"
                  variant="secondary"
                >
                  <Copy className="mr-1 size-3" />
                  Duplicate
                </Button>
                {agent.kind === "custom" ? (
                  <Button
                    className="h-6 px-2 text-[10px]"
                    onClick={(event) => {
                      event.stopPropagation();
                      onDeleteAgent(agent);
                    }}
                    type="button"
                    variant="ghost"
                  >
                    <Trash2 className="mr-1 size-3" />
                    Delete
                  </Button>
                ) : null}
              </span>
            </button>
          ))
        ) : (
          <div className="rounded-md border border-dashed border-[color:var(--app-border)] px-3 py-4 text-center text-xs text-muted-foreground">
            {emptyLabel}
          </div>
        )}
      </div>
    </section>
  );
}

function WorkflowToolbar({
  agent,
  onPause,
  onReset,
  onRun,
  onStop,
  workflow,
}: {
  agent: AgentDefinition;
  onPause: () => void;
  onReset: () => void;
  onRun: () => void;
  onStop: () => void;
  workflow: AgentWorkflow;
}) {
  return (
    <header className="grid min-h-14 grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] px-4">
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <h2 className="truncate text-sm font-semibold">{workflow.name}</h2>
          <StatusBadge status={agent.status} />
          <Badge className="rounded px-1.5 py-0.5 text-[10px]" variant="secondary">
            Serializable
          </Badge>
        </div>
      </div>
      <div className="flex items-center gap-1.5">
        <Button className="h-8 gap-1.5 text-xs" onClick={onRun} type="button">
          <Play className="size-3.5" />
          Run
        </Button>
        <IconButton label="Pause" onClick={onPause}>
          <Pause className="size-3.5" />
        </IconButton>
        <IconButton label="Stop" onClick={onStop}>
          <Square className="size-3.5" />
        </IconButton>
        <IconButton label="Reset" onClick={onReset}>
          <ListRestart className="size-3.5" />
        </IconButton>
      </div>
    </header>
  );
}

function AgentConfigPanel({
  agent,
  isOpen,
  onToggleOpen,
  onUpdateAgent,
  workflow,
}: {
  agent: AgentDefinition;
  isOpen: boolean;
  onToggleOpen: () => void;
  onUpdateAgent: (patch: Partial<AgentDefinition>) => void;
  workflow: AgentWorkflow;
}) {
  const provider = providerById(agent.provider.providerId);
  const modelOptions = providerModelOptions(agent.provider);
  const canEditIdentity = agent.kind === "custom";

  return (
    <aside
      className={cn(
        "grid min-h-0 border-l border-[color:var(--app-border)] bg-[var(--app-panel)] transition-[width] duration-200",
        isOpen ? "grid-rows-[auto_1fr]" : "grid-rows-1",
      )}
    >
      <header className="border-b border-[color:var(--app-border)] px-4 py-3">
        <div
          className={cn(
            "flex items-center gap-2",
            isOpen ? "justify-between" : "justify-center",
          )}
        >
          <div className="min-w-0">
            <h2 className={cn("truncate text-sm font-semibold", !isOpen && "sr-only")}>
              Agent settings
            </h2>
            <p className={cn("mt-0.5 truncate text-[11px] text-muted-foreground", !isOpen && "sr-only")}>
              {workflow.nodes.length} nodes / {agent.tools.length} tools
            </p>
          </div>
          <IconButton
            label={isOpen ? "Collapse agent settings" : "Expand agent settings"}
            onClick={onToggleOpen}
          >
            {isOpen ? <ChevronRight className="size-3.5" /> : <ChevronLeft className="size-3.5" />}
          </IconButton>
        </div>
      </header>
      {isOpen ? (
      <ScrollArea className="min-h-0">
        <div className="space-y-5 p-4">
          <SettingsSection title="Identity">
            <LabelledField label="Name">
              <Input
                disabled={!canEditIdentity}
                onChange={(event) => onUpdateAgent({ name: event.target.value })}
                value={agent.name}
              />
            </LabelledField>
            <LabelledField label="Description">
              <textarea
                className="min-h-20 w-full resize-none rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                disabled={!canEditIdentity}
                onChange={(event) => onUpdateAgent({ description: event.target.value })}
                value={agent.description}
              />
            </LabelledField>
          </SettingsSection>

          <SettingsSection title="Model provider">
            <LabelledField label="Provider">
              <select
                className="h-9 w-full rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-sm outline-none"
                onChange={(event) => {
                  const nextProvider = providerById(event.target.value);
                  onUpdateAgent({
                    provider: {
                      providerId: nextProvider.id,
                      modelId: nextProvider.defaultModelId,
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
            {provider.id === "ollama" ? (
              <LabelledField label="Ollama endpoint">
                <Input
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
            <LabelledField label="Model">
              <select
                className="h-9 w-full rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-sm outline-none"
                onChange={(event) =>
                  onUpdateAgent({
                    provider: {
                      ...agent.provider,
                      modelId: event.target.value,
                    },
                  })
                }
                value={agent.provider.modelId}
              >
                {modelOptions.map((modelId) => (
                  <option key={modelId} value={modelId}>
                    {modelId}
                  </option>
                ))}
              </select>
            </LabelledField>
          </SettingsSection>

          <SettingsSection title="System prompt">
            <textarea
              className="min-h-36 w-full resize-none rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2 text-xs leading-5 outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              onChange={(event) => onUpdateAgent({ systemPrompt: event.target.value })}
              value={agent.systemPrompt}
            />
          </SettingsSection>

          <SettingsSection title="Tools">
            <div className="flex flex-wrap gap-1.5">
              {agent.tools.map((tool) => (
                <Badge className="rounded px-2 py-1 text-[11px]" key={tool} variant="secondary">
                  <Hammer className="mr-1 size-3" />
                  {tool}
                </Badge>
              ))}
            </div>
            <Input
              onBlur={(event) => {
                const tools = event.target.value
                  .split(",")
                  .map((tool) => tool.trim())
                  .filter(Boolean);

                if (tools.length) {
                  onUpdateAgent({ tools });
                }
              }}
              placeholder="tool.id, another.tool"
            />
          </SettingsSection>

          <SettingsSection title="Execution">
            <div className="grid grid-cols-2 gap-2">
              <LabelledField label="Mode">
                <select
                  className="h-9 w-full rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-sm outline-none"
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
            </div>
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
          </SettingsSection>
        </div>
      </ScrollArea>
      ) : (
        <button
          aria-label="Expand agent settings"
          className="flex min-h-0 items-start justify-center px-2 py-4 text-muted-foreground transition hover:text-foreground"
          onClick={onToggleOpen}
          title="Expand agent settings"
          type="button"
        >
          <span className="text-[10px] font-semibold uppercase tracking-wide [writing-mode:vertical-rl]">
            Agent settings
          </span>
        </button>
      )}
    </aside>
  );
}

function TracePanel({
  height,
  logs,
  onResize,
  report,
}: {
  height: number;
  logs: AgentExecutionLog[];
  onResize: (height: number) => void;
  report?: string;
}) {
  const panelRef = React.useRef<HTMLElement | null>(null);
  const resizeStateRef = React.useRef<{
    maxHeight: number;
    startHeight: number;
    startY: number;
  } | null>(null);

  React.useEffect(() => {
    return () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
  }, []);

  return (
    <section
      className="relative grid min-h-0 grid-rows-[auto_1fr] border-t border-[color:var(--app-border)] bg-[var(--app-panel)]"
      ref={panelRef}
    >
      <button
        aria-label="Resize execution logs"
        className="absolute left-0 right-0 top-0 z-10 h-2 cursor-row-resize bg-transparent transition hover:bg-primary/25"
        onMouseDown={startResize}
        title="Drag to resize execution logs"
        type="button"
      />
      <header className="flex items-center justify-between border-b border-[color:var(--app-border)] px-4 py-2">
        <div className="flex items-center gap-2 text-xs font-semibold">
          <Database className="size-3.5 text-primary" />
          Execution logs
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] tabular-nums text-muted-foreground">
            {height}px
          </span>
          <Badge className="rounded px-1.5 py-0.5 text-[10px]" variant="secondary">
            Trace view
          </Badge>
        </div>
      </header>
      <ScrollArea className="min-h-0">
        <div className="grid gap-2 p-3">
          {report ? (
            <section className="rounded border border-primary/25 bg-primary/5 p-2">
              <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-primary">
                Agent report
              </div>
              <pre className="max-h-28 overflow-auto whitespace-pre-wrap break-words text-[11px] leading-4 text-foreground">
                {report}
              </pre>
            </section>
          ) : null}
          {logs.length ? (
            logs.map((log) => (
              <div
                className="grid grid-cols-[72px_72px_minmax(0,1fr)] items-start gap-2 rounded border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 py-1.5 text-[11px]"
                key={log.id}
              >
                <span className="tabular-nums text-muted-foreground">
                  {new Date(log.timestamp).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                    second: "2-digit",
                  })}
                </span>
                <span className={cn("font-semibold uppercase", logLevelClass(log.level))}>
                  {log.level}
                </span>
                <span className="max-h-16 min-w-0 overflow-auto whitespace-pre-wrap break-words text-muted-foreground" title={log.message}>
                  {log.message}
                </span>
              </div>
            ))
          ) : (
            <div className="rounded-md border border-dashed border-[color:var(--app-border)] px-3 py-5 text-center text-xs text-muted-foreground">
              No logs
            </div>
          )}
        </div>
      </ScrollArea>
    </section>
  );

  function startResize(event: React.MouseEvent<HTMLButtonElement>) {
    event.preventDefault();
    const parentHeight = panelRef.current?.parentElement?.getBoundingClientRect().height ?? window.innerHeight;
    resizeStateRef.current = {
      maxHeight: Math.max(220, parentHeight - 180),
      startHeight: height,
      startY: event.clientY,
    };
    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", resize);
    window.addEventListener("mouseup", stopResize, { once: true });
  }

  function resize(event: MouseEvent) {
    const state = resizeStateRef.current;

    if (!state) {
      return;
    }

    const deltaY = state.startY - event.clientY;
    onResize(clamp(state.startHeight + deltaY, 160, state.maxHeight));
  }

  function stopResize() {
    resizeStateRef.current = null;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    window.removeEventListener("mousemove", resize);
  }
}

function SettingsSection({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="space-y-3">
      <h3 className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {title}
      </h3>
      <div className="space-y-3">{children}</div>
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
    <label className="grid gap-1.5 text-[11px] font-medium text-muted-foreground">
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
    <label className="flex h-9 items-center justify-between rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 text-xs">
      <span>{label}</span>
      <input
        checked={checked}
        className="size-4 accent-primary"
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
    </label>
  );
}

function IconButton({
  children,
  label,
  onClick,
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      aria-label={label}
      className="size-8 text-muted-foreground hover:text-foreground"
      onClick={onClick}
      size="icon-sm"
      title={label}
      type="button"
      variant="ghost"
    >
      {children}
    </Button>
  );
}

function AgentIcon({ agent }: { agent: AgentDefinition }) {
  const Icon = agentIcon(agent.name);

  return (
    <span className="grid size-8 place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] text-primary">
      <Icon className="size-4" />
    </span>
  );
}

function StatusBadge({ status }: { status: AgentStatus }) {
  return (
    <Badge
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px] font-semibold",
        status === "idle" && "bg-muted text-muted-foreground",
        status === "running" && "bg-sky-500/15 text-sky-300",
        status === "completed" && "bg-emerald-500/15 text-emerald-300",
        status === "failed" && "bg-red-500/15 text-red-300",
      )}
      variant="secondary"
    >
      {status}
    </Badge>
  );
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

function agentIcon(name: string) {
  const normalized = name.toLowerCase();

  if (normalized.includes("security")) {
    return ShieldCheck;
  }
  if (normalized.includes("index")) {
    return GitBranch;
  }
  if (normalized.includes("document")) {
    return FileText;
  }
  if (normalized.includes("test")) {
    return CheckCircle2;
  }
  if (normalized.includes("review")) {
    return Gauge;
  }

  return Bot;
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

function isAbortError(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError";
}

function latestAgentReport(logs: AgentExecutionLog[]) {
  const reportLog = logs.find((log) => log.message.startsWith("Agent report:\n"));

  return reportLog?.message.replace(/^Agent report:\n/, "").trim();
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}
