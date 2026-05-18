import React from "react";
import { Boxes, Network, Workflow } from "lucide-react";

import type {
  MoveCallGraph,
  MoveCallGraphNode,
  MovePackage,
  MoveProjectGraphs,
  MoveTypeGraphNode,
  MoveTypeGraph,
  PackageDependencyGraph,
} from "@/features/empty-project/filesystem-tree";
import type { TypeGraphSourceLocation } from "@/features/project-workspace/type-graph-view";
import {
  displayMovePackageName,
  loadMoveGraphs,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

const DependencyGraphView = React.lazy(() =>
  import("@/features/project-workspace/dependency-graph-view"),
);
const CallGraphView = React.lazy(() =>
  import("@/features/project-workspace/call-graph-view"),
);
const TypeGraphView = React.lazy(() =>
  import("@/features/project-workspace/type-graph-view"),
);
const TypeGraphPanel = React.lazy(() =>
  import("@/features/project-workspace/type-graph-panel").then((module) => ({
    default: module.TypeGraphPanel,
  })),
);
const CollapsedTypeGraphPanel = React.lazy(() =>
  import("@/features/project-workspace/type-graph-panel").then((module) => ({
    default: module.CollapsedTypeGraphPanel,
  })),
);

type OverviewGraphMode = "calls" | "dependencies" | "types";

type DependencyGraphScreenProps = {
  activeMovePackage: MovePackage | null;
  callGraph: MoveCallGraph;
  graph: PackageDependencyGraph;
  isDependencyGraphLoading?: boolean;
  onMoveGraphsLoaded?: (graphs: MoveProjectGraphs) => void;
  onOpenSourceLocation?: (location: TypeGraphSourceLocation) => void;
  packageName: string;
  rootPath?: string;
  typeGraph: MoveTypeGraph;
};

export function DependencyGraphScreen({
  activeMovePackage,
  callGraph,
  graph,
  isDependencyGraphLoading = false,
  onMoveGraphsLoaded,
  onOpenSourceLocation,
  packageName,
  rootPath,
  typeGraph,
}: DependencyGraphScreenProps) {
  const [graphMode, setGraphMode] = React.useState<OverviewGraphMode>("dependencies");
  const [isTypePanelOpen, setIsTypePanelOpen] = React.useState(true);
  const [selectedTypeId, setSelectedTypeId] = React.useState<string | null>(null);
  const [loadedMoveGraphs, setLoadedMoveGraphs] = React.useState<MoveProjectGraphs | null>(null);
  const [isLoadingMoveGraphs, setIsLoadingMoveGraphs] = React.useState(false);
  const [moveGraphError, setMoveGraphError] = React.useState<string | null>(null);
  const activePackagePath = activeMovePackage?.path ?? null;
  const activeTypeGraph = loadedMoveGraphs?.typeGraph ?? typeGraph;
  const activeCallGraph = loadedMoveGraphs?.callGraph ?? callGraph;
  const typeGraphReady =
    hasTypeGraphPayload(typeGraph, activeMovePackage) || loadedMoveGraphs !== null;
  const callGraphReady =
    hasCallGraphPayload(callGraph, activeMovePackage) || loadedMoveGraphs !== null;
  const firstTypeId = React.useMemo(
    () => (graphMode === "types" && typeGraphReady
      ? firstSelectableTypeId(activeTypeGraph.nodes, activeMovePackage)
      : null),
    [activeMovePackage, activeTypeGraph.nodes, graphMode, typeGraphReady],
  );
  const typeCount = React.useMemo(() => {
    if (typeGraphReady) {
      return selectableTypeCount(activeTypeGraph.nodes, activeMovePackage);
    }

    return packageTypeCount(activeMovePackage);
  }, [activeMovePackage, activeTypeGraph.nodes, typeGraphReady]);
  const callCount = React.useMemo(() => {
    if (callGraphReady) {
      return selectableCallFunctionCount(activeCallGraph.nodes, activeMovePackage);
    }

    return packageFunctionCount(activeMovePackage);
  }, [activeCallGraph.nodes, activeMovePackage, callGraphReady]);
  const navigatorTypeCount = React.useMemo(
    () => (typeGraphReady ? typeNavigatorCount(activeTypeGraph.nodes) : typeCount),
    [activeTypeGraph.nodes, typeCount, typeGraphReady],
  );

  React.useEffect(() => {
    setLoadedMoveGraphs(null);
    setMoveGraphError(null);
    setSelectedTypeId(null);
  }, [activePackagePath, rootPath, typeGraph]);

  const ensureMoveGraphs = React.useCallback(async () => {
    if (loadedMoveGraphs || isLoadingMoveGraphs || !rootPath) {
      return;
    }

    setIsLoadingMoveGraphs(true);
    setMoveGraphError(null);

    try {
      const graphs = await loadMoveGraphs(
        rootPath,
        activeMovePackage ? activeMovePackage.path : undefined,
      );

      setLoadedMoveGraphs(graphs);
      onMoveGraphsLoaded?.(graphs);
    } catch (error) {
      setMoveGraphError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingMoveGraphs(false);
    }
  }, [activeMovePackage, isLoadingMoveGraphs, loadedMoveGraphs, onMoveGraphsLoaded, rootPath]);

  React.useEffect(() => {
    if (
      (graphMode === "types" && !typeGraphReady)
      || (graphMode === "calls" && !callGraphReady)
    ) {
      void ensureMoveGraphs();
    }
  }, [callGraphReady, ensureMoveGraphs, graphMode, typeGraphReady]);

  const selectedTypeExists = React.useCallback(
    (typeId: string) => activeTypeGraph.nodes.some((node) => node.id === typeId),
    [activeTypeGraph.nodes],
  );

  React.useEffect(() => {
    if (graphMode !== "types" || !typeGraphReady) {
      return;
    }

    const selectedStillExists = selectedTypeId ? selectedTypeExists(selectedTypeId) : false;

    if (!selectedStillExists) {
      setSelectedTypeId(firstTypeId);
    }
  }, [firstTypeId, graphMode, selectedTypeExists, selectedTypeId, typeGraphReady]);

  const selectGraphMode = React.useCallback(
    (nextMode: OverviewGraphMode) => {
      setGraphMode(nextMode);

      if (nextMode === "types" && typeGraphReady) {
        setSelectedTypeId((current) =>
          current && selectedTypeExists(current) ? current : firstTypeId,
        );
      }
    },
    [firstTypeId, selectedTypeExists, typeGraphReady],
  );

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <div className="row-start-1 flex min-w-0 justify-start px-5 pb-2 pt-2">
        <OverviewGraphModeSwitch
          callCount={callCount}
          mode={graphMode}
          onModeChange={selectGraphMode}
          typeCount={typeCount}
        />
      </div>

      <div className="row-start-2 min-h-0 px-5 pb-3">
        {graphMode === "types" && !typeGraphReady ? (
          <DeferredGraphState
            description={`Mapping storage, authority, generics, and external type links for ${displayMovePackageName(activeMovePackage?.name ?? packageName)}.`}
            error={moveGraphError ?? (!rootPath ? "Move graph loading is unavailable in this view." : null)}
            graphLabel="Type Graph"
            isLoading={isLoadingMoveGraphs}
            onRetry={() => void ensureMoveGraphs()}
          />
        ) : graphMode === "calls" && !callGraphReady ? (
          <DeferredGraphState
            description={`Resolving package entrypoints, internal calls, external calls, and unresolved targets for ${displayMovePackageName(activeMovePackage?.name ?? packageName)}.`}
            error={moveGraphError ?? (!rootPath ? "Move graph loading is unavailable in this view." : null)}
            graphLabel="Call Graph"
            isLoading={isLoadingMoveGraphs}
            onRetry={() => void ensureMoveGraphs()}
          />
        ) : graphMode === "types" ? (
          <div
            className={cn(
              "grid h-full min-h-0 animate-in gap-3 fade-in slide-in-from-right-3 duration-200",
              isTypePanelOpen
                ? "grid-rows-[minmax(12rem,32vh)_minmax(0,1fr)] lg:grid-cols-[clamp(250px,24vw,340px)_minmax(0,1fr)] lg:grid-rows-1"
                : "grid-rows-[2.75rem_minmax(0,1fr)] lg:grid-cols-[2.75rem_minmax(0,1fr)] lg:grid-rows-1",
            )}
          >
            {isTypePanelOpen ? (
              <React.Suspense fallback={<GraphLoadingState label="Loading type navigator..." />}>
                <TypeGraphPanel
                  className="min-h-0"
                  movePackage={activeMovePackage}
                  onCollapse={() => setIsTypePanelOpen(false)}
                  onSelectedTypeIdChange={setSelectedTypeId}
                  packageName={packageName}
                  selectedTypeId={selectedTypeId}
                  typeGraph={activeTypeGraph}
                />
              </React.Suspense>
            ) : (
              <React.Suspense fallback={<GraphLoadingState label="Loading navigator..." />}>
                <CollapsedTypeGraphPanel
                  className="min-h-0"
                  onExpand={() => setIsTypePanelOpen(true)}
                  packageName={activeMovePackage?.name ?? packageName}
                  typeCount={navigatorTypeCount}
                />
              </React.Suspense>
            )}
            <React.Suspense fallback={<GraphLoadingState label="Loading type graph..." />}>
              <TypeGraphView
                className="h-full min-h-0"
                movePackage={activeMovePackage}
                onOpenSourceLocation={onOpenSourceLocation}
                onSelectType={setSelectedTypeId}
                packageName={packageName}
                selectedTypeId={selectedTypeId}
                typeGraph={activeTypeGraph}
              />
            </React.Suspense>
          </div>
        ) : graphMode === "calls" ? (
          <div className="h-full min-h-0 animate-in fade-in slide-in-from-right-2 duration-150">
            <React.Suspense fallback={<GraphLoadingState label="Loading call graph..." />}>
              <CallGraphView
                className="h-full rounded-md border"
                graph={activeCallGraph}
                movePackage={activeMovePackage}
                onOpenSourceLocation={onOpenSourceLocation}
                packageName={packageName}
              />
            </React.Suspense>
          </div>
        ) : (
          <div className="h-full min-h-0 animate-in fade-in slide-in-from-left-2 duration-150">
            <React.Suspense fallback={<GraphLoadingState />}>
              {isDependencyGraphLoading && !graph.summaryPath ? (
                <GraphLoadingState label="Loading dependency graph..." />
              ) : (
                <DependencyGraphView
                  className="h-full rounded-md border"
                  graph={graph}
                  packageName={packageName}
                />
              )}
            </React.Suspense>
          </div>
        )}
      </div>
    </section>
  );
}

function OverviewGraphModeSwitch({
  callCount,
  mode,
  onModeChange,
  typeCount,
}: {
  callCount: number;
  mode: OverviewGraphMode;
  onModeChange: (mode: OverviewGraphMode) => void;
  typeCount: number;
}) {
  return (
    <div className="grid h-8 shrink-0 grid-cols-3 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-0.5 shadow-sm">
      <GraphModeButton
        active={mode === "dependencies"}
        icon={Network}
        label="Dependencies"
        onClick={() => onModeChange("dependencies")}
      />
      <GraphModeButton
        active={mode === "types"}
        count={typeCount}
        icon={Boxes}
        label="Types"
        onClick={() => onModeChange("types")}
      />
      <GraphModeButton
        active={mode === "calls"}
        count={callCount}
        icon={Workflow}
        label="Calls"
        onClick={() => onModeChange("calls")}
      />
    </div>
  );
}

function GraphModeButton({
  active,
  count,
  icon: Icon,
  label,
  onClick,
}: {
  active: boolean;
  count?: number;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-pressed={active}
      className={cn(
        "inline-flex h-7 min-w-0 items-center justify-center gap-1.5 rounded px-2.5 text-xs font-medium leading-none text-muted-foreground transition hover:text-foreground",
        active && "bg-[var(--app-elevated)] text-foreground shadow-sm",
      )}
      onClick={onClick}
      type="button"
    >
      <Icon className="size-3.5 shrink-0" aria-hidden="true" />
      <span>{label}</span>
      {typeof count === "number" ? (
        <span className={cn("rounded bg-muted px-1 py-0.5 text-[10px] leading-none", active && "bg-sky-500/15 text-sky-200")}>
          {count}
        </span>
      ) : null}
    </button>
  );
}

function GraphLoadingState({ label = "Loading graph..." }: { label?: string }) {
  return (
    <div className="flex h-full min-h-0 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-card text-sm text-muted-foreground">
      {label}
    </div>
  );
}

function DeferredGraphState({
  description,
  error,
  graphLabel,
  isLoading,
  onRetry,
}: {
  description: string;
  error: string | null;
  graphLabel: string;
  isLoading: boolean;
  onRetry: () => void;
}) {
  return (
    <div className="grid h-full min-h-0 place-items-center rounded-md border border-[color:var(--app-border)] bg-card px-6 text-center">
      <div className="max-w-md">
        <div className="text-sm font-semibold text-foreground">
          {error ? `${graphLabel} unavailable` : `Building ${graphLabel}`}
        </div>
        <p className="mt-2 text-sm leading-6 text-muted-foreground">
          {error ? error : description}
        </p>
        {error ? (
          <button
            className="mt-4 h-8 rounded-md border border-[color:var(--app-border)] px-3 text-xs font-semibold text-foreground transition hover:bg-[var(--app-subtle)]"
            onClick={onRetry}
            type="button"
          >
            Retry
          </button>
        ) : (
          <div className="mt-4 text-xs text-muted-foreground">
            {isLoading ? "Analyzing Move package..." : "Starting analysis..."}
          </div>
        )}
      </div>
    </div>
  );
}

const SELECTABLE_TYPE_KINDS = new Set(["struct", "enum", "datatype", "summaryType"]);
const NAVIGATOR_TYPE_KINDS = new Set([...SELECTABLE_TYPE_KINDS, "builtin"]);

function firstSelectableTypeId(nodes: MoveTypeGraphNode[], movePackage: MovePackage | null) {
  return selectableTypes(nodes, movePackage)[0]?.id ?? null;
}

function selectableTypeCount(nodes: MoveTypeGraphNode[], movePackage: MovePackage | null) {
  return selectableTypes(nodes, movePackage).length;
}

function typeNavigatorCount(nodes: MoveTypeGraphNode[]) {
  return nodes.filter((node) => NAVIGATOR_TYPE_KINDS.has(node.kind)).length;
}

function packageTypeCount(movePackage: MovePackage | null) {
  return movePackage?.modules.reduce((total, module) => total + module.structs.length, 0) ?? 0;
}

function hasTypeGraphPayload(graph: MoveTypeGraph, movePackage: MovePackage | null) {
  if (!graph.nodes.length && !graph.edges.length && !graph.unresolvedTypes.length) {
    return false;
  }

  if (!movePackage) {
    return true;
  }

  return graph.nodes.some((node) => node.packagePath === movePackage.path);
}

function hasCallGraphPayload(graph: MoveCallGraph, movePackage: MovePackage | null) {
  if (!graph.nodes.length && !graph.edges.length && !graph.unresolvedCalls.length) {
    return false;
  }

  if (!movePackage) {
    return true;
  }

  return graph.nodes.some((node) => node.packagePath === movePackage.path)
    || graph.unresolvedCalls.some((call) => graph.nodes.some((node) => node.id === call.source && node.packagePath === movePackage.path));
}

function selectableCallFunctionCount(nodes: MoveCallGraphNode[], movePackage: MovePackage | null) {
  const packagePath = movePackage?.path ?? null;

  if (packagePath === null) {
    return nodes.filter((node) => !node.isExternal && !node.id.startsWith("unresolved:call:")).length;
  }

  return nodes.filter((node) => node.packagePath === packagePath).length;
}

function packageFunctionCount(movePackage: MovePackage | null) {
  return movePackage?.modules.reduce((total, module) => total + module.functions.length, 0) ?? 0;
}

function selectableTypes(nodes: MoveTypeGraphNode[], movePackage: MovePackage | null) {
  const packagePath = movePackage?.path ?? null;
  const localTypes = nodes.filter(
    (node) =>
      packagePath !== null
      && node.packagePath === packagePath
      && SELECTABLE_TYPE_KINDS.has(node.kind),
  );

  return [...localTypes].sort(
    (left, right) =>
      selectableTypePriority(right) - selectableTypePriority(left)
      || (left.moduleName ?? "").localeCompare(right.moduleName ?? "")
      || left.name.localeCompare(right.name)
      || left.qualifiedName.localeCompare(right.qualifiedName),
  );
}

function selectableTypePriority(node: MoveTypeGraphNode) {
  const name = node.name.toLowerCase();
  let score = 0;

  if (/(vault|pool|escrow|market|registry|admincap|cap|bucket|receipt)/.test(name)) {
    score += 100;
  }

  if (node.abilities.includes("key")) {
    score += 40;
  }

  if (name.includes("cap") || name.includes("admin") || name.includes("authority")) {
    score += 35;
  }

  if (node.qualifiedName.includes("<")) {
    score += 10;
  }

  return score;
}
