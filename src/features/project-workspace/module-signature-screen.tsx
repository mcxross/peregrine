import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  ArrowLeft,
  Box,
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  FileCode2,
  GitBranch,
  Loader2,
  X,
} from "lucide-react";
import React from "react";

import type {
  MoveFunctionSignature,
  MoveModule,
  MovePackage,
  MoveStateAccessGraph,
  MoveStateAccessGraphEdge,
  MoveStateAccessGraphNode,
} from "@/features/empty-project/filesystem-tree";
import { loadMoveStateAccessGraph } from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

export type SelectedMoveModule = {
  moveModule: MoveModule;
  movePackage: MovePackage;
};

type ModuleSignatureScreenProps = {
  onClose?: () => void;
  rootPath?: string;
  selectedModule: SelectedMoveModule;
  stateAccessGraph?: MoveStateAccessGraph;
};

type FunctionCategoryId =
  | "entry"
  | "private"
  | "public"
  | "public-entry"
  | "public-friend"
  | "public-package"
  | "view";

type FunctionCategory = {
  functions: MoveFunctionSignature[];
  id: FunctionCategoryId;
  label: string;
  tone: "entry" | "friend" | "package" | "private" | "public" | "publicEntry" | "view";
};

export function ModuleSignatureScreen({
  onClose,
  rootPath,
  selectedModule,
  stateAccessGraph,
}: ModuleSignatureScreenProps) {
  const { moveModule, movePackage } = selectedModule;
  const structs = moveModule.structs ?? [];
  const functions = moveModule.functions ?? [];
  const hasSurface = structs.length || functions.length;
  const [openFunctionKey, setOpenFunctionKey] = React.useState<string | null>(
    null,
  );
  const [loadedStateAccessGraph, setLoadedStateAccessGraph] =
    React.useState<MoveStateAccessGraph | null>(null);
  const [isLoadingStateAccessGraph, setIsLoadingStateAccessGraph] =
    React.useState(false);
  const [stateAccessGraphError, setStateAccessGraphError] = React.useState<
    string | null
  >(null);
  const [stateAccessGraphRetryNonce, setStateAccessGraphRetryNonce] =
    React.useState(0);
  const [collapsedSurfaceSections, setCollapsedSurfaceSections] =
    React.useState<Set<string>>(() => new Set());
  const [collapsedFunctionGroups, setCollapsedFunctionGroups] =
    React.useState<Set<string>>(() => new Set());
  const loadedStateAccessGraphKeyRef = React.useRef<string | null>(null);
  const stateAccessGraphRequestRef = React.useRef(0);
  const selectedFunction =
    functions.find((signature) => functionKey(signature) === openFunctionKey) ??
    null;
  const selectedFunctionName = selectedFunction?.name ?? null;
  const stateGraphRequestKey =
    rootPath && selectedFunctionName
      ? [
          rootPath,
          movePackage.path,
          moveModule.address ?? "_",
          moveModule.name,
          selectedFunctionName,
        ].join("\n")
      : null;
  const hasPackageStateAccessGraph = hasStateAccessGraphPayload(stateAccessGraph);
  const hasLoadedStateAccessGraphForSelection = Boolean(
    stateGraphRequestKey &&
      loadedStateAccessGraph &&
      loadedStateAccessGraphKeyRef.current === stateGraphRequestKey,
  );
  const activeStateAccessGraph = hasPackageStateAccessGraph
    ? (stateAccessGraph ?? null)
    : hasLoadedStateAccessGraphForSelection
      ? loadedStateAccessGraph
      : null;

  React.useEffect(() => {
    stateAccessGraphRequestRef.current += 1;
    loadedStateAccessGraphKeyRef.current = null;
    setOpenFunctionKey(null);
    setLoadedStateAccessGraph(null);
    setIsLoadingStateAccessGraph(false);
    setStateAccessGraphError(null);
  }, [moveModule.filePath, movePackage.path, rootPath]);

  React.useEffect(() => {
    if (
      !stateGraphRequestKey ||
      hasPackageStateAccessGraph ||
      hasLoadedStateAccessGraphForSelection ||
      !rootPath ||
      !selectedFunctionName
    ) {
      return;
    }

    const requestId = stateAccessGraphRequestRef.current + 1;
    stateAccessGraphRequestRef.current = requestId;
    let settled = false;
    setIsLoadingStateAccessGraph(true);
    setStateAccessGraphError(null);

    const timeout = window.setTimeout(() => {
      if (stateAccessGraphRequestRef.current !== requestId || settled) {
        return;
      }

      settled = true;
      stateAccessGraphRequestRef.current += 1;
      setIsLoadingStateAccessGraph(false);
      setStateAccessGraphError(
        "State access analysis timed out after 15 seconds for this function.",
      );
    }, 15_000);

    void loadMoveStateAccessGraph(
      rootPath,
      movePackage.path,
      moveModule.address,
      moveModule.name,
      selectedFunctionName,
    )
      .then((graph) => {
        if (stateAccessGraphRequestRef.current !== requestId || settled) {
          return;
        }

        settled = true;
        loadedStateAccessGraphKeyRef.current = stateGraphRequestKey;
        setLoadedStateAccessGraph(graph);
        setStateAccessGraphError(null);
      })
      .catch((error) => {
        if (stateAccessGraphRequestRef.current !== requestId || settled) {
          return;
        }

        settled = true;
        loadedStateAccessGraphKeyRef.current = null;
        setLoadedStateAccessGraph(null);
        setStateAccessGraphError(formatStateAccessGraphError(error));
      })
      .finally(() => {
        window.clearTimeout(timeout);

        if (stateAccessGraphRequestRef.current === requestId) {
          setIsLoadingStateAccessGraph(false);
        }
      });

    return () => {
      window.clearTimeout(timeout);

      if (stateAccessGraphRequestRef.current === requestId) {
        stateAccessGraphRequestRef.current += 1;
      }
    };
  }, [
    hasLoadedStateAccessGraphForSelection,
    hasPackageStateAccessGraph,
    moveModule.address,
    moveModule.name,
    movePackage.path,
    rootPath,
    selectedFunctionName,
    stateAccessGraphRetryNonce,
    stateGraphRequestKey,
  ]);

  const retryStateAccessGraph = React.useCallback(() => {
    stateAccessGraphRequestRef.current += 1;
    loadedStateAccessGraphKeyRef.current = null;
    setLoadedStateAccessGraph(null);
    setIsLoadingStateAccessGraph(false);
    setStateAccessGraphError(null);
    setStateAccessGraphRetryNonce((current) => current + 1);
  }, []);
  const functionGroups = React.useMemo(
    () => groupFunctionsByCategory(functions),
    [functions],
  );
  const toggleSurfaceSection = React.useCallback((section: string) => {
    setCollapsedSurfaceSections((current) => toggleSetValue(current, section));
  }, []);
  const toggleFunctionGroup = React.useCallback((groupId: string) => {
    setCollapsedFunctionGroups((current) =>
      toggleSetValue(current, groupId),
    );
  }, []);

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] bg-[var(--app-window)]">
      <header className="flex min-h-12 min-w-0 items-center justify-between gap-4 border-b border-[color:var(--app-border)] px-5 py-2">
        <div className="flex min-w-0 items-center gap-3">
          {selectedFunction ? (
            <button
              aria-label="Back to module details"
              className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
              onClick={() => setOpenFunctionKey(null)}
              type="button"
            >
              <ArrowLeft className="size-4" aria-hidden="true" />
            </button>
          ) : null}
          {selectedFunction ? (
            <GitBranch
              className="size-4 shrink-0 text-cyan-300"
              aria-hidden="true"
            />
          ) : null}
          {selectedFunction ? (
            <div className="flex min-w-0 items-center gap-2">
              <h2 className="truncate text-base font-semibold leading-5">
                State access
              </h2>
              <span className="hidden shrink-0 text-xs text-muted-foreground sm:inline">
                {moveModule.name}::{selectedFunction.name}
              </span>
              <span className="hidden min-w-0 truncate text-[11px] text-muted-foreground xl:inline">
                {movePackage.name} / {moveModule.filePath}
              </span>
            </div>
          ) : (
            <div className="min-w-0">
              <h2 className="truncate text-base font-semibold leading-5">
                {moveModule.name}
              </h2>
              <p className="mt-0.5 truncate text-[11px] leading-4 text-muted-foreground">
                {movePackage.name} / {moveModule.filePath}
              </p>
            </div>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {onClose ? (
            <button
              aria-label="Close module surface"
              className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
              onClick={onClose}
              type="button"
            >
              <X className="size-4" aria-hidden="true" />
            </button>
          ) : null}
        </div>
      </header>

      {selectedFunction ? (
        <StateAccessWorkspace
          graph={activeStateAccessGraph}
          isLoading={isLoadingStateAccessGraph}
          error={stateAccessGraphError}
          moveModule={moveModule}
          movePackage={movePackage}
          onRetry={retryStateAccessGraph}
          rootPath={rootPath}
          signature={selectedFunction}
        />
      ) : (
        <div className="min-h-0 min-w-0 overflow-auto px-6 py-5">
          {hasSurface ? (
            <div className="space-y-6">
              <SurfaceSection
                count={structs.length}
                emptyText="No structs found for this module."
                isOpen={!collapsedSurfaceSections.has("structs")}
                onToggle={() => toggleSurfaceSection("structs")}
                title="Structs"
              >
                <div className="space-y-3">
                  {structs.map((signature) => (
                    <article
                      key={`${signature.name}-${signature.signature}`}
                      className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-4"
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div className="flex min-w-0 items-center gap-2">
                          <Box
                            className="size-4 shrink-0 text-muted-foreground"
                            aria-hidden="true"
                          />
                          <h3 className="truncate text-sm font-semibold">
                            {signature.name}
                          </h3>
                        </div>
                        <div className="flex shrink-0 flex-wrap justify-end gap-2">
                          {signature.abilities.length ? (
                            signature.abilities.map((ability) => (
                              <Badge key={ability} tone="ability">
                                {ability}
                              </Badge>
                            ))
                          ) : (
                            <Badge tone="private">no abilities</Badge>
                          )}
                        </div>
                      </div>
                      <SignatureCodeBlock source={signature.signature} />
                    </article>
                  ))}
                </div>
              </SurfaceSection>

              <SurfaceSection
                count={functions.length}
                emptyText="No function signatures found for this module."
                isOpen={!collapsedSurfaceSections.has("functions")}
                onToggle={() => toggleSurfaceSection("functions")}
                title="Functions"
              >
                <div className="space-y-4">
                  {functionGroups.map((group) => {
                    const isGroupOpen = !collapsedFunctionGroups.has(
                      group.id,
                    );

                    return (
                      <CollapsibleSurfaceGroup
                        count={group.functions.length}
                        isOpen={isGroupOpen}
                        key={group.id}
                        onToggle={() =>
                          toggleFunctionGroup(group.id)
                        }
                        title={group.label}
                        tone={group.tone}
                      >
                        <div className="space-y-3">
                          {group.functions.map((signature) => {
                            const key = functionKey(signature);
                            const isOpen = openFunctionKey === key;

                            return (
                              <FunctionSignatureCard
                                key={`${signature.name}-${signature.signature}`}
                                isOpen={isOpen}
                                onToggle={() => {
                                  setOpenFunctionKey((current) =>
                                    current === key ? null : key,
                                  );
                                }}
                                signature={signature}
                              />
                            );
                          })}
                        </div>
                      </CollapsibleSurfaceGroup>
                    );
                  })}
                </div>
              </SurfaceSection>
            </div>
          ) : (
            <div className="flex h-full min-h-48 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] text-sm text-muted-foreground">
              No structs or function signatures found for this module.
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function StateAccessWorkspace({
  error,
  graph,
  isLoading,
  moveModule,
  movePackage,
  onRetry,
  rootPath,
  signature,
}: {
  error: string | null;
  graph: MoveStateAccessGraph | null;
  isLoading: boolean;
  moveModule: MoveModule;
  movePackage: MovePackage;
  onRetry: () => void;
  rootPath?: string;
  signature: MoveFunctionSignature;
}) {
  return (
    <div className="grid h-full min-h-0 min-w-0 bg-[var(--app-window)]">
      <div className="h-full min-h-0 min-w-0 overflow-hidden">
        <FunctionStateAccessGraphPanel
          fullscreen
          graph={graph}
          isLoading={isLoading}
          error={error}
          onRetry={onRetry}
          moveModule={moveModule}
          movePackage={movePackage}
          rootPath={rootPath}
          signature={signature}
        />
      </div>
    </div>
  );
}

function FunctionSignatureCard({
  isOpen,
  onToggle,
  signature,
}: {
  isOpen: boolean;
  onToggle: () => void;
  signature: MoveFunctionSignature;
}) {
  const source =
    isOpen && signature.body ? signature.body : signature.signature;

  return (
    <article
      className={cn(
        "rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-4 transition-colors",
        isOpen && "border-primary/45 bg-primary/5",
      )}
    >
      <button
        className="flex w-full min-w-0 items-center justify-between gap-3 text-left"
        onClick={onToggle}
        type="button"
      >
        <div className="flex min-w-0 items-center gap-2">
          <FileCode2
            className="size-4 shrink-0 text-muted-foreground"
            aria-hidden="true"
          />
          <h3 className="truncate text-sm font-semibold">{signature.name}</h3>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Badge tone={functionBadgeTone(signature)}>
            {functionBadgeLabel(signature)}
          </Badge>
          {signature.body ? (
            <ChevronDown
              className={cn(
                "size-4 text-muted-foreground transition-transform",
                isOpen && "rotate-180",
              )}
              aria-hidden="true"
            />
          ) : null}
        </div>
      </button>
      <SignatureCodeBlock maxHeight source={source} />
    </article>
  );
}

type FunctionStateAccessSummary = {
  functionId: string;
  functionNode: MoveStateAccessGraphNode | null;
  nodeById: Map<string, MoveStateAccessGraphNode>;
  reachedFunctionIds: Set<string>;
  stateEdges: MoveStateAccessGraphEdge[];
  stateNodes: MoveStateAccessGraphNode[];
};

type StateAccessDiagramRow = {
  accessKinds: string[];
  direct: boolean;
  edges: MoveStateAccessGraphEdge[];
  id: string;
  label: string;
  node: MoveStateAccessGraphNode;
  subtitle: string;
  y: number;
};

type StateAccessDiagramGroup = {
  height: number;
  id: string;
  rows: StateAccessDiagramRow[];
  subtitle: string;
  title: string;
  typeNode: MoveStateAccessGraphNode | null;
  y: number;
};

type StateAccessDiagramModel = {
  groups: StateAccessDiagramGroup[];
  height: number;
};

type StateAccessFlowRowData = {
  accessKinds: string[];
  direct: boolean;
  id: string;
  label: string;
  meta: string;
};

type StateAccessFlowNodeData =
  | {
      kind: "function";
      meta: string;
      title: string;
    }
  | {
      kind: "stateGroup";
      meta: string;
      rows: StateAccessFlowRowData[];
      title: string;
    };

const STATE_ACCESS_GROUP_HEADER_HEIGHT = 64;
const STATE_ACCESS_GROUP_ROW_HEIGHT = 66;
const STATE_ACCESS_GROUP_GAP = 28;
const STATE_ACCESS_GROUP_START_Y = 40;
const STATE_ACCESS_GROUP_X = 520;

const STATE_ACCESS_FUNCTION_HEIGHT = 104;

type StateAccessFlowModelGroup = {
  height: number;
  rows: StateAccessFlowRowData[];
  title: string;
};

const STATE_ACCESS_NODE_TYPES = {
  stateAccess: StateAccessFlowNode,
};

function FunctionStateAccessGraphPanel({
  error,
  fullscreen = false,
  graph,
  isLoading,
  onRetry,
  moveModule,
  movePackage,
  rootPath,
  signature,
}: {
  error: string | null;
  fullscreen?: boolean;
  graph: MoveStateAccessGraph | null;
  isLoading: boolean;
  onRetry: () => void;
  moveModule: MoveModule;
  movePackage: MovePackage;
  rootPath?: string;
  signature: MoveFunctionSignature;
}) {
  const functionId = functionIdForSignature(movePackage, moveModule, signature);
  const summary = React.useMemo(
    () => buildFunctionStateAccessSummary(graph, functionId),
    [functionId, graph],
  );

  if (isLoading && !graph) {
    return (
      <div
        className={cn(
          "flex items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-3 text-xs text-muted-foreground",
          fullscreen && "h-full justify-center",
        )}
      >
        <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
        Building state access graph from the Move AST...
      </div>
    );
  }

  if (error && !graph) {
    return (
      <div
        className={cn(
          "flex min-w-0 items-center justify-between gap-3 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-3 text-xs text-red-200",
          fullscreen && "h-full",
        )}
      >
        <span className="min-w-0 truncate">{error}</span>
        <button
          className="shrink-0 rounded border border-red-300/30 px-2 py-1 text-[11px] font-medium text-red-100 transition hover:bg-red-300/10"
          onClick={onRetry}
          type="button"
        >
          Retry
        </button>
      </div>
    );
  }

  if (!graph) {
    return (
      <div
        className={cn(
          "rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-3 text-xs text-muted-foreground",
          fullscreen && "grid h-full place-items-center",
        )}
      >
        {rootPath
          ? "State graph is not loaded yet."
          : "State graph is unavailable in this view."}
      </div>
    );
  }

  if (!summary.stateNodes.length) {
    return (
      <div
        className={cn(
          "rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-3 text-xs text-muted-foreground",
          fullscreen && "grid h-full place-items-center",
        )}
      >
        No package state access was found for this function in the current AST
        graph.
      </div>
    );
  }

  return (
    <div
      className={cn(
        "overflow-hidden border border-[color:var(--app-border)] bg-[var(--app-window)]",
        fullscreen
          ? "grid h-full min-h-0 rounded-none border-x-0"
          : "rounded-md",
      )}
    >
      <StateAccessDiagram
        fullscreen={fullscreen}
        moveModule={moveModule}
        signature={signature}
        summary={summary}
      />
      {!fullscreen ? (
        <div className="grid gap-2 border-t border-[color:var(--app-border)] bg-[var(--app-window)] px-4 py-3">
          {summary.stateNodes.slice(0, 6).map((node) => {
            const accessKinds = accessKindsForNode(summary.stateEdges, node.id);
            const direct = summary.stateEdges.some(
              (edge) =>
                edge.target === node.id && edge.source === summary.functionId,
            );

            return (
              <div
                className="flex min-w-0 items-center justify-between gap-3 text-xs"
                key={node.id}
              >
                <div className="min-w-0">
                  <div className="truncate font-medium text-foreground">
                    {node.qualifiedName}
                  </div>
                  <div className="truncate text-muted-foreground">
                    {node.kind === "field" ? "field" : "state type"}
                  </div>
                </div>
                <div className="flex shrink-0 flex-wrap justify-end gap-1.5">
                  {direct ? (
                    <StateAccessBadge label="direct" tone="direct" />
                  ) : (
                    <StateAccessBadge label="via call" tone="indirect" />
                  )}
                  {accessKinds.slice(0, 3).map((accessKind) => (
                    <StateAccessBadge
                      key={accessKind}
                      label={accessKindLabel(accessKind)}
                      tone={accessKindTone(accessKind)}
                    />
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function StateAccessDiagram({
  fullscreen = false,
  moveModule,
  signature,
  summary,
}: {
  fullscreen?: boolean;
  moveModule: MoveModule;
  signature: MoveFunctionSignature;
  summary: FunctionStateAccessSummary;
}) {
  const flow = React.useMemo(
    () => buildStateAccessFlowModel(summary, moveModule, signature),
    [moveModule, signature, summary],
  );

  return (
    <div
      className={cn(
        "state-access-flow relative bg-[var(--app-window)]",
        fullscreen ? "h-full min-h-0" : "min-h-[32rem]",
      )}
    >
      <div className="pointer-events-none absolute right-3 top-3 z-10 rounded bg-[var(--app-subtle)] px-2 py-0.5 text-[11px] text-muted-foreground">
        {summary.stateNodes.length} touched
      </div>
      <ReactFlow
        colorMode="dark"
        edges={flow.edges}
        edgesFocusable={false}
        fitView
        fitViewOptions={{ padding: fullscreen ? 0.06 : 0.12 }}
        maxZoom={2.4}
        minZoom={0.28}
        nodeTypes={STATE_ACCESS_NODE_TYPES}
        nodes={flow.nodes}
        nodesDraggable={false}
        nodesFocusable={false}
        onlyRenderVisibleElements
        panOnScroll
        proOptions={{ hideAttribution: true }}
      >
        <Background color="var(--border)" gap={18} size={1} />
        <Controls
          className="!right-3 !top-11 !bg-background/90 !shadow-none [&_button]:!border-border [&_button]:!bg-background [&_button]:!text-foreground"
          position="top-right"
          showInteractive={false}
        />
      </ReactFlow>
    </div>
  );
}

function StateAccessFlowNode({ data }: NodeProps<Node<StateAccessFlowNodeData>>) {
  if (data.kind === "function") {
    return (
      <div className="relative h-[104px] w-72 overflow-hidden border border-cyan-400/70 bg-cyan-400/[0.04] text-left shadow-[0_0_0_1px_rgba(34,211,238,0.08)]">
        <Handle
          className="!size-2 !border-background"
          id="out"
          position={Position.Right}
          style={{ backgroundColor: "rgb(34 211 238)" }}
          type="source"
        />
        <div className="border-b border-cyan-400/20 px-4 py-3.5">
          <div className="truncate text-base font-semibold text-foreground">
            {truncateLabel(data.title, 26)}
          </div>
        </div>
        <div className="px-4 py-3">
          <div className="text-[11px] font-semibold text-muted-foreground">
            selected Move function
          </div>
          <div className="mt-1 truncate text-[11px] text-slate-500">
            {truncateLabel(data.meta, 38)}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="relative w-[28rem] overflow-hidden border border-rose-400/75 bg-[var(--app-window)] text-left shadow-[0_0_0_1px_rgba(251,113,133,0.08)]">
      <div className="h-16 border-b border-slate-700/80 px-4 py-3">
        <div className="truncate text-sm font-semibold text-foreground">
          {truncateLabel(data.title, 36)}
        </div>
        <div className="mt-1 truncate text-[11px] text-muted-foreground">
          {truncateLabel(data.meta, 54)}
        </div>
      </div>
      <div>
        {data.rows.map((row) => {
          const color = accessKindColor(primaryAccessKind(row.accessKinds));

          return (
            <div
              className="relative grid h-[66px] grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-rose-400/70 px-4 last:border-b-0"
              key={row.id}
            >
              <Handle
                className="!left-0 !size-2.5 !-translate-x-1/2 !-translate-y-1/2 !border-background"
                id={row.id}
                position={Position.Left}
                style={{ backgroundColor: color }}
                type="target"
              />
              <div className="min-w-0">
                <div className="truncate text-sm font-semibold text-foreground">
                  {truncateLabel(row.label, 32)}
                </div>
                <div className="mt-1 truncate text-[11px] text-muted-foreground">
                  {truncateLabel(row.meta, 48)}
                </div>
              </div>
              <div className="flex max-w-36 shrink-0 flex-wrap justify-end gap-1.5">
                {row.direct ? (
                  <StateAccessBadge label="direct" tone="direct" />
                ) : (
                  <StateAccessBadge label="via call" tone="indirect" />
                )}
                {row.accessKinds.slice(0, 2).map((accessKind) => (
                  <StateAccessBadge
                    key={accessKind}
                    label={accessKindLabel(accessKind)}
                    tone={accessKindTone(accessKind)}
                  />
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function buildStateAccessFlowModel(
  summary: FunctionStateAccessSummary,
  moveModule: MoveModule,
  signature: MoveFunctionSignature,
): {
  edges: Edge[];
  nodes: Array<Node<StateAccessFlowNodeData>>;
} {
  const model = buildStateAccessDiagramModel(summary);
  const groups = model.groups.map<StateAccessFlowModelGroup>((group) => {
    const rows = group.rows.map<StateAccessFlowRowData>((row) => ({
      accessKinds: row.accessKinds,
      direct: row.direct,
      id: row.id,
      label: row.label,
      meta: row.subtitle,
    }));

    return {
      height:
        STATE_ACCESS_GROUP_HEADER_HEIGHT +
        rows.length * STATE_ACCESS_GROUP_ROW_HEIGHT,
      rows,
      title: group.title,
    };
  });
  const totalGroupHeight =
    groups.reduce((height, group) => height + group.height, 0) +
    Math.max(0, groups.length - 1) * STATE_ACCESS_GROUP_GAP;
  const functionY = Math.max(
    STATE_ACCESS_GROUP_START_Y,
    STATE_ACCESS_GROUP_START_Y +
      totalGroupHeight / 2 -
      STATE_ACCESS_FUNCTION_HEIGHT / 2,
  );
  const nodes: Array<Node<StateAccessFlowNodeData>> = [
    {
      id: "selected-function",
      position: { x: 48, y: functionY },
      type: "stateAccess",
      data: {
        kind: "function",
        meta: `${moveModule.name}::${signature.name}()`,
        title: signature.name,
      },
    },
  ];
  const edges: Edge[] = [];
  let groupY = STATE_ACCESS_GROUP_START_Y;

  model.groups.forEach((group, groupIndex) => {
    const flowGroup = groups[groupIndex];

    if (!flowGroup) {
      return;
    }

    const groupNodeId = `state-group:${group.id}`;

    nodes.push({
      id: groupNodeId,
      position: { x: STATE_ACCESS_GROUP_X, y: groupY },
      type: "stateAccess",
      data: {
        kind: "stateGroup",
        meta: group.subtitle,
        rows: flowGroup.rows,
        title: flowGroup.title,
      },
    });

    group.rows.forEach((row) => {
      const color = accessKindColor(primaryAccessKind(row.accessKinds));

      edges.push({
        id: `access:${row.id}`,
        animated: !row.direct,
        markerEnd: {
          color,
          type: MarkerType.ArrowClosed,
        },
        source: "selected-function",
        sourceHandle: "out",
        target: groupNodeId,
        targetHandle: row.id,
        type: "smoothstep",
        style: {
          stroke: color,
          strokeDasharray: row.direct ? undefined : "6 6",
          strokeWidth: row.direct ? 2 : 1.35,
        },
      });
    });

    groupY += flowGroup.height + STATE_ACCESS_GROUP_GAP;
  });

  return { edges, nodes };
}

function buildStateAccessDiagramModel(
  summary: FunctionStateAccessSummary,
): StateAccessDiagramModel {
  const groupsById = new Map<
    string,
    Omit<StateAccessDiagramGroup, "height" | "rows" | "y"> & {
      rows: Omit<StateAccessDiagramRow, "y">[];
    }
  >();
  const edgesByTarget = new Map<string, MoveStateAccessGraphEdge[]>();

  for (const edge of summary.stateEdges) {
    const edges = edgesByTarget.get(edge.target) ?? [];
    edges.push(edge);
    edgesByTarget.set(edge.target, edges);
  }

  for (const node of summary.stateNodes) {
    const edges = edgesByTarget.get(node.id) ?? [];

    if (!edges.length) {
      continue;
    }

    const ownerTypeId = ownerTypeIdForStateNode(node);
    const typeNode = ownerTypeId ? summary.nodeById.get(ownerTypeId) ?? null : null;
    const groupId = ownerTypeId ?? node.id;
    const groupTitle = typeNode?.name ?? ownerNameFromStateNode(node);
    const groupSubtitle = typeNode?.qualifiedName ?? node.qualifiedName;
    const group = groupsById.get(groupId) ?? {
      id: groupId,
      subtitle: groupSubtitle,
      title: groupTitle,
      typeNode,
      rows: [],
    };
    const accessKinds = accessKindsForNode(summary.stateEdges, node.id);

    group.rows.push({
      accessKinds,
      direct: edges.some((edge) => edge.source === summary.functionId),
      edges,
      id: node.id,
      label: node.kind === "field" ? node.name : "object value",
      node,
      subtitle: node.qualifiedName,
    });
    groupsById.set(groupId, group);
  }

  const groups: StateAccessDiagramGroup[] = Array.from(groupsById.values())
    .map((group) => ({
      ...group,
      rows: group.rows
        .sort(compareStateAccessRows)
        .map((row) => ({ ...row, y: 0 })),
      y: 0,
      height: 0,
    }))
    .sort((left, right) => left.title.localeCompare(right.title));
  let y = 86;

  for (const group of groups) {
    group.y = y;
    group.height = 46 + group.rows.length * 38;

    group.rows.forEach((row, index) => {
      row.y = group.y + 46 + index * 38;
    });

    y += group.height + 18;
  }

  return {
    groups,
    height: Math.max(260, y + 36),
  };
}

function compareStateAccessRows(
  left: Omit<StateAccessDiagramRow, "y">,
  right: Omit<StateAccessDiagramRow, "y">,
) {
  return (
    accessKindRank(primaryAccessKind(left.accessKinds)) -
      accessKindRank(primaryAccessKind(right.accessKinds)) ||
    Number(right.direct) - Number(left.direct) ||
    left.label.localeCompare(right.label)
  );
}

function ownerTypeIdForStateNode(node: MoveStateAccessGraphNode) {
  if (node.kind === "stateType") {
    return node.id;
  }

  if (!node.id.startsWith("stateField:")) {
    return null;
  }

  const fieldId = node.id.slice("stateField:".length);
  const separator = fieldId.lastIndexOf("::");

  return separator > 0 ? fieldId.slice(0, separator) : null;
}

function ownerNameFromStateNode(node: MoveStateAccessGraphNode) {
  if (node.kind !== "field") {
    return node.name;
  }

  const fieldSuffix = `.${node.name}`;

  if (node.qualifiedName.endsWith(fieldSuffix)) {
    const ownerQualifiedName = node.qualifiedName.slice(0, -fieldSuffix.length);
    const segments = ownerQualifiedName.split("::");

    return segments[segments.length - 1] ?? ownerQualifiedName;
  }

  return node.moduleName ?? "State";
}

function primaryAccessKind(accessKinds: string[]) {
  return accessKinds.slice().sort((left, right) => {
    return accessKindRank(left) - accessKindRank(right);
  })[0] ?? "read";
}

function accessKindRank(accessKind: string) {
  switch (accessKind) {
    case "write":
    case "borrowMut":
      return 0;
    case "move":
    case "return":
      return 1;
    case "borrowImm":
    case "copy":
    case "read":
      return 2;
    default:
      return 3;
  }
}

function buildFunctionStateAccessSummary(
  graph: MoveStateAccessGraph | null,
  functionId: string,
): FunctionStateAccessSummary {
  if (!graph) {
    return {
      functionId,
      functionNode: null,
      nodeById: new Map(),
      reachedFunctionIds: new Set([functionId]),
      stateEdges: [],
      stateNodes: [],
    };
  }

  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  const callsBySource = new Map<string, MoveStateAccessGraphEdge[]>();

  for (const edge of graph.edges) {
    if (edge.accessKind !== "call") {
      continue;
    }
    const edges = callsBySource.get(edge.source) ?? [];
    edges.push(edge);
    callsBySource.set(edge.source, edges);
  }

  const reachedFunctionIds = new Set<string>([functionId]);
  const queue: Array<{ depth: number; id: string }> = [
    { depth: 0, id: functionId },
  ];

  while (queue.length && reachedFunctionIds.size < 48) {
    const current = queue.shift();

    if (!current || current.depth >= 4) {
      continue;
    }

    for (const edge of callsBySource.get(current.id) ?? []) {
      if (reachedFunctionIds.has(edge.target)) {
        continue;
      }

      reachedFunctionIds.add(edge.target);
      queue.push({ depth: current.depth + 1, id: edge.target });
    }
  }

  const stateEdges = dedupeStateEdges(
    graph.edges.filter((edge) => {
      if (edge.accessKind === "call" || !reachedFunctionIds.has(edge.source)) {
        return false;
      }

      const target = nodeById.get(edge.target);

      return Boolean(target && target.kind !== "function");
    }),
  ).slice(0, 32);
  const stateNodes = Array.from(new Set(stateEdges.map((edge) => edge.target)))
    .map((id) => nodeById.get(id))
    .filter((node): node is MoveStateAccessGraphNode => Boolean(node))
    .slice(0, 18);

  return {
    functionId,
    functionNode: nodeById.get(functionId) ?? null,
    nodeById,
    reachedFunctionIds,
    stateEdges,
    stateNodes,
  };
}

function dedupeStateEdges(edges: MoveStateAccessGraphEdge[]) {
  const seen = new Set<string>();
  const result: MoveStateAccessGraphEdge[] = [];

  for (const edge of edges) {
    const key = `${edge.source}:${edge.target}:${edge.accessKind}:${edge.fieldName ?? ""}`;

    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    result.push(edge);
  }

  return result;
}

function accessKindsForNode(edges: MoveStateAccessGraphEdge[], nodeId: string) {
  return Array.from(
    new Set(
      edges
        .filter((edge) => edge.target === nodeId)
        .map((edge) => edge.accessKind),
    ),
  );
}

function functionIdForSignature(
  movePackage: MovePackage,
  moveModule: MoveModule,
  signature: MoveFunctionSignature,
) {
  return `function:${movePackage.path}:${moveModule.address ?? "_"}::${moveModule.name}::${signature.name}`;
}

function hasStateAccessGraphPayload(
  graph: MoveStateAccessGraph | null | undefined,
) {
  return Boolean(
    graph &&
    (graph.nodes.length > 0 ||
      graph.edges.length > 0 ||
      graph.unresolvedAccesses.length > 0),
  );
}

function formatStateAccessGraphError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "Failed to build the state access graph for this function.";
}

function StateAccessBadge({
  label,
  tone,
}: {
  label: string;
  tone: "direct" | "indirect" | "mutates" | "reads" | "moves" | "neutral";
}) {
  return (
    <span
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px] font-medium leading-none",
        tone === "direct" && "bg-cyan-500/12 text-cyan-200",
        tone === "indirect" && "bg-slate-500/20 text-slate-300",
        tone === "mutates" && "bg-rose-500/12 text-rose-200",
        tone === "reads" && "bg-sky-500/12 text-sky-200",
        tone === "moves" && "bg-violet-500/12 text-violet-200",
        tone === "neutral" && "bg-muted text-muted-foreground",
      )}
    >
      {label}
    </span>
  );
}

function accessKindLabel(accessKind: string) {
  switch (accessKind) {
    case "borrowMut":
      return "mut borrow";
    case "borrowImm":
      return "borrow";
    default:
      return accessKind;
  }
}

function accessKindTone(
  accessKind: string,
): "mutates" | "reads" | "moves" | "neutral" {
  if (accessKind === "write" || accessKind === "borrowMut") {
    return "mutates";
  }
  if (
    accessKind === "read" ||
    accessKind === "borrowImm" ||
    accessKind === "copy"
  ) {
    return "reads";
  }
  if (accessKind === "move" || accessKind === "return") {
    return "moves";
  }

  return "neutral";
}

function accessKindColor(accessKind: string) {
  switch (accessKind) {
    case "write":
    case "borrowMut":
      return "rgb(251 113 133)";
    case "move":
    case "return":
      return "rgb(167 139 250)";
    case "borrowImm":
    case "copy":
      return "rgb(125 211 252)";
    default:
      return "rgb(45 212 191)";
  }
}

function truncateLabel(value: string, maxLength: number) {
  return value.length > maxLength
    ? `${value.slice(0, Math.max(0, maxLength - 1))}...`
    : value;
}

function functionKey(signature: { name: string; signature: string }) {
  return `${signature.name}-${signature.signature}`;
}

function toggleSetValue<T>(set: Set<T>, value: T) {
  const next = new Set(set);

  if (next.has(value)) {
    next.delete(value);
  } else {
    next.add(value);
  }

  return next;
}

function groupFunctionsByCategory(functions: MoveFunctionSignature[]) {
  const groups = new Map<FunctionCategoryId, FunctionCategory>();

  for (const signature of functions) {
    const category = functionCategory(signature);
    const group = groups.get(category.id);

    if (group) {
      group.functions.push(signature);
      continue;
    }

    groups.set(category.id, {
      ...category,
      functions: [signature],
    });
  }

  return FUNCTION_CATEGORY_ORDER
    .map((id) => groups.get(id))
    .filter((group): group is FunctionCategory => Boolean(group))
    .map((group) => ({
      ...group,
      functions: [...group.functions].sort((left, right) =>
        left.name.localeCompare(right.name),
      ),
    }));
}

const FUNCTION_CATEGORY_ORDER: FunctionCategoryId[] = [
  "public-entry",
  "entry",
  "public-package",
  "public-friend",
  "public",
  "view",
  "private",
];

function functionCategory(signature: MoveFunctionSignature): Omit<FunctionCategory, "functions"> {
  const visibility = normalizedVisibility(signature.visibility);

  if (signature.isEntry && visibility === "public") {
    return { id: "public-entry", label: "Public entry", tone: "publicEntry" };
  }

  if (signature.isEntry) {
    return { id: "entry", label: "Entry only", tone: "entry" };
  }

  if (isViewFunction(signature)) {
    return { id: "view", label: "View", tone: "view" };
  }

  if (visibility === "public(package)") {
    return { id: "public-package", label: "Public(package)", tone: "package" };
  }

  if (visibility === "public(friend)") {
    return { id: "public-friend", label: "Public(friend)", tone: "friend" };
  }

  if (visibility === "public") {
    return { id: "public", label: "Public", tone: "public" };
  }

  return { id: "private", label: "Private", tone: "private" };
}

function isViewFunction(signature: MoveFunctionSignature) {
  const visibility = normalizedVisibility(signature.visibility);
  const isVisible =
    visibility === "public" ||
    visibility === "public(package)" ||
    visibility === "public(friend)";

  if (!isVisible || signature.isEntry || !signature.signature.includes("):")) {
    return false;
  }

  const source = `${signature.signature}\n${signature.body ?? ""}`;

  return ![
    "&mut",
    "borrow_global_mut",
    "move_to",
    "move_from",
    "table::add",
    "table::remove",
    "push_back",
    "pop_back",
    "swap_remove",
  ].some((token) => source.includes(token));
}

function normalizedVisibility(visibility: string) {
  return visibility.trim().toLowerCase() || "private";
}

function functionCategoryTextClass(tone: FunctionCategory["tone"]) {
  switch (tone) {
    case "publicEntry":
      return "text-emerald-300";
    case "entry":
      return "text-lime-300";
    case "view":
      return "text-fuchsia-300";
    case "package":
      return "text-orange-300";
    case "friend":
      return "text-yellow-300";
    case "public":
      return "text-cyan-300";
    case "private":
      return "text-muted-foreground";
  }
}

function functionCategoryDotClass(tone: FunctionCategory["tone"]) {
  switch (tone) {
    case "publicEntry":
      return "bg-emerald-300";
    case "entry":
      return "bg-lime-300";
    case "view":
      return "bg-fuchsia-300";
    case "package":
      return "bg-orange-300";
    case "friend":
      return "bg-yellow-300";
    case "public":
      return "bg-cyan-300";
    case "private":
      return "bg-slate-400";
  }
}

function functionBadgeTone(signature: MoveFunctionSignature): BadgeTone {
  if (signature.isEntry && normalizedVisibility(signature.visibility) === "public") {
    return "publicEntry";
  }

  if (signature.isEntry) {
    return "entry";
  }

  if (isViewFunction(signature)) {
    return "view";
  }

  const visibility = normalizedVisibility(signature.visibility);

  if (visibility === "public(package)") {
    return "package";
  }

  if (visibility === "public(friend)") {
    return "friend";
  }

  return visibility === "private" ? "private" : "public";
}

function functionBadgeLabel(signature: MoveFunctionSignature) {
  if (signature.isEntry && normalizedVisibility(signature.visibility) === "public") {
    return "public entry";
  }

  if (signature.isEntry) {
    return "entry only";
  }

  return normalizedVisibility(signature.visibility);
}

function SignatureCodeBlock({
  maxHeight,
  source,
}: {
  maxHeight?: boolean;
  source: string;
}) {
  const [copied, setCopied] = React.useState(false);

  React.useEffect(() => {
    if (!copied) {
      return;
    }

    const timeout = window.setTimeout(() => setCopied(false), 1200);

    return () => window.clearTimeout(timeout);
  }, [copied]);

  const copySource = React.useCallback(async () => {
    await navigator.clipboard.writeText(source);
    setCopied(true);
  }, [source]);

  return (
    <div className="group relative mt-3">
      <pre
        className={cn(
          "select-text overflow-auto rounded-md bg-[var(--app-subtle)] py-3 pl-3 pr-10 text-xs leading-5 [font-family:'JetBrains_Mono','JetBrains_Mono_NL','JetBrains_Mono_NF',ui-monospace,SFMono-Regular,'SF_Mono',Menlo,Monaco,Consolas,'Liberation_Mono',monospace]",
          maxHeight && "max-h-[420px]",
        )}
      >
        <code className="select-text">
          <HighlightedMoveSignature source={source} />
        </code>
      </pre>
      <button
        aria-label="Copy signature"
        className="absolute right-2 top-2 inline-flex size-6 select-none items-center justify-center rounded text-muted-foreground opacity-70 transition hover:bg-background/35 hover:text-foreground hover:opacity-100"
        onClick={copySource}
        type="button"
      >
        {copied ? (
          <Check className="size-3.5 text-emerald-300" aria-hidden="true" />
        ) : (
          <Copy className="size-3.5" aria-hidden="true" />
        )}
      </button>
    </div>
  );
}

function HighlightedMoveSignature({ source }: { source: string }) {
  return (
    <>
      {tokenizeMoveSignature(source).map((token, index) => (
        <span
          className={cn(
            token.kind === "keyword" && "text-sky-300",
            token.kind === "ability" && "text-emerald-300",
            token.kind === "type" && "text-violet-300",
            token.kind === "number" && "text-amber-300",
            token.kind === "punctuation" && "text-muted-foreground",
            token.kind === "module" && "text-cyan-300",
            token.kind === "identifier" && "text-foreground",
            token.kind === "plain" && "text-foreground",
          )}
          key={`${token.value}-${index}`}
        >
          {token.value}
        </span>
      ))}
    </>
  );
}

type MoveSignatureToken = {
  kind:
    | "ability"
    | "identifier"
    | "keyword"
    | "module"
    | "number"
    | "plain"
    | "punctuation"
    | "type";
  value: string;
};

const MOVE_SIGNATURE_TOKEN_PATTERN =
  /(::|[A-Za-z_][A-Za-z0-9_]*|\d+|[{}()[\]<>,:;.=*&]|\s+|.)/g;
const MOVE_KEYWORDS = new Set([
  "acquires",
  "entry",
  "fun",
  "has",
  "friend",
  "mut",
  "native",
  "package",
  "public",
  "struct",
]);
const MOVE_ABILITIES = new Set(["copy", "drop", "key", "store"]);
const MOVE_PRIMITIVE_TYPES = new Set([
  "address",
  "bool",
  "signer",
  "u8",
  "u16",
  "u32",
  "u64",
  "u128",
  "u256",
  "vector",
]);

function tokenizeMoveSignature(source: string): MoveSignatureToken[] {
  return Array.from(source.matchAll(MOVE_SIGNATURE_TOKEN_PATTERN), (match) => {
    const value = match[0];

    if (/^\s+$/.test(value)) {
      return { kind: "plain", value };
    }

    if (MOVE_KEYWORDS.has(value)) {
      return { kind: "keyword", value };
    }

    if (MOVE_ABILITIES.has(value)) {
      return { kind: "ability", value };
    }

    if (MOVE_PRIMITIVE_TYPES.has(value)) {
      return { kind: "type", value };
    }

    if (/^\d+$/.test(value)) {
      return { kind: "number", value };
    }

    if (value === "::") {
      return { kind: "module", value };
    }

    if (/^[{}()[\]<>,:;.=*&]$/.test(value)) {
      return { kind: "punctuation", value };
    }

    if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value)) {
      return { kind: "identifier", value };
    }

    return { kind: "plain", value };
  });
}

function SurfaceSection({
  children,
  count,
  emptyText,
  isOpen,
  onToggle,
  title,
}: {
  children: React.ReactNode;
  count: number;
  emptyText: string;
  isOpen: boolean;
  onToggle: () => void;
  title: string;
}) {
  return (
    <section>
      <button
        className="mb-3 flex w-full items-center justify-between gap-3 text-left"
        onClick={onToggle}
        type="button"
      >
        <span className="flex min-w-0 items-center gap-2">
          <ChevronDown
            className={cn(
              "size-4 shrink-0 text-muted-foreground transition-transform",
              !isOpen && "-rotate-90",
            )}
            aria-hidden="true"
          />
          <h3 className="truncate text-sm font-semibold text-foreground">
            {title}
          </h3>
        </span>
        <span className="rounded bg-[var(--app-subtle)] px-2 py-0.5 text-xs text-muted-foreground">
          {count}
        </span>
      </button>
      {isOpen && count ? (
        children
      ) : isOpen ? (
        <div className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-4 py-5 text-sm text-muted-foreground">
          {emptyText}
        </div>
      ) : null}
    </section>
  );
}

function CollapsibleSurfaceGroup({
  children,
  count,
  isOpen,
  onToggle,
  tone,
  title,
}: {
  children: React.ReactNode;
  count: number;
  isOpen: boolean;
  onToggle: () => void;
  tone?: FunctionCategory["tone"];
  title: string;
}) {
  return (
    <section className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-window)]">
      <button
        className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left"
        onClick={onToggle}
        type="button"
      >
        <span className="flex min-w-0 items-center gap-2">
          <ChevronRight
            className={cn(
              "size-4 shrink-0 text-muted-foreground transition-transform",
              isOpen && "rotate-90",
            )}
            aria-hidden="true"
          />
          {tone ? (
            <span
              className={cn("size-1.5 shrink-0 rounded-full", functionCategoryDotClass(tone))}
              aria-hidden="true"
            />
          ) : null}
          <span className={cn(
            "truncate text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground",
            tone && functionCategoryTextClass(tone),
          )}>
            {title}
          </span>
        </span>
        <span className="rounded bg-[var(--app-subtle)] px-2 py-0.5 text-xs text-muted-foreground">
          {count}
        </span>
      </button>
      {isOpen ? (
        <div className="border-t border-[color:var(--app-border)] p-3">
          {children}
        </div>
      ) : null}
    </section>
  );
}

type BadgeTone =
  | "ability"
  | "entry"
  | "friend"
  | "package"
  | "private"
  | "public"
  | "publicEntry"
  | "view";

function Badge({
  children,
  tone,
}: {
  children: string;
  tone: BadgeTone;
}) {
  return (
    <span
      className={cn(
        "rounded px-2 py-0.5 text-xs font-medium",
        tone === "ability" && "bg-sky-500/10 text-sky-300",
        tone === "publicEntry" && "bg-emerald-500/15 text-emerald-300",
        tone === "entry" && "bg-lime-500/15 text-lime-300",
        tone === "view" && "bg-fuchsia-500/15 text-fuchsia-300",
        tone === "package" && "bg-orange-500/15 text-orange-300",
        tone === "friend" && "bg-yellow-500/15 text-yellow-300",
        tone === "public" && "bg-cyan-500/15 text-cyan-300",
        tone === "private" && "bg-muted text-muted-foreground",
      )}
    >
      {children}
    </span>
  );
}
