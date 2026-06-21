import React from "react";
import type { ServerRequest } from "@peregrine/app-server-protocol";
import {
  Archive,
  ArrowLeft,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleDot,
  Clock3,
  Database,
  History,
  MessageSquareText,
  Mic,
  MoreHorizontal,
  Play,
  Plus,
  Server,
  Settings,
  Square,
  Terminal,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  agentStudioStateToProjectMetadata,
  createExecutionLog,
  listAgentServerModels,
  loadAgentServerSettings,
  loadAgentStudioState,
  loadAgentStudioStateFromProjectMetadata,
  ensurePrimaryAgentThreadState,
  markAgentThreadClosed,
  runAgentWorkflowWithAppServer,
  saveAgentServerSettings,
  saveAgentStudioState,
  syncAgentStudioStateWithServerThread,
} from "@peregrine/desktop-runtime";
import type {
  AgentDefinition,
  AgentExecutionLog,
  AgentRunStreamEvent,
  AgentServerRequestResolution,
  AgentServerSettings,
  AgentServerTarget,
  AgentStatus,
  AgentStudioState,
  AgentToolProjectContext,
  AgentWorkflow,
  MovePackage,
  PackageTree,
} from "@peregrine/desktop-runtime";
import type { Model } from "@peregrine/app-server-protocol/v2";
import {
  displayMovePackageName,
  loadProjectMetadata,
  saveProjectMetadata,
} from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";
import { SlashCommandPopup } from "./slash-command-popup";
import { SlashCommandDef, SLASH_COMMANDS } from "./slash-commands";
type AgentCategory = "Primary" | "Subagent";
type AgentFilter = "all" | AgentCategory;
type MainTab = "agents" | "details";
type RunStatus = "idle" | "running" | "completed" | "blocked" | "stopped";
type AgentUiMetadata = {
  capabilities: string[];
  category: AgentCategory;
};
type RunEvent = {
  id: string;
  level: AgentExecutionLog["level"];
  message: string;
  timestamp: number;
  title: string;
};
type AgentRunDetail = {
  agentId: string;
  completedAt?: number;
  displayName: string;
  events: RunEvent[];
  id: string;
  reasoningText: string;
  responseText: string;
  startedAt: number;
  status: RunStatus;
  workflowId: string;
  workflowName: string;
};
const AGENT_FILTERS: Array<{ label: string; value: AgentFilter }> = [
  { label: "Primary", value: "Primary" },
  { label: "Subagent", value: "Subagent" },
  { label: "All", value: "all" },
];
export function AgentsScreen({
  activeMovePackage,
  packageTree,
  projectRootPath,
}: {
  activeMovePackage?: MovePackage | null;
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
  const [state, setState] = React.useState<AgentStudioState>(() =>
    ensurePrimaryAgentThreadState(loadAgentStudioState()),
  );
  const [settings, setSettings] = React.useState<AgentServerSettings>(() =>
    loadAgentServerSettings(),
  );
  const [activeMainTab, setActiveMainTab] = React.useState<MainTab>("agents");
  const [agentFilter, setAgentFilter] = React.useState<AgentFilter>("all");
  const [isInspectorOpen, setIsInspectorOpen] = React.useState(true);
  const [runDetailsByAgentId, setRunDetailsByAgentId] = React.useState<
    Record<string, AgentRunDetail>
  >({});
  const [modelSummary, setModelSummary] = React.useState<ModelSummary>({
    label: "App-server model",
    status: "idle",
  });
  const [modelCatalog, setModelCatalog] = React.useState<Model[]>([]);
  const [selectedModelId, setSelectedModelId] = React.useState<string | null>(null);
  const [isProjectStateLoaded, setIsProjectStateLoaded] = React.useState(false);
  const activeRunControllerRef = React.useRef<AbortController | null>(null);
  const [slashInput, setSlashInput] = React.useState("");
  const [slashSelectedIndex, setSlashSelectedIndex] = React.useState(0);
  const [isPopupOpen, setIsPopupOpen] = React.useState(false);
  const slashContainerRef = React.useRef<HTMLDivElement>(null);
  React.useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        slashContainerRef.current &&
        !slashContainerRef.current.contains(event.target as Node)
      ) {
        setIsPopupOpen(false);
        setSlashInput("");
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);
  function executeSlashCommand(cmd: SlashCommandDef) {
    // Route to the app server the same way the TUI does
    console.log("Executing slash command via app server:", cmd.command);
    // TODO: Dispatch to app server RPC using Tauri invoke or sendAgentTurn
    setSlashInput("");
  }
  const selectedAgent =
    state.agents.find((agent) => agent.id === state.selectedAgentId) ??
    state.agents[0];
  const selectedWorkflow =
    state.workflows.find(
      (workflow) => workflow.id === state.selectedWorkflowId,
    ) ??
    state.workflows.find(
      (workflow) => workflow.id === selectedAgent?.workflowId,
    ) ??
    state.workflows[0];
  const visibleAgents = state.agents.filter((agent) =>
    agentFilter === "all"
      ? true
      : agentMetadata(agent).category === agentFilter,
  );
  const isRunInProgress = state.agents.some(
    (agent) => agent.status === "running",
  );
  const selectedRunDetail = selectedAgent
    ? runDetailsByAgentId[selectedAgent.id]
    : undefined;
  React.useEffect(() => {
    let cancelled = false;
    setIsProjectStateLoaded(false);
    if (!projectRootPath) {
      setState(ensurePrimaryAgentThreadState(loadAgentStudioState()));
      setIsProjectStateLoaded(true);
      return;
    }
    void loadProjectMetadata(projectRootPath)
      .then((metadata) => {
        if (!cancelled) {
          setState(
            ensurePrimaryAgentThreadState(
              loadAgentStudioStateFromProjectMetadata(metadata),
            ),
          );
          setIsProjectStateLoaded(true);
        }
      })
      .catch((error) => {
        console.warn(
          "Could not load project Agents metadata; using local fallback.",
          error,
        );
        if (!cancelled) {
          setState(ensurePrimaryAgentThreadState(loadAgentStudioState()));
          setIsProjectStateLoaded(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectRootPath]);
  React.useEffect(() => {
    saveAgentServerSettings(settings);
  }, [settings]);
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
  const refreshModels = React.useCallback(() => {
    setModelSummary({
      label: "Loading app-server models",
      status: "loading",
    });
    void listAgentServerModels({
      cwd: projectRootPath,
      target: settings.target,
    })
      .then((response) => {
        setModelCatalog(response.models.data);
        const defaultModel =
          response.models.data.find((model) => model.isDefault) ??
          response.models.data[0];
        const selectedProvider = response.providers.data.find(
          (provider) => provider.selected,
        );
        setSelectedModelId(defaultModel?.id ?? null);
        setModelSummary({
          label: defaultModel
            ? `${selectedProvider?.displayName ?? response.providers.selectedProviderId}/${defaultModel.model}`
            : (selectedProvider?.displayName ?? "No app-server models"),
          status: "ready",
          count: response.models.data.length,
        });
      })
      .catch((error) => {
        console.error("Failed to load models:", error);
        setModelSummary({
          error: error instanceof Error ? error.message : String(error),
          label: "Model list unavailable",
          status: "error",
        });
      });
  }, [projectRootPath, settings.target]);
  React.useEffect(() => {
    refreshModels();
  }, [refreshModels]);
  if (!selectedAgent || !selectedWorkflow) {
    return (
      <section className="grid h-full min-h-0 place-items-center bg-[var(--app-window)] p-6 text-xs text-muted-foreground">
        App-server thread state unavailable.
      </section>
    );
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
        {activeMainTab === "details" ? null : (
          <PageHeader
            agentFilter={agentFilter}
            isRunInProgress={isRunInProgress}
            modelSummary={modelSummary}
            onFilterChange={setAgentFilter}
            onRefreshModels={refreshModels}
            onRunWorkflow={() => {
              if (selectedAgent && selectedWorkflow) {
                void runWorkflowFor(selectedAgent, selectedWorkflow);
              }
            }}
            onStopRun={stopWorkflowRun}
          />
        )}
        {activeMainTab === "details" ? (
          <AgentDetailRouteScreen
            agent={selectedAgent}
            isRunInProgress={isRunInProgress}
            onBack={() => setActiveMainTab("agents")}
            onRunWorkflow={() => {
              if (selectedAgent && selectedWorkflow) {
                void runWorkflowFor(selectedAgent, selectedWorkflow);
              }
            }}
            onStopRun={stopWorkflowRun}
            run={selectedRunDetail}
            workflow={selectedWorkflow}
          />
        ) : (
          <section className="grid min-h-0 place-items-center overflow-hidden">
            <div className="flex w-2/3 max-w-3xl flex-col items-center gap-8 mb-[20vh]">
              <h1 className="text-3xl font-bold text-foreground">
                Which package are we dealing with today?
              </h1>
              <div
                ref={slashContainerRef}
                className="relative flex w-full flex-col rounded-2xl border border-[color:var(--app-border)] bg-[var(--app-panel)] shadow-sm transition-colors focus-within:border-ring focus-within:ring-[2px] focus-within:ring-ring/35"
              >
                {isPopupOpen && (
                  <SlashCommandPopup
                    input={slashInput}
                    selectedIndex={slashSelectedIndex}
                    onSelect={(cmd) => {
                      executeSlashCommand(cmd);
                      setIsPopupOpen(false);
                    }}
                  />
                )}
                <input
                  className="h-16 w-full bg-transparent px-6 text-lg outline-none placeholder:text-muted-foreground/50 placeholder:font-light"
                  placeholder="Surgical analysis"
                  value={slashInput}
                  onChange={(e) => {
                    setSlashInput(e.target.value);
                    setSlashSelectedIndex(0);
                    setIsPopupOpen(true);
                  }}
                  onFocus={() => setIsPopupOpen(true)}
                  onKeyDown={(e) => {
                    if (slashInput.startsWith("/")) {
                      const query = slashInput.slice(1).toLowerCase();
                      const filtered = SLASH_COMMANDS.filter((c) =>
                        c.command.toLowerCase().includes(query),
                      );
                      if (e.key === "ArrowDown") {
                        e.preventDefault();
                        setSlashSelectedIndex((prev) =>
                          Math.min(prev + 1, filtered.length - 1),
                        );
                      } else if (e.key === "ArrowUp") {
                        e.preventDefault();
                        setSlashSelectedIndex((prev) => Math.max(prev - 1, 0));
                      } else if (e.key === "Enter") {
                        e.preventDefault();
                        if (filtered[slashSelectedIndex]) {
                          executeSlashCommand(filtered[slashSelectedIndex]);
                        }
                      }
                    }
                  }}
                />
                <div className="flex items-center justify-between px-3 pb-2 pt-1">
                  <div className="flex items-center gap-1">
                    <Button
                      className="h-8 w-8 rounded-full text-muted-foreground hover:text-foreground"
                      size="icon"
                      type="button"
                      variant="ghost"
                    >
                      <Plus className="size-5" />
                    </Button>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button
                          className="h-8 gap-1.5 rounded-full px-3 text-xs text-muted-foreground hover:text-foreground"
                          type="button"
                          variant="ghost"
                        >
                          {modelSummary.label}
                          <ChevronDown className="size-3.5" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="start" className="w-[300px]">
                        <DropdownMenuLabel>Available Models</DropdownMenuLabel>
                        {modelSummary.status === "error" && modelSummary.error && (
                          <div className="px-2 py-1 text-xs text-destructive">
                            Error: {modelSummary.error}
                          </div>
                        )}
                        <DropdownMenuSeparator />
                        <DropdownMenuGroup>
                          {modelCatalog.filter(m => !m.hidden).map((model) => (
                            <DropdownMenuItem
                              key={model.id}
                              onClick={() => {
                                setSelectedModelId(model.id);
                                setModelSummary((current) => ({
                                  ...current,
                                  label: model.displayName || model.model,
                                }));
                              }}
                              className="flex flex-col items-start gap-1 p-2"
                            >
                              <div className="flex w-full items-center justify-between">
                                <span className="font-medium">{model.displayName || model.model}</span>
                                {model.id === selectedModelId && <CheckCircle2 className="size-4 text-primary" />}
                              </div>
                              {model.description && (
                                <span className="text-xs text-muted-foreground line-clamp-2">
                                  {model.description}
                                </span>
                              )}
                            </DropdownMenuItem>
                          ))}
                        </DropdownMenuGroup>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                  <Button
                    className="h-8 w-8 rounded-full text-muted-foreground hover:text-foreground"
                    size="icon"
                    type="button"
                    variant="ghost"
                  >
                    <Mic className="size-4" />
                  </Button>
                </div>
              </div>
            </div>
          </section>
        )}
      </main>
      <AgentInspector
        agent={selectedAgent}
        isOpen={isInspectorOpen}
        modelSummary={modelSummary}
        onOpenDetails={() => openAgentDetails(selectedAgent)}
        onRefreshModels={refreshModels}
        onSettingsChange={setSettings}
        onToggleOpen={() => setIsInspectorOpen((current) => !current)}
        settings={settings}
        workflow={selectedWorkflow}
      />
    </div>
  );
  function selectAgent(agent: AgentDefinition) {
    setIsInspectorOpen(true);
    setState((current) => ({
      ...current,
      selectedAgentId: agent.id,
      selectedWorkflowId: agent.workflowId,
    }));
  }
  function openAgentDetails(agent: AgentDefinition) {
    setActiveMainTab("details");
    setIsInspectorOpen(false);
    setState((current) => ({
      ...current,
      selectedAgentId: agent.id,
      selectedWorkflowId: agent.workflowId,
    }));
  }
  async function runWorkflowFor(
    runAgent: AgentDefinition,
    runWorkflowState: AgentWorkflow,
  ) {
    if (activeRunControllerRef.current) {
      return;
    }
    const controller = new AbortController();
    const previousStatus = runAgent.status;
    activeRunControllerRef.current = controller;
    startRunDetail(runAgent, runWorkflowState);
    setState((current) => ({
      ...current,
      selectedAgentId: runAgent.id,
      selectedWorkflowId: runWorkflowState.id,
      agents: current.agents.map((agent) =>
        agent.id === runAgent.id ? { ...agent, status: "running" } : agent,
      ),
      workflows: current.workflows.map((workflow) =>
        workflow.id === runWorkflowState.id
          ? markWorkflowStatus(workflow, "running")
          : workflow,
      ),
      logs: [
        ...current.logs,
        createExecutionLog({
          agentId: runAgent.id,
          workflowId: runWorkflowState.id,
          level: "trace",
          message: projectContext
            ? `${runWorkflowState.name} started against ${displayMovePackageName(projectContext.packageName)} through the Rust app-server.`
            : `${runWorkflowState.name} started through the Rust app-server without an open Move package.`,
        }),
      ].slice(-120),
    }));
    try {
      const result = await runAgentWorkflowWithAppServer({
        agent: runAgent,
        onServerRequest: resolveServerRequestWithPrompt,
        onStream: (event) => recordRunStreamEvent(runAgent.id, event),
        onTrace: (event) => {
          appendRunLog(runAgent.id, runWorkflowState.id, event);
          appendRunDetailEvent(runAgent.id, {
            level: event.level,
            message: event.message,
            title: event.level === "trace" ? "Runtime trace" : "Runtime update",
          });
        },
        projectContext,
        signal: controller.signal,
        target: settings.target,
        workflow: runWorkflowState,
      });
      if (result.text) {
        appendRunDetailText(runAgent.id, "responseText", result.text);
      }
      finishRunDetail(runAgent.id, "completed", {
        level: "info",
        message: `${runWorkflowState.name} completed through the Rust app-server.`,
        title: "Run completed",
      });
      setRunCompletedState(
        runAgent,
        runWorkflowState,
        previousStatus,
        "completed",
        "info",
      );
    } catch (error) {
      const aborted = isAbortError(error);
      finishRunDetail(runAgent.id, aborted ? "stopped" : "blocked", {
        level: aborted ? "warning" : "error",
        message: aborted
          ? `${runWorkflowState.name} stopped before completion.`
          : `App-server run failed: ${error instanceof Error ? error.message : String(error)}`,
        title: aborted ? "Run stopped" : "Run failed",
      });
      setRunCompletedState(
        runAgent,
        runWorkflowState,
        previousStatus,
        aborted ? "idle" : "blocked",
        aborted ? "warning" : "error",
      );
    } finally {
      if (activeRunControllerRef.current === controller) {
        activeRunControllerRef.current = null;
      }
    }
  }
  function startRunDetail(runAgent: AgentDefinition, workflow: AgentWorkflow) {
    const timestamp = Date.now();
    setActiveMainTab("details");
    setIsInspectorOpen(false);
    setRunDetailsByAgentId((current) => ({
      ...current,
      [runAgent.id]: {
        agentId: runAgent.id,
        displayName: workflow.name,
        events: [
          createRunEvent(
            {
              level: "trace",
              message: projectContext
                ? `${runAgent.name} started against ${displayMovePackageName(projectContext.packageName)}.`
                : `${runAgent.name} started without an open Move package.`,
              title: "Run started",
            },
            timestamp,
          ),
        ],
        id: createRunDetailId(runAgent.id),
        reasoningText: "",
        responseText: "",
        startedAt: timestamp,
        status: "running",
        workflowId: workflow.id,
        workflowName: workflow.name,
      },
    }));
  }
  function appendRunDetailEvent(
    agentId: string,
    event: Omit<RunEvent, "id" | "timestamp">,
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
          events: [...detail.events, createRunEvent(event, timestamp)].slice(
            -160,
          ),
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
  function finishRunDetail(
    agentId: string,
    status: RunStatus,
    event: Omit<RunEvent, "id" | "timestamp">,
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
          events: [...detail.events, createRunEvent(event, timestamp)].slice(
            -160,
          ),
          status,
        },
      };
    });
  }
  function recordRunStreamEvent(agentId: string, event: AgentRunStreamEvent) {
    switch (event.type) {
      case "text-delta":
        appendRunDetailText(agentId, "responseText", event.text);
        break;
      case "reasoning-delta":
        appendRunDetailText(agentId, "reasoningText", event.text);
        break;
      case "status":
        appendRunDetailEvent(agentId, {
          level: event.level,
          message: event.message,
          title: event.title,
        });
        break;
      case "server-request":
        appendRunDetailEvent(agentId, {
          level: "warning",
          message: serverRequestSummary(event.request),
          title: "Approval requested",
        });
        break;
      case "thread-started":
        setState((current) =>
          syncAgentStudioStateWithServerThread(current, event.thread, {
            isPrimary: event.isPrimary,
          }),
        );
        appendRunDetailEvent(agentId, {
          level: "trace",
          message: event.thread.id,
          title: event.thread.agentRole ? "Subagent started" : "Thread started",
        });
        break;
      case "thread-closed":
        setState((current) => markAgentThreadClosed(current, event.threadId));
        appendRunDetailEvent(agentId, {
          level: "trace",
          message: event.threadId,
          title: "Thread closed",
        });
        break;
      case "error":
        appendRunDetailEvent(agentId, {
          level: "error",
          message: event.message,
          title: "Stream error",
        });
        break;
      case "finish":
        appendRunDetailEvent(agentId, {
          level: "info",
          message: `Finish reason: ${event.finishReason ?? "completed"}.`,
          title: "Stream finished",
        });
        break;
      case "abort":
        appendRunDetailEvent(agentId, {
          level: "warning",
          message: event.reason ?? "Run stopped.",
          title: "Run stopped",
        });
        break;
      default:
        break;
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
  function setRunCompletedState(
    runAgent: AgentDefinition,
    workflow: AgentWorkflow,
    previousStatus: AgentStatus,
    workflowStatus: AgentStatus,
    level: AgentExecutionLog["level"],
  ) {
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
      workflows: current.workflows.map((candidate) =>
        candidate.id === workflow.id
          ? markWorkflowStatus(candidate, workflowStatus)
          : candidate,
      ),
      logs: [
        ...current.logs,
        createExecutionLog({
          agentId: runAgent.id,
          workflowId: workflow.id,
          level,
          message: `${workflow.name} ${workflowStatus === "completed" ? "completed" : "stopped"}.`,
        }),
      ].slice(-120),
    }));
  }
  function stopWorkflowRun() {
    activeRunControllerRef.current?.abort();
  }
}
type ModelSummary = {
  count?: number;
  error?: string;
  label: string;
  status: "idle" | "loading" | "ready" | "error";
};
function PageHeader({
  agentFilter,
  isRunInProgress,
  modelSummary,
  onFilterChange,
  onRefreshModels,
  onRunWorkflow,
  onStopRun,
}: {
  agentFilter: AgentFilter;
  isRunInProgress: boolean;
  modelSummary: ModelSummary;
  onFilterChange: (filter: AgentFilter) => void;
  onRefreshModels: () => void;
  onRunWorkflow: () => void;
  onStopRun: () => void;
}) {
  return (
    <header className="flex flex-wrap items-center justify-between gap-3 border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] px-4 py-3">
      <div className="flex items-center gap-2">
        <Button
          className="h-8 gap-1.5 text-xs text-muted-foreground"
          variant="ghost"
        >
          <History className="size-3.5" />
          Sessions
        </Button>
        <Button
          className="h-8 gap-1.5 text-xs text-muted-foreground"
          variant="ghost"
        >
          <Database className="size-3.5" />
          Memory
        </Button>
        <Button
          className="h-8 gap-1.5 text-xs text-muted-foreground"
          variant="ghost"
        >
          <Archive className="size-3.5" />
          Artifacts
        </Button>
      </div>
      <div className="flex flex-wrap items-center justify-end gap-2">
        {isRunInProgress ? (
          <Button
            className="h-8 gap-1.5 text-xs"
            onClick={onStopRun}
            type="button"
            variant="outline"
          >
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
    </header>
  );
}
function RunSummarySection({
  lastRunLabel,
  packageName,
  target,
}: {
  lastRunLabel: string;
  packageName?: string;
  target: AgentServerTarget;
}) {
  const cards = [
    { icon: Server, label: "Harness", value: targetLabel(target) },
    { icon: MessageSquareText, label: "Mode", value: "Chat/run" },
    {
      icon: Bot,
      label: "Package",
      value: packageName ? displayMovePackageName(packageName) : "None",
    },
    { icon: Clock3, label: "Last Run", value: lastRunLabel },
  ];
  return (
    <section className="border-b border-[color:var(--app-border)] bg-[var(--app-panel)] p-3">
      <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,180px),1fr))] gap-2">
        {cards.map((card) => (
          <div
            className="rounded border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 py-1.5"
            key={card.label}
          >
            <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
              <card.icon className="size-3 shrink-0" aria-hidden="true" />
              <span className="truncate">{card.label}</span>
            </div>
            <div className="mt-1 truncate font-mono text-xs font-semibold text-foreground">
              {card.value}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
function AgentsTable({
  agents,
  isRunInProgress,
  logs,
  onOpenAgentDetails,
  onRunAgent,
  onSelectAgent,
  onStopRun,
  selectedAgentId,
}: {
  agents: AgentDefinition[];
  isRunInProgress: boolean;
  logs: AgentExecutionLog[];
  onOpenAgentDetails: (agent: AgentDefinition) => void;
  onRunAgent: (agent: AgentDefinition) => void;
  onSelectAgent: (agent: AgentDefinition) => void;
  onStopRun: () => void;
  selectedAgentId: string;
}) {
  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <div className="flex min-w-0 items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-3">
        <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
          <Bot className="size-3.5" aria-hidden="true" />
          Agent Threads
        </div>
        <span className="text-[11px] text-muted-foreground">
          {agents.length} shown
        </span>
      </div>
      <div className="min-h-0 overflow-auto">
        <div className="min-w-[780px]">
          <div className="sticky top-0 z-10 grid grid-cols-[36px_minmax(250px,1.7fr)_112px_96px_112px_36px_36px] items-center gap-3 border-b border-[color:var(--app-border)] bg-[var(--app-window)] px-3 py-2 text-[10px] font-semibold uppercase text-muted-foreground">
            <span />
            <span>Agent</span>
            <span>Category</span>
            <span>Status</span>
            <span>Last Run</span>
            <span />
            <span />
          </div>
          {agents.length ? (
            agents.map((agent) => (
              <AgentRow
                agent={agent}
                isRunInProgress={isRunInProgress}
                key={agent.id}
                lastRun={agentLastRunLabel(logs, agent)}
                onOpenAgentDetails={onOpenAgentDetails}
                onRunAgent={onRunAgent}
                onSelectAgent={onSelectAgent}
                onStopRun={onStopRun}
                selected={selectedAgentId === agent.id}
              />
            ))
          ) : (
            <div className="px-4 py-8 text-center text-xs text-muted-foreground">
              No agent threads match this filter.
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
function AgentRow({
  agent,
  isRunInProgress,
  lastRun,
  onOpenAgentDetails,
  onRunAgent,
  onSelectAgent,
  onStopRun,
  selected,
}: {
  agent: AgentDefinition;
  isRunInProgress: boolean;
  lastRun: string;
  onOpenAgentDetails: (agent: AgentDefinition) => void;
  onRunAgent: (agent: AgentDefinition) => void;
  onSelectAgent: (agent: AgentDefinition) => void;
  onStopRun: () => void;
  selected: boolean;
}) {
  const metadata = agentMetadata(agent);
  const isAgentRunning = agent.status === "running";
  return (
    <div
      className={cn(
        "grid cursor-pointer grid-cols-[minmax(0,1fr)_36px_36px] items-center border-b border-[color:var(--app-border)] text-xs transition last:border-b-0",
        selected
          ? "bg-[var(--app-subtle)] text-foreground"
          : "hover:bg-[var(--app-subtle)] hover:text-foreground",
      )}
      onClick={() => onOpenAgentDetails(agent)}
      role="button"
      tabIndex={0}
    >
      <div className="grid min-h-12 w-full grid-cols-[36px_minmax(250px,1.7fr)_112px_96px_112px] items-center gap-3 px-3 py-2 text-left">
        <AgentIcon agent={agent} />
        <div className="min-w-0">
          <div className="truncate font-semibold text-foreground">
            {agent.name}
          </div>
          <div className="mt-0.5 truncate text-[11px] text-muted-foreground">
            {agent.description}
          </div>
        </div>
        <CategoryBadge category={metadata.category} />
        <StatusBadge status={agent.status} />
        <span className="text-[11px] text-muted-foreground">{lastRun}</span>
      </div>
      <div
        className="flex justify-end"
        onClick={(event) => event.stopPropagation()}
      >
        <Button
          aria-label={
            isAgentRunning ? `Stop ${agent.name}` : `Run ${agent.name}`
          }
          className="size-7"
          disabled={isRunInProgress && !isAgentRunning}
          onClick={isAgentRunning ? onStopRun : () => onRunAgent(agent)}
          size="icon-xs"
          title={isAgentRunning ? "Stop agent" : "Run agent"}
          type="button"
          variant={isAgentRunning ? "outline" : "ghost"}
        >
          {isAgentRunning ? (
            <Square className="size-3.5" />
          ) : (
            <Play className="size-3.5" />
          )}
        </Button>
      </div>
      <div
        className="flex justify-end pr-3"
        onClick={(event) => event.stopPropagation()}
      >
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              aria-label={`Open actions for ${agent.name}`}
              className="size-7 text-muted-foreground hover:text-foreground"
              size="icon-xs"
              type="button"
              variant="ghost"
            >
              <MoreHorizontal className="size-3.5" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            className="border-[color:var(--app-border)] bg-[var(--app-panel)]"
          >
            <DropdownMenuItem
              className="text-xs"
              onSelect={() => onSelectAgent(agent)}
            >
              Open settings
            </DropdownMenuItem>
            <DropdownMenuItem
              className="text-xs"
              onSelect={() => onOpenAgentDetails(agent)}
            >
              Open run
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
function AgentDetailRouteScreen({
  agent,
  isRunInProgress,
  onBack,
  onRunWorkflow,
  onStopRun,
  run,
  workflow,
}: {
  agent: AgentDefinition;
  isRunInProgress: boolean;
  onBack: () => void;
  onRunWorkflow: () => void;
  onStopRun: () => void;
  run?: AgentRunDetail;
  workflow: AgentWorkflow;
}) {
  const agentIsRunning =
    agent.status === "running" || run?.status === "running";
  return (
    <section className="grid h-full min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <header className="flex min-w-0 items-center border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] px-4 py-2">
        <div className="flex min-w-0 flex-1 items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <Button
              aria-label="Back to agents"
              className="size-7 shrink-0 text-muted-foreground hover:text-foreground"
              onClick={onBack}
              size="icon-xs"
              type="button"
              variant="ghost"
            >
              <ArrowLeft className="size-3.5" />
            </Button>
            <Bot
              className="size-4 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
            <span className="truncate text-sm font-semibold">
              Agent Threads
            </span>
            <ChevronRight
              className="size-3 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
            <span className="truncate text-sm text-muted-foreground">
              {agent.name}
            </span>
          </div>
          {agentIsRunning ? (
            <Button
              className="h-8 shrink-0 gap-1.5 text-xs"
              onClick={onStopRun}
              type="button"
              variant="outline"
            >
              <Square className="size-3.5" />
              Stop
            </Button>
          ) : (
            <Button
              className="h-8 shrink-0 gap-1.5 text-xs"
              disabled={isRunInProgress}
              onClick={onRunWorkflow}
              type="button"
            >
              <Play className="size-3.5" />
              Run
            </Button>
          )}
        </div>
      </header>
      <AgentDetailScreen agent={agent} run={run} workflow={workflow} />
    </section>
  );
}
function AgentDetailScreen({
  agent,
  run,
  workflow,
}: {
  agent: AgentDefinition;
  run?: AgentRunDetail;
  workflow: AgentWorkflow;
}) {
  const agentIsRunning =
    agent.status === "running" || run?.status === "running";
  const responseText = run?.responseText.trim();
  const reasoningText = run?.reasoningText.trim();
  const duration = run
    ? durationLabel((run.completedAt ?? Date.now()) - run.startedAt)
    : "Not run";
  return (
    <section className="grid h-full min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <div className="grid shrink-0 grid-cols-2 gap-2 border-b border-[color:var(--app-border)] bg-[var(--app-panel)] p-3 sm:grid-cols-4">
        <MetricTile
          label="Workflow"
          value={run?.displayName ?? workflow.name}
        />
        <MetricTile label="Status" value={run?.status ?? agent.status} />
        <MetricTile label="Duration" value={duration} />
        <MetricTile label="Runtime" value="Rust app-server" />
      </div>
      <section className="grid min-h-0 min-w-0 md:grid-cols-[minmax(0,1.4fr)_minmax(320px,0.8fr)]">
        <AgentResponsePanel
          agentIsRunning={agentIsRunning}
          reasoningText={reasoningText}
          responseText={responseText}
        />
        <RunActivityPanel events={run?.events ?? []} />
      </section>
    </section>
  );
}
function AgentResponsePanel({
  agentIsRunning,
  reasoningText,
  responseText,
}: {
  agentIsRunning: boolean;
  reasoningText?: string;
  responseText?: string;
}) {
  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border-r border-[color:var(--app-border)] bg-[var(--app-window)]">
      <header className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <MessageSquareText className="size-4 text-muted-foreground" />
          <h3 className="truncate text-sm font-semibold">Agent Response</h3>
        </div>
        {agentIsRunning ? (
          <span className="text-[11px] text-sky-300">Streaming</span>
        ) : null}
      </header>
      <div className="min-h-0 overflow-auto p-3">
        {responseText ? (
          <pre className="whitespace-pre-wrap break-words rounded border border-[color:var(--app-border)] bg-black/10 p-3 text-xs leading-5 text-foreground">
            {responseText}
          </pre>
        ) : (
          <div className="grid min-h-full place-items-center rounded border border-dashed border-[color:var(--app-border)] bg-black/10 p-6 text-center text-xs leading-5 text-muted-foreground">
            {agentIsRunning
              ? "Waiting for streamed app-server text."
              : "Run this thread to stream a response here."}
          </div>
        )}
        {reasoningText ? (
          <section className="mt-3 rounded border border-[color:var(--app-border)] bg-black/10 p-3">
            <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase text-muted-foreground">
              <Terminal className="size-3.5" />
              Reasoning
            </div>
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-words text-[11px] leading-4 text-muted-foreground">
              {reasoningText}
            </pre>
          </section>
        ) : null}
      </div>
    </section>
  );
}
function RunActivityPanel({ events }: { events: RunEvent[] }) {
  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-panel)]">
      <header className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <CircleDot className="size-4 text-muted-foreground" />
          <h3 className="truncate text-sm font-semibold">Run Activity</h3>
        </div>
        <span className="text-[11px] text-muted-foreground">
          {events.length}
        </span>
      </header>
      <div className="min-h-0 overflow-auto p-3">
        {events.length ? (
          <div className="grid gap-2">
            {events
              .slice()
              .reverse()
              .map((event) => (
                <div
                  className="rounded border border-[color:var(--app-border)] bg-[var(--app-window)] p-2"
                  key={event.id}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span
                      className={cn(
                        "text-xs font-semibold",
                        eventLevelClass(event.level),
                      )}
                    >
                      {event.title}
                    </span>
                    <span className="text-[10px] text-muted-foreground">
                      {timeLabel(event.timestamp)}
                    </span>
                  </div>
                  <p className="mt-1 whitespace-pre-wrap break-words text-[11px] leading-4 text-muted-foreground">
                    {event.message}
                  </p>
                </div>
              ))}
          </div>
        ) : (
          <div className="grid min-h-full place-items-center rounded border border-dashed border-[color:var(--app-border)] p-4 text-center text-xs text-muted-foreground">
            No run activity yet.
          </div>
        )}
      </div>
    </section>
  );
}
function AgentInspector({
  agent,
  isOpen,
  modelSummary,
  onOpenDetails,
  onRefreshModels,
  onSettingsChange,
  onToggleOpen,
  settings,
  workflow,
}: {
  agent: AgentDefinition;
  isOpen: boolean;
  modelSummary: ModelSummary;
  onOpenDetails: () => void;
  onRefreshModels: () => void;
  onSettingsChange: React.Dispatch<React.SetStateAction<AgentServerSettings>>;
  onToggleOpen: () => void;
  settings: AgentServerSettings;
  workflow: AgentWorkflow;
}) {
  const metadata = agentMetadata(agent);
  if (!isOpen) {
    return (
      <aside className="min-w-0 overflow-hidden border-l border-[color:var(--app-border)] bg-[var(--app-panel)]">
        <button
          aria-label="Open agent settings"
          className="flex h-full w-full items-start justify-center px-2 py-4 text-muted-foreground transition hover:text-foreground"
          onClick={onToggleOpen}
          type="button"
        >
          <span className="text-[10px] font-semibold uppercase [writing-mode:vertical-rl]">
            Inspector
          </span>
        </button>
      </aside>
    );
  }
  return (
    <aside className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border-l border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <header className="border-b border-[color:var(--app-border)] px-4 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              <h2 className="truncate text-sm font-semibold">Agent</h2>
              <StatusBadge status={agent.status} />
            </div>
          </div>
          <Button
            aria-label="Close agent settings"
            className="size-7 text-muted-foreground hover:text-foreground"
            onClick={onToggleOpen}
            size="icon-xs"
            type="button"
            variant="ghost"
          >
            <X className="size-3.5" />
          </Button>
        </div>
      </header>
      <ScrollArea className="min-h-0 min-w-0">
        <div className="space-y-4 px-4 pb-4 pt-3">{/* Content removed */}</div>
      </ScrollArea>
    </aside>
  );
}
function MetricTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 py-1.5">
      <div className="truncate text-[10px] text-muted-foreground">{label}</div>
      <div className="mt-1 truncate text-xs font-semibold text-foreground">
        {value}
      </div>
    </div>
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
    <section className="space-y-3 rounded border border-[color:var(--app-border)] bg-[var(--app-surface)] p-3">
      <div className="flex items-center gap-2 text-xs font-semibold text-foreground">
        <Settings className="size-3.5 text-muted-foreground" />
        {title}
      </div>
      {children}
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
      {label}
      {children}
    </label>
  );
}
function AgentIcon({ agent }: { agent: AgentDefinition }) {
  const metadata = agentMetadata(agent);
  return (
    <div
      className={cn(
        "grid size-8 place-items-center rounded border",
        metadata.category === "Primary" &&
          "border-sky-400/30 bg-sky-400/10 text-sky-200",
        metadata.category === "Subagent" &&
          "border-emerald-400/30 bg-emerald-400/10 text-emerald-200",
      )}
    >
      <Bot className="size-4" />
    </div>
  );
}
function CategoryBadge({ category }: { category: AgentCategory }) {
  return (
    <Badge
      className="w-fit rounded px-2 py-0.5 text-[10px]"
      variant="secondary"
    >
      {category}
    </Badge>
  );
}
function StatusBadge({ status }: { status: AgentStatus }) {
  return (
    <span
      className={cn(
        "inline-flex w-fit items-center gap-1 rounded px-2 py-0.5 text-[10px] font-medium",
        status === "running" && "bg-sky-500/15 text-sky-200",
        status === "active" && "bg-emerald-500/15 text-emerald-200",
        status === "blocked" && "bg-amber-500/15 text-amber-200",
        status === "failed" && "bg-red-500/15 text-red-200",
        status === "completed" && "bg-emerald-500/15 text-emerald-200",
        status === "idle" && "bg-muted text-muted-foreground",
        status === "needsApproval" && "bg-amber-500/15 text-amber-200",
      )}
    >
      <CheckCircle2 className="size-3" />
      {statusLabel(status)}
    </span>
  );
}
function statusLabel(status: AgentStatus) {
  return status === "needsApproval" ? "needs approval" : status;
}
function agentMetadata(agent: AgentDefinition): AgentUiMetadata {
  const category = agent.isPrimary ? "Primary" : "Subagent";
  const capabilities = [
    "App-server thread",
    agent.isClosed ? "Closed" : "Loaded",
    agent.roleName ? `Role ${agent.roleName}` : "Default thread",
  ];
  return { category, capabilities };
}
function markWorkflowStatus(workflow: AgentWorkflow, status: AgentStatus) {
  return {
    ...workflow,
    updatedAt: Date.now(),
    nodes: workflow.nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        status,
      },
    })),
  };
}
function lastRunLabel(logs: AgentExecutionLog[]) {
  const last = logs.at(-1);
  return last ? timeLabel(last.timestamp) : "Never";
}
function agentLastRunLabel(logs: AgentExecutionLog[], agent: AgentDefinition) {
  const last = logs.filter((log) => log.agentId === agent.id).at(-1);
  return last ? timeLabel(last.timestamp) : "Never";
}
function timeLabel(timestamp: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(timestamp);
}
function durationLabel(durationMs: number) {
  if (durationMs < 1_000) {
    return `${Math.max(1, Math.round(durationMs))}ms`;
  }
  return `${(durationMs / 1_000).toFixed(durationMs < 10_000 ? 1 : 0)}s`;
}
function createRunDetailId(agentId: string) {
  return `${agentId}-run-${Date.now().toString(36)}`;
}
function createRunEvent(
  event: Omit<RunEvent, "id" | "timestamp">,
  timestamp: number,
): RunEvent {
  return {
    ...event,
    id: `event-${timestamp.toString(36)}-${Math.random().toString(36).slice(2, 7)}`,
    timestamp,
  };
}
function eventLevelClass(level: AgentExecutionLog["level"]) {
  if (level === "error") {
    return "text-red-300";
  }
  if (level === "warning") {
    return "text-amber-300";
  }
  if (level === "info") {
    return "text-emerald-300";
  }
  return "text-muted-foreground";
}
function isAbortError(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError";
}
function targetLabel(target: AgentServerTarget) {
  if (target.mode === "remote") {
    return "Remote";
  }
  if (target.mode === "localDaemon") {
    return "Daemon";
  }
  return "Embedded";
}
function serverRequestSummary(request: ServerRequest) {
  switch (request.method) {
    case "item/commandExecution/requestApproval":
      return (
        request.params.command ??
        request.params.reason ??
        "Command approval requested."
      );
    case "item/fileChange/requestApproval":
      return request.params.reason ?? "File change approval requested.";
    case "item/tool/requestUserInput":
      return request.params.questions
        .map((question) => question.question)
        .join(" ");
    case "item/permissions/requestApproval":
      return request.params.reason ?? "Additional permissions requested.";
    case "mcpServer/elicitation/request":
      return "MCP server requested user input.";
    default:
      return request.method;
  }
}
function resolveServerRequestWithPrompt(
  request: ServerRequest,
): AgentServerRequestResolution {
  switch (request.method) {
    case "item/commandExecution/requestApproval": {
      const command = request.params.command ?? "command";
      const approved = window.confirm(
        `Allow app-server command?\n\n${command}`,
      );
      return {
        type: "resolve",
        result: { decision: approved ? "accept" : "decline" },
      };
    }
    case "item/fileChange/requestApproval": {
      const approved = window.confirm(
        `Allow app-server file change?\n\n${request.params.reason ?? "File change requested."}`,
      );
      return {
        type: "resolve",
        result: { decision: approved ? "accept" : "decline" },
      };
    }
    case "item/tool/requestUserInput": {
      const answers = Object.fromEntries(
        request.params.questions.map((question) => {
          const answer = window.prompt(question.question) ?? "";
          return [question.id, { answers: [answer] }];
        }),
      );
      return { type: "resolve", result: { answers } };
    }
    default:
      return {
        type: "reject",
        message: `${request.method} is not supported by the desktop app-server UI yet.`,
      };
  }
}
