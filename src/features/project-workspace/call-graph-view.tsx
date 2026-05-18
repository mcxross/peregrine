import {
  Background,
  BaseEdge,
  Controls,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  NodeToolbar,
  Position,
  ReactFlow,
  getSmoothStepPath,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  ArrowUpRight,
  Boxes,
  Braces,
  FileCode2,
  FunctionSquare,
  Maximize2,
  Minimize2,
  RadioTower,
  ShieldAlert,
  Workflow,
} from "lucide-react";
import React from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type {
  MoveCallGraph,
  MoveCallGraphEdge,
  MoveCallGraphNode,
  MovePackage,
  MoveSourceSpan,
  MoveUnresolvedCall,
} from "@/features/empty-project/filesystem-tree";
import { displayMovePackageName } from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

type CallGraphViewProps = {
  className?: string;
  graph: MoveCallGraph;
  movePackage: MovePackage | null;
  onOpenSourceLocation?: (location: { filePath: string; line: number }) => void;
  packageName: string;
};

type CallGraphViewMode = "functions" | "modules";
type CallNodeKind = "entry" | "external" | "internal" | "module" | "public" | "transaction" | "unresolved";

type RenderCallNode = {
  column: number;
  groupLabel: string | null;
  id: string;
  kind: CallNodeKind;
  label: string;
  mode: CallGraphViewMode;
  moduleName: string;
  node: MoveCallGraphNode | null;
  roleLabel: string;
  showGroupLabel: boolean;
  sourceLocation: MoveSourceSpan | null;
  subtitle: string;
  tags: string[];
  x: number;
  y: number;
};

type RenderCallEdge = {
  edge: MoveCallGraphEdge;
  id: string;
  source: string;
  target: string;
};

type CallRenderGraph = {
  edgeCount: number;
  edges: RenderCallEdge[];
  localFunctionCount: number;
  mode: CallGraphViewMode;
  nodeCount: number;
  nodes: RenderCallNode[];
  packageLabel: string;
  unresolvedCalls: MoveUnresolvedCall[];
};

type ModuleCallSummary = {
  entryCount: number;
  externalCallCount: number;
  functionCount: number;
  id: string;
  internalCount: number;
  kind: CallNodeKind;
  label: string;
  moduleName: string;
  packageLabel: string;
  publicCount: number;
  representativeNode: MoveCallGraphNode | null;
  selfCallCount: number;
  txCount: number;
  unresolvedCallCount: number;
};

type CallFlowNodeData = RenderCallNode & {
  color: string;
  dimmed: boolean;
  incoming: number;
  onOpenSource: (span: MoveSourceSpan | null) => void;
  onSelectNode: (id: string) => void;
  outgoing: number;
  selected: boolean;
};

type CallFlowEdgeData = {
  active: boolean;
  color: string;
  label: string | null;
};

const NODE_TYPES = {
  call: CallGraphNode,
};

const EDGE_TYPES = {
  call: CallGraphEdge,
};

const ENTRY_COLOR = "#38bdf8";
const TRANSACTION_COLOR = "#22d3ee";
const PUBLIC_COLOR = "#34d399";
const INTERNAL_COLOR = "#94a3b8";
const EXTERNAL_COLOR = "#a78bfa";
const UNRESOLVED_COLOR = "#f87171";
const METHOD_COLOR = "#eab308";
const MACRO_COLOR = "#fb7185";
const CALL_NODE_WIDTH = 250;
const CALL_MODULE_NODE_WIDTH = 280;
const CALL_COLUMN_GAP = 116;
const CALL_ROW_GAP = 148;
const CALL_MODULE_ROW_GAP = 150;
const MAX_CALL_NODES = 120;
const MAX_CALL_EDGES = 180;

export function CallGraphView({
  className = "h-72 rounded-md border",
  graph,
  movePackage,
  onOpenSourceLocation,
  packageName,
}: CallGraphViewProps) {
  const [hoveredNodeId, setHoveredNodeId] = React.useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = React.useState<string | null>(null);
  const [hoveredEdgeId, setHoveredEdgeId] = React.useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = React.useState<string | null>(null);
  const [isFullscreen, setIsFullscreen] = React.useState(false);
  const [viewMode, setViewMode] = React.useState<CallGraphViewMode>("modules");
  const renderGraph = React.useMemo(
    () => buildPackageCallRenderGraph(graph, movePackage, packageName, viewMode),
    [graph, movePackage, packageName, viewMode],
  );
  const stats = React.useMemo(() => callGraphStats(renderGraph.edges), [renderGraph.edges]);

  React.useEffect(() => {
    setHoveredEdgeId(null);
    setSelectedEdgeId(null);
    setHoveredNodeId(null);
    setSelectedNodeId(null);
  }, [viewMode]);

  React.useEffect(() => {
    if (!isFullscreen) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsFullscreen(false);
      }
    };

    window.addEventListener("keydown", onKeyDown);

    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isFullscreen]);

  const selectedEdge = React.useMemo(
    () => renderGraph.edges.find((edge) => edge.id === selectedEdgeId) ?? null,
    [renderGraph.edges, selectedEdgeId],
  );
  const selectedNode = React.useMemo(
    () => renderGraph.nodes.find((node) => node.id === selectedNodeId) ?? null,
    [renderGraph.nodes, selectedNodeId],
  );
  const focusedNodeIds = React.useMemo(() => {
    const ids = new Set<string>();

    if (selectedEdge) {
      ids.add(selectedEdge.source);
      ids.add(selectedEdge.target);
      return ids;
    }

    if (selectedNodeId) {
      ids.add(selectedNodeId);
      for (const edge of renderGraph.edges) {
        if (edge.source === selectedNodeId || edge.target === selectedNodeId) {
          ids.add(edge.source);
          ids.add(edge.target);
        }
      }
    } else if (hoveredNodeId) {
      ids.add(hoveredNodeId);
      for (const edge of renderGraph.edges) {
        if (edge.source === hoveredNodeId || edge.target === hoveredNodeId) {
          ids.add(edge.source);
          ids.add(edge.target);
        }
      }
    }

    return ids;
  }, [hoveredNodeId, renderGraph.edges, selectedEdge, selectedNodeId]);
  const openSource = React.useCallback(
    (span: MoveSourceSpan | null) => {
      if (!span || !onOpenSourceLocation) {
        return;
      }

      onOpenSourceLocation({ filePath: span.filePath, line: span.startLine || 1 });
    },
    [onOpenSourceLocation],
  );

  if (!renderGraph.nodes.length) {
    return (
      <CallGraphEmptyState
        className={className}
        packageName={movePackage?.name ?? packageName}
      />
    );
  }

  const flowNodes = renderGraph.nodes.map<Node<CallFlowNodeData>>((node) => {
    const selected = node.id === selectedNodeId;
    const active = !focusedNodeIds.size || focusedNodeIds.has(node.id);

    return {
      id: node.id,
      position: { x: node.x, y: node.y },
      type: "call",
      zIndex: selected ? 1000 : node.kind === "entry" || node.kind === "transaction" || node.kind === "module" ? 40 : 10,
      data: {
        ...node,
        color: selected ? ENTRY_COLOR : callNodeColor(node.kind),
        dimmed: !active,
        incoming: stats.incoming.get(node.id) ?? 0,
        onOpenSource: openSource,
        onSelectNode: (nodeId) => {
          setSelectedEdgeId(null);
          setSelectedNodeId(nodeId);
        },
        outgoing: stats.outgoing.get(node.id) ?? 0,
        selected,
      },
    };
  });
  const flowEdges = renderGraph.edges.map<Edge<CallFlowEdgeData>>((edge) => {
    const selected = edge.id === selectedEdgeId;
    const hovered = edge.id === hoveredEdgeId;
    const focusedByNode =
      selectedNodeId
        ? edge.source === selectedNodeId || edge.target === selectedNodeId
        : hoveredNodeId
          ? edge.source === hoveredNodeId || edge.target === hoveredNodeId
          : false;
    const active = selected || hovered || focusedByNode || !focusedNodeIds.size;
    const color = callEdgeColor(edge.edge);

    return {
      id: edge.id,
      source: edge.source,
      target: edge.target,
      data: {
        active,
        color,
        label: selected || hovered || focusedByNode ? callEdgeLabel(edge.edge) : null,
      },
      markerEnd: {
        type: MarkerType.ArrowClosed,
        color,
      },
      style: {
        opacity: active ? selected || hovered ? 0.98 : 0.72 : 0.16,
        stroke: color,
        strokeDasharray: callEdgeDash(edge.edge),
        strokeWidth: selected || hovered ? 2.6 : active ? 1.7 : 1.1,
      },
      type: "call",
    };
  });
  const flowKey = `${renderGraph.mode}:${renderGraph.packageLabel}:${flowNodes.length}:${flowEdges.length}:${isFullscreen ? "fullscreen" : "inline"}`;

  return (
    <div
      className={cn(
        className,
        "relative overflow-hidden border-[color:var(--app-border)] bg-[var(--app-surface)]",
        isFullscreen && "fixed bottom-3 left-3 right-3 top-[calc(58px+0.75rem)] z-[90] rounded-lg border bg-[var(--app-window)] shadow-2xl shadow-black/60",
      )}
    >
      <ReactFlow
        key={flowKey}
        colorMode="dark"
        edgeTypes={EDGE_TYPES}
        edges={flowEdges}
        edgesFocusable={false}
        fitView
        fitViewOptions={{ padding: 0.16 }}
        maxZoom={1.85}
        minZoom={0.24}
        nodeTypes={NODE_TYPES}
        nodes={flowNodes}
        nodesDraggable={false}
        nodesFocusable={false}
        onEdgeClick={(_, edge) => {
          setSelectedNodeId(null);
          setSelectedEdgeId(edge.id);
        }}
        onEdgeMouseEnter={(_, edge) => setHoveredEdgeId(edge.id)}
        onEdgeMouseLeave={() => setHoveredEdgeId(null)}
        onNodeClick={(_, node) => {
          setSelectedEdgeId(null);
          setSelectedNodeId(node.id);
        }}
        onNodeMouseEnter={(_, node) => setHoveredNodeId(node.id)}
        onNodeMouseLeave={() => setHoveredNodeId(null)}
        onPaneClick={() => {
          setSelectedEdgeId(null);
          setSelectedNodeId(null);
        }}
        proOptions={{ hideAttribution: true }}
      >
        <Background color="var(--border)" gap={18} size={1} />
        <Controls
          className="!bg-background/90 !shadow-none [&_button]:!border-border [&_button]:!bg-background [&_button]:!text-foreground"
          position="bottom-right"
          showInteractive={false}
        />
      </ReactFlow>

      <CallGraphLegend mode={viewMode} />
      <CallGraphModeControls mode={viewMode} onModeChange={setViewMode} />
      <CallGraphContextBar
        edge={selectedEdge}
        graph={renderGraph}
        node={selectedNode}
      />
      <Button
        aria-label={isFullscreen ? "Exit fullscreen call graph" : "Open call graph fullscreen"}
        className="absolute right-3 top-3 z-20 size-8 border border-[color:var(--app-border)] bg-background/90 text-muted-foreground shadow-sm backdrop-blur hover:bg-[var(--app-elevated)] hover:text-foreground"
        onClick={() => setIsFullscreen((current) => !current)}
        size="icon-sm"
        title={isFullscreen ? "Exit fullscreen" : "Open fullscreen"}
        type="button"
        variant="ghost"
      >
        {isFullscreen ? (
          <Minimize2 className="size-4" aria-hidden="true" />
        ) : (
          <Maximize2 className="size-4" aria-hidden="true" />
        )}
      </Button>
      {isFullscreen ? (
        <div className="pointer-events-none absolute left-1/2 top-3 z-20 -translate-x-1/2 rounded-md border border-[color:var(--app-border)] bg-background/88 px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground shadow-sm backdrop-blur">
          Call Graph · Fullscreen
        </div>
      ) : null}
    </div>
  );
}

export default CallGraphView;

function CallGraphNode({ data }: NodeProps<Node<CallFlowNodeData>>) {
  const Icon = callNodeIcon(data.kind);
  const sourceDisabled = !data.sourceLocation;

  return (
    <div
      className={cn(
        "group relative grid grid-rows-[auto_auto_minmax(0,1fr)] rounded-md border bg-[linear-gradient(135deg,rgba(255,255,255,0.055),rgba(255,255,255,0.012))] px-3 py-2.5 text-left shadow-sm backdrop-blur transition",
        data.mode === "modules" ? "h-[118px] w-[280px]" : "h-[112px] w-[250px]",
        data.dimmed && "opacity-35",
        data.selected && "shadow-[0_0_28px_rgba(56,189,248,0.24)]",
      )}
      style={{
        borderColor: data.color,
        boxShadow: data.selected ? `0 0 0 1px ${data.color}, 0 0 28px color-mix(in srgb, ${data.color} 28%, transparent)` : undefined,
      }}
    >
      {data.showGroupLabel && data.groupLabel ? (
        <div className="absolute -top-5 left-1 text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground/65">
          {data.groupLabel}
        </div>
      ) : null}
      <Handle
        className="!size-1.5 !border-0"
        position={Position.Left}
        style={{ background: data.color }}
        type="target"
      />
      <Handle
        className="!size-1.5 !border-0"
        position={Position.Right}
        style={{ background: data.color }}
        type="source"
      />
      <div className="flex min-w-0 items-start justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <span
            className="grid size-6 shrink-0 place-items-center rounded-md border bg-background/50"
            style={{ borderColor: data.color, color: data.color }}
          >
            <Icon className="size-3.5" aria-hidden="true" />
          </span>
          <h3 className="min-w-0 truncate text-sm font-semibold text-foreground">
            {data.label}
          </h3>
        </div>
        <span className="shrink-0 rounded bg-[var(--app-subtle)] px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground">
          {data.roleLabel}
        </span>
      </div>
      <div className="mt-1 min-w-0 truncate text-[11px] text-muted-foreground">
        {data.subtitle}
      </div>
      <div className="mt-2 flex min-w-0 flex-wrap content-start gap-1 overflow-hidden">
        {data.tags.slice(0, 3).map((tag) => (
          <Badge
            className="max-w-[7rem] truncate rounded px-1.5 py-0 text-[10px] font-semibold"
            key={tag}
            style={{
              backgroundColor: "color-mix(in srgb, currentColor 13%, transparent)",
              borderColor: "color-mix(in srgb, currentColor 18%, transparent)",
              color: data.color,
            }}
            variant="secondary"
          >
            {tag}
          </Badge>
        ))}
      </div>
      <div className="mt-auto flex min-w-0 items-center justify-between border-t border-[color:var(--app-border)] pt-1.5 text-[11px] text-muted-foreground">
        <span className="truncate">{data.incoming} in</span>
        <span className="truncate">{data.outgoing} out</span>
      </div>
      <NodeToolbar isVisible={data.selected} position={Position.Bottom}>
        <div className="flex items-center gap-1 rounded-md border border-[color:var(--app-border)] bg-background/95 p-1 shadow-sm">
          <button
            className="grid size-7 place-items-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
            disabled={sourceDisabled}
            onClick={(event) => {
              event.stopPropagation();
              data.onOpenSource(data.sourceLocation);
            }}
            title={sourceDisabled ? "Source unavailable" : "Open source"}
            type="button"
          >
            <FileCode2 className="size-3.5" aria-hidden="true" />
          </button>
        </div>
      </NodeToolbar>
    </div>
  );
}

function CallGraphEdge({
  markerEnd,
  sourcePosition,
  sourceX,
  sourceY,
  style,
  targetPosition,
  targetX,
  targetY,
  data,
}: EdgeProps) {
  const edgeData = data as CallFlowEdgeData | undefined;
  const [edgePath, labelX, labelY] = getSmoothStepPath({
    borderRadius: 16,
    sourcePosition,
    sourceX,
    sourceY,
    targetPosition,
    targetX,
    targetY,
  });

  return (
    <>
      <BaseEdge markerEnd={markerEnd} path={edgePath} style={style} />
      {edgeData?.label ? (
        <EdgeLabelRenderer>
          <div
            className="nodrag nopan pointer-events-none absolute z-[1000] rounded border border-[color:var(--app-border)] bg-background/92 px-1.5 py-0.5 text-[10px] font-semibold leading-none shadow-sm backdrop-blur"
            style={{
              color: edgeData.color,
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
            }}
          >
            {edgeData.label}
          </div>
        </EdgeLabelRenderer>
      ) : null}
    </>
  );
}

function CallGraphLegend({ mode }: { mode: CallGraphViewMode }) {
  const title = mode === "modules" ? "Module call map" : "Function call graph";
  const subtitle = mode === "modules"
    ? "modules aggregate internal, external, and unresolved calls"
    : "entry/public functions call internal or external targets";

  return (
    <div className="pointer-events-none absolute left-3 top-3 z-20 rounded-md border border-[color:var(--app-border)] bg-background/82 px-2.5 py-2 text-[11px] leading-tight text-muted-foreground shadow-sm backdrop-blur">
      <div className="font-medium text-foreground">{title}</div>
      <div className="mt-1">{subtitle}</div>
      <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-1">
        <LegendDot color={ENTRY_COLOR} label={mode === "modules" ? "entry module" : "entry"} />
        <LegendDot color={PUBLIC_COLOR} label="public" />
        <LegendDot color={EXTERNAL_COLOR} label="external" />
        <LegendDot color={UNRESOLVED_COLOR} label="unresolved" />
      </div>
    </div>
  );
}

function CallGraphModeControls({
  mode,
  onModeChange,
}: {
  mode: CallGraphViewMode;
  onModeChange: (mode: CallGraphViewMode) => void;
}) {
  const options: { label: string; mode: CallGraphViewMode }[] = [
    { label: "Modules", mode: "modules" },
    { label: "Functions", mode: "functions" },
  ];

  return (
    <div className="absolute left-3 top-[7.5rem] z-20 inline-flex overflow-hidden rounded-md border border-[color:var(--app-border)] bg-background/82 p-0.5 text-[11px] font-semibold shadow-sm backdrop-blur">
      {options.map((option) => (
        <button
          className={cn(
            "rounded px-2.5 py-1 text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground",
            mode === option.mode && "bg-cyan-500/14 text-cyan-200",
          )}
          key={option.mode}
          onClick={() => onModeChange(option.mode)}
          type="button"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function LegendDot({ color, label }: { color: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      <span className="size-1.5 rounded-full" style={{ backgroundColor: color }} />
      {label}
    </span>
  );
}

function CallGraphContextBar({
  edge,
  graph,
  node,
}: {
  edge: RenderCallEdge | null;
  graph: CallRenderGraph;
  node: RenderCallNode | null;
}) {
  const details = edge
    ? [
      "Call Graph",
      `${compactCallEndpoint(edge.source)} calls ${compactCallEndpoint(edge.target)}`,
      edge.edge.callKind,
      edge.edge.confidence,
    ]
    : node
      ? [
        "Call Graph",
        node.label,
        node.roleLabel,
        `${node.tags.join(", ") || "function"}`,
      ]
      : [
        "Call Graph",
        graph.packageLabel,
        graph.mode === "modules" ? `${graph.nodeCount} modules / targets` : `${graph.localFunctionCount} functions`,
        `${graph.edgeCount} calls`,
        graph.unresolvedCalls.length ? `${graph.unresolvedCalls.length} unresolved` : "resolved view",
      ];

  return (
    <div className="pointer-events-none absolute bottom-3 left-3 z-20 inline-flex max-w-[calc(100%-7rem)] items-center gap-1.5 truncate rounded border border-[color:var(--app-border)] bg-background/82 px-2.5 py-1.5 text-[11px] font-medium text-muted-foreground shadow-sm backdrop-blur">
      {details.map((detail, index) => (
        <React.Fragment key={`${detail}-${index}`}>
          {index > 0 ? <span className="text-muted-foreground/45">·</span> : null}
          <span className={cn("truncate", index === 0 && "text-foreground")}>{detail}</span>
        </React.Fragment>
      ))}
    </div>
  );
}

function CallGraphEmptyState({
  className,
  packageName,
}: {
  className: string;
  packageName: string;
}) {
  return (
    <div className={cn(className, "grid place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-6 text-center")}>
      <div className="max-w-sm">
        <Workflow className="mx-auto size-6 text-muted-foreground" aria-hidden="true" />
        <h3 className="mt-3 text-sm font-semibold text-foreground">No call graph available</h3>
        <p className="mt-1 text-sm leading-6 text-muted-foreground">
          No function call relationships were found for {displayMovePackageName(packageName)}.
        </p>
      </div>
    </div>
  );
}

function buildPackageCallRenderGraph(
  graph: MoveCallGraph,
  movePackage: MovePackage | null,
  packageName: string,
  mode: CallGraphViewMode,
): CallRenderGraph {
  if (mode === "modules") {
    return buildPackageModuleCallRenderGraph(graph, movePackage, packageName);
  }

  return buildPackageFunctionCallRenderGraph(graph, movePackage, packageName);
}

function buildPackageFunctionCallRenderGraph(
  graph: MoveCallGraph,
  movePackage: MovePackage | null,
  packageName: string,
): CallRenderGraph {
  const packagePath = movePackage?.path ?? null;
  const packageLabel = movePackage?.name ?? packageName;
  const displayPackageLabel = displayMovePackageName(packageLabel);
  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  const localNodes = graph.nodes
    .filter((node) => isPackageCallNode(node, packagePath, packageLabel))
    .sort(callNodeSort);
  const localIds = new Set(localNodes.map((node) => node.id));
  const outgoingEdges = graph.edges
    .filter((edge) => localIds.has(edge.source))
    .sort(callEdgeSort)
    .slice(0, MAX_CALL_EDGES);
  const visibleIds = new Set(localNodes.map((node) => node.id));

  for (const edge of outgoingEdges) {
    visibleIds.add(edge.target);
  }

  const visibleRawNodes = [...visibleIds]
    .map((id) => nodeById.get(id))
    .filter((node): node is MoveCallGraphNode => Boolean(node))
    .slice(0, MAX_CALL_NODES);
  const visibleNodeIds = new Set(visibleRawNodes.map((node) => node.id));
  const visibleEdges = outgoingEdges.filter((edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target));
  const depths = callNodeDepths(localNodes, visibleEdges);
  const externalColumn = Math.max(2, Math.min(4, Math.max(...[...depths.values(), 1]) + 1));
  const unresolvedColumn = externalColumn + 1;
  const columnRows = new Map<number, RenderCallNode[]>();
  const renderNodes = visibleRawNodes.map((node) => {
    const kind = callNodeKind(node, localIds);
    const column = kind === "external"
      ? externalColumn
      : kind === "unresolved"
        ? unresolvedColumn
        : Math.min(depths.get(node.id) ?? (node.isEntry || node.isTransactionCallable ? 0 : 1), externalColumn - 1);
    const renderNode: RenderCallNode = {
      column,
      groupLabel: callColumnLabel(kind, column),
      id: node.id,
      kind,
      label: node.functionName,
      mode: "functions",
      moduleName: node.moduleName,
      node,
      roleLabel: callNodeRoleLabel(kind, node),
      showGroupLabel: false,
      sourceLocation: node.span,
      subtitle: `${displayMovePackageName(node.packageName ?? node.address ?? "unresolved")}::${node.moduleName}`,
      tags: callNodeTags(kind, node),
      x: 80 + column * (CALL_NODE_WIDTH + CALL_COLUMN_GAP),
      y: 0,
    };
    const rows = columnRows.get(column) ?? [];
    rows.push(renderNode);
    columnRows.set(column, rows);
    return renderNode;
  });

  for (const [column, nodes] of columnRows) {
    nodes.sort((left, right) => callRenderNodeSort(left, right));
    nodes.forEach((node, index) => {
      node.y = index * CALL_ROW_GAP;
      node.showGroupLabel = index === 0;
      node.groupLabel = node.groupLabel ?? `Layer ${column}`;
    });
  }

  return {
    edgeCount: visibleEdges.reduce((total, edge) => total + edge.callCount, 0),
    edges: visibleEdges.map((edge) => ({
      edge,
      id: callEdgeId(edge),
      source: edge.source,
      target: edge.target,
    })),
    localFunctionCount: localNodes.length,
    mode: "functions",
    nodeCount: renderNodes.length,
    nodes: renderNodes,
    packageLabel: displayPackageLabel,
    unresolvedCalls: graph.unresolvedCalls.filter((call) => localIds.has(call.source)),
  };
}

function buildPackageModuleCallRenderGraph(
  graph: MoveCallGraph,
  movePackage: MovePackage | null,
  packageName: string,
): CallRenderGraph {
  const packagePath = movePackage?.path ?? null;
  const packageLabel = movePackage?.name ?? packageName;
  const displayPackageLabel = displayMovePackageName(packageLabel);
  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  const localNodes = graph.nodes
    .filter((node) => isPackageCallNode(node, packagePath, packageLabel))
    .sort(callNodeSort);
  const localIds = new Set(localNodes.map((node) => node.id));
  const summaries = new Map<string, ModuleCallSummary>();
  const aggregatedEdges = new Map<string, MoveCallGraphEdge>();

  const ensureSummary = (
    id: string,
    defaults: Pick<ModuleCallSummary, "kind" | "label" | "moduleName" | "packageLabel" | "representativeNode">,
  ) => {
    const existing = summaries.get(id);

    if (existing) {
      return existing;
    }

    const summary: ModuleCallSummary = {
      entryCount: 0,
      externalCallCount: 0,
      functionCount: 0,
      id,
      internalCount: 0,
      kind: defaults.kind,
      label: defaults.label,
      moduleName: defaults.moduleName,
      packageLabel: defaults.packageLabel,
      publicCount: 0,
      representativeNode: defaults.representativeNode,
      selfCallCount: 0,
      txCount: 0,
      unresolvedCallCount: 0,
    };
    summaries.set(id, summary);
    return summary;
  };

  for (const node of localNodes) {
    const id = callModuleNodeId(node.moduleName);
    const summary = ensureSummary(id, {
      kind: node.isEntry || node.isTransactionCallable ? "entry" : "module",
      label: node.moduleName,
      moduleName: node.moduleName,
      packageLabel: displayPackageLabel,
      representativeNode: node,
    });

    summary.functionCount += 1;
    summary.entryCount += node.isEntry ? 1 : 0;
    summary.txCount += node.isTransactionCallable ? 1 : 0;
    summary.publicCount += node.visibility === "public" || node.visibility === "public(package)" ? 1 : 0;
    summary.internalCount += node.visibility === "private" || node.visibility === "friend" ? 1 : 0;
    summary.kind = summary.entryCount || summary.txCount ? "entry" : "module";
  }

  const localOutgoingEdges = graph.edges
    .filter((edge) => localIds.has(edge.source))
    .sort(callEdgeSort);

  for (const edge of localOutgoingEdges) {
    const sourceNode = nodeById.get(edge.source);

    if (!sourceNode) {
      continue;
    }

    const sourceId = callModuleNodeId(sourceNode.moduleName);
    const sourceSummary = summaries.get(sourceId);

    if (!sourceSummary) {
      continue;
    }

    const targetNode = nodeById.get(edge.target);
    let targetId: string;
    let targetKind: CallNodeKind;
    let targetLabel: string;
    let targetModuleName: string;
    let targetPackageLabel: string;
    let isExternal = edge.isExternal;
    let isResolved = edge.isResolved;

    if (!edge.isResolved || !targetNode || edge.target.startsWith("unresolved:call:")) {
      targetId = CALL_UNRESOLVED_NODE_ID;
      targetKind = "unresolved";
      targetLabel = "Unresolved calls";
      targetModuleName = "unresolved";
      targetPackageLabel = "call target missing";
      isExternal = false;
      isResolved = false;
      sourceSummary.unresolvedCallCount += edge.callCount;
    } else if (localIds.has(targetNode.id)) {
      targetId = callModuleNodeId(targetNode.moduleName);
      targetKind = targetNode.isEntry || targetNode.isTransactionCallable ? "entry" : "module";
      targetLabel = targetNode.moduleName;
      targetModuleName = targetNode.moduleName;
      targetPackageLabel = displayPackageLabel;
    } else {
      targetId = callExternalModuleNodeId(targetNode.packageName, targetNode.moduleName, targetNode.address);
      targetKind = "external";
      targetLabel = targetNode.moduleName;
      targetModuleName = targetNode.moduleName;
      targetPackageLabel = displayMovePackageName(targetNode.packageName ?? targetNode.address ?? "external");
      isExternal = true;
      sourceSummary.externalCallCount += edge.callCount;
    }

    ensureSummary(targetId, {
      kind: targetKind,
      label: targetLabel,
      moduleName: targetModuleName,
      packageLabel: targetPackageLabel,
      representativeNode: targetNode ?? null,
    });

    if (sourceId === targetId) {
      sourceSummary.selfCallCount += edge.callCount;
      continue;
    }

    const syntheticCallKind = !isResolved ? "unresolved calls" : isExternal ? "external calls" : "module calls";
    const edgeKey = `${sourceId}->${targetId}:${syntheticCallKind}`;
    const existing = aggregatedEdges.get(edgeKey);

    if (existing) {
      existing.callCount += edge.callCount;
      existing.sourceSpans.push(...edge.sourceSpans);
      continue;
    }

    aggregatedEdges.set(edgeKey, {
      callCount: edge.callCount,
      callKind: syntheticCallKind,
      confidence: edge.confidence,
      isExternal,
      isResolved,
      rawTarget: targetLabel,
      source: sourceId,
      sourceSpans: [...edge.sourceSpans],
      target: targetId,
      typeArguments: [],
    });
  }

  for (const call of graph.unresolvedCalls.filter((call) => localIds.has(call.source))) {
    const sourceNode = nodeById.get(call.source);

    if (!sourceNode) {
      continue;
    }

    const sourceId = callModuleNodeId(sourceNode.moduleName);
    const sourceSummary = summaries.get(sourceId);

    if (sourceSummary) {
      sourceSummary.unresolvedCallCount += 1;
    }

    ensureSummary(CALL_UNRESOLVED_NODE_ID, {
      kind: "unresolved",
      label: "Unresolved calls",
      moduleName: "unresolved",
      packageLabel: "call target missing",
      representativeNode: null,
    });
  }

  const columns = new Map<number, ModuleCallSummary[]>();

  for (const summary of summaries.values()) {
    const column = moduleSummaryColumn(summary);
    const columnSummaries = columns.get(column) ?? [];
    columnSummaries.push(summary);
    columns.set(column, columnSummaries);
  }

  const renderNodes: RenderCallNode[] = [];

  for (const [column, summariesInColumn] of columns) {
    summariesInColumn.sort(moduleSummarySort);
    summariesInColumn.forEach((summary, index) => {
      renderNodes.push({
        column,
        groupLabel: moduleColumnLabel(summary, column),
        id: summary.id,
        kind: summary.kind,
        label: summary.label,
        mode: "modules",
        moduleName: summary.moduleName,
        node: summary.representativeNode,
        roleLabel: moduleRoleLabel(summary),
        showGroupLabel: index === 0,
        sourceLocation: null,
        subtitle: moduleSubtitle(summary),
        tags: moduleTags(summary),
        x: 80 + column * (CALL_MODULE_NODE_WIDTH + CALL_COLUMN_GAP + 38),
        y: index * CALL_MODULE_ROW_GAP,
      });
    });
  }

  const visibleNodeIds = new Set(renderNodes.map((node) => node.id));
  const visibleEdges = [...aggregatedEdges.values()]
    .filter((edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target))
    .sort(callEdgeSort);

  return {
    edgeCount: visibleEdges.reduce((total, edge) => total + edge.callCount, 0),
    edges: visibleEdges.map((edge) => ({
      edge,
      id: callEdgeId(edge),
      source: edge.source,
      target: edge.target,
    })),
    localFunctionCount: localNodes.length,
    mode: "modules",
    nodeCount: renderNodes.length,
    nodes: renderNodes,
    packageLabel: displayPackageLabel,
    unresolvedCalls: graph.unresolvedCalls.filter((call) => localIds.has(call.source)),
  };
}

const CALL_UNRESOLVED_NODE_ID = "call-module:unresolved";

function callModuleNodeId(moduleName: string) {
  return `call-module:local:${moduleName}`;
}

function callExternalModuleNodeId(packageName: string | null, moduleName: string, address: string | null) {
  return `call-module:external:${packageName ?? address ?? "external"}:${moduleName}`;
}

function moduleSummaryColumn(summary: ModuleCallSummary) {
  if (summary.kind === "external") {
    return 2;
  }

  if (summary.kind === "unresolved") {
    return 3;
  }

  return summary.entryCount || summary.txCount ? 0 : 1;
}

function moduleColumnLabel(summary: ModuleCallSummary, column: number) {
  if (summary.kind === "external") {
    return "External Targets";
  }

  if (summary.kind === "unresolved") {
    return "Unresolved";
  }

  return column === 0 ? "Entrypoint Modules" : "Internal Modules";
}

function moduleRoleLabel(summary: ModuleCallSummary) {
  if (summary.kind === "external") {
    return "external";
  }

  if (summary.kind === "unresolved") {
    return "unresolved";
  }

  return summary.entryCount || summary.txCount ? "entry module" : "module";
}

function moduleSubtitle(summary: ModuleCallSummary) {
  if (summary.kind === "unresolved") {
    return "calls that could not be resolved";
  }

  if (summary.kind === "external") {
    return `${summary.packageLabel}::${summary.moduleName}`;
  }

  return `${summary.packageLabel}::${summary.moduleName}`;
}

function moduleTags(summary: ModuleCallSummary) {
  if (summary.kind === "unresolved") {
    return [`${summary.unresolvedCallCount || 1} calls`, "needs review"];
  }

  if (summary.kind === "external") {
    return ["external", "trust boundary"];
  }

  const tags: string[] = [];

  if (summary.entryCount) {
    tags.push(`${summary.entryCount} entry`);
  }

  if (summary.txCount && !summary.entryCount) {
    tags.push(`${summary.txCount} tx callable`);
  }

  if (summary.publicCount) {
    tags.push(`${summary.publicCount} public`);
  }

  tags.push(`${summary.functionCount} fns`);

  if (summary.externalCallCount) {
    tags.push(`${summary.externalCallCount} external`);
  }

  if (summary.selfCallCount) {
    tags.push(`${summary.selfCallCount} internal calls`);
  }

  if (summary.unresolvedCallCount) {
    tags.push(`${summary.unresolvedCallCount} unresolved`);
  }

  return tags.slice(0, 4);
}

function moduleSummarySort(left: ModuleCallSummary, right: ModuleCallSummary) {
  return right.entryCount - left.entryCount
    || right.txCount - left.txCount
    || right.functionCount - left.functionCount
    || right.externalCallCount - left.externalCallCount
    || left.label.localeCompare(right.label);
}

function isPackageCallNode(
  node: MoveCallGraphNode,
  packagePath: string | null,
  packageName: string,
) {
  if (packagePath !== null) {
    return node.packagePath === packagePath;
  }

  return node.packageName === packageName && !node.isExternal;
}

function callNodeDepths(localNodes: MoveCallGraphNode[], edges: MoveCallGraphEdge[]) {
  const localIds = new Set(localNodes.map((node) => node.id));
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, string[]>();

  for (const edge of edges) {
    if (!localIds.has(edge.source) || !localIds.has(edge.target)) {
      continue;
    }

    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + 1);
    const targets = outgoing.get(edge.source) ?? [];
    targets.push(edge.target);
    outgoing.set(edge.source, targets);
  }

  const roots = localNodes
    .filter((node) => node.isEntry || node.isTransactionCallable || !incoming.has(node.id))
    .sort(callNodeSort);
  const queue = roots.map((node) => node.id);
  const depths = new Map<string, number>();

  for (const node of roots) {
    depths.set(node.id, 0);
  }

  while (queue.length) {
    const current = queue.shift()!;
    const currentDepth = depths.get(current) ?? 0;

    for (const target of outgoing.get(current) ?? []) {
      const nextDepth = Math.min(currentDepth + 1, 4);
      const previousDepth = depths.get(target);

      if (previousDepth == null || nextDepth < previousDepth) {
        depths.set(target, nextDepth);
        queue.push(target);
      }
    }
  }

  for (const node of localNodes) {
    if (!depths.has(node.id)) {
      depths.set(node.id, node.visibility === "public" ? 1 : 2);
    }
  }

  return depths;
}

function callGraphStats(edges: RenderCallEdge[]) {
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, number>();

  for (const edge of edges) {
    outgoing.set(edge.source, (outgoing.get(edge.source) ?? 0) + edge.edge.callCount);
    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + edge.edge.callCount);
  }

  return { incoming, outgoing };
}

function callNodeKind(node: MoveCallGraphNode, localIds: Set<string>): CallNodeKind {
  if (node.id.startsWith("unresolved:call:")) {
    return "unresolved";
  }

  if (!localIds.has(node.id) || node.isExternal) {
    return "external";
  }

  if (node.isEntry) {
    return "entry";
  }

  if (node.isTransactionCallable) {
    return "transaction";
  }

  if (node.visibility === "public" || node.visibility === "public(package)") {
    return "public";
  }

  return "internal";
}

function callNodeColor(kind: CallNodeKind) {
  switch (kind) {
    case "entry":
      return ENTRY_COLOR;
    case "transaction":
      return TRANSACTION_COLOR;
    case "public":
      return PUBLIC_COLOR;
    case "module":
      return PUBLIC_COLOR;
    case "external":
      return EXTERNAL_COLOR;
    case "unresolved":
      return UNRESOLVED_COLOR;
    case "internal":
      return INTERNAL_COLOR;
  }
}

function callEdgeColor(edge: MoveCallGraphEdge) {
  if (!edge.isResolved && !edge.isExternal) {
    return UNRESOLVED_COLOR;
  }

  if (edge.isExternal) {
    return EXTERNAL_COLOR;
  }

  if (edge.callKind.toLowerCase().includes("macro")) {
    return MACRO_COLOR;
  }

  if (edge.callKind.toLowerCase().includes("method")) {
    return METHOD_COLOR;
  }

  return PUBLIC_COLOR;
}

function callEdgeDash(edge: MoveCallGraphEdge) {
  if (!edge.isResolved || edge.confidence === "low") {
    return "1 6";
  }

  if (edge.confidence === "medium") {
    return "6 7";
  }

  return undefined;
}

function callNodeIcon(kind: CallNodeKind) {
  switch (kind) {
    case "entry":
    case "transaction":
      return RadioTower;
    case "external":
      return ArrowUpRight;
    case "unresolved":
      return ShieldAlert;
    case "public":
      return FunctionSquare;
    case "module":
      return Boxes;
    case "internal":
      return Braces;
  }
}

function callNodeRoleLabel(kind: CallNodeKind, node: MoveCallGraphNode) {
  if (kind === "entry") {
    return "entry";
  }

  if (kind === "transaction") {
    return "tx callable";
  }

  if (kind === "external") {
    return "external";
  }

  if (kind === "unresolved") {
    return "unresolved";
  }

  if (kind === "module") {
    return "module";
  }

  return node.visibility || "function";
}

function callNodeTags(kind: CallNodeKind, node: MoveCallGraphNode) {
  const tags: string[] = [];

  if (node.isEntry) {
    tags.push("entry");
  }

  if (node.isTransactionCallable && !tags.includes("entry")) {
    tags.push("tx surface");
  }

  if (node.visibility === "public(package)") {
    tags.push("package public");
  } else if (node.visibility === "public") {
    tags.push("public");
  }

  if (kind === "external") {
    tags.push("external");
    tags.push("trust boundary");
  }

  if (kind === "unresolved") {
    tags.push("unresolved");
  }

  if (node.attributes.length) {
    tags.push(node.attributes[0]!);
  }

  return tags.length ? tags : ["internal"];
}

function callColumnLabel(kind: CallNodeKind, column: number) {
  if (column === 0) {
    return "Entrypoints";
  }

  if (kind === "external") {
    return "External Calls";
  }

  if (kind === "unresolved") {
    return "Unresolved";
  }

  return "Package Internals";
}

function callEdgeLabel(edge: MoveCallGraphEdge) {
  const count = edge.callCount > 1 ? ` x${edge.callCount}` : "";
  return `${edge.callKind}${count}`;
}

function callNodeSort(left: MoveCallGraphNode, right: MoveCallGraphNode) {
  return Number(right.isEntry) - Number(left.isEntry)
    || Number(right.isTransactionCallable) - Number(left.isTransactionCallable)
    || visibilityRank(left.visibility) - visibilityRank(right.visibility)
    || left.moduleName.localeCompare(right.moduleName)
    || left.functionName.localeCompare(right.functionName);
}

function callRenderNodeSort(left: RenderCallNode, right: RenderCallNode) {
  return callKindRank(left.kind) - callKindRank(right.kind)
    || left.moduleName.localeCompare(right.moduleName)
    || left.label.localeCompare(right.label);
}

function callKindRank(kind: CallNodeKind) {
  switch (kind) {
    case "entry":
      return 0;
    case "transaction":
      return 1;
    case "public":
      return 2;
    case "module":
      return 3;
    case "internal":
      return 4;
    case "external":
      return 5;
    case "unresolved":
      return 6;
  }
}

function visibilityRank(visibility: string) {
  switch (visibility) {
    case "public":
      return 0;
    case "public(package)":
      return 1;
    case "friend":
      return 2;
    case "private":
      return 3;
    default:
      return 4;
  }
}

function callEdgeSort(left: MoveCallGraphEdge, right: MoveCallGraphEdge) {
  return left.source.localeCompare(right.source)
    || left.target.localeCompare(right.target)
    || left.rawTarget.localeCompare(right.rawTarget);
}

function callEdgeId(edge: MoveCallGraphEdge) {
  return [
    edge.source,
    edge.target,
    edge.callKind,
    edge.rawTarget,
    edge.typeArguments.join(","),
  ].join("|");
}

function compactFunctionLabel(id: string) {
  return id.split("::").slice(-2).join("::").replace(/^function:[^:]+:/, "");
}

function compactCallEndpoint(id: string) {
  if (id === CALL_UNRESOLVED_NODE_ID) {
    return "unresolved";
  }

  if (id.startsWith("call-module:local:")) {
    return id.replace("call-module:local:", "");
  }

  if (id.startsWith("call-module:external:")) {
    return id.split(":").slice(-1)[0] ?? "external";
  }

  return compactFunctionLabel(id);
}
