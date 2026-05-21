import {
  Background,
  BaseEdge,
  Controls,
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
  applyNodeChanges,
  useReactFlow,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeChange,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { open } from "@tauri-apps/plugin-dialog";
import React from "react";
import {
  AlertTriangle,
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Bug,
  ChevronDown,
  ChevronRight,
  CheckCircle2,
  Circle,
  Clock3,
  FileCheck2,
  FileText,
  FolderOpen,
  Gauge,
  Hammer,
  Loader2,
  MoreVertical,
  Play,
  Plus,
  Rocket,
  SquareFunction,
  Terminal,
  Trash2,
  XCircle,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  buildMovePackage,
  defaultProjectMetadata,
  displayMovePackageName,
  loadPackageTree,
  loadProjectMetadata,
  listenProjectMetadataChanged,
  projectMoveCoverageScriptPath,
  projectMoveTestScriptPath,
  runFormalVerification,
  runSecurityScript,
  runSecurityCommand,
  type CommandOutput,
  type MovePackage,
  type PackageTree,
  type ProjectMetadata,
  type SecurityCommandKind,
} from "@/features/empty-project/filesystem-tree";
import type {
  BuildLogRun,
  BuildLogUpdateOptions,
} from "@/features/project-workspace/build-log-sheet";
import { cn } from "@/lib/utils";

type ExecutionBuilderScreenProps = {
  activeMovePackage: MovePackage | null;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  packageTree: PackageTree;
  onProjectSelected: (packageTree: PackageTree) => void;
};

type ExecutionStepKind =
  | "build"
  | "coverage"
  | "test"
  | "fuzz"
  | "formal"
  | "publish";

type PublishTarget = "localnet" | "devnet" | "testnet" | "mainnet";
type SequenceDirection = "horizontal" | "vertical";

type ExecutionStepDefinition = {
  kind: ExecutionStepKind;
  label: string;
  shortLabel: string;
  description: string;
  whenToUse: string;
  category: "Build" | "Test" | "Verify" | "Deploy";
  command: string;
  commandKind?: SecurityCommandKind;
  defaultStopOnFailure: boolean;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  locked?: boolean;
};

type ExecutionStep = {
  id: string;
  kind: ExecutionStepKind;
  config: {
    publishDryRun?: boolean;
    publishTarget?: PublishTarget;
    scriptPath?: string;
    stopOnFailure: boolean;
    useScript?: boolean;
  };
  locked?: boolean;
};

type ExecutionStepUpdate = Omit<Partial<ExecutionStep>, "config"> & {
  config?: Partial<ExecutionStep["config"]>;
};

type ExecutionStepState =
  | "idle"
  | "running"
  | "success"
  | "attention"
  | "error"
  | "skipped";

type StepExecutionResult = {
  detail: string | null;
  finishedAt: Date | null;
  output: CommandOutput | null;
  startedAt: Date | null;
  state: ExecutionStepState;
  summary: string | null;
};

type StepExecutionOutcome = StepExecutionResult & {
  packageTree?: PackageTree;
};

type ProjectConfiguredScript = {
  args: string[];
  modeLabel: string;
  scriptPath: string;
};

type ExecutionRun = {
  finishedAt: Date | null;
  id: number;
  startedAt: Date;
  state: "running" | "success" | "attention" | "error";
};

type ExecutionWorkflowNodeData = {
  activeMovePackage: MovePackage | null;
  definition: ExecutionStepDefinition;
  direction: SequenceDirection;
  index: number;
  isRunning: boolean;
  onMoveStep: (stepId: string, direction: -1 | 1) => void;
  onRemoveStep: (stepId: string) => void;
  onSelectStep: (stepId: string) => void;
  onUpdateStep: (stepId: string, nextStep: ExecutionStepUpdate) => void;
  result: StepExecutionResult | undefined;
  packageTree: PackageTree;
  projectMetadata: ProjectMetadata | null;
  selected: boolean;
  sequenceLength: number;
  step: ExecutionStep;
};

type ExecutionDropNodeData = {
  direction: SequenceDirection;
};

type ExecutionSequenceEdgeData = {
  direction: SequenceDirection;
};

type ExecutionFlowNode = Node<ExecutionWorkflowNodeData>;
type ExecutionDropNode = Node<ExecutionDropNodeData>;
type ExecutionCanvasNode = ExecutionFlowNode | ExecutionDropNode;

type DragPayload =
  | { source: "palette"; kind: ExecutionStepKind }
  | { source: "sequence"; stepId: string };

const DRAG_MIME = "application/x-peregrine-execution-step";
const DRAG_TEXT_PREFIX = "peregrine-execution-step:";
let currentExecutionDragPayload: DragPayload | null = null;
const EXECUTION_NODE_WIDTH = 760;
const EXECUTION_HORIZONTAL_NODE_WIDTH = 520;
const EXECUTION_NODE_HEIGHT = 96;
const EXECUTION_FAILED_NODE_HEIGHT = 214;
const EXECUTION_PUBLISH_NODE_HEIGHT = 156;
const EXECUTION_HORIZONTAL_GAP = 88;
const EXECUTION_VERTICAL_GAP = 54;
const EXECUTION_FLOW_START_X = 184;
const EXECUTION_FLOW_START_Y = 92;
const EXECUTION_TIMELINE_OFFSET = 48;
const EXECUTION_DROP_TARGET_ID = "execution-drop-target";
const EXECUTION_NODE_TYPES = {
  dropTarget: ExecutionDropTargetNode,
  executionStep: ExecutionWorkflowNode,
};
const EXECUTION_EDGE_TYPES = {
  sequence: ExecutionSequenceEdge,
};
const publishTargets: { label: string; value: PublishTarget }[] = [
  { label: "Local", value: "localnet" },
  { label: "Devnet", value: "devnet" },
  { label: "Testnet", value: "testnet" },
  { label: "Mainnet", value: "mainnet" },
];
const FORMAL_VERIFICATION_TIMEOUT_SECONDS = 45;

const stepDefinitions: ExecutionStepDefinition[] = [
  {
    kind: "build",
    label: "Build",
    shortLabel: "Build",
    description:
      "Runs `sui move build` so Peregrine gets fresh package_summaries before dependent checks.",
    whenToUse:
      "Use this before checks that depend on generated package summaries or fresh bytecode.",
    category: "Build",
    command: "sui move build",
    defaultStopOnFailure: true,
    icon: Hammer,
  },
  {
    kind: "coverage",
    label: "Coverage",
    shortLabel: "Coverage",
    description:
      "Runs Move tests with coverage enabled so developers can see which package code is exercised.",
    whenToUse:
      "Use this after tests when you want evidence that core entry points and branches are actually covered.",
    category: "Test",
    command: "sui move test --coverage",
    commandKind: "move-coverage",
    defaultStopOnFailure: false,
    icon: Gauge,
  },
  {
    kind: "test",
    label: "Test",
    shortLabel: "Tests",
    description:
      "Runs package tests with `sui move test` and stops the sequence on failing tests by default.",
    whenToUse:
      "Use this after the build whenever the package has Move tests or regression checks in place.",
    category: "Test",
    command: "sui move test",
    commandKind: "move-test",
    defaultStopOnFailure: true,
    icon: FileCheck2,
  },
  {
    kind: "fuzz",
    label: "Fuzz",
    shortLabel: "Fuzz",
    description:
      "Runs randomized Move tests with generated values using a sensible default iteration count.",
    whenToUse:
      "Use this for property-style tests and code paths annotated for randomized test inputs.",
    category: "Test",
    command: "sui move test --rand-num-iters 256",
    commandKind: "move-fuzz",
    defaultStopOnFailure: true,
    icon: Bug,
  },
  {
    kind: "formal",
    label: "Formal Verification",
    shortLabel: "Formal",
    description:
      "Runs bundled Sui Prover formal verification for each source module in the active package.",
    whenToUse:
      "Use this before publish to verify formal specifications against the active package.",
    category: "Verify",
    command: "bundled sui-prover --path <package> --modules <module>",
    defaultStopOnFailure: false,
    icon: SquareFunction,
  },
  {
    kind: "publish",
    label: "Publish",
    shortLabel: "Publish",
    description:
      "Publishes the active Move package to local, devnet, testnet, or mainnet. Dry-run is on by default.",
    whenToUse:
      "Use this as the final block after build, tests, fuzzing, coverage, and formal verification checks.",
    category: "Deploy",
    command: "sui client publish --dry-run --client.env localnet .",
    defaultStopOnFailure: true,
    icon: Rocket,
  },
];

const definitionByKind = stepDefinitions.reduce(
  (definitions, definition) => {
    definitions[definition.kind] = definition;
    return definitions;
  },
  {} as Record<ExecutionStepKind, ExecutionStepDefinition>,
);

let stepIdCounter = 0;

export function ExecutionBuilderScreen({
  activeMovePackage,
  onCommandLog,
  onProjectSelected,
  packageTree,
}: ExecutionBuilderScreenProps) {
  const [sequence, setSequence] = React.useState<ExecutionStep[]>(createInitialSequence);
  const [selectedStepId, setSelectedStepId] = React.useState<string>("build");
  const [dropIndex, setDropIndex] = React.useState<number | null>(null);
  const [stepResults, setStepResults] = React.useState<Record<string, StepExecutionResult>>({});
  const [run, setRun] = React.useState<ExecutionRun | null>(null);
  const [sequenceDirection, setSequenceDirection] = React.useState<SequenceDirection>("horizontal");
  const [projectMetadata, setProjectMetadata] = React.useState<ProjectMetadata | null>(null);
  const activeDragPayloadRef = React.useRef<DragPayload | null>(null);
  const isRunning = run?.state === "running";

  React.useEffect(() => {
    let isCancelled = false;

    void loadProjectMetadata(packageTree.rootPath)
      .then((metadata) => {
        if (!isCancelled) {
          setProjectMetadata(metadata);
        }
      })
      .catch((error) => {
        console.warn("Could not load project configuration for execution workflow.", error);
        if (!isCancelled) {
          setProjectMetadata(null);
        }
      });

    return () => {
      isCancelled = true;
    };
  }, [packageTree.rootPath]);

  React.useEffect(() =>
    listenProjectMetadataChanged(({ metadata, rootPath }) => {
      if (rootPath === packageTree.rootPath) {
        setProjectMetadata(metadata);
      }
    }),
  [packageTree.rootPath]);

  React.useEffect(() => {
    if (!selectedStepId || sequence.some((step) => step.id === selectedStepId)) {
      return;
    }

    setSelectedStepId(sequence[0]?.id ?? "");
  }, [selectedStepId, sequence]);

  const updateStepResult = React.useCallback(
    (stepId: string, nextResult: Partial<StepExecutionResult>) => {
      setStepResults((current) => ({
        ...current,
        [stepId]: {
          ...emptyStepResult(),
          ...current[stepId],
          ...nextResult,
        },
      }));
    },
    [],
  );

  const addStep = React.useCallback((kind: ExecutionStepKind, targetIndex?: number) => {
    setSequence((current) => {
      const definition = definitionByKind[kind];

      if (definition.locked || current.some((step) => step.kind === kind)) {
        return current;
      }

      const insertionIndex = Math.max(0, Math.min(targetIndex ?? current.length, current.length));
      const nextStep = createStep(kind);
      const next = [...current];

      next.splice(insertionIndex, 0, nextStep);
      setSelectedStepId(nextStep.id);

      return next;
    });
  }, []);

  const removeStep = React.useCallback((stepId: string) => {
    setSequence((current) => current.filter((step) => step.id !== stepId));
    setStepResults((current) => {
      const next = { ...current };

      delete next[stepId];

      return next;
    });
  }, []);

  const reorderStep = React.useCallback((stepId: string, targetIndex: number) => {
    setSequence((current) => {
      const sourceIndex = current.findIndex((step) => step.id === stepId);
      const sourceStep = current[sourceIndex];

      if (!sourceStep || sourceStep.locked) {
        return current;
      }

      const next = [...current];
      next.splice(sourceIndex, 1);

      const adjustedTargetIndex = sourceIndex < targetIndex ? targetIndex - 1 : targetIndex;
      next.splice(Math.max(0, adjustedTargetIndex), 0, sourceStep);

      return next;
    });
  }, []);

  const moveStep = React.useCallback(
    (stepId: string, direction: -1 | 1) => {
      setSequence((current) => {
        const sourceIndex = current.findIndex((step) => step.id === stepId);

        if (sourceIndex < 0) {
          return current;
        }

        const targetIndex = sourceIndex + direction;

        if (targetIndex < 0 || targetIndex >= current.length) {
          return current;
        }

        const next = [...current];
        const [step] = next.splice(sourceIndex, 1);

        next.splice(targetIndex, 0, step);

        return next;
      });
    },
    [],
  );

  const updateStep = React.useCallback((stepId: string, nextStep: ExecutionStepUpdate) => {
    setSequence((current) =>
      current.map((step) =>
        step.id === stepId
          ? {
              ...step,
              ...nextStep,
              config: {
                ...step.config,
                ...nextStep.config,
              },
            }
          : step,
      ),
    );
  }, []);

  const beginDrag = React.useCallback((event: React.DragEvent<HTMLElement>, payload: DragPayload) => {
    currentExecutionDragPayload = payload;
    activeDragPayloadRef.current = payload;
    writeDragPayload(event, payload);
  }, []);

  const clearDrag = React.useCallback(() => {
    currentExecutionDragPayload = null;
    activeDragPayloadRef.current = null;
    setDropIndex(null);
  }, []);

  const scheduleClearDrag = React.useCallback(() => {
    window.setTimeout(clearDrag, 250);
  }, [clearDrag]);

  const handleDrop = React.useCallback(
    (event: React.DragEvent<HTMLElement>, targetIndex: number) => {
      const payload = readDragPayload(event) ?? activeDragPayloadRef.current ?? currentExecutionDragPayload;

      if (!payload || isRunning) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      clearDrag();

      if (payload.source === "palette") {
        addStep(payload.kind, targetIndex);
      } else {
        reorderStep(payload.stepId, targetIndex);
      }
    },
    [addStep, clearDrag, isRunning, reorderStep],
  );

  const clearCanvas = React.useCallback(() => {
    if (isRunning) {
      return;
    }

    clearDrag();
    setRun(null);
    setSelectedStepId("");
    setSequence([]);
    setStepResults({});
  }, [clearDrag, isRunning]);

  const handleRunSequence = React.useCallback(async () => {
    if (!activeMovePackage || isRunning || sequence.length === 0) {
      return;
    }

    const stepsToRun = sequence;
    const startedAt = new Date();
    let currentTree = packageTree;
    let currentPackage = resolveActiveMovePackage(currentTree, activeMovePackage);
    const runProjectMetadata = await loadProjectMetadata(packageTree.rootPath).catch((error) => {
      console.warn("Could not load project configuration for execution run.", error);
      return projectMetadata ?? defaultProjectMetadata();
    });
    let hasAttention = false;
    let hasError = false;

    setProjectMetadata(runProjectMetadata);

    setRun({
      finishedAt: null,
      id: startedAt.getTime(),
      startedAt,
      state: "running",
    });
    setStepResults(
      Object.fromEntries(stepsToRun.map((step) => [step.id, emptyStepResult()])) as Record<
        string,
        StepExecutionResult
      >,
    );

    for (let index = 0; index < stepsToRun.length; index += 1) {
      const step = stepsToRun[index];

      if (!currentPackage) {
        updateStepResult(step.id, {
          detail: "The active package could not be resolved after the previous step.",
          finishedAt: new Date(),
          state: "error",
          summary: "Active package unavailable.",
        });
        markRemainingStepsSkipped(stepsToRun, index + 1, setStepResults);
        setRun((current) =>
          current ? { ...current, finishedAt: new Date(), state: "error" } : current,
        );
        return;
      }

      const stepStartedAt = new Date();
      const runningLog = executionLogRun({
        movePackage: currentPackage,
        packageTree: currentTree,
        projectMetadata: runProjectMetadata,
        startedAt: stepStartedAt,
        state: "running",
        step,
      });

      onCommandLog(runningLog, { reset: index === 0 });
      updateStepResult(step.id, {
        startedAt: stepStartedAt,
        state: "running",
        summary: "Running...",
      });

      try {
        const outcome = await executeStep({
          movePackage: currentPackage,
          onProjectSelected,
          packageTree: currentTree,
          projectMetadata: runProjectMetadata,
          onCommandOutput: (output) => {
            onCommandLog({
              ...runningLog,
              output,
            });
          },
          startedAt: stepStartedAt,
          streamId: runningLog.id,
          step,
        });

        const packageForLog = currentPackage;
        currentTree = outcome.packageTree ?? currentTree;
        currentPackage = resolveActiveMovePackage(currentTree, packageForLog);
        hasAttention = hasAttention || outcome.state === "attention";
        hasError = hasError || outcome.state === "error";
        updateStepResult(step.id, outcome);
        onCommandLog(
          executionLogRun({
            detail: outcome.detail,
            finishedAt: outcome.finishedAt ?? new Date(),
            movePackage: currentPackage ?? packageForLog,
            output: outcome.output,
            packageTree: currentTree,
            projectMetadata: runProjectMetadata,
            startedAt: outcome.startedAt ?? stepStartedAt,
            state: outcome.state === "error" ? "error" : "success",
            step,
            summary: outcome.summary,
          }),
        );

        if (outcome.state === "error" && step.config.stopOnFailure) {
          markRemainingStepsSkipped(stepsToRun, index + 1, setStepResults);
          setRun((current) =>
            current ? { ...current, finishedAt: new Date(), state: "error" } : current,
          );
          return;
        }
      } catch (error) {
        const finishedAt = new Date();
        const errorMessage = getErrorMessage(error);
        updateStepResult(step.id, {
          detail: errorMessage,
          finishedAt,
          state: "error",
          summary: "Step failed before it could complete.",
        });
        onCommandLog(
          executionLogRun({
            detail: errorMessage,
            error: errorMessage,
            finishedAt,
            movePackage: currentPackage,
            packageTree: currentTree,
            projectMetadata: runProjectMetadata,
            startedAt: stepStartedAt,
            state: "error",
            step,
            summary: "Step failed before it could complete.",
          }),
        );
        markRemainingStepsSkipped(stepsToRun, index + 1, setStepResults);
        setRun((current) =>
          current ? { ...current, finishedAt: new Date(), state: "error" } : current,
        );
        return;
      }
    }

    setRun((current) =>
      current
        ? {
            ...current,
            finishedAt: new Date(),
            state: hasError ? "error" : hasAttention ? "attention" : "success",
          }
        : current,
    );
  }, [
    activeMovePackage,
    isRunning,
    onCommandLog,
    onProjectSelected,
    packageTree,
    projectMetadata,
    sequence,
    updateStepResult,
  ]);

  return (
    <section className="relative h-full min-h-0 bg-[var(--app-window)] text-foreground">
      <div className="absolute right-4 top-4 z-20 flex shrink-0 items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)]/95 p-1 shadow-sm backdrop-blur">
        <div className="flex rounded bg-[var(--app-window)] p-0.5">
          <Button
            aria-label="Use horizontal workflow layout"
            aria-pressed={sequenceDirection === "horizontal"}
            className="size-7 px-0"
            onClick={() => setSequenceDirection("horizontal")}
            title="Horizontal layout"
            type="button"
            variant={sequenceDirection === "horizontal" ? "secondary" : "ghost"}
          >
            <ArrowRight className="size-3.5" aria-hidden="true" />
          </Button>
          <Button
            aria-label="Use vertical workflow layout"
            aria-pressed={sequenceDirection === "vertical"}
            className="size-7 px-0"
            onClick={() => setSequenceDirection("vertical")}
            title="Vertical layout"
            type="button"
            variant={sequenceDirection === "vertical" ? "secondary" : "ghost"}
          >
            <ArrowDown className="size-3.5" aria-hidden="true" />
          </Button>
        </div>
        <RunStateBadge run={run} />
        <Button
          className="h-8 w-34 gap-2 px-3"
          disabled={!activeMovePackage || isRunning || sequence.length === 0}
          onClick={handleRunSequence}
          type="button"
        >
          {isRunning ? (
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          ) : (
            <Play className="size-4" aria-hidden="true" />
          )}
          {isRunning ? "Running" : "Run sequence"}
        </Button>
      </div>

      <div className="grid h-full min-h-0 grid-cols-[176px_minmax(0,1fr)]">
        <StepPalette
          onDragEnd={scheduleClearDrag}
          onDragStart={beginDrag}
          isRunning={isRunning}
          onAddStep={addStep}
          sequence={sequence}
        />

        <SequenceCanvas
          activeMovePackage={activeMovePackage}
          direction={sequenceDirection}
          dropIndex={dropIndex}
          isRunning={isRunning}
          onDropPreview={setDropIndex}
          onDropIndex={handleDrop}
          onMoveStep={moveStep}
          onRemoveStep={removeStep}
          onReorderStep={reorderStep}
          onClearCanvas={clearCanvas}
          onSelectStep={setSelectedStepId}
          onUpdateStep={updateStep}
          packageTree={packageTree}
          projectMetadata={projectMetadata}
          selectedStepId={selectedStepId}
          sequence={sequence}
          stepResults={stepResults}
        />
      </div>
    </section>
  );
}

function StepPalette({
  isRunning,
  onAddStep,
  onDragEnd,
  onDragStart,
  sequence,
}: {
  isRunning: boolean;
  onAddStep: (kind: ExecutionStepKind) => void;
  onDragEnd: () => void;
  onDragStart: (event: React.DragEvent<HTMLElement>, payload: DragPayload) => void;
  sequence: ExecutionStep[];
}) {
  const activeKinds = new Set(sequence.map((step) => step.kind));
  const paletteSteps = stepDefinitions.filter((definition) => !definition.locked);
  const paletteRef = React.useRef<HTMLElement | null>(null);
  const [hoveredStep, setHoveredStep] = React.useState<{
    definition: ExecutionStepDefinition;
    top: number;
  } | null>(null);

  const showStepDescription = React.useCallback(
    (event: React.FocusEvent<HTMLElement> | React.MouseEvent<HTMLElement>, definition: ExecutionStepDefinition) => {
      const paletteRect = paletteRef.current?.getBoundingClientRect();
      const rowRect = event.currentTarget.getBoundingClientRect();

      setHoveredStep({
        definition,
        top: paletteRect ? rowRect.top - paletteRect.top : 8,
      });
    },
    [],
  );

  return (
    <aside
      className="relative z-30 grid min-h-0 border-r border-[color:var(--app-border)] bg-[var(--app-panel)]"
      ref={paletteRef}
    >
      <ScrollArea className="min-h-0">
        <div className="grid gap-1.5 p-2">
          {paletteSteps.map((definition) => {
            const isAdded = activeKinds.has(definition.kind);
            const canAdd = !isRunning && !isAdded;
            const Icon = definition.icon;

            return (
              <Card
                aria-label={canAdd ? `Add ${definition.label}` : definition.label}
                aria-disabled={!canAdd}
                className={cn(
                  "group gap-0 rounded-md px-2.5 py-2 shadow-none transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50",
                  !canAdd
                    ? "border-[color:var(--app-border)] opacity-60"
                    : "cursor-pointer hover:border-primary/45 hover:bg-[var(--app-subtle)] active:cursor-grabbing",
                )}
                draggable={canAdd}
                key={definition.kind}
                onClick={() => {
                  if (canAdd) {
                    onAddStep(definition.kind);
                  }
                }}
                onBlur={() => setHoveredStep(null)}
                onDragEnd={onDragEnd}
                onDragStart={(event) => {
                  onDragStart(event, {
                    kind: definition.kind,
                    source: "palette",
                  });
                }}
                onFocus={(event) => showStepDescription(event, definition)}
                onKeyDown={(event) => {
                  if (!canAdd || (event.key !== "Enter" && event.key !== " ")) {
                    return;
                  }

                  event.preventDefault();
                  onAddStep(definition.kind);
                }}
                onMouseEnter={(event) => showStepDescription(event, definition)}
                onMouseLeave={() => setHoveredStep(null)}
                role="button"
                tabIndex={canAdd ? 0 : -1}
                title={
                  canAdd
                    ? `${definition.label}: ${definition.description}`
                    : isRunning
                      ? "Wait for the current run to finish"
                      : "Already on canvas"
                }
              >
                <div className="grid min-w-0 grid-cols-[30px_minmax(0,1fr)] items-center gap-2">
                  <span className="inline-flex size-7 shrink-0 items-center justify-center rounded-md bg-[var(--app-elevated)] text-muted-foreground group-hover:text-primary">
                    <Icon className="size-3.5" aria-hidden="true" />
                  </span>
                  <div className="grid min-w-0 gap-0.5">
                    <h3 className="truncate text-xs font-semibold leading-4">
                      {definition.shortLabel}
                    </h3>
                    <span className="truncate text-[10px] font-medium leading-3 text-muted-foreground">
                      {definition.category}
                    </span>
                  </div>
                </div>
              </Card>
            );
          })}
        </div>
      </ScrollArea>
      {hoveredStep ? (
        <div
          className="pointer-events-none absolute left-[calc(100%+8px)] z-50 w-72 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-3 text-xs shadow-xl shadow-black/35"
          style={{ top: hoveredStep.top }}
        >
          <div className="flex min-w-0 items-center justify-between gap-3">
            <div className="truncate text-sm font-semibold">{hoveredStep.definition.label}</div>
            <Badge className="rounded px-1.5 py-0 text-[10px]" variant="secondary">
              {hoveredStep.definition.category}
            </Badge>
          </div>
          <p className="mt-2 leading-5 text-muted-foreground">{hoveredStep.definition.description}</p>
          <p className="mt-2 rounded bg-[var(--app-subtle)] px-2 py-1.5 font-mono text-[11px] leading-4 text-muted-foreground">
            {hoveredStep.definition.command}
          </p>
        </div>
      ) : null}
    </aside>
  );
}

type SequenceCanvasProps = {
  activeMovePackage: MovePackage | null;
  direction: SequenceDirection;
  dropIndex: number | null;
  isRunning: boolean;
  onClearCanvas: () => void;
  onDropIndex: (event: React.DragEvent<HTMLElement>, index: number) => void;
  onDropPreview: (index: number | null) => void;
  onMoveStep: (stepId: string, direction: -1 | 1) => void;
  onRemoveStep: (stepId: string) => void;
  onReorderStep: (stepId: string, targetIndex: number) => void;
  onSelectStep: (stepId: string) => void;
  onUpdateStep: (stepId: string, nextStep: ExecutionStepUpdate) => void;
  packageTree: PackageTree;
  projectMetadata: ProjectMetadata | null;
  selectedStepId: string;
  sequence: ExecutionStep[];
  stepResults: Record<string, StepExecutionResult>;
};

function SequenceCanvas(props: SequenceCanvasProps) {
  return (
    <ReactFlowProvider>
      <ExecutionFlowCanvas {...props} />
    </ReactFlowProvider>
  );
}

function ExecutionFlowCanvas({
  activeMovePackage,
  direction,
  dropIndex,
  isRunning,
  onClearCanvas,
  onDropIndex,
  onDropPreview,
  onMoveStep,
  onRemoveStep,
  onReorderStep,
  onSelectStep,
  onUpdateStep,
  packageTree,
  projectMetadata,
  selectedStepId,
  sequence,
  stepResults,
}: SequenceCanvasProps) {
  const { fitView, screenToFlowPosition } = useReactFlow();
  const layoutNodes = React.useMemo(
    () =>
      createExecutionFlowNodes({
        activeMovePackage,
        direction,
        isRunning,
        onMoveStep,
        onRemoveStep,
        onSelectStep,
        onUpdateStep,
        packageTree,
        projectMetadata,
        selectedStepId,
        sequence,
        stepResults,
      }),
    [
      direction,
      activeMovePackage,
      isRunning,
      onMoveStep,
      onRemoveStep,
      onSelectStep,
      onUpdateStep,
      packageTree,
      projectMetadata,
      selectedStepId,
      sequence,
      stepResults,
    ],
  );
  const [flowNodes, setFlowNodes] = React.useState<ExecutionFlowNode[]>(layoutNodes);

  React.useEffect(() => {
    setFlowNodes(layoutNodes);
  }, [layoutNodes]);

  React.useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      void fitView({ duration: 220, padding: 0.18 });
    });

    return () => window.cancelAnimationFrame(frame);
  }, [direction, fitView, sequence.length, stepResults]);

  const displayedNodes = React.useMemo<ExecutionCanvasNode[]>(() => {
    if (dropIndex === null) {
      return flowNodes;
    }

    return [
      ...flowNodes,
      createDropTargetNode(
        clampInsertionIndex(dropIndex, sequence.length),
        direction,
        sequence,
        stepResults,
      ),
    ];
  }, [direction, dropIndex, flowNodes, sequence, stepResults]);

  const flowEdges = React.useMemo(() => createExecutionFlowEdges(sequence, direction), [
    direction,
    sequence,
  ]);

  const handleNodesChange = React.useCallback((changes: NodeChange[]) => {
    setFlowNodes((nodes) => applyNodeChanges(changes, nodes) as ExecutionFlowNode[]);
  }, []);

  const handlePaneDragOver = React.useCallback(
    (event: React.DragEvent<HTMLElement>) => {
      if (isRunning) {
        return;
      }

      event.preventDefault();
      event.dataTransfer.dropEffect = "move";

      const payload = currentExecutionDragPayload ?? readDragPayload(event);

      if (!payload) {
        return;
      }

      onDropPreview(
        insertionIndexForFlowPosition(
          screenToFlowPosition({ x: event.clientX, y: event.clientY }),
          sequence,
          direction,
          stepResults,
        ),
      );
    },
    [direction, isRunning, onDropPreview, screenToFlowPosition, sequence, stepResults],
  );

  const handlePaneDrop = React.useCallback(
    (event: React.DragEvent<HTMLElement>) => {
      const payload = readDragPayload(event) ?? currentExecutionDragPayload;

      if (!payload || isRunning) {
        return;
      }

      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      const targetIndex =
        dropIndex ?? insertionIndexForFlowPosition(position, sequence, direction, stepResults);

      onDropIndex(event, targetIndex);
    },
    [direction, dropIndex, isRunning, onDropIndex, screenToFlowPosition, sequence, stepResults],
  );

  const handleNodeDrag = React.useCallback(
    (_event: React.MouseEvent, node: ExecutionCanvasNode) => {
      if (isRunning || node.id === EXECUTION_DROP_TARGET_ID) {
        return;
      }

      onDropPreview(insertionIndexForFlowPosition(node.position, sequence, direction, stepResults));
    },
    [direction, isRunning, onDropPreview, sequence, stepResults],
  );

  const handleNodeDragStop = React.useCallback(
    (_event: React.MouseEvent, node: ExecutionCanvasNode) => {
      if (isRunning || node.id === EXECUTION_DROP_TARGET_ID) {
        return;
      }

      onSelectStep(node.id);
      onReorderStep(
        node.id,
        dropIndex ?? insertionIndexForFlowPosition(node.position, sequence, direction, stepResults),
      );
      onDropPreview(null);
    },
    [direction, dropIndex, isRunning, onDropPreview, onReorderStep, sequence, stepResults],
  );

  const handleDragLeave = React.useCallback(
    (event: React.DragEvent<HTMLElement>) => {
      const bounds = event.currentTarget.getBoundingClientRect();
      const isInside =
        event.clientX >= bounds.left &&
        event.clientX <= bounds.right &&
        event.clientY >= bounds.top &&
        event.clientY <= bounds.bottom;

      if (isInside) {
        return;
      }

      onDropPreview(null);
    },
    [onDropPreview],
  );

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <main
          className="h-full min-h-0 bg-[var(--app-window)]"
          onDragLeave={handleDragLeave}
          onDragOverCapture={handlePaneDragOver}
          onDropCapture={handlePaneDrop}
        >
          <ReactFlow
            key={`execution-flow-${direction}`}
            colorMode="dark"
            defaultViewport={{ x: 40, y: 80, zoom: 1 }}
            edges={flowEdges}
            edgeTypes={EXECUTION_EDGE_TYPES}
            fitView
            fitViewOptions={{ padding: 0.18 }}
            maxZoom={1.5}
            minZoom={0.22}
            nodes={displayedNodes}
            nodesDraggable={!isRunning}
            nodesFocusable={false}
            nodeTypes={EXECUTION_NODE_TYPES}
            onDragOver={handlePaneDragOver}
            onDrop={handlePaneDrop}
            onNodeClick={(_event, node) => {
              if (node.id !== EXECUTION_DROP_TARGET_ID) {
                onSelectStep(node.id);
              }
            }}
            onNodeDrag={handleNodeDrag}
            onNodeDragStop={handleNodeDragStop}
            onNodesChange={handleNodesChange}
            panOnDrag
            proOptions={{ hideAttribution: true }}
          >
            <Background color="var(--border)" gap={18} size={1} />
            <Controls
              className="!bg-background/90 !shadow-none [&_button]:!border-border [&_button]:!bg-background [&_button]:!text-foreground"
              position="bottom-right"
              showInteractive={false}
            />
          </ReactFlow>
        </main>
      </ContextMenuTrigger>
      <ContextMenuContent className="w-44 border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground">
        <ContextMenuItem
          disabled={isRunning || sequence.length === 0}
          onSelect={onClearCanvas}
          variant="destructive"
        >
          <Trash2 className="size-4" aria-hidden="true" />
          clear canvas
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function ExecutionWorkflowNode({ data }: NodeProps<Node<ExecutionWorkflowNodeData>>) {
  const Icon = data.definition.icon;
  const state = data.result?.state ?? "idle";
  const shouldShowState = state !== "idle";
  const isHorizontal = data.direction === "horizontal";
  const PreviousIcon = isHorizontal ? ArrowLeft : ArrowUp;
  const NextIcon = isHorizontal ? ArrowRight : ArrowDown;
  const sourcePosition = isHorizontal ? Position.Right : Position.Left;
  const targetPosition = Position.Left;
  const usesScript = data.step.config.useScript === true;
  const commandLabel = commandPreview(
    data.definition,
    data.step,
    data.projectMetadata,
    data.activeMovePackage,
  );
  const durationLabel = executionDurationLabel(data.result);
  const isError = state === "error";
  const handleStyle = !isHorizontal
    ? {
        left: -EXECUTION_TIMELINE_OFFSET,
        top: "50%",
        transform: "translateY(-50%)",
      }
    : undefined;

  return (
    <article
      className={cn(
        "group relative min-w-0 overflow-visible rounded-lg border border-[color:var(--app-border)] bg-[var(--app-surface)] text-foreground shadow-[0_14px_30px_rgba(0,0,0,0.22)] transition",
        isHorizontal ? "w-[520px]" : "w-[760px]",
        state === "running" && "execution-step-active border-primary/45",
        data.selected && "border-primary/55 bg-[var(--app-elevated)] shadow-lg",
        !data.step.locked && !data.isRunning && "cursor-grab active:cursor-grabbing",
      )}
      title={data.step.locked ? undefined : "Drag to reorder"}
    >
      <StepTimelineMarker direction={data.direction} state={state} />
      <Handle
        className="!size-1 !border-0 !bg-transparent !opacity-0"
        id="target"
        isConnectable={false}
        position={targetPosition}
        style={handleStyle}
        type="target"
      />
      <Handle
        className="!size-1 !border-0 !bg-transparent !opacity-0"
        id="source"
        isConnectable={false}
        position={sourcePosition}
        style={handleStyle}
        type="source"
      />

      <div
        className={cn(
          "grid min-h-[96px] min-w-0 grid-cols-[52px_46px_minmax(0,1fr)_auto_auto] items-center gap-5 px-6 py-4",
          isHorizontal && "grid-cols-[48px_40px_minmax(0,1fr)] gap-x-4 gap-y-3",
        )}
      >
        <span className="grid size-12 shrink-0 place-items-center rounded-lg bg-[var(--app-subtle)] text-base font-semibold text-muted-foreground">
          {data.index + 1}
        </span>
        <span
          className={cn(
            "grid size-10 shrink-0 place-items-center rounded-md bg-[var(--app-elevated)] shadow-sm",
            stepIconToneClass(state),
          )}
        >
          <Icon className="size-6" aria-hidden="true" />
        </span>

        <div className="grid min-w-0 gap-2">
          <div className="flex min-w-0 items-center gap-3 leading-none">
            <h3 className="min-w-0 truncate text-xl font-semibold leading-6 tracking-normal">
              {data.definition.label}
            </h3>
            {shouldShowState ? <StepStateBadge state={state} compact /> : null}
          </div>
          {usesScript ? (
            <ScriptPathPicker
              activeMovePackage={data.activeMovePackage}
              disabled={data.isRunning}
              onUpdateStep={data.onUpdateStep}
              packageTree={data.packageTree}
              step={data.step}
            />
          ) : (
            <p className="truncate font-mono text-sm leading-5 text-muted-foreground">
              {isError ? commandLabel : data.result?.summary ?? commandLabel}
            </p>
          )}
        </div>

        <div
          className={cn(
            "flex min-w-[76px] items-center justify-end gap-1.5 text-sm text-muted-foreground",
            isHorizontal && "col-start-2 col-span-2 justify-start",
          )}
        >
          {durationLabel ? (
            <>
              <Clock3 className="size-4 shrink-0" aria-hidden="true" />
              <span className="tabular-nums">{durationLabel}</span>
            </>
          ) : null}
        </div>

        <div
          className={cn(
            "nodrag nopan grid h-12 shrink-0 grid-cols-3 overflow-hidden rounded-lg border border-[color:var(--app-border)] bg-[var(--app-elevated)]",
            isHorizontal && "col-span-3 w-full grid-cols-3",
          )}
        >
          <Button
            aria-label="Select step"
            className="h-12 rounded-none border-r border-[color:var(--app-border)] text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground"
            onClick={() => data.onSelectStep(data.step.id)}
            type="button"
            variant="ghost"
          >
            <ChevronRight className="size-5" aria-hidden="true" />
          </Button>
          <Button
            aria-label={usesScript ? "Use default command" : "Use bash script"}
            aria-pressed={usesScript}
            className={cn(
              "h-12 rounded-none border-r border-[color:var(--app-border)] text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
              usesScript && "bg-primary/15 text-primary hover:bg-primary/20 hover:text-primary",
            )}
            disabled={data.isRunning}
            onClick={() =>
              data.onUpdateStep(data.step.id, {
                config: {
                  scriptPath: data.step.config.scriptPath,
                  useScript: !usesScript,
                },
              })
            }
            title={usesScript ? "Use default command" : "Use bash script"}
            type="button"
            variant="ghost"
          >
            <FileText className="size-5" aria-hidden="true" />
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                aria-label="Step actions"
                className="h-12 rounded-none text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground"
                disabled={data.isRunning}
                type="button"
                variant="ghost"
              >
                <MoreVertical className="size-5" aria-hidden="true" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
              <DropdownMenuItem
                disabled={data.step.locked || data.index <= 0}
                onClick={() => data.onMoveStep(data.step.id, -1)}
              >
                <PreviousIcon className="mr-2 size-3.5" aria-hidden="true" />
                Move {isHorizontal ? "left" : "up"}
              </DropdownMenuItem>
              <DropdownMenuItem
                disabled={data.step.locked || data.index >= data.sequenceLength - 1}
                onClick={() => data.onMoveStep(data.step.id, 1)}
              >
                <NextIcon className="mr-2 size-3.5" aria-hidden="true" />
                Move {isHorizontal ? "right" : "down"}
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={() =>
                  data.onUpdateStep(data.step.id, {
                    config: {
                      scriptPath: data.step.config.scriptPath,
                      useScript: !usesScript,
                    },
                  })
                }
              >
                <Terminal className="mr-2 size-3.5" aria-hidden="true" />
                {usesScript ? "Use default" : "Use script"}
              </DropdownMenuItem>
              <DropdownMenuCheckboxItem
                checked={!data.step.config.stopOnFailure}
                onCheckedChange={(checked) =>
                  data.onUpdateStep(data.step.id, {
                    config: {
                      stopOnFailure: checked !== true,
                    },
                  })
                }
              >
                Continue after failure
              </DropdownMenuCheckboxItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                className="text-destructive focus:text-destructive"
                disabled={data.step.locked}
                onClick={() => data.onRemoveStep(data.step.id)}
              >
                <Trash2 className="mr-2 size-3.5" aria-hidden="true" />
                Remove
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      {data.step.kind === "publish" && !usesScript ? (
        <div className="px-6 pb-4">
          <PublishConfig
            disabled={data.isRunning}
            step={data.step}
            onUpdateStep={data.onUpdateStep}
          />
        </div>
      ) : null}

      {isError ? (
        <div className="border-t border-[color:var(--app-border)] px-5 pb-5 pt-4">
          <div className="grid min-h-[76px] grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-3 rounded-md border border-destructive/30 bg-destructive/10 px-5">
            <div className="min-w-0">
              <div className="text-base font-semibold text-destructive">Step failed</div>
              <p className="mt-1 truncate font-mono text-sm leading-5 text-muted-foreground">
                {commandLabel}
              </p>
            </div>
            <Button
              className="h-10 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-5 text-sm text-foreground hover:bg-[var(--app-elevated)]"
              onClick={() => data.onSelectStep(data.step.id)}
              type="button"
              variant="ghost"
            >
              View logs
            </Button>
            <Button
              aria-label="Collapse failure details"
              className="size-10 rounded-md text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground"
              type="button"
              variant="ghost"
            >
              <ChevronDown className="size-4" aria-hidden="true" />
            </Button>
          </div>
        </div>
      ) : null}
    </article>
  );
}

function StepTimelineMarker({
  direction,
  state,
}: {
  direction: SequenceDirection;
  state: ExecutionStepState;
}) {
  const isHorizontal = direction === "horizontal";

  return (
    <span
      className={cn(
        "pointer-events-none absolute z-10 grid size-7 place-items-center rounded-full border border-[color:var(--app-border)] bg-[var(--app-surface)] text-[13px] text-muted-foreground shadow-[0_0_0_4px_var(--app-window)]",
        isHorizontal
          ? "left-[-38px] top-1/2 -translate-y-1/2"
          : "left-[-62px] top-1/2 -translate-y-1/2",
        state === "success" && "text-emerald-400",
        state === "error" && "text-red-400",
        state === "attention" && "text-amber-400",
        state === "running" && "text-primary",
      )}
    >
      {state === "success" ? <CheckCircle2 className="size-4" aria-hidden="true" /> : null}
      {state === "error" ? <XCircle className="size-4" aria-hidden="true" /> : null}
      {state === "attention" ? <AlertTriangle className="size-4" aria-hidden="true" /> : null}
      {state === "running" ? <Loader2 className="size-4 animate-spin" aria-hidden="true" /> : null}
      {state === "idle" || state === "skipped" ? <Circle className="size-3.5" aria-hidden="true" /> : null}
    </span>
  );
}

function stepIconToneClass(state: ExecutionStepState) {
  switch (state) {
    case "success":
      return "text-emerald-400";
    case "attention":
      return "text-amber-400";
    case "error":
      return "text-red-400";
    case "running":
      return "text-primary";
    case "idle":
    case "skipped":
      return "text-muted-foreground";
  }
}

function executionDurationLabel(result: StepExecutionResult | undefined) {
  if (!result?.startedAt) {
    return null;
  }

  const finishedAt = result.finishedAt ?? (result.state === "running" ? new Date() : null);

  if (!finishedAt) {
    return null;
  }

  const elapsedSeconds = Math.max(
    1,
    Math.round((finishedAt.getTime() - result.startedAt.getTime()) / 1000),
  );

  return `${elapsedSeconds}s`;
}

function ExecutionDropTargetNode({ data }: NodeProps<Node<ExecutionDropNodeData>>) {
  const isHorizontal = data.direction === "horizontal";

  return (
    <div
      className={cn(
        "pointer-events-none grid place-items-center rounded-lg border border-dashed border-primary/70 bg-primary/10 text-xs font-medium text-primary shadow-sm",
        isHorizontal ? "h-[92px] w-16" : "h-12 w-[760px]",
      )}
    >
      <span className="inline-flex items-center gap-1.5 whitespace-nowrap">
        <Plus className="size-3.5" aria-hidden="true" />
        Drop here
      </span>
    </div>
  );
}

function ExecutionSequenceEdge({
  data,
  sourceX,
  sourceY,
  targetX,
  targetY,
}: EdgeProps<Edge<ExecutionSequenceEdgeData>>) {
  const path =
    data?.direction === "vertical"
      ? `M ${sourceX},${sourceY} L ${sourceX},${targetY}`
      : `M ${sourceX},${sourceY} L ${targetX},${sourceY}`;

  return (
    <BaseEdge
      path={path}
      style={{
        stroke: "var(--app-border)",
        strokeWidth: 1.7,
      }}
    />
  );
}

function createExecutionFlowNodes({
  activeMovePackage,
  direction,
  isRunning,
  onMoveStep,
  onRemoveStep,
  onSelectStep,
  onUpdateStep,
  packageTree,
  projectMetadata,
  selectedStepId,
  sequence,
  stepResults,
}: {
  activeMovePackage: MovePackage | null;
  direction: SequenceDirection;
  isRunning: boolean;
  onMoveStep: (stepId: string, direction: -1 | 1) => void;
  onRemoveStep: (stepId: string) => void;
  onSelectStep: (stepId: string) => void;
  onUpdateStep: (stepId: string, nextStep: ExecutionStepUpdate) => void;
  packageTree: PackageTree;
  projectMetadata: ProjectMetadata | null;
  selectedStepId: string;
  sequence: ExecutionStep[];
  stepResults: Record<string, StepExecutionResult>;
}): ExecutionFlowNode[] {
  return sequence.map((step, index) => ({
    id: step.id,
    type: "executionStep",
    data: {
      activeMovePackage,
      definition: definitionByKind[step.kind],
      direction,
      index,
      isRunning,
      onMoveStep,
      onRemoveStep,
      onSelectStep,
      onUpdateStep,
      packageTree,
      projectMetadata,
      result: stepResults[step.id],
      selected: selectedStepId === step.id,
      sequenceLength: sequence.length,
      step,
    },
    draggable: !step.locked && !isRunning,
    focusable: false,
    position: executionNodePosition(index, direction, sequence, stepResults),
    selectable: true,
    zIndex: selectedStepId === step.id ? 20 : 10,
  }));
}

function createExecutionFlowEdges(
  sequence: ExecutionStep[],
  direction: SequenceDirection,
): Edge<ExecutionSequenceEdgeData>[] {
  return sequence.slice(1).map((step, index) => {
    const previousStep = sequence[index];
    const color = "#71717a";

    return {
      id: `${direction}:${previousStep.id}->${step.id}`,
      data: { direction },
      source: previousStep.id,
      sourceHandle: "source",
      target: step.id,
      targetHandle: "target",
      style: {
        stroke: color,
        strokeWidth: 1.7,
      },
      type: "sequence",
      zIndex: 1,
    };
  });
}

function createDropTargetNode(
  dropIndex: number,
  direction: SequenceDirection,
  sequence: ExecutionStep[],
  stepResults: Record<string, StepExecutionResult>,
): ExecutionDropNode {
  return {
    id: EXECUTION_DROP_TARGET_ID,
    type: "dropTarget",
    data: { direction },
    draggable: false,
    focusable: false,
    position: dropTargetPosition(dropIndex, direction, sequence, stepResults),
    selectable: false,
    zIndex: 100,
  };
}

function insertionIndexForFlowPosition(
  position: { x: number; y: number },
  sequence: ExecutionStep[],
  direction: SequenceDirection,
  stepResults: Record<string, StepExecutionResult>,
) {
  const coordinate = direction === "horizontal" ? position.x : position.y;

  for (let index = 0; index < sequence.length; index += 1) {
    const nodePosition = executionNodePosition(index, direction, sequence, stepResults);
    const nodeSize = executionNodeSize(sequence[index], stepResults[sequence[index].id], direction);
    const midpoint =
      direction === "horizontal"
        ? nodePosition.x + nodeSize.width / 2
        : nodePosition.y + nodeSize.height / 2;

    if (coordinate < midpoint) {
      return index;
    }
  }

  return sequence.length;
}

function executionNodePosition(
  index: number,
  direction: SequenceDirection,
  sequence: ExecutionStep[],
  stepResults: Record<string, StepExecutionResult>,
) {
  if (direction === "horizontal") {
    return {
      x: EXECUTION_FLOW_START_X + index * (EXECUTION_HORIZONTAL_NODE_WIDTH + EXECUTION_HORIZONTAL_GAP),
      y: EXECUTION_FLOW_START_Y,
    };
  }

  const yOffset = sequence.slice(0, index).reduce((total, step) => {
    return total + executionNodeSize(step, stepResults[step.id], direction).height + EXECUTION_VERTICAL_GAP;
  }, 0);

  return {
    x: EXECUTION_FLOW_START_X,
    y: EXECUTION_FLOW_START_Y + yOffset,
  };
}

function dropTargetPosition(
  index: number,
  direction: SequenceDirection,
  sequence: ExecutionStep[],
  stepResults: Record<string, StepExecutionResult>,
) {
  const insertionIndex = Math.max(0, index);

  if (direction === "horizontal") {
    return {
      x:
        EXECUTION_FLOW_START_X +
        insertionIndex * (EXECUTION_HORIZONTAL_NODE_WIDTH + EXECUTION_HORIZONTAL_GAP) -
        EXECUTION_HORIZONTAL_GAP / 2 -
        32,
      y: EXECUTION_FLOW_START_Y + 2,
    };
  }

  const yOffset = sequence.slice(0, insertionIndex).reduce((total, step) => {
    return total + executionNodeSize(step, stepResults[step.id], direction).height + EXECUTION_VERTICAL_GAP;
  }, 0);

  return {
    x: EXECUTION_FLOW_START_X,
    y: EXECUTION_FLOW_START_Y + yOffset - EXECUTION_VERTICAL_GAP / 2 - 24,
  };
}

function executionNodeSize(
  step: ExecutionStep,
  result: StepExecutionResult | undefined,
  direction: SequenceDirection,
) {
  const hasInlinePublishConfig = step.kind === "publish" && step.config.useScript !== true;
  const isFailed = result?.state === "error";

  return {
    height: isFailed
      ? EXECUTION_FAILED_NODE_HEIGHT
      : hasInlinePublishConfig
        ? EXECUTION_PUBLISH_NODE_HEIGHT
        : EXECUTION_NODE_HEIGHT,
    width: direction === "horizontal" ? EXECUTION_HORIZONTAL_NODE_WIDTH : EXECUTION_NODE_WIDTH,
  };
}

function clampInsertionIndex(index: number, sequenceLength: number) {
  return Math.max(0, Math.min(index, sequenceLength));
}

function ScriptPathPicker({
  activeMovePackage,
  disabled,
  onUpdateStep,
  packageTree,
  step,
}: {
  activeMovePackage: MovePackage | null;
  disabled: boolean;
  onUpdateStep: (stepId: string, nextStep: ExecutionStepUpdate) => void;
  packageTree: PackageTree;
  step: ExecutionStep;
}) {
  const [isPicking, setIsPicking] = React.useState(false);
  const scriptPath = normalizedScriptPath(step);

  const handlePickScript = React.useCallback(async () => {
    if (disabled || isPicking || !activeMovePackage) {
      return;
    }

    setIsPicking(true);

    try {
      const packageDirectory = absolutePackagePath(packageTree, activeMovePackage.path);
      const selectedPath = await open({
        defaultPath: packageDirectory,
        directory: false,
        multiple: false,
        title: "Choose bash script",
      });

      if (!selectedPath || Array.isArray(selectedPath)) {
        return;
      }

      const relativePath = packageRelativePath(packageDirectory, selectedPath);

      if (!relativePath) {
        onUpdateStep(step.id, {
          config: {
            scriptPath: "",
          },
        });
        return;
      }

      onUpdateStep(step.id, {
        config: {
          scriptPath: relativePath,
        },
      });
    } finally {
      setIsPicking(false);
    }
  }, [activeMovePackage, disabled, isPicking, onUpdateStep, packageTree, step.id]);

  return (
    <div className="nodrag nopan flex h-6 min-w-0 items-center gap-1.5">
      <button
        aria-label={scriptPath ? `Selected bash script ${scriptPath}` : "Choose bash script"}
        className={cn(
          "flex h-6 min-w-0 flex-1 items-center gap-1.5 rounded border border-[color:var(--app-border)] bg-[var(--app-window)]/85 px-2 font-mono text-[10.5px] leading-4 text-muted-foreground transition hover:border-primary/45 hover:text-foreground",
          !scriptPath && "text-muted-foreground/75",
        )}
        disabled={disabled || isPicking || !activeMovePackage}
        onClick={handlePickScript}
        type="button"
      >
        {isPicking ? (
          <Loader2 className="size-3.5 shrink-0 animate-spin" aria-hidden="true" />
        ) : (
          <FolderOpen className="size-3.5 shrink-0" aria-hidden="true" />
        )}
        <span className="truncate">{scriptPath || scriptPathPlaceholder(step.kind)}</span>
      </button>
    </div>
  );
}

function PublishConfig({
  disabled,
  onUpdateStep,
  step,
}: {
  disabled: boolean;
  onUpdateStep: (stepId: string, nextStep: ExecutionStepUpdate) => void;
  step: ExecutionStep;
}) {
  const target = step.config.publishTarget ?? "localnet";
  const dryRun = step.config.publishDryRun !== false;

  return (
    <div className="nodrag nopan col-span-3 grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded border border-[color:var(--app-border)] bg-[var(--app-window)]/80 p-1.5">
      <div className="grid min-w-0 grid-cols-4 gap-1">
        {publishTargets.map((option) => (
          <Button
            className="h-6 min-w-0 justify-center rounded px-1 text-[10.5px]"
            disabled={disabled}
            key={option.value}
            onClick={() =>
              onUpdateStep(step.id, {
                config: {
                  publishTarget: option.value,
                },
              })
            }
            type="button"
            variant={target === option.value ? "secondary" : "ghost"}
          >
            <span className="truncate">{option.label}</span>
          </Button>
        ))}
      </div>
      <label className="flex h-6 shrink-0 items-center gap-1.5 rounded px-1.5 text-[10.5px] text-muted-foreground">
        <input
          checked={dryRun}
          className="size-3 shrink-0 accent-primary"
          disabled={disabled}
          onChange={(event) =>
            onUpdateStep(step.id, {
              config: {
                publishDryRun: event.target.checked,
              },
            })
          }
          type="checkbox"
        />
        <span className="whitespace-nowrap">Dry-run</span>
      </label>
    </div>
  );
}

function commandPreview(
  definition: ExecutionStepDefinition,
  step: ExecutionStep,
  projectMetadata: ProjectMetadata | null = null,
  movePackage: MovePackage | null = null,
) {
  if (isLocalScriptEnabled(step)) {
    return normalizedScriptPath(step) || "Bash script";
  }

  const configuredScript = projectMetadata && movePackage
    ? configuredProjectScriptForStep(step, projectMetadata, movePackage)
    : null;

  if (configuredScript) {
    return configuredScriptCommand(configuredScript);
  }

  if (step.kind === "publish") {
    const target = step.config.publishTarget ?? "localnet";
    const prefix = step.config.publishDryRun === false ? "sui client publish" : "sui client publish --dry-run";
    const pubfile = step.config.publishDryRun === false ? "" : " --pubfile-path <temporary>";

    return `${prefix} --client.env ${target}${pubfile} .`;
  }

  return definition.command;
}

function configuredProjectScriptForStep(
  step: ExecutionStep,
  metadata: ProjectMetadata,
  movePackage: MovePackage,
): ProjectConfiguredScript | null {
  if (isLocalScriptEnabled(step)) {
    return null;
  }

  if (step.kind === "test") {
    const scriptPath = projectMoveTestScriptPath(metadata, movePackage);

    return scriptPath
      ? {
          args: [],
          modeLabel: "Project test script",
          scriptPath,
        }
      : null;
  }

  if (step.kind === "coverage") {
    const coverageScriptPath = projectMoveCoverageScriptPath(metadata, movePackage);

    if (coverageScriptPath) {
      return {
        args: [],
        modeLabel: "Project coverage script",
        scriptPath: coverageScriptPath,
      };
    }

    const testScriptPath = projectMoveTestScriptPath(metadata, movePackage);

    return testScriptPath
      ? {
          args: ["--coverage"],
          modeLabel: "Project test script",
          scriptPath: testScriptPath,
        }
      : null;
  }

  return null;
}

function configuredScriptCommand(configuredScript: ProjectConfiguredScript) {
  return `bash ${configuredScript.scriptPath}${configuredScript.args.length ? ` ${configuredScript.args.join(" ")}` : ""}`;
}

function scriptPathPlaceholder(kind: ExecutionStepKind) {
  switch (kind) {
    case "build":
      return "scripts/build.sh";
    case "coverage":
      return "scripts/coverage.sh";
    case "test":
      return "scripts/test.sh";
    case "fuzz":
      return "scripts/fuzz.sh";
    case "formal":
      return "scripts/prove.sh";
    case "publish":
      return "scripts/publish-testnet.sh";
  }
}

function normalizedScriptPath(step: ExecutionStep) {
  return step.config.scriptPath?.trim() ?? "";
}

function isLocalScriptEnabled(step: ExecutionStep) {
  return step.config.useScript === true;
}

function executionLogRun({
  detail,
  error = null,
  finishedAt = null,
  movePackage,
  output = null,
  packageTree,
  projectMetadata,
  startedAt,
  state,
  step,
  summary,
}: {
  detail?: string | null;
  error?: string | null;
  finishedAt?: Date | null;
  movePackage: MovePackage;
  output?: CommandOutput | null;
  packageTree: PackageTree;
  projectMetadata: ProjectMetadata;
  startedAt: Date;
  state: BuildLogRun["state"];
  step: ExecutionStep;
  summary?: string | null;
}): BuildLogRun {
  const definition = definitionByKind[step.kind];
  const localScriptPath = isLocalScriptEnabled(step) ? normalizedScriptPath(step) : "";
  const configuredScript = configuredProjectScriptForStep(step, projectMetadata, movePackage);
  const command = executionLogCommand(definition, step, configuredScript);

  return {
    canRerun: false,
    command,
    emptyText: "Execution step finished without command output.",
    error,
    finishedAt,
    id: `${step.id}:${startedAt.getTime()}`.split("").reduce((hash, character) => {
      return (hash * 31 + character.charCodeAt(0)) >>> 0;
    }, 7),
    metadata: [
      { label: "Step", value: definition.label },
      {
        label: "Mode",
        value: localScriptPath ? "Bash script" : configuredScript?.modeLabel ?? "Default",
      },
      ...(configuredScript && configuredScript.args.length
        ? [{ label: "Args", value: configuredScript.args.join(" ") }]
        : []),
      ...(summary ? [{ label: "Summary", value: summary }] : []),
    ],
    note: detail ?? null,
    output,
    packageName: movePackage.name,
    packagePath: movePackage.path || ".",
    runningText: `Running ${definition.label.toLowerCase()}...`,
    startedAt,
    state,
    title: "Execution step",
    workingDirectory: absolutePackagePath(packageTree, movePackage.path),
  };
}

function executionLogCommand(
  definition: ExecutionStepDefinition,
  step: ExecutionStep,
  configuredScript: ProjectConfiguredScript | null,
) {
  const scriptPath = isLocalScriptEnabled(step) ? normalizedScriptPath(step) : "";

  if (step.kind === "build" && scriptPath) {
    return `sui move build && bash ${scriptPath}`;
  }

  if (scriptPath) {
    return `bash ${scriptPath}`;
  }

  if (configuredScript) {
    return configuredScriptCommand(configuredScript);
  }

  return commandPreview(definition, step);
}

function absolutePackagePath(packageTree: PackageTree, packagePath: string) {
  const rootPath = packageTree.rootPath.replace(/\/+$/, "");
  const relativePath = packagePath.replace(/^\/+|\/+$/g, "");

  return relativePath ? `${rootPath}/${relativePath}` : rootPath;
}

function packageRelativePath(packageDirectory: string, selectedPath: string) {
  const normalizedPackageDirectory = normalizeFilePath(packageDirectory).replace(/\/+$/, "");
  const normalizedSelectedPath = normalizeFilePath(selectedPath);
  const prefix = `${normalizedPackageDirectory}/`;

  if (!normalizedSelectedPath.startsWith(prefix)) {
    return null;
  }

  return normalizedSelectedPath.slice(prefix.length);
}

function normalizeFilePath(path: string) {
  return path.replace(/\\/g, "/");
}

function RunStateBadge({ run }: { run: ExecutionRun | null }) {
  if (!run || run.state === "running") {
    return null;
  }

  return (
    <Badge
      className={cn(
        "gap-1 rounded px-2 py-1 text-xs",
        run.state === "success" && "bg-emerald-500/15 text-emerald-400",
        run.state === "attention" && "bg-amber-500/15 text-amber-400",
        run.state === "error" && "bg-red-500/15 text-red-400",
      )}
      variant="secondary"
    >
      {run.state === "success" ? <CheckCircle2 className="size-3" /> : null}
      {run.state === "attention" ? <AlertTriangle className="size-3" /> : null}
      {run.state === "error" ? <XCircle className="size-3" /> : null}
      {run.state === "success" ? "Passed" : run.state === "attention" ? "Needs review" : "Failed"}
    </Badge>
  );
}

function StepStateBadge({
  compact = false,
  state,
}: {
  compact?: boolean;
  state: ExecutionStepState;
}) {
  return (
    <Badge
      className={cn(
        "shrink-0 gap-1 rounded-md px-2 py-1 text-sm leading-none",
        compact && "max-w-[7rem]",
        state === "idle" && "bg-muted text-muted-foreground",
        state === "running" && "bg-muted text-muted-foreground",
        state === "success" && "bg-emerald-500/15 text-emerald-400",
        state === "attention" && "bg-amber-500/15 text-amber-400",
        state === "error" && "bg-red-500/15 text-red-400",
        state === "skipped" && "bg-muted text-muted-foreground",
      )}
      variant="secondary"
    >
      {state === "idle" ? <Circle className="size-3" /> : null}
      {state === "running" ? <Loader2 className="size-3 animate-spin" /> : null}
      {state === "success" ? <CheckCircle2 className="size-3" /> : null}
      {state === "attention" ? <AlertTriangle className="size-3" /> : null}
      {state === "error" ? <XCircle className="size-3" /> : null}
      <span className="truncate">{stepStateLabel(state)}</span>
    </Badge>
  );
}

async function executeStep({
  movePackage,
  onCommandOutput,
  onProjectSelected,
  packageTree,
  projectMetadata,
  startedAt,
  streamId,
  step,
}: {
  movePackage: MovePackage;
  onCommandOutput?: (output: CommandOutput) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  packageTree: PackageTree;
  projectMetadata: ProjectMetadata;
  startedAt: Date;
  streamId?: number | string;
  step: ExecutionStep;
}): Promise<StepExecutionOutcome> {
  const definition = definitionByKind[step.kind];

  if (step.kind === "build") {
    const output = await buildMovePackage(packageTree, movePackage.path, {
      onOutput: onCommandOutput,
      streamId,
    });
    const finishedAt = new Date();

    if (output.status !== 0) {
      return {
        detail: "The sequence stopped before analysis because package summaries may be stale or missing.",
        finishedAt,
        output,
        startedAt,
        state: "error",
        summary: "`sui move build` failed.",
      };
    }

    const refreshedPackageTree = await loadPackageTree(packageTree.rootPath);
    const activePackageManifestPath = refreshedPackageTree.movePackages.some(
      (candidate) => candidate.manifestPath === movePackage.manifestPath,
    )
      ? movePackage.manifestPath
      : refreshedPackageTree.movePackages[0]?.manifestPath ?? null;
    const nextPackageTree = {
      ...refreshedPackageTree,
      activePackageManifestPath,
    };

    onProjectSelected(nextPackageTree);

    if (isLocalScriptEnabled(step)) {
      const refreshedMovePackage =
        resolveActiveMovePackage(nextPackageTree, movePackage) ?? movePackage;
      const scriptOutcome = await executeScriptStep({
        movePackage: refreshedMovePackage,
        onCommandOutput,
        packageTree: nextPackageTree,
        startedAt,
        streamId,
        step,
      });

      return {
        ...scriptOutcome,
        detail: [
          nextPackageTree.dependencyGraph.summaryPath
            ? `Summary directory: ${nextPackageTree.dependencyGraph.summaryPath}`
            : "Package summaries were not found after rescanning; dependency graph detail may be limited.",
          scriptOutcome.detail,
        ]
          .filter(Boolean)
          .join("\n"),
        packageTree: nextPackageTree,
        state: scriptOutcome.state,
        summary: scriptOutcome.summary,
      };
    }

    return {
      detail: nextPackageTree.dependencyGraph.summaryPath
        ? `Summary directory: ${nextPackageTree.dependencyGraph.summaryPath}`
        : "Package summaries were not found after rescanning; dependency graph detail may be limited.",
      finishedAt,
      output,
      packageTree: nextPackageTree,
      startedAt,
      state: "success",
      summary: nextPackageTree.dependencyGraph.summaryPath
        ? "Build succeeded and package summaries were refreshed."
        : "Build succeeded.",
    };
  }

  if (isLocalScriptEnabled(step)) {
    return executeScriptStep({
      movePackage,
      onCommandOutput,
      packageTree,
      startedAt,
      streamId,
      step,
    });
  }

  const configuredScript = configuredProjectScriptForStep(step, projectMetadata, movePackage);

  if (configuredScript) {
    return executeConfiguredProjectScriptStep({
      configuredScript,
      movePackage,
      onCommandOutput,
      packageTree,
      startedAt,
      streamId,
      step,
    });
  }

  if (step.kind === "publish") {
    const commandKind = publishCommandKind(step);
    const output = await runSecurityCommand(packageTree, movePackage.path, commandKind, {
      onOutput: onCommandOutput,
      streamId,
    });
    const targetLabel = publishTargetLabel(step.config.publishTarget ?? "localnet");
    const finishedAt = new Date();

    return {
      detail:
        output.status === 0
          ? `${step.config.publishDryRun === false ? "Publish" : "Publish dry run"} completed for ${targetLabel}.`
          : `${step.config.publishDryRun === false ? "Publish" : "Publish dry run"} failed for ${targetLabel}.`,
      finishedAt,
      output,
      startedAt,
      state: output.status === 0 ? "success" : "error",
      summary:
        output.status === 0
          ? `${targetLabel} publish ${step.config.publishDryRun === false ? "completed" : "dry run passed"}.`
          : `${targetLabel} publish ${step.config.publishDryRun === false ? "failed" : "dry run failed"}.`,
    };
  }

  if (step.kind === "formal") {
    return executeFormalVerificationStep({
      movePackage,
      onCommandOutput,
      packageTree,
      startedAt,
      streamId,
    });
  }

  if (definition.commandKind) {
    const output = await runSecurityCommand(
      packageTree,
      movePackage.path,
      definition.commandKind,
      {
        onOutput: onCommandOutput,
        streamId,
      },
    );
    const finishedAt = new Date();

    return {
      detail:
        output.status === 0
          ? "Command completed successfully."
          : "Command exited with a non-zero status.",
      finishedAt,
      output,
      startedAt,
      state: output.status === 0 ? "success" : "error",
      summary:
        output.status === 0
          ? `${definition.command} passed.`
          : `${definition.command} failed.`,
    };
  }

  return {
    detail: null,
    finishedAt: new Date(),
    output: null,
    state: "error",
    startedAt,
    summary: "Unsupported execution step.",
  };
}

async function executeFormalVerificationStep({
  movePackage,
  onCommandOutput,
  packageTree,
  startedAt,
  streamId,
}: {
  movePackage: MovePackage;
  onCommandOutput?: (output: CommandOutput) => void;
  packageTree: PackageTree;
  startedAt: Date;
  streamId?: number | string;
}): Promise<StepExecutionOutcome> {
  if (movePackage.modules.length === 0) {
    return {
      detail: "No parseable Move modules were found for the active package.",
      finishedAt: new Date(),
      output: null,
      startedAt,
      state: "attention",
      summary: "Formal verification requires parseable Move modules.",
    };
  }

  const aggregateOutput: CommandOutput = {
    status: 0,
    stderr: "",
    stdout: "",
  };
  const failedModules: string[] = [];

  for (const moveModule of movePackage.modules) {
    const output = await runFormalVerification(
      packageTree,
      movePackage.path,
      moveModule.filePath,
      moveModule.name,
      {
        onOutput: onCommandOutput,
        streamId,
        timeoutSeconds: FORMAL_VERIFICATION_TIMEOUT_SECONDS,
      },
    );

    aggregateOutput.stdout += output.stdout;
    aggregateOutput.stderr += output.stderr;

    if (output.status !== 0) {
      aggregateOutput.status = output.status ?? 1;
      failedModules.push(moveModule.name);
    }

    onCommandOutput?.({ ...aggregateOutput });
  }

  const finishedAt = new Date();

  if (failedModules.length > 0) {
    return {
      detail: `Failed modules: ${failedModules.join(", ")}`,
      finishedAt,
      output: aggregateOutput,
      startedAt,
      state: "error",
      summary: `${failedModules.length} formal verification target${failedModules.length === 1 ? "" : "s"} failed.`,
    };
  }

  return {
    detail: `${movePackage.modules.length} module${movePackage.modules.length === 1 ? "" : "s"} verified with bundled Sui Prover.`,
    finishedAt,
    output: aggregateOutput,
    startedAt,
    state: "success",
    summary: "Formal verification passed.",
  };
}

async function executeScriptStep({
  movePackage,
  onCommandOutput,
  packageTree,
  startedAt,
  streamId,
  step,
}: {
  movePackage: MovePackage;
  onCommandOutput?: (output: CommandOutput) => void;
  packageTree: PackageTree;
  startedAt: Date;
  streamId?: number | string;
  step: ExecutionStep;
}): Promise<StepExecutionOutcome> {
  const scriptPath = normalizedScriptPath(step);
  const definition = definitionByKind[step.kind];

  if (!scriptPath) {
    return {
      detail: "Add a script path or turn off the bash script override for this step.",
      finishedAt: new Date(),
      output: null,
      startedAt,
      state: "error",
      summary: "Bash script path is empty.",
    };
  }

  const output = await runSecurityScript(packageTree, movePackage.path, scriptPath, {
    onOutput: onCommandOutput,
    streamId,
  });
  const finishedAt = new Date();

  return {
    detail:
      output.status === 0
        ? `Bash script completed in ${displayMovePackageName(movePackage.name)}.`
        : `Bash script exited with a non-zero status in ${displayMovePackageName(movePackage.name)}.`,
    finishedAt,
    output,
    startedAt,
    state: output.status === 0 ? "success" : "error",
    summary:
      output.status === 0
        ? `${definition.label} script passed.`
        : `${definition.label} script failed.`,
  };
}

async function executeConfiguredProjectScriptStep({
  configuredScript,
  movePackage,
  onCommandOutput,
  packageTree,
  startedAt,
  streamId,
  step,
}: {
  configuredScript: ProjectConfiguredScript;
  movePackage: MovePackage;
  onCommandOutput?: (output: CommandOutput) => void;
  packageTree: PackageTree;
  startedAt: Date;
  streamId?: number | string;
  step: ExecutionStep;
}): Promise<StepExecutionOutcome> {
  const definition = definitionByKind[step.kind];
  const output = await runSecurityScript(packageTree, movePackage.path, configuredScript.scriptPath, {
    args: configuredScript.args,
    onOutput: onCommandOutput,
    streamId,
  });
  const finishedAt = new Date();

  return {
    detail:
      output.status === 0
        ? `${configuredScript.modeLabel} completed in ${displayMovePackageName(movePackage.name)}.`
        : `${configuredScript.modeLabel} exited with a non-zero status in ${displayMovePackageName(movePackage.name)}.`,
    finishedAt,
    output,
    startedAt,
    state: output.status === 0 ? "success" : "error",
    summary:
      output.status === 0
        ? `${definition.label} project script passed.`
        : `${definition.label} project script failed.`,
  };
}

function publishCommandKind(step: ExecutionStep): SecurityCommandKind {
  const target = step.config.publishTarget ?? "localnet";

  if (step.config.publishDryRun === false) {
    return `publish-${target}` as SecurityCommandKind;
  }

  return `publish-dry-run-${target}` as SecurityCommandKind;
}

function publishTargetLabel(target: PublishTarget) {
  switch (target) {
    case "localnet":
      return "Local";
    case "devnet":
      return "Devnet";
    case "testnet":
      return "Testnet";
    case "mainnet":
      return "Mainnet";
  }
}

function markRemainingStepsSkipped(
  sequence: ExecutionStep[],
  startIndex: number,
  setStepResults: React.Dispatch<React.SetStateAction<Record<string, StepExecutionResult>>>,
) {
  setStepResults((current) => {
    const next = { ...current };

    for (const step of sequence.slice(startIndex)) {
      next[step.id] = {
        ...emptyStepResult(),
        finishedAt: new Date(),
        state: "skipped",
        summary: "Skipped because an earlier required step failed.",
      };
    }

    return next;
  });
}

function createInitialSequence(): ExecutionStep[] {
  return [
    {
      id: "build",
      kind: "build",
      config: {
        scriptPath: "",
        stopOnFailure: true,
        useScript: false,
      },
    },
  ];
}

function createStep(kind: ExecutionStepKind): ExecutionStep {
  const definition = definitionByKind[kind];

  return {
    id: `${kind}-${Date.now()}-${stepIdCounter += 1}`,
    kind,
    config: {
      publishDryRun: true,
      publishTarget: "localnet",
      scriptPath: "",
      stopOnFailure: definition.defaultStopOnFailure,
      useScript: false,
    },
    locked: definition.locked,
  };
}

function emptyStepResult(): StepExecutionResult {
  return {
    detail: null,
    finishedAt: null,
    output: null,
    startedAt: null,
    state: "idle",
    summary: null,
  };
}

function resolveActiveMovePackage(
  packageTree: PackageTree,
  activeMovePackage: MovePackage,
) {
  return (
    packageTree.movePackages.find(
      (candidate) => candidate.manifestPath === activeMovePackage.manifestPath,
    ) ??
    packageTree.movePackages.find(
      (candidate) => candidate.name === activeMovePackage.name && candidate.path === activeMovePackage.path,
    ) ??
    packageTree.movePackages[0] ??
    null
  );
}

function writeDragPayload(event: React.DragEvent<HTMLElement>, payload: DragPayload) {
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.dropEffect = "move";
  event.dataTransfer.setData(DRAG_MIME, JSON.stringify(payload));
  event.dataTransfer.setData("text/plain", `${DRAG_TEXT_PREFIX}${JSON.stringify(payload)}`);
}

function readDragPayload(event: React.DragEvent<HTMLElement>): DragPayload | null {
  try {
    const customPayload = event.dataTransfer.getData(DRAG_MIME);
    const textPayload = event.dataTransfer.getData("text/plain");
    const rawPayload = customPayload || (
      textPayload.startsWith(DRAG_TEXT_PREFIX)
        ? textPayload.slice(DRAG_TEXT_PREFIX.length)
        : ""
    );

    if (!rawPayload) {
      return null;
    }

    const payload = JSON.parse(rawPayload) as DragPayload;

    if (isValidDragPayload(payload)) {
      return payload;
    }
  } catch {
    return null;
  }

  return null;
}

function isValidDragPayload(payload: unknown): payload is DragPayload {
  if (!payload || typeof payload !== "object" || !("source" in payload)) {
    return false;
  }

  if (payload.source === "palette") {
    return (
      "kind" in payload &&
      typeof payload.kind === "string" &&
      payload.kind in definitionByKind
    );
  }

  return payload.source === "sequence" && "stepId" in payload && typeof payload.stepId === "string";
}

function stepStateLabel(state: ExecutionStepState) {
  switch (state) {
    case "idle":
      return "Ready";
    case "running":
      return "Running";
    case "success":
      return "Passed";
    case "attention":
      return "Review";
    case "error":
      return "Failed";
    case "skipped":
      return "Skipped";
  }
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Unknown execution error.";
}
