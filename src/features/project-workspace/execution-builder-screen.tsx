import {
  Background,
  BaseEdge,
  Controls,
  Handle,
  MarkerType,
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
  CheckCircle2,
  Circle,
  FileCheck2,
  FolderOpen,
  Gauge,
  GripVertical,
  Hammer,
  Loader2,
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
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  buildMovePackage,
  loadFilePreview,
  loadPackageTree,
  runSecurityScript,
  runSecurityCommand,
  type CommandOutput,
  type FilePreview,
  type MovePackage,
  type PackageTree,
  type SecurityCommandKind,
} from "@/features/empty-project/filesystem-tree";
import type { BuildLogRun } from "@/features/project-workspace/build-log-sheet";
import { cn } from "@/lib/utils";

type ExecutionBuilderScreenProps = {
  activeMovePackage: MovePackage | null;
  onCommandLog: (run: BuildLogRun) => void;
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
const EXECUTION_NODE_WIDTH = 360;
const EXECUTION_NODE_HEIGHT = 88;
const EXECUTION_HORIZONTAL_GAP = 92;
const EXECUTION_VERTICAL_GAP = 74;
const EXECUTION_FLOW_START_X = 120;
const EXECUTION_FLOW_START_Y = 160;
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

const stepDefinitions: ExecutionStepDefinition[] = [
  {
    kind: "build",
    label: "Build",
    shortLabel: "Build",
    description:
      "Runs `sui move build` first so Peregrine gets fresh package_summaries before any other block depends on them.",
    whenToUse:
      "Always first. Package summaries are generated by the Sui build, and later checks depend on those summaries.",
    category: "Build",
    command: "sui move build",
    defaultStopOnFailure: true,
    icon: Hammer,
    locked: true,
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
      "Checks the active package for formal-spec coverage signals such as `spec`, `ensures`, `aborts_if`, and invariants.",
    whenToUse:
      "Use this before publish to confirm formal verification work exists and to see where specs are concentrated.",
    category: "Verify",
    command: "Local formal spec scan",
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
  const activeDragPayloadRef = React.useRef<DragPayload | null>(null);
  const isRunning = run?.state === "running";

  React.useEffect(() => {
    if (!selectedStepId || sequence.some((step) => step.id === selectedStepId)) {
      return;
    }

    setSelectedStepId(sequence[0]?.id ?? "build");
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

      const insertionIndex = Math.max(1, Math.min(targetIndex ?? current.length, current.length));
      const nextStep = createStep(kind);
      const next = [...current];

      next.splice(insertionIndex, 0, nextStep);
      setSelectedStepId(nextStep.id);

      return next;
    });
  }, []);

  const removeStep = React.useCallback((stepId: string) => {
    setSequence((current) => current.filter((step) => step.locked || step.id !== stepId));
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
      next.splice(Math.max(1, adjustedTargetIndex), 0, sourceStep);

      return next;
    });
  }, []);

  const moveStep = React.useCallback(
    (stepId: string, direction: -1 | 1) => {
      setSequence((current) => {
        const sourceIndex = current.findIndex((step) => step.id === stepId);

        if (sourceIndex < 1) {
          return current;
        }

        const targetIndex = sourceIndex + direction;

        if (targetIndex < 1 || targetIndex >= current.length) {
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

  const handleRunSequence = React.useCallback(async () => {
    if (!activeMovePackage || isRunning) {
      return;
    }

    const stepsToRun = sequence;
    const startedAt = new Date();
    let currentTree = packageTree;
    let currentPackage = resolveActiveMovePackage(currentTree, activeMovePackage);
    let hasAttention = false;

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
        startedAt: stepStartedAt,
        state: "running",
        step,
      });

      onCommandLog(runningLog);
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
        updateStepResult(step.id, outcome);
        onCommandLog(
          executionLogRun({
            detail: outcome.detail,
            finishedAt: outcome.finishedAt ?? new Date(),
            movePackage: currentPackage ?? packageForLog,
            output: outcome.output,
            packageTree: currentTree,
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
            state: hasAttention ? "attention" : "success",
          }
        : current,
    );
  }, [
    activeMovePackage,
    isRunning,
    onCommandLog,
    onProjectSelected,
    packageTree,
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
          disabled={!activeMovePackage || isRunning}
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

      <div className="grid h-full min-h-0 grid-cols-[280px_minmax(0,1fr)]">
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
          onSelectStep={setSelectedStepId}
          onUpdateStep={updateStep}
          packageTree={packageTree}
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

  return (
    <aside className="grid min-h-0 border-r border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <ScrollArea className="min-h-0">
        <div className="grid gap-2 p-3">
          {paletteSteps.map((definition) => {
            const isAdded = activeKinds.has(definition.kind);
            const Icon = definition.icon;

            return (
              <Card
                className={cn(
                  "group gap-0 rounded-md p-3 shadow-none transition",
                  isAdded
                    ? "border-[color:var(--app-border)] opacity-60"
                    : "cursor-grab hover:border-primary/45 hover:bg-[var(--app-subtle)]",
                )}
                draggable={!isRunning && !isAdded}
                key={definition.kind}
                onDragEnd={onDragEnd}
                onDragStart={(event) => {
                  onDragStart(event, {
                    kind: definition.kind,
                    source: "palette",
                  });
                }}
              >
                <div className="flex min-w-0 items-start gap-3">
                  <span className="mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-md bg-[var(--app-elevated)] text-muted-foreground group-hover:text-primary">
                    <Icon className="size-4" aria-hidden="true" />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex min-w-0 items-center justify-between gap-2">
                      <h3 className="truncate text-sm font-semibold">
                        {definition.shortLabel}
                      </h3>
                      <Badge className="rounded px-1.5 py-0 text-[10px]" variant="secondary">
                        {definition.category}
                      </Badge>
                    </div>
                    <p className="mt-1 line-clamp-2 text-xs leading-5 text-muted-foreground">
                      {definition.description}
                    </p>
                    <Button
                      className="mt-3 h-7 w-full gap-1.5 text-xs"
                      disabled={isRunning || isAdded}
                      onClick={() => onAddStep(definition.kind)}
                      type="button"
                      variant="outline"
                    >
                      <Plus className="size-3.5" aria-hidden="true" />
                      {isAdded ? "Added" : "Add step"}
                    </Button>
                  </div>
                </div>
              </Card>
            );
          })}
        </div>
      </ScrollArea>
    </aside>
  );
}

type SequenceCanvasProps = {
  activeMovePackage: MovePackage | null;
  direction: SequenceDirection;
  dropIndex: number | null;
  isRunning: boolean;
  onDropIndex: (event: React.DragEvent<HTMLElement>, index: number) => void;
  onDropPreview: (index: number | null) => void;
  onMoveStep: (stepId: string, direction: -1 | 1) => void;
  onRemoveStep: (stepId: string) => void;
  onReorderStep: (stepId: string, targetIndex: number) => void;
  onSelectStep: (stepId: string) => void;
  onUpdateStep: (stepId: string, nextStep: ExecutionStepUpdate) => void;
  packageTree: PackageTree;
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
  onDropIndex,
  onDropPreview,
  onMoveStep,
  onRemoveStep,
  onReorderStep,
  onSelectStep,
  onUpdateStep,
  packageTree,
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
  }, [direction, fitView, sequence.length]);

  const displayedNodes = React.useMemo<ExecutionCanvasNode[]>(() => {
    if (dropIndex === null) {
      return flowNodes;
    }

    return [
      ...flowNodes,
      createDropTargetNode(clampInsertionIndex(dropIndex, sequence.length), direction),
    ];
  }, [direction, dropIndex, flowNodes, sequence.length]);

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
        ),
      );
    },
    [direction, isRunning, onDropPreview, screenToFlowPosition, sequence],
  );

  const handlePaneDrop = React.useCallback(
    (event: React.DragEvent<HTMLElement>) => {
      const payload = readDragPayload(event) ?? currentExecutionDragPayload;

      if (!payload || isRunning) {
        return;
      }

      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      const targetIndex =
        dropIndex ?? insertionIndexForFlowPosition(position, sequence, direction);

      onDropIndex(event, targetIndex);
    },
    [direction, dropIndex, isRunning, onDropIndex, screenToFlowPosition, sequence],
  );

  const handleNodeDrag = React.useCallback(
    (_event: React.MouseEvent, node: ExecutionCanvasNode) => {
      if (isRunning || node.id === "build" || node.id === EXECUTION_DROP_TARGET_ID) {
        return;
      }

      onDropPreview(insertionIndexForFlowPosition(node.position, sequence, direction));
    },
    [direction, isRunning, onDropPreview, sequence],
  );

  const handleNodeDragStop = React.useCallback(
    (_event: React.MouseEvent, node: ExecutionCanvasNode) => {
      if (isRunning || node.id === "build" || node.id === EXECUTION_DROP_TARGET_ID) {
        return;
      }

      onSelectStep(node.id);
      onReorderStep(
        node.id,
        dropIndex ?? insertionIndexForFlowPosition(node.position, sequence, direction),
      );
      onDropPreview(null);
    },
    [direction, dropIndex, isRunning, onDropPreview, onReorderStep, sequence],
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
  );
}

function ExecutionWorkflowNode({ data }: NodeProps<Node<ExecutionWorkflowNodeData>>) {
  const Icon = data.definition.icon;
  const state = data.result?.state ?? "idle";
  const shouldShowState = state !== "idle";
  const isHorizontal = data.direction === "horizontal";
  const PreviousIcon = isHorizontal ? ArrowLeft : ArrowUp;
  const NextIcon = isHorizontal ? ArrowRight : ArrowDown;
  const sourcePosition = isHorizontal ? Position.Right : Position.Bottom;
  const targetPosition = isHorizontal ? Position.Left : Position.Top;
  const usesScript = data.step.config.useScript === true;
  const commandLabel = commandPreview(data.definition, data.step);

  return (
    <article
      className={cn(
        "group relative grid min-h-[72px] w-[360px] min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2.5 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2.5 py-2 shadow-[0_14px_30px_rgba(0,0,0,0.22)] transition",
        data.step.kind === "publish" && !usesScript && "min-h-[100px] w-[420px] items-start",
        state === "running" && "execution-step-active border-primary/45",
        data.selected && "border-primary/55 bg-[var(--app-elevated)] shadow-lg",
        !data.step.locked && !data.isRunning && "cursor-grab active:cursor-grabbing",
      )}
      title={data.step.locked ? undefined : "Drag to reorder"}
    >
      <Handle
        className="!size-1 !border-0 !bg-transparent !opacity-0"
        id="target"
        isConnectable={false}
        position={targetPosition}
        type="target"
      />
      <Handle
        className="!size-1 !border-0 !bg-transparent !opacity-0"
        id="source"
        isConnectable={false}
        position={sourcePosition}
        type="source"
      />

      <div className="flex items-center gap-1.5">
        <span className="grid size-8 shrink-0 place-items-center rounded-md bg-[var(--app-subtle)] text-xs font-semibold text-muted-foreground">
          {data.index + 1}
        </span>
        <span
          className={cn(
            "grid size-8 shrink-0 place-items-center rounded-md bg-[var(--app-elevated)] text-muted-foreground shadow-sm",
            state === "running" && "text-primary",
            state === "success" && "text-emerald-400",
            state === "attention" && "text-amber-400",
            state === "error" && "text-red-400",
          )}
        >
          <Icon className="size-4" aria-hidden="true" />
        </span>
      </div>

      <div className="grid min-w-0 gap-1 self-center">
        <div className="flex min-w-0 items-center gap-2 leading-none">
          <h3 className="min-w-0 truncate text-sm font-semibold leading-4">
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
          <p className="truncate font-mono text-[10.5px] leading-4 text-muted-foreground">
            {data.result?.summary ?? commandLabel}
          </p>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-0.5 self-center">
        <Button
          aria-label={isHorizontal ? "Move step left" : "Move step up"}
          className="nodrag nopan size-5.5 rounded text-muted-foreground hover:text-foreground"
          disabled={data.isRunning || data.step.locked || data.index <= 1}
          onClick={() => data.onMoveStep(data.step.id, -1)}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <PreviousIcon className="size-3.5" aria-hidden="true" />
        </Button>
        <Button
          aria-label={isHorizontal ? "Move step right" : "Move step down"}
          className="nodrag nopan size-5.5 rounded text-muted-foreground hover:text-foreground"
          disabled={data.isRunning || data.step.locked || data.index >= data.sequenceLength - 1}
          onClick={() => data.onMoveStep(data.step.id, 1)}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <NextIcon className="size-3.5" aria-hidden="true" />
        </Button>
        <Button
          aria-label={usesScript ? "Use default command" : "Use bash script"}
          aria-pressed={usesScript}
          className={cn(
            "nodrag nopan size-5.5 rounded text-muted-foreground hover:text-foreground",
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
          size="icon-xs"
          title={usesScript ? "Use default command" : "Use bash script"}
          type="button"
          variant="ghost"
        >
          <Terminal className="size-3.5" aria-hidden="true" />
        </Button>
        <Button
          aria-label="Remove step"
          className="nodrag nopan size-5.5 rounded text-muted-foreground hover:text-destructive"
          disabled={data.isRunning || data.step.locked}
          onClick={() => data.onRemoveStep(data.step.id)}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <Trash2 className="size-3.5" aria-hidden="true" />
        </Button>
        <span
          className={cn(
            "grid size-5.5 place-items-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground",
            data.step.locked || data.isRunning ? "opacity-30" : "cursor-grab active:cursor-grabbing",
          )}
          title={data.step.locked ? "Build stays first" : "Drag to reorder"}
        >
          <GripVertical className="size-3.5" aria-hidden="true" />
        </span>
      </div>

      {data.step.kind === "publish" && !usesScript ? (
        <PublishConfig
          disabled={data.isRunning}
          step={data.step}
          onUpdateStep={data.onUpdateStep}
        />
      ) : null}
    </article>
  );
}

function ExecutionDropTargetNode({ data }: NodeProps<Node<ExecutionDropNodeData>>) {
  const isHorizontal = data.direction === "horizontal";

  return (
    <div
      className={cn(
        "pointer-events-none grid place-items-center rounded-md border border-dashed border-primary/70 bg-primary/10 text-xs font-medium text-primary shadow-sm",
        isHorizontal ? "h-[76px] w-14" : "h-12 w-[320px]",
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
  markerEnd,
  sourceX,
  sourceY,
  style,
  targetX,
  targetY,
}: EdgeProps<Edge<ExecutionSequenceEdgeData>>) {
  const path =
    data?.direction === "vertical"
      ? `M ${sourceX},${sourceY} L ${sourceX},${targetY}`
      : `M ${sourceX},${sourceY} L ${targetX},${sourceY}`;

  return <BaseEdge markerEnd={markerEnd} path={path} style={style} />;
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
      result: stepResults[step.id],
      selected: selectedStepId === step.id,
      sequenceLength: sequence.length,
      step,
    },
    draggable: !step.locked && !isRunning,
    focusable: false,
    position: executionNodePosition(index, direction),
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
      markerEnd: {
        color,
        type: MarkerType.ArrowClosed,
      },
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
): ExecutionDropNode {
  return {
    id: EXECUTION_DROP_TARGET_ID,
    type: "dropTarget",
    data: { direction },
    draggable: false,
    focusable: false,
    position: dropTargetPosition(dropIndex, direction),
    selectable: false,
    zIndex: 100,
  };
}

function insertionIndexForFlowPosition(
  position: { x: number; y: number },
  sequence: ExecutionStep[],
  direction: SequenceDirection,
) {
  const coordinate = direction === "horizontal" ? position.x : position.y;

  for (let index = 1; index < sequence.length; index += 1) {
    const nodePosition = executionNodePosition(index, direction);
    const midpoint =
      direction === "horizontal"
        ? nodePosition.x + EXECUTION_NODE_WIDTH / 2
        : nodePosition.y + EXECUTION_NODE_HEIGHT / 2;

    if (coordinate < midpoint) {
      return index;
    }
  }

  return sequence.length;
}

function executionNodePosition(index: number, direction: SequenceDirection) {
  if (direction === "horizontal") {
    return {
      x: EXECUTION_FLOW_START_X + index * (EXECUTION_NODE_WIDTH + EXECUTION_HORIZONTAL_GAP),
      y: EXECUTION_FLOW_START_Y,
    };
  }

  return {
    x: EXECUTION_FLOW_START_X,
    y: EXECUTION_FLOW_START_Y + index * (EXECUTION_NODE_HEIGHT + EXECUTION_VERTICAL_GAP),
  };
}

function dropTargetPosition(index: number, direction: SequenceDirection) {
  const insertionIndex = Math.max(1, index);

  if (direction === "horizontal") {
    return {
      x:
        EXECUTION_FLOW_START_X +
        insertionIndex * (EXECUTION_NODE_WIDTH + EXECUTION_HORIZONTAL_GAP) -
        EXECUTION_HORIZONTAL_GAP / 2 -
        32,
      y: EXECUTION_FLOW_START_Y - 9,
    };
  }

  return {
    x: EXECUTION_FLOW_START_X,
    y:
      EXECUTION_FLOW_START_Y +
      insertionIndex * (EXECUTION_NODE_HEIGHT + EXECUTION_VERTICAL_GAP) -
      EXECUTION_VERTICAL_GAP / 2 -
      28,
  };
}

function clampInsertionIndex(index: number, sequenceLength: number) {
  return Math.max(1, Math.min(index, sequenceLength));
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

function commandPreview(definition: ExecutionStepDefinition, step: ExecutionStep) {
  if (step.config.useScript) {
    return normalizedScriptPath(step) || "Bash script";
  }

  if (step.kind === "publish") {
    const target = step.config.publishTarget ?? "localnet";
    const prefix = step.config.publishDryRun === false ? "sui client publish" : "sui client publish --dry-run";
    const pubfile = step.config.publishDryRun === false ? "" : " --pubfile-path <temporary>";

    return `${prefix} --client.env ${target}${pubfile} .`;
  }

  return definition.command;
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

function executionLogRun({
  detail,
  error = null,
  finishedAt = null,
  movePackage,
  output = null,
  packageTree,
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
  startedAt: Date;
  state: BuildLogRun["state"];
  step: ExecutionStep;
  summary?: string | null;
}): BuildLogRun {
  const definition = definitionByKind[step.kind];
  const scriptPath = normalizedScriptPath(step);
  const command = executionLogCommand(definition, step);

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
      { label: "Mode", value: scriptPath ? "Bash script" : "Default" },
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

function executionLogCommand(definition: ExecutionStepDefinition, step: ExecutionStep) {
  const scriptPath = normalizedScriptPath(step);

  if (step.kind === "build" && scriptPath) {
    return `sui move build && bash ${scriptPath}`;
  }

  if (scriptPath) {
    return `bash ${scriptPath}`;
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
        "shrink-0 gap-1 rounded px-1.5 py-0 text-[10px]",
        compact && "max-w-[5rem]",
        state === "idle" && "bg-muted text-muted-foreground",
        state === "running" && "bg-muted text-muted-foreground",
        state === "success" && "bg-emerald-500/15 text-emerald-400",
        state === "attention" && "bg-amber-500/15 text-amber-400",
        state === "error" && "bg-red-500/15 text-red-400",
        state === "skipped" && "bg-muted text-muted-foreground",
      )}
      variant="secondary"
    >
      {state === "idle" ? <Circle className="size-2.5" /> : null}
      {state === "running" ? <Loader2 className="size-2.5 animate-spin" /> : null}
      {state === "success" ? <CheckCircle2 className="size-2.5" /> : null}
      {state === "attention" ? <AlertTriangle className="size-2.5" /> : null}
      {state === "error" ? <XCircle className="size-2.5" /> : null}
      <span className="truncate">{stepStateLabel(state)}</span>
    </Badge>
  );
}

async function executeStep({
  movePackage,
  onCommandOutput,
  onProjectSelected,
  packageTree,
  startedAt,
  streamId,
  step,
}: {
  movePackage: MovePackage;
  onCommandOutput?: (output: CommandOutput) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  packageTree: PackageTree;
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

    if (step.config.useScript) {
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

      const buildRefreshState: ExecutionStepState = nextPackageTree.dependencyGraph.summaryPath
        ? "success"
        : "attention";

      return {
        ...scriptOutcome,
        detail: [
          nextPackageTree.dependencyGraph.summaryPath
            ? `Summary directory: ${nextPackageTree.dependencyGraph.summaryPath}`
            : "Build completed, but Peregrine did not find package_summaries after rescanning.",
          scriptOutcome.detail,
        ]
          .filter(Boolean)
          .join("\n"),
        packageTree: nextPackageTree,
        state:
          scriptOutcome.state === "success" && buildRefreshState === "attention"
            ? "attention"
            : scriptOutcome.state,
        summary:
          scriptOutcome.state === "success" && buildRefreshState === "attention"
            ? "Bash script passed, but no package_summaries directory was found."
            : scriptOutcome.summary,
      };
    }

    return {
      detail: nextPackageTree.dependencyGraph.summaryPath
        ? `Summary directory: ${nextPackageTree.dependencyGraph.summaryPath}`
        : "Build completed, but Peregrine did not find package_summaries after rescanning.",
      finishedAt,
      output,
      packageTree: nextPackageTree,
      startedAt,
      state: nextPackageTree.dependencyGraph.summaryPath ? "success" : "attention",
      summary: nextPackageTree.dependencyGraph.summaryPath
        ? "Build succeeded and package summaries were refreshed."
        : "Build succeeded, but no package_summaries directory was found.",
    };
  }

  if (step.config.useScript) {
    return executeScriptStep({
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
    ...(await analyzeStep(step.kind, packageTree, movePackage)),
    finishedAt: new Date(),
    output: null,
    startedAt,
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
        ? `Bash script completed in ${movePackage.name}.`
        : `Bash script exited with a non-zero status in ${movePackage.name}.`,
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

async function analyzeStep(
  kind: ExecutionStepKind,
  packageTree: PackageTree,
  movePackage: MovePackage,
): Promise<Pick<StepExecutionOutcome, "detail" | "state" | "summary">> {
  switch (kind) {
    case "formal":
      return analyzeFormalVerification(packageTree, movePackage);
    case "build":
    case "coverage":
    case "test":
    case "fuzz":
    case "publish":
      return {
        detail: null,
        state: "error",
        summary: "Unsupported local analysis block.",
      };
  }
}

async function analyzeFormalVerification(
  packageTree: PackageTree,
  movePackage: MovePackage,
): Promise<Pick<StepExecutionOutcome, "detail" | "state" | "summary">> {
  const previews = await Promise.all(
    movePackage.modules.map((moveModule) =>
      loadFilePreview(packageTree, moveModule.filePath).catch(() => null),
    ),
  );
  const sourceFiles = previews.filter(isTextPreview);
  let specBlocks = 0;
  let conditions = 0;
  let invariants = 0;
  const filesWithSpecs: string[] = [];

  for (const preview of sourceFiles) {
    const source = preview.source;
    const nextSpecBlocks = countMatches(source, /\bspec\b/g);
    const nextConditions = countMatches(source, /\b(aborts_if|ensures|requires)\b/g);
    const nextInvariants = countMatches(source, /\binvariant\b/g);

    if (nextSpecBlocks || nextConditions || nextInvariants) {
      filesWithSpecs.push(
        `${preview.path}: ${nextSpecBlocks} spec blocks, ${nextConditions} conditions, ${nextInvariants} invariants`,
      );
    }

    specBlocks += nextSpecBlocks;
    conditions += nextConditions;
    invariants += nextInvariants;
  }

  if (!sourceFiles.length) {
    return {
      detail: "No readable Move source files were found for the active package.",
      state: "attention",
      summary: "Formal verification scan could not read package sources.",
    };
  }

  if (!specBlocks && !conditions && !invariants) {
    return {
      detail:
        "No `spec`, `ensures`, `requires`, `aborts_if`, or `invariant` declarations were found.",
      state: "attention",
      summary: "No formal verification specs found.",
    };
  }

  return {
    detail: filesWithSpecs.join("\n"),
    state: "success",
    summary: `${specBlocks} specs, ${conditions} conditions, and ${invariants} invariants found.`,
  };
}

function isTextPreview(preview: FilePreview | null): preview is Extract<FilePreview, { kind: "text" }> {
  return preview?.kind === "text";
}

function countMatches(source: string, pattern: RegExp) {
  return source.match(pattern)?.length ?? 0;
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
      locked: true,
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
