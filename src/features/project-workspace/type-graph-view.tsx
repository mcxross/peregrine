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
  useReactFlow,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  Box,
  Boxes,
  Braces,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Crosshair,
  FileCode2,
  Hexagon,
  KeyRound,
  Layers3,
  ListTree,
  Maximize2,
  Minimize2,
  MousePointerClick,
  Search,
  ShieldAlert,
  Workflow,
} from "lucide-react";
import React from "react";

import { Badge } from "@/components/ui/badge";
import type {
  MovePackage,
  MoveSourceSpan,
  MoveTypeGraph,
} from "@peregrine/desktop-runtime";
import { displayMovePackageName } from "@peregrine/desktop-runtime";
import { CanvasNotice, EmptyTypeGraphState } from "@/features/project-workspace/type-graph/components";
import {
  BUILTIN_COLOR,
  CAPABILITY_COLOR,
  EDGE_LABEL_ZOOM_THRESHOLD,
  EXTERNAL_COLOR,
  FRAMEWORK_COLOR,
  GENERIC_COLOR,
  LOCAL_COLOR,
  MAX_EDGE_LABELS,
  SELECTED_COLOR,
  TYPE_GRAPH_COLUMN_GAP,
  TYPE_GRAPH_LENSES,
  TYPE_GRAPH_SCOPES,
  type RenderEdge,
  type RenderNode,
  type TypeFlowEdgeData,
  type TypeFlowNodeData,
  type TypeGraphLens,
  type TypeGraphScope,
  type TypeGraphSourceLocation,
  type TypeRenderGraph,
} from "@peregrine/desktop-runtime";
import * as typeGraphModel from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";

export type { TypeGraphSourceLocation } from "@peregrine/desktop-runtime";

type TypeGraphViewProps = {
  className?: string;
  movePackage: MovePackage | null;
  onOpenSourceLocation?: (location: TypeGraphSourceLocation) => void;
  onSelectType: (typeId: string) => void;
  packageName: string;
  selectedTypeId: string | null;
  typeGraph: MoveTypeGraph;
};


const NODE_TYPES = {
  type: TypeGraphNode,
};

const EDGE_TYPES = {
  typeRelationship: TypeGraphEdge,
};


export function TypeGraphView({
  className = "h-72",
  movePackage,
  onOpenSourceLocation,
  onSelectType,
  packageName,
  selectedTypeId,
  typeGraph,
}: TypeGraphViewProps) {
  const [lens, setLens] = React.useState<TypeGraphLens>("storage");
  const [scope, setScope] = React.useState<TypeGraphScope>("oneHop");
  const [customQuery, setCustomQuery] = React.useState("");
  const [selectedEdgeId, setSelectedEdgeId] = React.useState<string | null>(null);
  const [hoveredEdgeId, setHoveredEdgeId] = React.useState<string | null>(null);
  const [viewportZoom, setViewportZoom] = React.useState(1);
  const [expandedNodeIds, setExpandedNodeIds] = React.useState<Set<string>>(() => new Set());
  const [collapsedNodeIds, setCollapsedNodeIds] = React.useState<Set<string>>(() => new Set());
  const [isInspectorOpen, setIsInspectorOpen] = React.useState(false);
  const [isFullscreen, setIsFullscreen] = React.useState(false);
  const [inspectorTab, setInspectorTab] = React.useState<"overview" | "fields" | "functions" | "security">("overview");
  const functionIndex = React.useMemo(() => typeGraphModel.buildFunctionIndex(movePackage), [movePackage]);
  const fallbackTypeId = React.useMemo(
    () => typeGraphModel.importantLocalTypeId(typeGraph, movePackage),
    [movePackage, typeGraph],
  );

  React.useEffect(() => {
    if (!selectedTypeId && fallbackTypeId) {
      onSelectType(fallbackTypeId);
    }
  }, [fallbackTypeId, onSelectType, selectedTypeId]);

  React.useEffect(() => {
    setSelectedEdgeId(null);
    setHoveredEdgeId(null);
  }, [selectedTypeId]);

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

  const renderGraph = React.useMemo(
    () => typeGraphModel.buildTypeRenderGraph({
      collapsedNodeIds,
      customQuery,
      expandedNodeIds,
      functionIndex,
      graph: typeGraph,
      lens,
      movePackage,
      scope,
      selectedTypeId,
    }),
    [collapsedNodeIds, customQuery, expandedNodeIds, functionIndex, lens, movePackage, scope, selectedTypeId, typeGraph],
  );
  const stats = React.useMemo(() => typeGraphModel.graphStats(renderGraph.edges), [renderGraph.edges]);
  const selectedEdge = React.useMemo(
    () =>
      renderGraph.edgeEvidence.find((edge) => edge.id === selectedEdgeId)
      ?? renderGraph.edges.find((edge) => edge.id === selectedEdgeId)
      ?? null,
    [renderGraph.edgeEvidence, renderGraph.edges, selectedEdgeId],
  );
  const hoveredEdge = React.useMemo(
    () => renderGraph.edges.find((edge) => edge.id === hoveredEdgeId) ?? null,
    [hoveredEdgeId, renderGraph.edges],
  );
  const selectedNeighborhood = React.useMemo(
    () => typeGraphModel.selectedNeighborhoodIds(renderGraph.edges, selectedTypeId, selectedEdge ?? hoveredEdge),
    [hoveredEdge, renderGraph.edges, selectedEdge, selectedTypeId],
  );
  const selectedOrHoveredEdge = selectedEdge ?? hoveredEdge;
  const activeNodeIds = React.useMemo(() => {
    const ids = new Set<string>();

    if (selectedOrHoveredEdge) {
      selectedNeighborhood.forEach((id) => ids.add(id));
    } else if (selectedTypeId) {
      ids.add(selectedTypeId);
    }

    if (selectedOrHoveredEdge) {
      ids.add(selectedOrHoveredEdge.source);
      ids.add(selectedOrHoveredEdge.target);
    }
    return ids;
  }, [selectedNeighborhood, selectedOrHoveredEdge, selectedTypeId]);
  const openSource = React.useCallback(
    (span: MoveSourceSpan | null) => {
      if (!span || !onOpenSourceLocation) {
        console.warn("[TypeGraph] source open ignored", {
          hasHandler: Boolean(onOpenSourceLocation),
          selectedEdgeId,
          selectedTypeId,
          span,
        });
        return;
      }

      if (import.meta.env.DEV) {
        console.info("[TypeGraph] source open requested", {
          filePath: span.filePath,
          line: span.startLine || 1,
          selectedEdgeId,
          selectedTypeId,
        });
      }
      onOpenSourceLocation({ filePath: span.filePath, line: span.startLine || 1 });
    },
    [onOpenSourceLocation, selectedEdgeId, selectedTypeId],
  );
  const flowNodes = React.useMemo(
    () =>
      renderGraph.nodes.map<Node<TypeFlowNodeData>>((node) => {
        const selected = node.selectTypeId === selectedTypeId;
        const color = selected ? SELECTED_COLOR : typeGraphModel.nodeColor(node.kind, node.node);
        const activeEdgeFocus = Boolean(selectedOrHoveredEdge);
        const dimmed = activeEdgeFocus
          ? selectedNeighborhood.size > 0 && !selectedNeighborhood.has(node.id)
          : Boolean(
              selectedTypeId
              && !selected
              && selectedNeighborhood.size > 0
              && !selectedNeighborhood.has(node.id),
            );
        const focused = selected || activeNodeIds.has(node.id);

        return {
          id: node.id,
          type: "type",
          position: { x: node.x, y: node.y },
          zIndex: selected ? 1000 : node.kind === "local" ? 30 : node.kind === "function" || node.kind === "field" ? 20 : 10,
          data: {
            ...node,
            color,
            dimmed,
            focused,
            incoming: stats.incoming.get(node.id) ?? 0,
            onCollapseNeighbors: (typeId) => {
              setCollapsedNodeIds((current) => new Set(current).add(typeId));
              setExpandedNodeIds((current) => {
                const next = new Set(current);
                next.delete(typeId);
                return next;
              });
            },
            onExpandNode: (typeId) => {
              setCollapsedNodeIds((current) => {
                const next = new Set(current);
                next.delete(typeId);
                return next;
              });
              setExpandedNodeIds((current) => new Set(current).add(typeId));
            },
            onOpenSource: openSource,
            onSelectEvidenceEdge: (edgeId) => {
              setSelectedEdgeId(edgeId);
              setInspectorTab("overview");
              setIsInspectorOpen(true);
            },
            onShowFields: (typeId) => {
              setSelectedEdgeId(null);
              setInspectorTab("fields");
              setIsInspectorOpen(true);
              onSelectType(typeId);
            },
            onShowFunctions: (typeId) => {
              setSelectedEdgeId(null);
              setInspectorTab("functions");
              setIsInspectorOpen(true);
              onSelectType(typeId);
            },
            onSelectType: (typeId) => {
              setSelectedEdgeId(null);
              setInspectorTab("overview");
              onSelectType(typeId);
            },
            outgoing: stats.outgoing.get(node.id) ?? 0,
            selected,
          },
        };
      }),
    [activeNodeIds, onSelectType, openSource, renderGraph.nodes, selectedNeighborhood, selectedOrHoveredEdge, selectedTypeId, stats],
  );
  const flowEdges = React.useMemo(() => {
    const zoomLabels = viewportZoom >= EDGE_LABEL_ZOOM_THRESHOLD;
    const fieldCauseLabelIds = typeGraphModel.fieldCauseEdgeLabelIds(
      renderGraph.edges,
      selectedTypeId,
      selectedEdgeId,
    );
    const prioritizedZoomLabelIds = zoomLabels
      ? typeGraphModel.prioritizedLabelEdgeIds(renderGraph.edges, selectedTypeId, MAX_EDGE_LABELS)
      : new Set<string>();
    const activePathFieldNodeId = typeGraphModel.fieldNodeEndpoint(selectedOrHoveredEdge);

    return renderGraph.edges.map<Edge<TypeFlowEdgeData>>((edge) => {
      const selectedPath =
        edge.id === selectedEdgeId
        || edge.id === hoveredEdgeId
        || Boolean(activePathFieldNodeId && (edge.source === activePathFieldNodeId || edge.target === activePathFieldNodeId));
      const active =
        selectedPath
        || !selectedTypeId
        || edge.source === selectedTypeId
        || edge.target === selectedTypeId
        || (selectedNeighborhood.has(edge.source) && selectedNeighborhood.has(edge.target));
      const color = active ? typeGraphModel.relationshipColor(edge.category) : "#64748b";
      const showLabel =
        (edge.id === selectedEdgeId || edge.id === hoveredEdgeId)
        || (
          !typeGraphModel.edgeTouchesFieldNode(edge)
          && (fieldCauseLabelIds.has(edge.id) || prioritizedZoomLabelIds.has(edge.id))
        );
      const dimmed = Boolean(selectedTypeId && !active);
      const contextualFieldPath = active && !selectedPath && typeGraphModel.edgeTouchesFieldNode(edge);
      const opacity = selectedPath ? 0.96 : active ? (contextualFieldPath ? 0.46 : 0.64) : 0.14;
      const strokeWidth = selectedPath
        ? 3
        : active
          ? contextualFieldPath ? 1.25 : Math.min(typeGraphModel.edgeStrokeWidth(edge), 1.55)
          : 1;

      return {
        id: edge.id,
        source: edge.source,
        target: edge.target,
        data: {
          active,
          category: edge.category,
          color,
          count: edge.count,
          dimmed,
          label: showLabel ? typeGraphModel.edgeLabel(edge.edge, edge.count, edge.category) : null,
          routeCount: edge.routeCount,
          routeIndex: edge.routeIndex,
        },
        markerEnd: {
          type: MarkerType.ArrowClosed,
          color,
        },
        style: {
          opacity,
          stroke: color,
          strokeDasharray: typeGraphModel.edgeStrokeDash(edge),
          strokeWidth,
        },
        type: "typeRelationship",
      };
    });
  }, [hoveredEdgeId, renderGraph.edges, selectedEdgeId, selectedNeighborhood, selectedOrHoveredEdge, selectedTypeId, viewportZoom]);
  const flowGraphKey = React.useMemo(
    () => `${lens}:${scope}:${customQuery}:${selectedTypeId ?? "overview"}:${flowNodes.length}:${flowEdges.length}:${isFullscreen ? "fullscreen" : "inline"}`,
    [customQuery, flowEdges.length, flowNodes.length, isFullscreen, lens, scope, selectedTypeId],
  );
  const selectType = React.useCallback(
    (typeId: string) => {
      setSelectedEdgeId(null);
      onSelectType(typeId);
    },
    [onSelectType],
  );

  if (!renderGraph.nodes.length) {
    return <EmptyTypeGraphState className={className} packageName={packageName} />;
  }

  return (
    <section
      className={cn(
        className,
        "grid min-h-0 gap-3 overflow-hidden animate-in fade-in slide-in-from-right-3 duration-200",
        isFullscreen && "fixed bottom-3 left-3 right-3 top-[calc(58px+0.75rem)] z-[90] rounded-lg border border-[color:var(--app-border)] bg-[var(--app-window)] p-3 shadow-2xl shadow-black/60",
        isInspectorOpen
          ? "grid-rows-[minmax(0,1fr)_minmax(18rem,34vh)] lg:grid-cols-[minmax(0,1fr)_minmax(18rem,21rem)] lg:grid-rows-1"
          : "grid-cols-[minmax(0,1fr)_2.75rem]",
      )}
    >
      <div className="grid min-h-0 grid-rows-[2.5rem_minmax(0,1fr)] overflow-hidden">
        <TypeGraphLensTabs activeLens={lens} onLensChange={setLens} />
        <div className="type-graph-canvas relative min-h-0 bg-[radial-gradient(circle_at_1px_1px,rgba(148,163,184,0.18)_1px,transparent_0)] [background-size:18px_18px]">
          <ReactFlow
            colorMode="dark"
            edgeTypes={EDGE_TYPES}
            edges={flowEdges}
            edgesFocusable={false}
            fitView
            fitViewOptions={{ padding: 0.16 }}
            maxZoom={1.9}
            minZoom={0.28}
            nodeTypes={NODE_TYPES}
            nodes={flowNodes}
            nodesDraggable={false}
            nodesFocusable={false}
            onEdgeClick={(_, edge) => {
              if (import.meta.env.DEV) {
                console.info("[TypeGraph] edge clicked", {
                  edgeId: edge.id,
                  selectedTypeId,
                });
              }
              setSelectedEdgeId(edge.id);
              setIsInspectorOpen(true);
            }}
            onEdgeMouseEnter={(_, edge) => setHoveredEdgeId(edge.id)}
            onEdgeMouseLeave={() => setHoveredEdgeId(null)}
            onMove={(_, viewport) => setViewportZoom(viewport.zoom)}
            onPaneClick={() => {
              setSelectedEdgeId(null);
              setHoveredEdgeId(null);
            }}
            onlyRenderVisibleElements
            proOptions={{ hideAttribution: true }}
          >
            <FitTypeGraphOnChange graphKey={flowGraphKey} nodeCount={flowNodes.length} />
            <Background color="var(--border)" gap={18} size={1} />
            <Controls
              className="!bg-background/90 !shadow-none [&_button]:!border-border [&_button]:!bg-background [&_button]:!text-foreground"
              position="top-right"
              showInteractive={false}
            />
          </ReactFlow>

          <div className="type-graph-footer pointer-events-none absolute bottom-3 left-3 right-3 z-10 flex flex-nowrap items-end justify-between gap-2">
            <TypeGraphCanvasHeader
              lens={lens}
              packageName={packageName}
              renderGraph={renderGraph}
              scope={scope}
              selectedEdge={selectedEdge}
            />
            <TypeGraphScopeControls
              customQuery={customQuery}
              onCustomQueryChange={setCustomQuery}
              onScopeChange={setScope}
              scope={scope}
            />
          </div>
          <TypeGraphLegend />
          <TypeGraphModeState
            lens={lens}
            renderGraph={renderGraph}
            selectedTypeId={selectedTypeId}
          />
          {isFullscreen ? (
            <div className="pointer-events-none absolute left-1/2 top-3 z-20 -translate-x-1/2 rounded-md border border-[color:var(--app-border)] bg-background/88 px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground shadow-sm backdrop-blur">
              Type Graph · Fullscreen
            </div>
          ) : null}
          <button
            aria-label={isFullscreen ? "Exit fullscreen type graph" : "Open type graph fullscreen"}
            className="absolute right-3 top-[7.75rem] z-20 grid size-8 place-items-center rounded border border-[color:var(--app-border)] bg-background/90 text-muted-foreground shadow-sm backdrop-blur transition hover:bg-[var(--app-elevated)] hover:text-foreground"
            onClick={() => setIsFullscreen((current) => !current)}
            title={isFullscreen ? "Exit fullscreen" : "Open fullscreen"}
            type="button"
          >
            {isFullscreen ? (
              <Minimize2 className="size-4" aria-hidden="true" />
            ) : (
              <Maximize2 className="size-4" aria-hidden="true" />
            )}
          </button>
        </div>
      </div>

      {isInspectorOpen ? (
        <TypeGraphInspector
          edge={selectedEdge}
          inspectorTab={inspectorTab}
          lens={lens}
          node={renderGraph.selectedNode}
          onCollapse={() => setIsInspectorOpen(false)}
          onInspectorTabChange={setInspectorTab}
          onLensChange={setLens}
          onOpenSource={openSource}
          onScopeChange={setScope}
          onSelectType={selectType}
          renderGraph={renderGraph}
          scope={scope}
        />
      ) : (
        <CollapsedTypeGraphInspector
          onExpand={() => setIsInspectorOpen(true)}
          selectedLabel={renderGraph.selectedLabel}
        />
      )}
    </section>
  );
}

export default TypeGraphView;

function TypeGraphLensTabs({
  activeLens,
  onLensChange,
}: {
  activeLens: TypeGraphLens;
  onLensChange: (lens: TypeGraphLens) => void;
}) {
  return (
    <header className="h-10 min-w-0 overflow-x-auto border-b border-[color:var(--app-border)] bg-[color-mix(in_oklch,var(--app-chrome)_82%,transparent)] backdrop-blur-sm">
      <div className="grid h-full min-w-[35rem] grid-cols-5">
        {TYPE_GRAPH_LENSES.map((lens) => (
          <button
            aria-pressed={activeLens === lens.id}
            className={cn(
              "relative grid h-full min-w-0 content-center gap-0.5 border-r border-[color:var(--app-border)] px-2 text-center transition hover:bg-[var(--app-subtle)]",
              activeLens === lens.id
                ? "bg-sky-500/10 text-sky-100 after:absolute after:inset-x-2 after:bottom-0 after:h-px after:bg-sky-400/70"
                : "text-muted-foreground",
            )}
            key={lens.id}
            onClick={() => onLensChange(lens.id)}
            title={lens.caption}
            type="button"
          >
            <span className={cn(
              "truncate text-[11px] font-semibold leading-4",
              activeLens === lens.id ? "text-sky-50" : "text-foreground/85",
            )}>
              {lens.label}
            </span>
          </button>
        ))}
      </div>
    </header>
  );
}

function FitTypeGraphOnChange({ graphKey, nodeCount }: { graphKey: string; nodeCount: number }) {
  const { fitView } = useReactFlow();

  React.useEffect(() => {
    const minZoom = nodeCount <= 14 ? 0.82 : nodeCount <= 24 ? 0.66 : 0.4;
    const frame = window.requestAnimationFrame(() => {
      fitView({ duration: 160, maxZoom: 1.24, minZoom, padding: 0.08 });
    });

    return () => window.cancelAnimationFrame(frame);
  }, [fitView, graphKey, nodeCount]);

  return null;
}

function TypeGraphCanvasHeader({
  lens,
  packageName,
  renderGraph,
  scope,
  selectedEdge,
}: {
  lens: TypeGraphLens;
  packageName: string;
  renderGraph: TypeRenderGraph;
  scope: TypeGraphScope;
  selectedEdge: RenderEdge | null;
}) {
  const lensLabel = TYPE_GRAPH_LENSES.find((item) => item.id === lens)?.label ?? "Type Graph";
  const scopeLabel = typeGraphModel.graphScopeLabel(scope);
  const nodeCount = typeGraphModel.visibleCountLabel(renderGraph.nodes.length, renderGraph.hiddenNodeCount);
  const edgeCount = typeGraphModel.visibleCountLabel(renderGraph.rawEdgeCount, renderGraph.hiddenEdgeCount);
  const selectedPartialSource = typeGraphModel.selectedLocalSourceMissing(renderGraph);
  const selectedFieldName = selectedEdge?.edge.fieldName ?? null;
  const selectedFieldType = selectedEdge?.edge.typeExpression
    ? typeGraphModel.simplifyMoveTypeExpression(selectedEdge.edge.typeExpression)
    : null;
  const selectedTargetReferences = !selectedFieldName && renderGraph.selectedNode
    ? typeGraphModel.referencedByFieldEdges(renderGraph.edgeEvidence, renderGraph.selectedNode.id).length
    : 0;

  return (
    <div className="type-graph-footer-summary pointer-events-auto min-w-0 flex-[1_1_auto]">
      <div className="type-graph-footer-summary-chip inline-flex max-w-full items-center gap-1.5 rounded border border-[color:var(--app-border)] bg-background/45 px-2 py-1 text-[10px] font-medium text-muted-foreground/75 shadow-sm backdrop-blur-[2px]">
        <span className="text-foreground/70">{lensLabel}</span>
        <span className="text-muted-foreground/45">·</span>
        <span className="type-graph-footer-scope whitespace-nowrap">{selectedFieldName ? `field ${selectedFieldName}` : selectedTargetReferences ? `target ${renderGraph.selectedLabel}` : scopeLabel}</span>
        <span className="type-graph-footer-detail-separator text-muted-foreground/45">·</span>
        <span className="type-graph-footer-detail min-w-0 truncate text-muted-foreground/80">
          {selectedFieldName && selectedFieldType
            ? selectedFieldType
            : selectedTargetReferences
              ? `referenced by ${typeGraphModel.pluralize(selectedTargetReferences, "field")}`
              : `centered on ${renderGraph.selectedLabel ?? packageName}`}
        </span>
        <span className="type-graph-footer-count-separator text-muted-foreground/45">·</span>
        <span className="type-graph-footer-count whitespace-nowrap text-muted-foreground/60">
          {selectedEdge ? typeGraphModel.edgeConfidence(selectedEdge) : `${nodeCount} nodes / ${edgeCount} edges`}
        </span>
        {selectedPartialSource ? (
          <>
            <span className="type-graph-footer-partial-separator text-muted-foreground/45">·</span>
            <span
              className="type-graph-footer-partial whitespace-nowrap text-amber-200/90"
              title="Partial source means some local nodes or relationships do not have full file and line metadata yet."
            >
              partial source
            </span>
          </>
        ) : null}
      </div>
    </div>
  );
}

function TypeGraphScopeControls({
  customQuery,
  onCustomQueryChange,
  onScopeChange,
  scope,
}: {
  customQuery: string;
  onCustomQueryChange: (query: string) => void;
  onScopeChange: (scope: TypeGraphScope) => void;
  scope: TypeGraphScope;
}) {
  return (
    <div className="type-graph-scope-controls pointer-events-auto flex max-w-full flex-[0_0_auto] flex-nowrap items-center justify-end gap-1 rounded border border-[color:var(--app-border)] bg-background/45 p-1 shadow-sm backdrop-blur-[2px]">
      {TYPE_GRAPH_SCOPES.map((item) => (
        <button
          aria-pressed={scope === item.id}
          className={cn(
            "h-6 shrink-0 rounded px-2 text-[10px] font-semibold text-muted-foreground/75 transition hover:bg-[var(--app-subtle)] hover:text-foreground",
            scope === item.id && "bg-sky-500/12 text-sky-200/90",
          )}
          key={item.id}
          onClick={() => onScopeChange(item.id)}
          type="button"
        >
          <span className="type-graph-scope-label-full">{item.label}</span>
          <span className="type-graph-scope-label-short hidden">{item.shortLabel}</span>
        </button>
      ))}
      {scope === "custom" ? (
        <label className="ml-1 grid h-6 min-w-0 max-w-full grid-cols-[14px_minmax(0,1fr)] items-center gap-1 rounded border border-[color:var(--app-border)] bg-background/50 px-2 sm:w-40">
          <Search className="size-3 text-muted-foreground/70" aria-hidden="true" />
          <input
            className="min-w-0 bg-transparent text-[10px] text-foreground outline-none placeholder:text-muted-foreground/70"
            onChange={(event) => onCustomQueryChange(event.target.value)}
            placeholder="type, module, ability:key"
            value={customQuery}
          />
        </label>
      ) : null}
    </div>
  );
}

function TypeGraphInspector({
  edge,
  inspectorTab,
  lens,
  node,
  onCollapse,
  onInspectorTabChange,
  onLensChange,
  onOpenSource,
  onScopeChange,
  onSelectType,
  renderGraph,
  scope,
}: {
  edge: RenderEdge | null;
  inspectorTab: "overview" | "fields" | "functions" | "security";
  lens: TypeGraphLens;
  node: RenderNode | null;
  onCollapse: () => void;
  onInspectorTabChange: (tab: "overview" | "fields" | "functions" | "security") => void;
  onLensChange: (lens: TypeGraphLens) => void;
  onOpenSource: (span: MoveSourceSpan | null) => void;
  onScopeChange: (scope: TypeGraphScope) => void;
  onSelectType: (typeId: string) => void;
  renderGraph: TypeRenderGraph;
  scope: TypeGraphScope;
}) {
  if (edge) {
    return (
      <TypeGraphEdgeInspector
        edge={edge}
        onCollapse={onCollapse}
        onOpenSource={onOpenSource}
        renderGraph={renderGraph}
      />
    );
  }

  if (!node) {
    return (
      <aside className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-transparent animate-in fade-in slide-in-from-right-3 duration-200">
        <TypeInspectorColumnHeader onCollapse={onCollapse} />
        <div className="grid place-items-center px-4 text-center text-xs text-muted-foreground">
          Select a Move type to inspect storage shape, authority, generics, and external dependency links.
        </div>
      </aside>
    );
  }

  const fieldEdges = typeGraphModel.selectedNodeEdges(renderGraph.edgeEvidence, node.id, "field");
  const functionEdges = typeGraphModel.selectedNodeEdges(renderGraph.edgeEvidence, node.id, "input")
    .concat(typeGraphModel.selectedNodeEdges(renderGraph.edgeEvidence, node.id, "return"));
  const securityNotes = typeGraphModel.nodeSecurityNotes(node, renderGraph.edgeEvidence);

  return (
    <aside className="grid min-h-0 grid-rows-[auto_auto_auto_minmax(0,1fr)_auto] overflow-hidden bg-transparent animate-in fade-in slide-in-from-right-3 duration-200">
      <TypeInspectorColumnHeader onCollapse={onCollapse} />
      <header className="border-b border-[color:var(--app-border)] px-3 py-3">
        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-2">
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold text-foreground">{node.label}</h3>
            <p className="mt-0.5 truncate text-[11px] text-muted-foreground">{node.subtitle}</p>
          </div>
          <span className={cn("h-6 rounded px-2 py-1 text-[10px] font-semibold", typeGraphModel.nodeRiskClass(node))}>
            {node.riskTags[0] ?? node.roleLabel}
          </span>
        </div>
        <div className="mt-2 flex flex-wrap gap-1">
          {node.tags.map((tag) => (
            <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground" key={tag}>
              {tag}
            </span>
          ))}
        </div>
      </header>

      <div className="grid grid-cols-4 border-b border-[color:var(--app-border)] p-1">
        {(["overview", "fields", "functions", "security"] as const).map((item) => (
          <button
            className={cn(
              "h-7 rounded text-[11px] font-semibold text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
              inspectorTab === item && "bg-sky-500/15 text-sky-200",
            )}
            key={item}
            onClick={() => onInspectorTabChange(item)}
            type="button"
          >
            {item}
          </button>
        ))}
      </div>

      <div className="min-h-0 overflow-auto px-3 py-3 text-xs">
        {inspectorTab === "overview" ? (
          <TypeInspectorOverview node={node} onOpenSource={onOpenSource} renderGraph={renderGraph} />
        ) : inspectorTab === "fields" ? (
          <TypeInspectorEdges
            emptyLabel="No field relationships in this scope."
            edges={fieldEdges}
            nodeId={node.id}
            onSelectType={onSelectType}
          />
        ) : inspectorTab === "functions" ? (
          <TypeInspectorEdges
            emptyLabel="No function surface in this scope."
            edges={functionEdges}
            nodeId={node.id}
            onSelectType={onSelectType}
          />
        ) : (
          <SecurityNotes notes={securityNotes} />
        )}
      </div>

      <footer className="flex flex-wrap gap-1 border-t border-[color:var(--app-border)] px-3 py-2">
        <QuickAction icon={Crosshair} label="Focus" onClick={() => onSelectType(node.id)} />
        <QuickAction
          active={scope === "oneHop"}
          icon={ChevronDown}
          label="1-hop"
          onClick={() => onScopeChange("oneHop")}
        />
        <QuickAction
          active={scope === "twoHop"}
          icon={Layers3}
          label="2-hop"
          onClick={() => onScopeChange("twoHop")}
        />
        <QuickAction
          active={lens === "storage"}
          icon={Boxes}
          label="Storage"
          onClick={() => onLensChange("storage")}
        />
        <QuickAction
          active={lens === "capabilities"}
          icon={ShieldAlert}
          label="Caps"
          onClick={() => onLensChange("capabilities")}
        />
        <QuickAction
          disabled={!node.sourceLocation}
          icon={FileCode2}
          label="Source"
          onClick={() => onOpenSource(node.sourceLocation)}
        />
      </footer>
    </aside>
  );
}

function TypeGraphEdgeInspector({
  edge,
  onCollapse,
  onOpenSource,
  renderGraph,
}: {
  edge: RenderEdge;
  onCollapse: () => void;
  onOpenSource: (span: MoveSourceSpan | null) => void;
  renderGraph: TypeRenderGraph;
}) {
  const sourceLabel = typeGraphModel.edgeEndpointLabel(edge.source, edge);
  const targetLabel = typeGraphModel.edgeEndpointLabel(edge.target, edge);
  const fieldLocation = typeGraphModel.edgeSourceLocation(edge.edge);
  const resolvedTypeNodeId = typeGraphModel.resolvedTypeNodeIdForEdge(renderGraph, edge);
  const resolvedTypeLocation = resolvedTypeNodeId
    ? typeGraphModel.renderNodeSourceLocation(renderGraph, resolvedTypeNodeId)
    : null;
  const location = fieldLocation ?? resolvedTypeLocation;
  const evidenceItems = typeGraphModel.edgeEvidenceItems(edge);
  const declaredIn = edge.edge.declaringTypeId
    ? typeGraphModel.compactEdgeEndpoint(edge.edge.declaringTypeId)
    : typeGraphModel.compactEdgeEndpoint(edge.source);
  const isFieldEvidence = Boolean(edge.edge.fieldName || edge.source.startsWith("field-node:") || edge.target.startsWith("field-node:"));
  const resolvedType = edge.edge.typeExpression
    ? typeGraphModel.simplifyMoveTypeExpression(edge.edge.typeExpression)
    : targetLabel;
  const genericArguments = edge.edge.typeExpression
    ? typeGraphModel.genericArgumentsFromTypeExpression(edge.edge.typeExpression, null)
    : [];
  const baseType = edge.edge.fieldName ? typeGraphModel.compactEdgeEndpoint(edge.edge.target ?? edge.target) : targetLabel;

  return (
    <aside className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden bg-transparent text-xs animate-in fade-in slide-in-from-right-3 duration-200">
      <TypeInspectorColumnHeader onCollapse={onCollapse} title={isFieldEvidence ? "Field evidence" : "Edge evidence"} />
      <div className="min-h-0 overflow-auto p-3">
        <h3 className="text-sm font-semibold text-foreground">
          {isFieldEvidence && edge.edge.fieldName ? edge.edge.fieldName : "Relationship"}
        </h3>
        <p className="mt-1 truncate text-muted-foreground">
          {isFieldEvidence && edge.edge.fieldName
            ? resolvedType
            : `${sourceLabel} -> ${targetLabel}`}
        </p>
        <dl className="mt-3 grid gap-2">
          <InspectorRow label="Relation" value={typeGraphModel.edgeRelationName(edge)} />
          {edge.edge.fieldName ? <InspectorRow label="Field" value={edge.edge.fieldName} /> : null}
          <InspectorRow label="Declared in" value={declaredIn} />
          {isFieldEvidence ? <InspectorRow label="Resolved type" value={resolvedType} /> : null}
          {isFieldEvidence ? <InspectorRow label="Base type" value={baseType} /> : null}
          {!isFieldEvidence ? <InspectorRow label="Label" value={typeGraphModel.edgeLabel(edge.edge, edge.count, edge.category)} /> : null}
          <InspectorRow label="Confidence" value={typeGraphModel.edgeConfidence(edge)} />
          <InspectorRow label="Risk" value={typeGraphModel.edgeRiskLevel(edge)} />
          {!isFieldEvidence && edge.edge.typeExpression ? <InspectorRow label="Type" value={edge.edge.typeExpression} /> : null}
          {edge.edge.typeArgumentName ? <InspectorRow label="Argument" value={`${edge.edge.typeArgumentName}${edge.edge.typeExpression ? ` = ${edge.edge.typeExpression}` : ""}`} /> : null}
          {edge.edge.functionName ? <InspectorRow label="Function" value={edge.edge.functionName} /> : null}
          {edge.edge.parameterName ? <InspectorRow label="Parameter" value={edge.edge.parameterName} /> : null}
          {location ? (
            <InspectorRow
              label="Location"
              value={`${typeGraphModel.compactPath(location.filePath)}:${location.startLine}`}
            />
          ) : (
            <InspectorRow label="Source" value="source unavailable" />
          )}
        </dl>
        {genericArguments.length ? (
          <div className="mt-3 flex flex-wrap gap-1">
            {genericArguments.map((argument) => (
              <span
                className="rounded border border-yellow-500/20 bg-yellow-500/10 px-1.5 py-0.5 text-[10px] font-semibold text-yellow-200"
                key={`${argument.label}:${argument.value}`}
              >
                {argument.label} = {argument.value}
              </span>
            ))}
          </div>
        ) : null}
        <div className="mt-4">
          <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
            Evidence
          </div>
          <div className="mt-2 grid gap-1.5">
            {evidenceItems.map((item) => (
              <p
                className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-2 py-1.5 text-[11px] leading-5 text-muted-foreground"
                key={item}
              >
                {item}
              </p>
            ))}
          </div>
        </div>
      </div>
      <footer className="flex gap-1 border-t border-[color:var(--app-border)] px-3 py-2">
        <QuickAction
          disabled={!fieldLocation}
          icon={FileCode2}
          label={isFieldEvidence ? "Field source" : "Open source"}
          onClick={() => onOpenSource(fieldLocation)}
        />
        {isFieldEvidence ? (
          <QuickAction
            disabled={!resolvedTypeLocation}
            icon={FileCode2}
            label="Type source"
            onClick={() => onOpenSource(resolvedTypeLocation)}
          />
        ) : null}
      </footer>
    </aside>
  );
}

function TypeInspectorColumnHeader({
  onCollapse,
  title = "Type details",
}: {
  onCollapse: () => void;
  title?: string;
}) {
  return (
    <div className="flex h-9 items-center justify-between border-b border-[color:var(--app-border)] px-3">
      <span className="truncate text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
        {title}
      </span>
      <button
        aria-label="Collapse type details"
        className="grid size-7 place-items-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground active:scale-95"
        onClick={onCollapse}
        title="Collapse details"
        type="button"
      >
        <ChevronRight className="size-4" aria-hidden="true" />
      </button>
    </div>
  );
}

function CollapsedTypeGraphInspector({
  onExpand,
  selectedLabel,
}: {
  onExpand: () => void;
  selectedLabel: string | null;
}) {
  return (
    <aside className="flex min-h-0 flex-col items-center gap-2 overflow-hidden bg-transparent p-1.5 animate-in fade-in slide-in-from-right-3 duration-200">
      <button
        aria-label="Show type details"
        className="grid size-8 place-items-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground active:scale-95"
        onClick={onExpand}
        title="Show details"
        type="button"
      >
        <ChevronLeft className="size-4" aria-hidden="true" />
      </button>
      <div className="min-h-0 flex-1 [writing-mode:vertical-rl]">
        <span className="block truncate text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
          {selectedLabel ?? "Type details"}
        </span>
      </div>
    </aside>
  );
}

function QuickAction({
  active,
  disabled,
  icon: Icon,
  label,
  onClick,
}: {
  active?: boolean;
  disabled?: boolean;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      className={cn(
        "inline-flex h-7 items-center gap-1 rounded border border-[color:var(--app-border)] px-2 text-[11px] font-semibold text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground disabled:cursor-not-allowed disabled:opacity-45",
        active && "border-sky-500/40 bg-sky-500/15 text-sky-200",
      )}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <Icon className="size-3.5" aria-hidden="true" />
      {label}
    </button>
  );
}

function TypeInspectorOverview({
  node,
  onOpenSource,
  renderGraph,
}: {
  node: RenderNode;
  onOpenSource: (span: MoveSourceSpan | null) => void;
  renderGraph: TypeRenderGraph;
}) {
  const source = node.sourceLocation;
  const references = typeGraphModel.referencedByFieldEdges(renderGraph.edgeEvidence, node.id);
  return (
    <div className="grid gap-3">
      <dl className="grid gap-1.5">
        <InspectorRow label="Kind" value={node.roleLabel} />
        <InspectorRow label="Origin" value={node.originLabel} />
        <InspectorRow label="Abilities" value={typeGraphModel.abilitySummary(node)} />
        <InspectorRow label="Fields" value={String(node.fieldCount)} />
        <InspectorRow label="Functions" value={String(node.functionCount)} />
        <InspectorRow label="Entry functions" value={String(node.entryFunctionCount)} />
        <InspectorRow label="Visible edges" value={String(renderGraph.rawEdgeCount)} />
        {node.addressLabel ? <InspectorRow label="Address" value={node.addressLabel} /> : null}
        {node.node?.packageName ? <InspectorRow label="Package" value={displayMovePackageName(node.node.packageName)} /> : null}
        {node.node?.moduleName ? <InspectorRow label="Module" value={node.node.moduleName} /> : null}
        {source ? <InspectorRow label="Source" value={`${typeGraphModel.compactPath(source.filePath)}:${source.startLine}`} /> : null}
      </dl>
      {references.length ? (
        <div className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-2">
          <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
            Referenced by fields
          </div>
          <div className="mt-2 grid gap-1">
            {references.slice(0, 6).map((edge) => (
              <div
                className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-2 text-[11px]"
                key={edge.id}
              >
                <span className="truncate text-foreground">
                  {typeGraphModel.compactEdgeEndpoint(edge.edge.declaringTypeId ?? edge.source)}.{edge.edge.fieldName ?? edge.edge.declaringFieldName}
                </span>
                <span className="truncate text-muted-foreground">
                  {edge.edge.typeExpression ? typeGraphModel.simplifyMoveTypeExpression(edge.edge.typeExpression) : typeGraphModel.compactEdgeEndpoint(edge.source)}
                </span>
              </div>
            ))}
          </div>
        </div>
      ) : null}
      {node.genericArguments.length ? (
        <div className="flex flex-wrap gap-1">
          {node.genericArguments.map((argument) => (
            <span
              className="rounded border border-yellow-500/20 bg-yellow-500/10 px-1.5 py-0.5 text-[10px] font-semibold text-yellow-200"
              key={`${argument.label}:${argument.value}`}
            >
              {argument.label}: {argument.value}
            </span>
          ))}
        </div>
      ) : null}
      <p className="rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-2 text-[11px] leading-5 text-muted-foreground">
        {typeGraphModel.nodeInterpretation(node)}
      </p>
      {source ? (
        <button
          className="inline-flex h-8 items-center justify-center gap-1.5 rounded-md border border-[color:var(--app-border)] px-2 text-[11px] font-semibold text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
          onClick={() => onOpenSource(source)}
          type="button"
        >
          <FileCode2 className="size-3.5" aria-hidden="true" />
          Open source
        </button>
      ) : node.kind === "local" && !node.isSynthetic ? (
        <p className="rounded-md border border-dashed border-[color:var(--app-border)] px-2 py-1.5 text-[11px] text-muted-foreground">
          Partial graph: source metadata unavailable for this type.
        </p>
      ) : null}
    </div>
  );
}

function TypeInspectorEdges({
  edges,
  emptyLabel,
  nodeId,
  onSelectType,
}: {
  edges: RenderEdge[];
  emptyLabel: string;
  nodeId: string;
  onSelectType: (typeId: string) => void;
}) {
  if (!edges.length) {
    return (
      <div className="grid h-28 place-items-center rounded-md border border-dashed border-[color:var(--app-border)] text-[11px] text-muted-foreground">
        {emptyLabel}
      </div>
    );
  }

  return (
    <div className="grid gap-1.5">
      {edges.slice(0, 12).map((edge) => (
        <button
          className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-2 py-1.5 text-left hover:border-sky-500/40"
          key={edge.id}
          onClick={() => onSelectType(typeGraphModel.edgeNavigationTarget(edge, nodeId))}
          type="button"
        >
          <span className="grid min-w-0 gap-0.5">
            <span className="min-w-0 truncate text-[11px] font-semibold text-foreground">
              {edge.edge.fieldName ?? typeGraphModel.edgeLabel(edge.edge, edge.count, edge.category)}
            </span>
            {edge.edge.fieldName ? (
              <span className="min-w-0 truncate text-[10px] text-muted-foreground">
                {typeGraphModel.edgeLabel(edge.edge, edge.count, edge.category)}
              </span>
            ) : null}
          </span>
          <span className="truncate text-right text-[10px] text-muted-foreground">
            {typeGraphModel.edgeDisplayEndpoint(edge, nodeId)}
          </span>
        </button>
      ))}
    </div>
  );
}

function SecurityNotes({ notes }: { notes: string[] }) {
  return (
    <div className="grid gap-2">
      {notes.map((note) => (
        <div
          className="rounded-md border border-rose-500/25 bg-rose-500/10 px-2 py-1.5 text-[11px] leading-5 text-rose-100"
          key={note}
        >
          {note}
        </div>
      ))}
    </div>
  );
}

function InspectorRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[7.5rem_minmax(0,1fr)] gap-3 text-[11px]">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="truncate text-right text-foreground">{value}</dd>
    </div>
  );
}

function TypeGraphNode({ data }: NodeProps<Node<TypeFlowNodeData>>) {
  const isTypeNode = Boolean(data.node);
  const Icon = typeGraphNodeIcon(data);
  const selectTypeId = data.selectTypeId;
  const showInlineActions = isTypeNode && (data.selected || data.kind === "local");
  const isFieldNode = data.kind === "field";
  const [hovered, setHovered] = React.useState(false);
  const fieldInfo = data.fieldInfo;
  const selectable = Boolean(isTypeNode || (isFieldNode && data.evidenceEdgeId));
  const selectNode = React.useCallback(() => {
    if (isFieldNode && data.evidenceEdgeId) {
      data.onSelectEvidenceEdge(data.evidenceEdgeId);
      return;
    }

    if (isTypeNode) {
      data.onSelectType(selectTypeId);
    }
  }, [data, isFieldNode, isTypeNode, selectTypeId]);

  return (
    <div
      className={cn(
        "group relative overflow-visible rounded-md border bg-[linear-gradient(135deg,rgba(255,255,255,0.055),rgba(255,255,255,0.015))] text-left shadow-sm backdrop-blur transition",
        isFieldNode ? "h-[78px] w-52 px-2.5 py-2" : "h-[132px] w-64 px-3 py-2.5",
        selectable && "cursor-pointer hover:bg-[var(--app-panel)]",
        data.dimmed && "opacity-40",
        data.focused && !data.selected && !isFieldNode && "shadow-[0_0_0_1px_rgba(148,163,184,0.18),0_12px_28px_rgba(15,23,42,0.18)]",
      )}
      onClick={selectable ? selectNode : undefined}
      onDoubleClick={selectable ? selectNode : undefined}
      onKeyDown={
        selectable
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                selectNode();
              }
            }
          : undefined
      }
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      role={selectable ? "button" : undefined}
      style={{
        borderColor: data.color,
        boxShadow: isFieldNode
          ? data.focused
            ? `0 0 0 1px color-mix(in oklch, ${data.color} 34%, transparent), 0 8px 18px rgba(20,184,166,0.10)`
            : `0 0 0 1px color-mix(in oklch, ${data.color} 16%, transparent)`
          : data.selected
            ? `0 0 0 1px color-mix(in oklch, ${data.color} 48%, transparent), 0 16px 32px rgba(14,165,233,0.14)`
            : data.focused
              ? `0 0 0 1px color-mix(in oklch, ${data.color} 34%, transparent), 0 12px 24px rgba(0,0,0,0.18)`
            : `0 0 0 1px color-mix(in oklch, ${data.color} 18%, transparent)`,
      }}
      tabIndex={selectable ? 0 : undefined}
    >
      {isFieldNode && fieldInfo ? (
        <NodeToolbar
          className="nodrag nopan rounded-md border border-[color:var(--app-border)] bg-background/95 px-2 py-1.5 text-[10px] shadow-xl backdrop-blur"
          isVisible={hovered}
          offset={8}
          position={Position.Top}
        >
          <div className="grid min-w-52 gap-1 text-left">
            <div className="font-semibold text-foreground">Field: {fieldInfo.fieldName}</div>
            <div className="truncate text-muted-foreground">Type: {fieldInfo.resolvedType}</div>
            <div className="truncate text-muted-foreground">Declared in: {typeGraphModel.compactEdgeEndpoint(fieldInfo.declaringTypeId)}</div>
            <div className="text-muted-foreground">Confidence: {fieldInfo.confidence || "syntactic"}</div>
          </div>
        </NodeToolbar>
      ) : null}
      <Handle
        className="!border-background"
        position={Position.Left}
        style={{ backgroundColor: data.color }}
        type="target"
      />
      <Handle
        className="!border-background"
        position={Position.Right}
        style={{ backgroundColor: data.color }}
        type="source"
      />

      {data.showGroupLabel && data.groupLabel ? (
        <div className="pointer-events-none absolute -top-5 left-0 flex w-full items-center gap-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-muted-foreground/65">
          <span>{data.groupLabel}</span>
          <span className="h-px flex-1 bg-[var(--app-border)]/70" />
        </div>
      ) : null}

      <div className={cn("grid min-w-0 items-center gap-2", isFieldNode ? "grid-cols-[18px_minmax(0,1fr)_auto]" : "grid-cols-[20px_minmax(0,1fr)_auto]")}>
        <span
          className={cn("grid place-items-center rounded-md border", isFieldNode ? "size-[18px] bg-teal-500/8" : "size-5")}
          style={{ borderColor: data.color, color: data.color }}
        >
          <Icon className={cn(isFieldNode ? "size-3" : "size-3.5")} aria-hidden="true" />
        </span>
        <span className={cn("min-w-0 truncate font-semibold text-card-foreground", isFieldNode ? "text-[13px]" : "text-sm")}>
          {data.label}
        </span>
        <Badge
          className={cn(
            "rounded bg-muted px-1.5 py-0.5 text-[10px]",
            isFieldNode && "border border-teal-400/15 bg-teal-500/10 text-teal-100",
          )}
          variant="secondary"
        >
          {data.roleLabel}
        </Badge>
      </div>

      {isFieldNode ? (
        <>
          <div className="mt-1 min-w-0 truncate text-[10px] font-medium text-muted-foreground">
            <span className="text-teal-200/75">field</span>
            <span className="mx-1 text-muted-foreground/45">·</span>
            <span>{fieldInfo?.resolvedType ?? data.subtitle}</span>
          </div>
          {data.tags.length ? (
            <div className="mt-2 flex min-h-4 min-w-0 flex-wrap gap-1">
              {data.tags.slice(0, 2).map((tag) => (
                <span
                  className="rounded border border-teal-400/15 bg-teal-500/10 px-1.5 py-0.5 text-[9px] font-semibold leading-none text-teal-100/90"
                  key={tag}
                >
                  {tag}
                </span>
              ))}
            </div>
          ) : null}
        </>
      ) : (
        <div className="mt-1 truncate text-xs text-muted-foreground">
          {data.subtitle}
        </div>
      )}

      {!isFieldNode ? (
      <div className="mt-2 flex min-h-5 min-w-0 flex-wrap gap-1">
        {data.tags.length ? (
          data.tags.slice(0, 3).map((tag) => (
            <span
              className={cn(
                "rounded px-1.5 py-0.5 text-[10px] font-semibold leading-none",
                tag === "key" || tag === "key object" || tag === "capability" || tag === "privileged"
                  ? "bg-rose-500/15 text-rose-200"
                  : tag === "store" || tag === "resource" || tag === "field target"
                    ? "bg-muted text-slate-200"
                  : tag === "generic asset" || tag === "generic container" || tag === "generic"
                    ? "bg-yellow-500/10 text-yellow-200"
                    : tag === "external" || tag === "trust boundary"
                      ? "bg-violet-500/12 text-violet-200"
                    : "bg-sky-500/10 text-sky-200",
              )}
              key={tag}
            >
              {tag}
            </span>
          ))
        ) : !data.abilitiesKnown ? (
          <span className="rounded bg-muted/70 px-1.5 py-0.5 text-[10px] text-muted-foreground">
            abilities unknown
          </span>
        ) : (
          <span className="rounded bg-muted/70 px-1.5 py-0.5 text-[10px] text-muted-foreground">
            no abilities
          </span>
        )}
      </div>
      ) : null}

      {data.genericArguments.length && !isFieldNode ? (
        <div className="mt-1 flex min-h-4 min-w-0 flex-wrap gap-1">
          {data.genericArguments.slice(0, 2).map((argument) => (
            <span
              className="rounded border border-yellow-500/20 bg-yellow-500/10 px-1.5 py-0.5 text-[9px] font-semibold leading-none text-yellow-200"
              key={`${argument.label}:${argument.value}`}
            >
              {argument.label}: {typeGraphModel.compactTypeLabel(argument.value)}
            </span>
          ))}
        </div>
      ) : null}

      {!isFieldNode ? (
      <div className="mt-2 grid grid-cols-2 border-t border-[color:var(--app-border)] pt-2 text-[11px] text-muted-foreground">
        <span className="truncate">{data.originLabel}</span>
        <span className="truncate text-right">
          {data.metricLabel}
        </span>
      </div>
      ) : null}
      {showInlineActions ? (
        <div className="absolute bottom-1.5 right-1.5 hidden gap-1 rounded border border-[color:var(--app-border)] bg-background/90 p-1 shadow-sm backdrop-blur-sm group-hover:flex group-focus-within:flex">
          <NodeCardAction icon={Crosshair} label="Focus here" onClick={(event) => {
            event.stopPropagation();
            data.onSelectType(selectTypeId);
          }} />
          <NodeCardAction icon={Layers3} label="Expand 1-hop" onClick={(event) => {
            event.stopPropagation();
            data.onExpandNode(selectTypeId);
          }} />
          <NodeCardAction icon={ChevronDown} label="Collapse neighbors" onClick={(event) => {
            event.stopPropagation();
            data.onCollapseNeighbors(selectTypeId);
          }} />
          <NodeCardAction icon={Boxes} label="Show fields" onClick={(event) => {
            event.stopPropagation();
            data.onShowFields(selectTypeId);
          }} />
          <NodeCardAction icon={Workflow} label="Show functions using this type" onClick={(event) => {
            event.stopPropagation();
            data.onShowFunctions(selectTypeId);
          }} />
          <NodeCardAction disabled={!data.sourceLocation} icon={FileCode2} label="Open source" onClick={(event) => {
            event.stopPropagation();
            data.onOpenSource(data.sourceLocation);
          }} />
        </div>
      ) : null}
    </div>
  );
}

function typeGraphNodeIcon(node: RenderNode) {
  if (node.kind === "function") {
    return Workflow;
  }

  if (node.kind === "field") {
    return ListTree;
  }

  if (node.node?.kind === "enum") {
    return Hexagon;
  }

  if (typeGraphModel.isCapabilityLike(node.node)) {
    return ShieldAlert;
  }

  if (typeGraphModel.isResourceLike(node.node)) {
    return KeyRound;
  }

  if (node.kind === "builtin") {
    return Braces;
  }

  if (node.kind === "framework") {
    return Box;
  }

  if (node.kind === "external") {
    return FileCode2;
  }

  return Boxes;
}

function NodeCardAction({
  disabled,
  icon: Icon,
  label,
  onClick,
}: {
  disabled?: boolean;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  label: string;
  onClick: React.MouseEventHandler<HTMLButtonElement>;
}) {
  return (
    <button
      aria-label={label}
      className="grid size-7 place-items-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon className="size-3.5" aria-hidden="true" />
    </button>
  );
}

function TypeGraphEdge({
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
  const edgeData = data as TypeFlowEdgeData | undefined;
  const routeOffset = edgeRouteOffset(edgeData?.routeIndex ?? 0, edgeData?.routeCount ?? 1);
  const [edgePath, labelX, labelY] = getSmoothStepPath({
    sourcePosition,
    sourceX,
    sourceY,
    targetPosition,
    targetX,
    targetY,
    borderRadius: 16,
    offset: routeOffset,
  });
  const labelPlacement = edgeLabelPlacement(
    sourceX,
    targetX,
    labelX,
    labelY,
    edgeData?.routeIndex ?? 0,
    edgeData?.routeCount ?? 1,
  );

  return (
    <>
      <BaseEdge markerEnd={markerEnd} path={edgePath} style={style} />
      {edgeData?.label ? (
        <EdgeLabelRenderer>
          <div
            className="nodrag nopan pointer-events-none absolute z-[1000] rounded bg-background/90 px-1.5 py-0.5 text-[10px] font-semibold leading-none shadow-sm"
            style={{
              color: edgeData.color,
              maxWidth: labelPlacement.maxWidth,
              opacity: edgeData.dimmed ? 0.52 : 1,
              transform: labelPlacement.transform,
            }}
          >
            <span className="block min-w-0 truncate">{edgeData.label}</span>
          </div>
        </EdgeLabelRenderer>
      ) : null}
    </>
  );
}

function edgeRouteOffset(routeIndex: number, routeCount: number) {
  if (routeCount <= 1) {
    return 24;
  }

  const centeredIndex = routeIndex - (routeCount - 1) / 2;
  return 24 + centeredIndex * 11;
}

function edgeLabelPlacement(
  sourceX: number,
  targetX: number,
  labelX: number,
  labelY: number,
  routeIndex: number,
  routeCount: number,
) {
  const laneStart = Math.min(sourceX, targetX);
  const laneEnd = Math.max(sourceX, targetX);
  const laneWidth = laneEnd - laneStart;
  const yNudge = routeCount > 1 ? (routeIndex - (routeCount - 1) / 2) * 7 : 0;

  if (targetX > sourceX && laneWidth >= 160) {
    const labelX = targetX - Math.min(104, Math.max(78, laneWidth * 0.18));

    return {
      maxWidth: Math.min(128, laneWidth - 84),
      transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY + yNudge}px)`,
    };
  }

  return {
    maxWidth: Math.max(72, Math.min(116, TYPE_GRAPH_COLUMN_GAP - 82)),
    transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY + yNudge}px)`,
  };
}

function TypeGraphLegend() {
  const [open, setOpen] = React.useState(false);
  const nodeItems: Array<[string, string]> = [
    ["Local", LOCAL_COLOR],
    ["Framework", FRAMEWORK_COLOR],
    ["External", EXTERNAL_COLOR],
    ["Builtin", BUILTIN_COLOR],
    ["Generic", GENERIC_COLOR],
    ["Capability", CAPABILITY_COLOR],
    ["Field node", typeGraphModel.relationshipColor("field")],
  ];
  const edgeItems: Array<[string, string]> = [
    ["Field", typeGraphModel.relationshipColor("field")],
    ["Generic", typeGraphModel.relationshipColor("generic")],
    ["Auth", typeGraphModel.relationshipColor("capability")],
    ["Trust", typeGraphModel.relationshipColor("external")],
    ["I/O", typeGraphModel.relationshipColor("input")],
  ];

  return (
    <div className="pointer-events-auto absolute left-3 top-3 z-20 max-w-[min(34rem,calc(100%-7.5rem))]">
      <div className="flex min-w-0 items-center gap-2 rounded border border-[color:var(--app-border)] bg-background/55 px-2 py-1.5 shadow-sm backdrop-blur-[2px]">
        <button
          aria-expanded={open}
          className="inline-flex h-5 items-center gap-1.5 rounded px-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
          onClick={() => setOpen((current) => !current)}
          type="button"
        >
          Legend
          <ChevronDown
            className={cn("size-3 transition", open && "rotate-180")}
            aria-hidden="true"
          />
        </button>
        <div className="hidden min-w-0 items-center gap-1.5 text-[10px] text-muted-foreground 2xl:flex">
          {nodeItems.slice(0, 4).map(([label, color]) => (
            <LegendDot color={color} key={label} label={label} />
          ))}
          <span className="mx-0.5 h-3 w-px bg-[var(--app-border)]" />
          {edgeItems.slice(0, 3).map(([label, color]) => (
            <LegendDot color={color} key={label} label={label} />
          ))}
        </div>
      </div>
      {open ? (
        <div className="mt-1 grid w-[min(26rem,calc(100vw-3rem))] gap-2 rounded-md border border-[color:var(--app-border)] bg-background/92 p-2 shadow-xl backdrop-blur">
          <LegendGroup items={nodeItems} title="Nodes" />
          <LegendGroup items={edgeItems} title="Edges" />
          <div className="grid gap-1 border-t border-[color:var(--app-border)] pt-2 text-[10px] text-muted-foreground">
            <div>
              <span className="font-semibold text-muted-foreground/85">Storage Shape:</span>
              {" source type -> field node -> resolved type"}
            </div>
            <div>
              <span className="font-semibold text-muted-foreground/85">Tags:</span>
              {" value-holding, key object, capability, trust boundary, external, generic container"}
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2 border-t border-[color:var(--app-border)] pt-2 text-[10px] text-muted-foreground">
            <LegendLine label="syntactic" />
            <LegendLine dash="6 7" label="inferred" />
            <LegendLine dash="1 6" label="heuristic" />
          </div>
        </div>
      ) : null}
    </div>
  );
}

function LegendDot({ color, label }: { color: string; label: string }) {
  return (
    <span className="inline-flex min-w-0 items-center gap-1">
      <span className="size-2 shrink-0 rounded-full" style={{ backgroundColor: color }} />
      <span className="truncate">{label}</span>
    </span>
  );
}

function LegendGroup({
  items,
  title,
}: {
  items: Array<[string, string]>;
  title: string;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1.5 text-[10px] text-muted-foreground">
      <span className="font-semibold uppercase tracking-[0.08em] text-muted-foreground/80">
        {title}
      </span>
      {items.map(([label, color]) => (
        <LegendDot color={color} key={label} label={label} />
      ))}
    </div>
  );
}

function LegendLine({ dash, label }: { dash?: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      <svg className="h-2.5 w-5" aria-hidden="true" viewBox="0 0 20 10">
        <line
          stroke="currentColor"
          strokeDasharray={dash}
          strokeLinecap="round"
          strokeWidth="1.5"
          x1="1"
          x2="19"
          y1="5"
          y2="5"
        />
      </svg>
      {label}
    </span>
  );
}

function TypeGraphModeState({
  lens,
  renderGraph,
  selectedTypeId,
}: {
  lens: TypeGraphLens;
  renderGraph: TypeRenderGraph;
  selectedTypeId: string | null;
}) {
  if (!selectedTypeId) {
    return (
      <CanvasNotice
        icon={MousePointerClick}
        title="Select a type"
        message="Select a type to inspect its graph."
      />
    );
  }

  if (lens !== "storage" && renderGraph.rawEdgeCount === 0) {
    return (
      <CanvasNotice
        icon={Workflow}
        title="Mode preview"
        message="This graph mode is available as a focused preview. Storage Shape is fully populated in this pass."
      />
    );
  }

  if (lens === "storage" && renderGraph.rawEdgeCount === 0) {
    if (typeGraphModel.selectedHasStorageEvidence(renderGraph, selectedTypeId)) {
      return null;
    }

    return (
      <CanvasNotice
        icon={ListTree}
        title="No storage relationships"
        message="No storage relationships found for this type."
      />
    );
  }

  return null;
}
