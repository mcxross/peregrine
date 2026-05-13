import {
  Box,
  Check,
  ChevronDown,
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

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] bg-[var(--app-window)]">
      <header className="flex min-w-0 items-center justify-between gap-4 border-b border-[color:var(--app-border)] px-6 pb-3.5 pt-4">
        <div className="min-w-0">
          <h2 className="truncate text-xl font-semibold leading-6">
            {moveModule.name}
          </h2>
          <p className="mt-1 truncate text-xs leading-5 text-muted-foreground">
            {movePackage.name} / {moveModule.filePath}
          </p>
        </div>
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
      </header>

      <div className="min-h-0 overflow-auto px-6 py-5">
        {hasSurface ? (
          <div className="space-y-6">
            <SurfaceSection
              count={structs.length}
              emptyText="No structs found for this module."
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
              title="Functions"
            >
              <div className="space-y-3">
                {functions.map((signature) => {
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
                    >
                      {isOpen ? (
                        <FunctionStateAccessGraphPanel
                          graph={activeStateAccessGraph}
                          isLoading={isLoadingStateAccessGraph}
                          error={stateAccessGraphError}
                          onRetry={retryStateAccessGraph}
                          moveModule={moveModule}
                          movePackage={movePackage}
                          rootPath={rootPath}
                          signature={signature}
                        />
                      ) : null}
                    </FunctionSignatureCard>
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
    </section>
  );
}

function FunctionSignatureCard({
  children,
  isOpen,
  onToggle,
  signature,
}: {
  children?: React.ReactNode;
  isOpen: boolean;
  onToggle: () => void;
  signature: MoveFunctionSignature;
}) {
  const source =
    isOpen && signature.body ? signature.body : signature.signature;

  return (
    <article className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] p-4">
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
          <Badge tone={visibilityTone(signature.visibility)}>
            {signature.visibility}
          </Badge>
          {signature.isEntry ? <Badge tone="entry">entry</Badge> : null}
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
      {children}
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

function FunctionStateAccessGraphPanel({
  error,
  graph,
  isLoading,
  onRetry,
  moveModule,
  movePackage,
  rootPath,
  signature,
}: {
  error: string | null;
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
      <div className="mt-4 flex items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-3 text-xs text-muted-foreground">
        <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
        Building state access graph from the Move AST...
      </div>
    );
  }

  if (error && !graph) {
    return (
      <div className="mt-4 flex min-w-0 items-center justify-between gap-3 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-3 text-xs text-red-200">
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
      <div className="mt-4 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-3 text-xs text-muted-foreground">
        {rootPath
          ? "State graph is not loaded yet."
          : "State graph is unavailable in this view."}
      </div>
    );
  }

  if (!summary.stateNodes.length) {
    return (
      <div className="mt-4 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-3 text-xs text-muted-foreground">
        No package state access was found for this function in the current AST
        graph.
      </div>
    );
  }

  return (
    <div className="mt-4 overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <div className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <GitBranch
            className="size-3.5 shrink-0 text-cyan-300"
            aria-hidden="true"
          />
          <span className="truncate text-xs font-semibold text-foreground">
            State access
          </span>
        </div>
        <span className="shrink-0 rounded bg-[var(--app-subtle)] px-2 py-0.5 text-[11px] text-muted-foreground">
          {summary.stateNodes.length} touched
        </span>
      </div>
      <StateAccessDiagram
        moveModule={moveModule}
        signature={signature}
        summary={summary}
      />
      <div className="grid gap-2 border-t border-[color:var(--app-border)] px-3 py-3">
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
    </div>
  );
}

function StateAccessDiagram({
  moveModule,
  signature,
  summary,
}: {
  moveModule: MoveModule;
  signature: MoveFunctionSignature;
  summary: FunctionStateAccessSummary;
}) {
  const model = React.useMemo(
    () => buildStateAccessDiagramModel(summary),
    [summary],
  );
  const functionBox = {
    height: 118,
    width: 292,
    x: 34,
    y: Math.max(42, model.height / 2 - 59),
  };
  const startX = functionBox.x + functionBox.width;
  const startY = functionBox.y + functionBox.height / 2;
  const stateX = 482;
  const stateWidth = 396;

  return (
    <div className="overflow-x-auto bg-[var(--app-window)]">
      <svg
        aria-label={`AST state access diagram for ${signature.name}`}
        className="block min-w-[820px]"
        role="img"
        viewBox={`0 0 920 ${model.height}`}
      >
        <defs>
          <filter
            id="state-access-diagram-shadow"
            x="-18%"
            y="-18%"
            width="136%"
            height="136%"
          >
            <feDropShadow dx="0" dy="3" stdDeviation="3" floodOpacity="0.22" />
          </filter>
          <linearGradient id="state-access-function-fill" x1="0" x2="1" y1="0" y2="1">
            <stop offset="0%" stopColor="rgb(15 23 42)" />
            <stop offset="100%" stopColor="rgb(8 13 24)" />
          </linearGradient>
        </defs>

        <rect
          x="1"
          y="1"
          width="918"
          height={model.height - 2}
          rx="10"
          fill="rgb(3 7 18 / 0.34)"
          stroke="rgb(148 163 184 / 0.12)"
        />
        <path
          d={`M ${functionBox.x + 18} ${functionBox.y - 28} L ${functionBox.x + 18} ${functionBox.y}`}
          fill="none"
          stroke="rgb(71 85 105 / 0.9)"
          strokeWidth="1.2"
        />
        <rect
          x={functionBox.x}
          y={functionBox.y - 68}
          width="292"
          height="40"
          rx="7"
          fill="rgb(15 23 42 / 0.55)"
          stroke="rgb(71 85 105 / 0.72)"
        />
        <text
          x={functionBox.x + 16}
          y={functionBox.y - 44}
          fill="rgb(148 163 184)"
          fontSize="12"
          fontWeight="600"
        >
          {truncateLabel(moveModule.name, 28)}
        </text>

        {model.groups.map((group) => {
          const groupCenterX = stateX + stateWidth / 2;

          return (
            <g key={`source-${group.id}`}>
              <path
                d={`M ${groupCenterX} ${group.y - 20} L ${groupCenterX} ${group.y}`}
                fill="none"
                stroke="rgb(71 85 105 / 0.72)"
                strokeWidth="1.2"
              />
              <rect
                x={stateX}
                y={group.y - 60}
                width={stateWidth}
                height="40"
                rx="7"
                fill="rgb(15 23 42 / 0.55)"
                stroke="rgb(71 85 105 / 0.72)"
              />
              <text
                x={stateX + 16}
                y={group.y - 36}
                fill="rgb(148 163 184)"
                fontSize="12"
                fontWeight="600"
              >
                {truncateLabel(group.subtitle, 42)}
              </text>
            </g>
          );
        })}

        {model.groups.flatMap((group) =>
          group.rows.map((row) => {
            const targetX = stateX;
            const targetY = row.y + 18;
            const stroke = accessKindColor(primaryAccessKind(row.accessKinds));
            const midX = startX + Math.max(70, (targetX - startX) * 0.44);

            return (
              <g key={`edge-${row.id}`}>
                <path
                  d={`M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${targetY}, ${targetX} ${targetY}`}
                  fill="none"
                  stroke={stroke}
                  strokeDasharray={row.direct ? undefined : "5 6"}
                  strokeLinecap="round"
                  strokeOpacity={row.direct ? "0.92" : "0.56"}
                  strokeWidth={row.direct ? "2" : "1.4"}
                />
                <path
                  d={`M ${targetX - 8} ${targetY - 5} L ${targetX} ${targetY} L ${targetX - 8} ${targetY + 5}`}
                  fill="none"
                  stroke={stroke}
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeOpacity={row.direct ? "0.92" : "0.6"}
                  strokeWidth={row.direct ? "2" : "1.4"}
                />
              </g>
            );
          }),
        )}

        <g filter="url(#state-access-diagram-shadow)">
          <rect
            x={functionBox.x}
            y={functionBox.y}
            width={functionBox.width}
            height={functionBox.height}
            rx="8"
            fill="url(#state-access-function-fill)"
            stroke="rgb(34 211 238 / 0.68)"
            strokeWidth="1.4"
          />
          <line
            x1={functionBox.x}
            x2={functionBox.x + functionBox.width}
            y1={functionBox.y + 46}
            y2={functionBox.y + 46}
            stroke="rgb(34 211 238 / 0.22)"
          />
          <text
            x={functionBox.x + 18}
            y={functionBox.y + 28}
            fill="rgb(226 232 240)"
            fontSize="16"
            fontWeight="700"
          >
            {truncateLabel(signature.name, 24)}
          </text>
          <text
            x={functionBox.x + 18}
            y={functionBox.y + 72}
            fill="rgb(148 163 184)"
            fontSize="12"
            fontWeight="600"
          >
            selected Move function
          </text>
          <text
            x={functionBox.x + 18}
            y={functionBox.y + 94}
            fill="rgb(100 116 139)"
            fontSize="11"
          >
            {truncateLabel(`${moveModule.name}::${signature.name}()`, 36)}
          </text>
        </g>

        {model.groups.map((group) => (
          <g filter="url(#state-access-diagram-shadow)" key={group.id}>
            <rect
              x={stateX}
              y={group.y}
              width={stateWidth}
              height={group.height}
              rx="8"
              fill="rgb(15 23 42 / 0.94)"
              stroke="rgb(99 102 241 / 0.64)"
              strokeWidth="1.35"
            />
            <line
              x1={stateX}
              x2={stateX + stateWidth}
              y1={group.y + 46}
              y2={group.y + 46}
              stroke="rgb(99 102 241 / 0.3)"
            />
            <text
              x={stateX + 20}
              y={group.y + 28}
              fill="rgb(226 232 240)"
              fontSize="15"
              fontWeight="700"
            >
              {truncateLabel(group.title, 34)}
            </text>
            {group.rows.map((row, index) => {
              const rowY = group.y + 46 + index * 38;
              const accessKinds = row.accessKinds.map(accessKindLabel).join(" / ");
              const stroke = accessKindColor(primaryAccessKind(row.accessKinds));

              return (
                <g key={row.id}>
                  <rect
                    x={stateX + 1}
                    y={rowY}
                    width={stateWidth - 2}
                    height="38"
                    fill={row.direct ? "rgb(15 23 42 / 0.72)" : "rgb(15 23 42 / 0.45)"}
                  />
                  <line
                    x1={stateX}
                    x2={stateX + stateWidth}
                    y1={rowY}
                    y2={rowY}
                    stroke="rgb(99 102 241 / 0.22)"
                  />
                  <circle cx={stateX + 18} cy={rowY + 19} r="4" fill={stroke} />
                  <text
                    x={stateX + 34}
                    y={rowY + 23}
                    fill="rgb(226 232 240)"
                    fontSize="13"
                    fontWeight="600"
                  >
                    {truncateLabel(row.label, 24)}
                  </text>
                  <text
                    x={stateX + stateWidth - 18}
                    y={rowY + 23}
                    fill={row.direct ? "rgb(34 211 238)" : "rgb(148 163 184)"}
                    fontSize="11"
                    fontWeight="700"
                    textAnchor="end"
                  >
                    {truncateLabel(accessKinds, 18)}
                  </text>
                </g>
              );
            })}
          </g>
        ))}
      </svg>
    </div>
  );
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
  title,
}: {
  children: React.ReactNode;
  count: number;
  emptyText: string;
  title: string;
}) {
  return (
    <section>
      <div className="mb-3 flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold text-foreground">{title}</h3>
        <span className="rounded bg-[var(--app-subtle)] px-2 py-0.5 text-xs text-muted-foreground">
          {count}
        </span>
      </div>
      {count ? (
        children
      ) : (
        <div className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-4 py-5 text-sm text-muted-foreground">
          {emptyText}
        </div>
      )}
    </section>
  );
}

function Badge({
  children,
  tone,
}: {
  children: string;
  tone: "ability" | "entry" | "private" | "public";
}) {
  return (
    <span
      className={cn(
        "rounded px-2 py-0.5 text-xs font-medium",
        tone === "ability" && "bg-sky-500/10 text-sky-300",
        tone === "public" && "bg-emerald-500/10 text-emerald-300",
        tone === "private" && "bg-muted text-muted-foreground",
        tone === "entry" && "bg-primary/15 text-primary",
      )}
    >
      {children}
    </span>
  );
}

function visibilityTone(visibility: string) {
  return visibility === "private" ? "private" : "public";
}
