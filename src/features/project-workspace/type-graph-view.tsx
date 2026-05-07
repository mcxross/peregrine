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
  MoveTypeGraphEdge,
  MoveTypeGraphNode,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

type TypeGraphViewProps = {
  className?: string;
  movePackage: MovePackage | null;
  onOpenSourceLocation?: (location: TypeGraphSourceLocation) => void;
  onSelectType: (typeId: string) => void;
  packageName: string;
  selectedTypeId: string | null;
  typeGraph: MoveTypeGraph;
};

export type TypeGraphSourceLocation = {
  filePath: string;
  line: number;
};

type TypeGraphLens = "storage" | "functions" | "capabilities" | "generics" | "external";
type TypeGraphScope = "oneHop" | "twoHop" | "module" | "package" | "custom";
type TypeNodeKind = "builtin" | "external" | "field" | "framework" | "function" | "local";
type TypeGraphLayoutRole = "field" | "function" | "parent" | "target";
type RelationshipCategory =
  | "annotation"
  | "capability"
  | "external"
  | "field"
  | "generic"
  | "input"
  | "mutation"
  | "return";

type FunctionContext = {
  functionName: string;
  id: string;
  isEntry: boolean;
  isTransactionCallable: boolean;
  label: string;
  moduleName: string;
  signature: string;
  visibility: string;
};

type NormalizedEdge = {
  edge: MoveTypeGraphEdge;
  source: string;
  sourceKind: "function" | "type";
  sourceNode: MoveTypeGraphNode | null;
  target: string;
  targetKind: "function" | "type";
  targetNode: MoveTypeGraphNode | null;
};

type RenderNode = {
  addressLabel: string | null;
  abilitiesKnown: boolean;
  evidenceEdgeId: string | null;
  fieldInfo: FieldNodeInfo | null;
  entryFunctionCount: number;
  fieldCount: number;
  functionContext: FunctionContext | null;
  functionCount: number;
  groupLabel: string | null;
  genericArguments: Array<{ label: string; value: string }>;
  id: string;
  isGenericInstance: boolean;
  isSynthetic: boolean;
  kind: TypeNodeKind;
  label: string;
  layoutRole: TypeGraphLayoutRole;
  metricLabel: string;
  node: MoveTypeGraphNode | null;
  originLabel: string;
  riskTags: string[];
  roleLabel: string;
  selectTypeId: string;
  showGroupLabel: boolean;
  subtitle: string;
  tags: string[];
  sourceLocation: MoveSourceSpan | null;
  x: number;
  y: number;
};

type FieldNodeInfo = {
  baseType: string;
  confidence: string;
  declaredIn: string;
  declaringTypeId: string;
  fieldName: string;
  genericArguments: Array<{ label: string; value: string }>;
  resolvedType: string;
  sourceLocation: MoveSourceSpan | null;
  tags: string[];
  targetTypeId: string;
};

type RenderEdge = {
  category: RelationshipCategory;
  count: number;
  edge: MoveTypeGraphEdge;
  id: string;
  routeIndex: number;
  routeCount: number;
  source: string;
  target: string;
};

type GenericInstanceInfo = {
  arguments: Array<{ label: string; value: string }>;
  baseTypeId: string;
  declaringFieldName: string;
  declaringTypeId: string;
  id: string;
  label: string;
  sourceLocation: MoveSourceSpan | null;
};

type TypeRenderGraph = {
  capabilityCount: number;
  edges: RenderEdge[];
  edgeEvidence: RenderEdge[];
  externalCount: number;
  frameworkCount: number;
  functionCount: number;
  genericEdgeCount: number;
  hiddenEdgeCount: number;
  hiddenNodeCount: number;
  localCount: number;
  nodes: RenderNode[];
  rawEdgeCount: number;
  resourceCount: number;
  selectedLabel: string | null;
  selectedNode: RenderNode | null;
};

type TypeFlowNodeData = RenderNode & {
  color: string;
  dimmed: boolean;
  focused: boolean;
  incoming: number;
  onCollapseNeighbors: (typeId: string) => void;
  onExpandNode: (typeId: string) => void;
  onOpenSource: (span: MoveSourceSpan | null) => void;
  onSelectEvidenceEdge: (edgeId: string) => void;
  onShowFields: (typeId: string) => void;
  onShowFunctions: (typeId: string) => void;
  onSelectType: (typeId: string) => void;
  outgoing: number;
  selected: boolean;
};

type TypeFlowEdgeData = {
  active: boolean;
  category: RelationshipCategory;
  color: string;
  count: number;
  dimmed: boolean;
  label: string | null;
  routeCount: number;
  routeIndex: number;
};

const NODE_TYPES = {
  type: TypeGraphNode,
};

const EDGE_TYPES = {
  typeRelationship: TypeGraphEdge,
};

const TYPE_GRAPH_LENSES: Array<{
  caption: string;
  id: TypeGraphLens;
  label: string;
}> = [
  { caption: "Who owns what", id: "storage", label: "Storage Shape" },
  { caption: "Inputs & outputs", id: "functions", label: "Function Surface" },
  { caption: "Authority & permissions", id: "capabilities", label: "Capability View" },
  { caption: "Concrete forms", id: "generics", label: "Generic Instantiations" },
  { caption: "Imports & dependencies", id: "external", label: "External Types" },
];
const TYPE_GRAPH_SCOPES: Array<{
  id: TypeGraphScope;
  label: string;
  shortLabel: string;
}> = [
  { id: "oneHop", label: "1-hop", shortLabel: "1" },
  { id: "twoHop", label: "2-hop", shortLabel: "2" },
  { id: "module", label: "Module", shortLabel: "Mod" },
  { id: "package", label: "Package", shortLabel: "Pkg" },
  { id: "custom", label: "Query", shortLabel: "Q" },
];

const LOCAL_COLOR = "#38bdf8";
const SELECTED_COLOR = "#38bdf8";
const FRAMEWORK_COLOR = "#34d399";
const EXTERNAL_COLOR = "#a78bfa";
const BUILTIN_COLOR = "#94a3b8";
const FUNCTION_COLOR = "#60a5fa";
const GENERIC_COLOR = "#eab308";
const CAPABILITY_COLOR = "#f87171";
const DISPLAY_TYPE_KINDS = new Set(["struct", "enum", "datatype", "summaryType", "builtin"]);
const FRAMEWORK_ADDRESSES = new Set([
  "std",
  "sui",
  "0x1",
  "0x2",
  "0x0000000000000000000000000000000000000000000000000000000000000001",
  "0x0000000000000000000000000000000000000000000000000000000000000002",
]);
const FRAMEWORK_MODULES = new Set([
  "balance",
  "clock",
  "coin",
  "dynamic_field",
  "dynamic_object_field",
  "event",
  "object",
  "table",
  "transfer",
  "tx_context",
  "vec_map",
  "vec_set",
]);
const STORAGE_RELATIONSHIPS = new Set([
  "field",
  "genericArgument",
  "variantField",
  "vectorElement",
]);
const FUNCTION_RELATIONSHIPS = new Set([
  "annotation",
  "callTypeArgument",
  "cast",
  "parameter",
  "return",
]);
const GENERIC_RELATIONSHIPS = new Set([
  "callTypeArgument",
  "genericArgument",
  "phantomTypeParameter",
  "typeParameter",
  "vectorElement",
]);
const MAX_OVERVIEW_NODES = 72;
const MAX_FOCUSED_NODES = 110;
const MAX_RENDER_EDGES = 180;
const MAX_EDGE_LABELS = 10;
const MAX_FIELD_CAUSE_LABELS = 5;
const EDGE_LABEL_ZOOM_THRESHOLD = 1.15;
const TYPE_GRAPH_NODE_WIDTH = 256;
const TYPE_GRAPH_NODE_HEIGHT = 132;
const TYPE_GRAPH_FIELD_NODE_WIDTH = 208;
const TYPE_GRAPH_FIELD_NODE_HEIGHT = 78;
const TYPE_GRAPH_COLUMN_GAP = 192;
const TYPE_GRAPH_ROW_GAP = 152;
const TYPE_GRAPH_FUNCTION_COLUMN_X = 0;
const TYPE_GRAPH_LOCAL_COLUMN_X = TYPE_GRAPH_NODE_WIDTH + TYPE_GRAPH_COLUMN_GAP;
const TYPE_GRAPH_DEPENDENCY_COLUMN_X = TYPE_GRAPH_LOCAL_COLUMN_X + TYPE_GRAPH_NODE_WIDTH + TYPE_GRAPH_COLUMN_GAP;
const TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X = 112;
const TYPE_GRAPH_STORAGE_FIELD_COLUMN_X = TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X + TYPE_GRAPH_NODE_WIDTH + 170;
const TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X = TYPE_GRAPH_STORAGE_FIELD_COLUMN_X + TYPE_GRAPH_NODE_WIDTH + 180;
const TYPE_GRAPH_STAR_FIELD_OFFSET_X = 350;
const TYPE_GRAPH_STAR_FIELD_ARC_X = 76;
const TYPE_GRAPH_STAR_FIELD_ARC_Y = 245;
const TYPE_GRAPH_STAR_TARGET_OFFSET_X = 670;
const TYPE_GRAPH_STAR_TARGET_ARC_X = 70;
const TYPE_GRAPH_STAR_TARGET_ARC_Y = 345;
const TYPE_GRAPH_STAR_FIELD_MIN_GAP = 94;
const TYPE_GRAPH_STAR_TARGET_MIN_GAP = 166;
const TYPE_GRAPH_STAR_TARGET_GROUP_GAP = 26;
const COPY_DROP_STORE_ABILITIES = ["copy", "drop", "store"] as const;
const BUILTIN_TYPE_ABILITIES: Record<string, readonly string[]> = {
  address: COPY_DROP_STORE_ABILITIES,
  bool: COPY_DROP_STORE_ABILITIES,
  signer: ["drop"],
  u8: COPY_DROP_STORE_ABILITIES,
  u16: COPY_DROP_STORE_ABILITIES,
  u32: COPY_DROP_STORE_ABILITIES,
  u64: COPY_DROP_STORE_ABILITIES,
  u128: COPY_DROP_STORE_ABILITIES,
  u256: COPY_DROP_STORE_ABILITIES,
  vector: COPY_DROP_STORE_ABILITIES,
};
const SUI_FRAMEWORK_TYPE_ABILITIES: Record<string, readonly string[]> = {
  "object::ID": COPY_DROP_STORE_ABILITIES,
  "object::UID": ["store"],
  "table::Table": ["key", "store"],
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
  const functionIndex = React.useMemo(() => buildFunctionIndex(movePackage), [movePackage]);
  const fallbackTypeId = React.useMemo(
    () => importantLocalTypeId(typeGraph, movePackage),
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
    () => buildTypeRenderGraph({
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
  const stats = React.useMemo(() => graphStats(renderGraph.edges), [renderGraph.edges]);
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
    () => selectedNeighborhoodIds(renderGraph.edges, selectedTypeId, selectedEdge ?? hoveredEdge),
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

      console.info("[TypeGraph] source open requested", {
        filePath: span.filePath,
        line: span.startLine || 1,
        selectedEdgeId,
        selectedTypeId,
      });
      onOpenSourceLocation({ filePath: span.filePath, line: span.startLine || 1 });
    },
    [onOpenSourceLocation, selectedEdgeId, selectedTypeId],
  );
  const flowNodes = React.useMemo(
    () =>
      renderGraph.nodes.map<Node<TypeFlowNodeData>>((node) => {
        const selected = node.selectTypeId === selectedTypeId;
        const color = selected ? SELECTED_COLOR : nodeColor(node.kind, node.node);
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
    const fieldCauseLabelIds = fieldCauseEdgeLabelIds(
      renderGraph.edges,
      selectedTypeId,
      selectedEdgeId,
    );
    const prioritizedZoomLabelIds = zoomLabels
      ? prioritizedLabelEdgeIds(renderGraph.edges, selectedTypeId, MAX_EDGE_LABELS)
      : new Set<string>();
    const activePathFieldNodeId = fieldNodeEndpoint(selectedOrHoveredEdge);

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
      const color = active ? relationshipColor(edge.category) : "#64748b";
      const showLabel =
        (edge.id === selectedEdgeId || edge.id === hoveredEdgeId)
        || (
          !edgeTouchesFieldNode(edge)
          && (fieldCauseLabelIds.has(edge.id) || prioritizedZoomLabelIds.has(edge.id))
        );
      const dimmed = Boolean(selectedTypeId && !active);
      const contextualFieldPath = active && !selectedPath && edgeTouchesFieldNode(edge);
      const opacity = selectedPath ? 0.96 : active ? (contextualFieldPath ? 0.46 : 0.64) : 0.14;
      const strokeWidth = selectedPath
        ? 3
        : active
          ? contextualFieldPath ? 1.25 : Math.min(edgeStrokeWidth(edge), 1.55)
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
          label: showLabel ? edgeLabel(edge.edge, edge.count, edge.category) : null,
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
          strokeDasharray: edgeStrokeDash(edge),
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
        "grid min-h-0 gap-3 overflow-hidden",
        isFullscreen && "fixed bottom-3 left-3 right-3 top-[calc(58px+0.75rem)] z-[90] rounded-lg border border-[color:var(--app-border)] bg-[var(--app-window)] p-3 shadow-2xl shadow-black/60",
        isInspectorOpen
          ? "grid-rows-[minmax(0,1fr)_minmax(18rem,34vh)] lg:grid-cols-[minmax(0,1fr)_minmax(18rem,21rem)] lg:grid-rows-1"
          : "grid-cols-[minmax(0,1fr)_2.75rem]",
      )}
    >
      <div className="grid min-h-0 grid-rows-[40px_minmax(0,1fr)] overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
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
              console.info("[TypeGraph] edge clicked", {
                edgeId: edge.id,
                selectedTypeId,
              });
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
    <header className="min-w-0 overflow-x-auto border-b border-[color:var(--app-border)] bg-[color-mix(in_oklch,var(--app-chrome)_82%,transparent)] backdrop-blur-sm">
      <div className="grid min-w-[35rem] grid-cols-5">
        {TYPE_GRAPH_LENSES.map((lens) => (
          <button
            aria-pressed={activeLens === lens.id}
            className={cn(
              "relative grid min-w-0 content-center gap-0.5 border-r border-[color:var(--app-border)] px-2 pb-1 pt-1.5 text-center transition hover:bg-[var(--app-subtle)]",
              activeLens === lens.id
                ? "bg-sky-500/10 text-sky-100 after:absolute after:inset-x-2 after:bottom-0 after:h-px after:bg-sky-400/70"
                : "text-muted-foreground",
            )}
            key={lens.id}
            onClick={() => onLensChange(lens.id)}
            type="button"
          >
            <span className={cn(
              "truncate text-[11px] font-semibold leading-3.5",
              activeLens === lens.id ? "text-sky-50" : "text-foreground/85",
            )}>
              {lens.label}
            </span>
            <span className="truncate text-[9px] leading-3 text-muted-foreground/80">
              {lens.caption}
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
  const scopeLabel = graphScopeLabel(scope);
  const nodeCount = visibleCountLabel(renderGraph.nodes.length, renderGraph.hiddenNodeCount);
  const edgeCount = visibleCountLabel(renderGraph.rawEdgeCount, renderGraph.hiddenEdgeCount);
  const selectedPartialSource = selectedLocalSourceMissing(renderGraph);
  const selectedFieldName = selectedEdge?.edge.fieldName ?? null;
  const selectedFieldType = selectedEdge?.edge.typeExpression
    ? simplifyMoveTypeExpression(selectedEdge.edge.typeExpression)
    : null;
  const selectedTargetReferences = !selectedFieldName && renderGraph.selectedNode
    ? referencedByFieldEdges(renderGraph.edgeEvidence, renderGraph.selectedNode.id).length
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
              ? `referenced by ${pluralize(selectedTargetReferences, "field")}`
              : `centered on ${renderGraph.selectedLabel ?? packageName}`}
        </span>
        <span className="type-graph-footer-count-separator text-muted-foreground/45">·</span>
        <span className="type-graph-footer-count whitespace-nowrap text-muted-foreground/60">
          {selectedEdge ? edgeConfidence(selectedEdge) : `${nodeCount} nodes / ${edgeCount} edges`}
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
      <aside className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden rounded-md border border-[color:var(--app-border)] bg-background/92">
        <TypeInspectorColumnHeader onCollapse={onCollapse} />
        <div className="grid place-items-center px-4 text-center text-xs text-muted-foreground">
          Select a Move type to inspect storage shape, authority, generics, and external dependency links.
        </div>
      </aside>
    );
  }

  const fieldEdges = selectedNodeEdges(renderGraph.edgeEvidence, node.id, "field");
  const functionEdges = selectedNodeEdges(renderGraph.edgeEvidence, node.id, "input")
    .concat(selectedNodeEdges(renderGraph.edgeEvidence, node.id, "return"));
  const securityNotes = nodeSecurityNotes(node, renderGraph.edgeEvidence);

  return (
    <aside className="grid min-h-0 grid-rows-[auto_auto_auto_minmax(0,1fr)_auto] overflow-hidden rounded-md border border-[color:var(--app-border)] bg-background/92">
      <TypeInspectorColumnHeader onCollapse={onCollapse} />
      <header className="border-b border-[color:var(--app-border)] px-3 py-3">
        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-2">
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold text-foreground">{node.label}</h3>
            <p className="mt-0.5 truncate text-[11px] text-muted-foreground">{node.subtitle}</p>
          </div>
          <span className={cn("h-6 rounded px-2 py-1 text-[10px] font-semibold", nodeRiskClass(node))}>
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
  const sourceLabel = edgeEndpointLabel(edge.source, edge);
  const targetLabel = edgeEndpointLabel(edge.target, edge);
  const fieldLocation = edgeSourceLocation(edge.edge);
  const resolvedTypeNodeId = resolvedTypeNodeIdForEdge(renderGraph, edge);
  const resolvedTypeLocation = resolvedTypeNodeId
    ? renderNodeSourceLocation(renderGraph, resolvedTypeNodeId)
    : null;
  const location = fieldLocation ?? resolvedTypeLocation;
  const evidenceItems = edgeEvidenceItems(edge);
  const declaredIn = edge.edge.declaringTypeId
    ? compactEdgeEndpoint(edge.edge.declaringTypeId)
    : compactEdgeEndpoint(edge.source);
  const isFieldEvidence = Boolean(edge.edge.fieldName || edge.source.startsWith("field-node:") || edge.target.startsWith("field-node:"));
  const resolvedType = edge.edge.typeExpression
    ? simplifyMoveTypeExpression(edge.edge.typeExpression)
    : targetLabel;
  const genericArguments = edge.edge.typeExpression
    ? genericArgumentsFromTypeExpression(edge.edge.typeExpression, null)
    : [];
  const baseType = edge.edge.fieldName ? compactEdgeEndpoint(edge.edge.target ?? edge.target) : targetLabel;

  return (
    <aside className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden rounded-md border border-[color:var(--app-border)] bg-background/92 text-xs">
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
          <InspectorRow label="Relation" value={edgeRelationName(edge)} />
          {edge.edge.fieldName ? <InspectorRow label="Field" value={edge.edge.fieldName} /> : null}
          <InspectorRow label="Declared in" value={declaredIn} />
          {isFieldEvidence ? <InspectorRow label="Resolved type" value={resolvedType} /> : null}
          {isFieldEvidence ? <InspectorRow label="Base type" value={baseType} /> : null}
          {!isFieldEvidence ? <InspectorRow label="Label" value={edgeLabel(edge.edge, edge.count, edge.category)} /> : null}
          <InspectorRow label="Confidence" value={edgeConfidence(edge)} />
          <InspectorRow label="Risk" value={edgeRiskLevel(edge)} />
          {!isFieldEvidence && edge.edge.typeExpression ? <InspectorRow label="Type" value={edge.edge.typeExpression} /> : null}
          {edge.edge.typeArgumentName ? <InspectorRow label="Argument" value={`${edge.edge.typeArgumentName}${edge.edge.typeExpression ? ` = ${edge.edge.typeExpression}` : ""}`} /> : null}
          {edge.edge.functionName ? <InspectorRow label="Function" value={edge.edge.functionName} /> : null}
          {edge.edge.parameterName ? <InspectorRow label="Parameter" value={edge.edge.parameterName} /> : null}
          {location ? (
            <InspectorRow
              label="Location"
              value={`${compactPath(location.filePath)}:${location.startLine}`}
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
        className="grid size-7 place-items-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
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
    <aside className="flex min-h-0 flex-col items-center gap-2 overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-1.5">
      <button
        aria-label="Show type details"
        className="grid size-8 place-items-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
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
  const references = referencedByFieldEdges(renderGraph.edgeEvidence, node.id);
  return (
    <div className="grid gap-3">
      <dl className="grid gap-1.5">
        <InspectorRow label="Kind" value={node.roleLabel} />
        <InspectorRow label="Origin" value={node.originLabel} />
        <InspectorRow label="Abilities" value={abilitySummary(node)} />
        <InspectorRow label="Fields" value={String(node.fieldCount)} />
        <InspectorRow label="Functions" value={String(node.functionCount)} />
        <InspectorRow label="Entry functions" value={String(node.entryFunctionCount)} />
        <InspectorRow label="Visible edges" value={String(renderGraph.rawEdgeCount)} />
        {node.addressLabel ? <InspectorRow label="Address" value={node.addressLabel} /> : null}
        {node.node?.packageName ? <InspectorRow label="Package" value={node.node.packageName} /> : null}
        {node.node?.moduleName ? <InspectorRow label="Module" value={node.node.moduleName} /> : null}
        {source ? <InspectorRow label="Source" value={`${compactPath(source.filePath)}:${source.startLine}`} /> : null}
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
                  {compactEdgeEndpoint(edge.edge.declaringTypeId ?? edge.source)}.{edge.edge.fieldName ?? edge.edge.declaringFieldName}
                </span>
                <span className="truncate text-muted-foreground">
                  {edge.edge.typeExpression ? simplifyMoveTypeExpression(edge.edge.typeExpression) : compactEdgeEndpoint(edge.source)}
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
        {nodeInterpretation(node)}
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
          onClick={() => onSelectType(edgeNavigationTarget(edge, nodeId))}
          type="button"
        >
          <span className="grid min-w-0 gap-0.5">
            <span className="min-w-0 truncate text-[11px] font-semibold text-foreground">
              {edge.edge.fieldName ?? edgeLabel(edge.edge, edge.count, edge.category)}
            </span>
            {edge.edge.fieldName ? (
              <span className="min-w-0 truncate text-[10px] text-muted-foreground">
                {edgeLabel(edge.edge, edge.count, edge.category)}
              </span>
            ) : null}
          </span>
          <span className="truncate text-right text-[10px] text-muted-foreground">
            {edgeDisplayEndpoint(edge, nodeId)}
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
  const Icon = nodeIcon(data.kind, data.node);
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
            <div className="truncate text-muted-foreground">Declared in: {compactEdgeEndpoint(fieldInfo.declaringTypeId)}</div>
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
              {argument.label}: {compactTypeLabel(argument.value)}
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
    ["Field node", relationshipColor("field")],
  ];
  const edgeItems: Array<[string, string]> = [
    ["Field", relationshipColor("field")],
    ["Generic", relationshipColor("generic")],
    ["Auth", relationshipColor("capability")],
    ["Trust", relationshipColor("external")],
    ["I/O", relationshipColor("input")],
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
    if (selectedHasStorageEvidence(renderGraph, selectedTypeId)) {
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

function selectedHasStorageEvidence(renderGraph: TypeRenderGraph, selectedTypeId: string | null) {
  if (!selectedTypeId) {
    return false;
  }

  return renderGraph.edgeEvidence.some(
    (edge) =>
      (edge.source === selectedTypeId || edge.target === selectedTypeId)
      && (edge.category === "field" || edge.category === "generic"),
  );
}

function selectedLocalSourceMissing(renderGraph: TypeRenderGraph) {
  const node = renderGraph.selectedNode;

  return Boolean(
    node
    && node.kind === "local"
    && node.node
    && !node.isSynthetic
    && !node.sourceLocation,
  );
}

function CanvasNotice({
  icon: Icon,
  message,
  title,
}: {
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  message: string;
  title: string;
}) {
  return (
    <div className="pointer-events-none absolute left-1/2 top-1/2 z-10 grid w-[min(24rem,calc(100%-3rem))] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-md border border-dashed border-[color:var(--app-border)] bg-background/70 px-4 py-5 text-center shadow-sm backdrop-blur-[2px]">
      <Icon className="size-5 text-muted-foreground" aria-hidden="true" />
      <div className="mt-2 text-sm font-semibold text-foreground">{title}</div>
      <p className="mt-1 text-xs leading-5 text-muted-foreground">{message}</p>
    </div>
  );
}

function EmptyTypeGraphState({
  className,
  packageName,
}: {
  className: string;
  packageName: string;
}) {
  return (
    <div className={cn(className, "grid place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-6 text-center")}>
      <div className="max-w-md">
        <div className="text-sm font-semibold text-foreground">No type graph found</div>
        <p className="mt-2 text-sm text-muted-foreground">
          Peregrine did not find graphable types for {packageName}.
        </p>
      </div>
    </div>
  );
}

function visibleCountLabel(visible: number, hidden: number) {
  return hidden > 0 ? `${visible}/${visible + hidden}` : String(visible);
}

function graphScopeLabel(scope: TypeGraphScope) {
  switch (scope) {
    case "oneHop":
      return "1-hop neighborhood";
    case "twoHop":
      return "2-hop neighborhood";
    case "module":
      return "module scope";
    case "package":
      return "package scope";
    case "custom":
      return "custom query scope";
  }
}

function selectedNodeEdges(edges: RenderEdge[], nodeId: string, category?: RelationshipCategory) {
  return edges.filter(
    (edge) =>
      (edge.source === nodeId || edge.target === nodeId)
      && (!category || edge.category === category),
  );
}

function referencedByFieldEdges(edges: RenderEdge[], nodeId: string) {
  return edges.filter(
    (edge) =>
      edge.target === nodeId
      && (edge.category === "field" || edge.category === "generic")
      && Boolean(edge.edge.fieldName ?? edge.edge.declaringFieldName),
  );
}

function nodeRiskClass(node: RenderNode) {
  if (node.riskTags.some((tag) => tag.includes("capability") || tag.includes("high"))) {
    return "bg-rose-500/15 text-rose-200";
  }

  if (node.riskTags.some((tag) => tag.includes("external"))) {
    return "bg-violet-500/15 text-violet-200";
  }

  if (node.riskTags.some((tag) => tag.includes("generic"))) {
    return "bg-yellow-500/15 text-yellow-200";
  }

  return "bg-sky-500/15 text-sky-200";
}

function nodeInterpretation(node: RenderNode) {
  if (node.riskTags.includes("capability")) {
    return "Authority-bearing type. Review creation, transfer, and every public entry function that accepts this type.";
  }

  if (node.riskTags.includes("resource")) {
    return "Resource-like state type. Inspect storage fields, mutable references, and public entry points that can move value.";
  }

  if (node.riskTags.includes("trust boundary")) {
    return "External trust-boundary type. Confirm package address, freshness assumptions, and validation behavior before value movement.";
  }

  if (node.riskTags.includes("generic container") || node.riskTags.includes("generic asset")) {
    return "Generic/container type. Review concrete type arguments and phantom markers used for accounting or authorization.";
  }

  return "Type participates in the focused Move type graph. Switch modes to inspect storage, function surface, authority, generics, or external dependency links.";
}

function nodeSecurityNotes(node: RenderNode, edges: RenderEdge[]) {
  const notes: string[] = [];

  if (node.riskTags.includes("resource")) {
    notes.push("High impact type candidate: has key ability or participates in storage edges.");
  }
  if (node.riskTags.includes("capability")) {
    notes.push("Capability-like type: confirm it is not copyable and cannot be publicly obtained.");
  }
  if (node.riskTags.includes("trust boundary")) {
    notes.push("External dependency type: review package address and whether it controls validation or value movement.");
  }
  if (edges.some((edge) => edge.category === "capability" && (edge.source === node.id || edge.target === node.id))) {
    notes.push("Capability-linked path visible in this scope.");
  }
  if (edges.some((edge) => edge.category === "input" && edge.edge.isMutable && (edge.source === node.id || edge.target === node.id))) {
    notes.push("Mutable reference path visible from callable code.");
  }

  return notes.length ? notes : ["No high-signal security notes in the current focused scope."];
}

function edgeRelationName(edge: RenderEdge) {
  if (edge.category === "input" && edge.edge.isMutable) {
    return "mutable_borrow";
  }

  if (edge.category === "input" && edge.edge.isReference) {
    return "immutable_borrow";
  }

  if (edge.category === "capability") {
    return "authorizes";
  }

  if (edge.category === "generic") {
    return edge.edge.relationship === "phantomTypeParameter" ? "phantom_arg" : "generic_arg";
  }

  if (edge.category === "external") {
    return "external_dependency";
  }

  if (edge.category === "field") {
    return "field";
  }

  if (edge.category === "return") {
    return "function_output";
  }

  return edge.edge.relationship;
}

function edgeConfidence(edge: RenderEdge) {
  if (["syntactic", "inferred", "heuristic"].includes(edge.edge.confidence)) {
    return edge.edge.confidence;
  }

  if (edge.category === "capability" || edge.category === "mutation") {
    return "inferred";
  }

  if (edge.category === "external" || edge.edge.confidence === "summary") {
    return "heuristic";
  }

  return "syntactic";
}

function edgeRiskLevel(edge: RenderEdge) {
  if (edge.category === "capability") {
    return "high";
  }

  if (edge.category === "external" || edge.category === "mutation") {
    return "medium";
  }

  if (edge.edge.isMutable) {
    return "low";
  }

  return "none";
}

function edgeSourceLocation(edge: MoveTypeGraphEdge) {
  return Array.isArray(edge.sourceSpans) ? edge.sourceSpans[0] ?? null : null;
}

function edgeEvidenceItems(edge: RenderEdge) {
  return Array.isArray(edge.edge.evidence) && edge.edge.evidence.length
    ? edge.edge.evidence
    : [edgeLabel(edge.edge, edge.count, edge.category)];
}

function renderNodeSourceLocation(renderGraph: TypeRenderGraph, nodeId: string) {
  const node = renderGraph.nodes.find((item) => item.id === nodeId || item.selectTypeId === nodeId);
  return node?.sourceLocation ?? null;
}

function resolvedTypeNodeIdForEdge(renderGraph: TypeRenderGraph, edge: RenderEdge) {
  const fieldId = fieldNodeEndpoint(edge);

  if (fieldId) {
    const fieldNode = renderGraph.nodes.find((node) => node.id === fieldId);
    return fieldNode?.fieldInfo?.targetTypeId ?? null;
  }

  return edge.target;
}

function edgeStrokeDash(edge: RenderEdge) {
  const confidence = edgeConfidence(edge);

  if (confidence === "heuristic") {
    return "1 6";
  }

  if (confidence === "inferred") {
    return "6 7";
  }

  return undefined;
}

function edgeStrokeWidth(edge: RenderEdge) {
  const risk = edgeRiskLevel(edge);

  if (risk === "high") {
    return 2.4;
  }

  if (risk === "medium") {
    return 2;
  }

  return 1.7;
}

function compactPath(path: string) {
  return path.split("/").slice(-2).join("/");
}

function compactEdgeEndpoint(endpoint: string) {
  if (endpoint.startsWith("field-node:")) {
    const parts = endpoint.split(":");
    return parts.length >= 4 ? `field(${parts[2]})` : "field";
  }

  if (endpoint.startsWith("generic-instance:")) {
    return "generic instance";
  }

  const parts = endpoint.split("::");
  return parts.length >= 2 ? parts.slice(-2).join("::") : endpoint.replace(/^.*:/, "");
}

function edgeEndpointLabel(endpoint: string, edge: RenderEdge) {
  if (endpoint.startsWith("field-node:")) {
    return edge.edge.fieldName ? `field(${edge.edge.fieldName})` : "field";
  }

  if (endpoint.startsWith("generic-instance:")) {
    return edge.edge.typeExpression ?? "generic instance";
  }

  return compactEdgeEndpoint(endpoint);
}

function edgeNavigationTarget(edge: RenderEdge, currentNodeId: string | null) {
  const candidates = [edge.source, edge.target].filter(
    (id) => id !== currentNodeId && !id.startsWith("function:"),
  );

  if (candidates.length) {
    return candidates[0];
  }

  if (edge.source.startsWith("function:")) {
    return edge.target;
  }

  if (edge.target.startsWith("function:")) {
    return edge.source;
  }

  return edge.target === currentNodeId ? edge.source : edge.target;
}

function edgeDisplayEndpoint(edge: RenderEdge, currentNodeId: string | null) {
  const endpoint = currentNodeId && edge.target === currentNodeId ? edge.source : edge.target;
  return compactEdgeEndpoint(endpoint);
}

function shortAddress(address: string | null | undefined) {
  if (!address) {
    return null;
  }

  return address.length > 16 ? `${address.slice(0, 8)}...${address.slice(-6)}` : address;
}

function sourceLocationFromMoveNode(node: MoveTypeGraphNode | null | undefined): MoveSourceSpan | null {
  if (!node?.filePath) {
    return null;
  }

  return {
    endByte: 0,
    endLine: 1,
    filePath: node.filePath,
    startByte: 0,
    startLine: 1,
  };
}

function buildTypeRenderGraph({
  collapsedNodeIds,
  customQuery,
  expandedNodeIds,
  functionIndex,
  graph,
  lens,
  movePackage,
  scope,
  selectedTypeId,
}: {
  collapsedNodeIds: Set<string>;
  customQuery: string;
  expandedNodeIds: Set<string>;
  functionIndex: Map<string, FunctionContext>;
  graph: MoveTypeGraph;
  lens: TypeGraphLens;
  movePackage: MovePackage | null;
  scope: TypeGraphScope;
  selectedTypeId: string | null;
}): TypeRenderGraph {
  const packagePath = movePackage?.path ?? null;
  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  const localIds = new Set(
    graph.nodes
      .filter((node) => isLocalPackageType(node, packagePath))
      .map((node) => node.id),
  );
  const selectedNode = selectedTypeId ? nodeById.get(selectedTypeId) ?? null : null;
  const selectedRenderableId =
    selectedNode && isRenderableTypeNode(selectedNode) ? selectedNode.id : null;
  const allNormalizedEdges = graph.edges
    .map((edge) => normalizeEdge(edge, nodeById, packagePath))
    .filter((edge): edge is NormalizedEdge => Boolean(edge));
  const normalizedEdges = allNormalizedEdges
    .filter((edge) => edgeMatchesLens(edge, lens, localIds, packagePath));
  const compactStorageDefault = lens === "storage" && scope === "oneHop" && !customQuery.trim();
  const selectedIds = graphScopeIds({
    customQuery,
    edges: normalizedEdges,
    nodes: graph.nodes,
    packagePath,
    collapsedNodeIds,
    expandedNodeIds,
    scope,
    selectedId: selectedRenderableId,
    selectedNode,
  });
  const fieldFocusLayout = Boolean(
    lens === "storage"
    && scope === "oneHop"
    && selectedRenderableId
    && !customQuery.trim(),
  );
  const visibleNormalizedEdges = normalizedEdges.filter((normalized) => {
    const shouldInclude = selectedIds
      ? selectedIds.has(normalized.source) && selectedIds.has(normalized.target)
      : edgeTouchesPackage(normalized, localIds, packagePath);

    return shouldInclude;
  });
  const exactGenericArgumentKeys = exactGenericArgumentEdgeKeys(visibleNormalizedEdges);
  const renderNormalizedEdges = visibleNormalizedEdges.filter(
    (edge) =>
      !isOwnerGenericShortcut(edge, exactGenericArgumentKeys)
      && !(compactStorageDefault && edge.edge.relationship === "genericArgument"),
  );
  const genericInstances = genericInstanceInfo(visibleNormalizedEdges, nodeById, localIds, packagePath);
  const outgoingFieldFocusEdges = fieldFocusLayout && selectedRenderableId
    ? renderNormalizedEdges
      .filter((edge) =>
        edge.source === selectedRenderableId
        && isStorageFieldRelationship(edge.edge.relationship)
        && Boolean(edge.edge.fieldName),
      )
    : [];
  const incomingFieldFocusEdges = fieldFocusLayout && selectedRenderableId && outgoingFieldFocusEdges.length === 0
    ? renderNormalizedEdges
      .filter((edge) =>
        edge.target === selectedRenderableId
        && isStorageFieldRelationship(edge.edge.relationship)
        && Boolean(edge.edge.fieldName),
      )
    : [];
  const fieldFocusEdges = [...outgoingFieldFocusEdges, ...incomingFieldFocusEdges]
    .sort((left, right) => fieldEdgeSort(left, right));
  const fieldFocusSourceIds = new Set(fieldFocusEdges.map((edge) => edge.source));
  const fieldFocusTargetIds = new Set(fieldFocusEdges.map((edge) => edge.target));
  const includedTypeIds = new Set<string>();
  const includedFunctionIds = new Set<string>();
  const includedGenericInstanceIds = new Set<string>();
  const syntheticFieldNodes: RenderNode[] = [];
  const edgeGroups = new Map<string, RenderEdge>();

  if (selectedRenderableId) {
    includedTypeIds.add(selectedRenderableId);
  } else {
    localIds.forEach((id) => includedTypeIds.add(id));
  }

  const edgesForRender = fieldFocusLayout ? fieldFocusEdges : renderNormalizedEdges;

  for (const normalized of edgesForRender) {
    const genericInstance = genericInstanceForEdge(normalized, genericInstances);
    const targetId =
      isStorageFieldRelationship(normalized.edge.relationship) && genericInstance && !fieldFocusLayout
        ? genericInstance.id
        : normalized.target;

    if (fieldFocusLayout && selectedRenderableId) {
      const renderEdge = normalizedEdgeForRender(normalized, nodeById);
      const category = relationshipCategory(normalized, localIds, packagePath);
      const fieldId = fieldNodeId(
        normalized.source,
        normalized.edge.fieldName ?? "field",
        targetId,
      );
      const fieldEvidenceEdgeId = edgeGroupKey(fieldId, targetId, renderEdge, category);
      const fieldNode = syntheticFieldNode(
        normalized,
        syntheticFieldNodes.length,
        fieldFocusEdges.length,
        normalized.source,
        targetId,
        fieldEvidenceEdgeId,
        genericInstance,
      );
      syntheticFieldNodes.push(fieldNode);

      if (normalized.sourceKind === "type") {
        includedTypeIds.add(normalized.source);
      }

      if (normalized.targetKind === "type") {
        includedTypeIds.add(normalized.target);
      }

      addRenderEdge(edgeGroups, {
        category,
        edge: renderEdge,
        source: normalized.source,
        target: fieldNode.id,
      });
      addRenderEdge(edgeGroups, {
        category,
        edge: renderEdge,
        source: fieldNode.id,
        target: targetId,
      });
      continue;
    }

    const renderSource =
      normalized.edge.relationship === "genericArgument" && genericInstance
        ? genericInstance.id
        : normalized.source;
    const renderTarget = targetId;

    if (normalized.sourceKind === "function") {
      includedFunctionIds.add(normalized.source);
    } else if (renderSource === normalized.source) {
      includedTypeIds.add(normalized.source);
    } else {
      includedGenericInstanceIds.add(renderSource);
    }

    if (normalized.targetKind === "function") {
      includedFunctionIds.add(normalized.target);
    } else if (renderTarget === normalized.target) {
      includedTypeIds.add(normalized.target);
    } else {
      includedGenericInstanceIds.add(renderTarget);
    }

    addRenderEdge(edgeGroups, {
      category: relationshipCategory(normalized, localIds, packagePath),
      edge: normalizedEdgeForRender(normalized, nodeById),
      source: renderSource,
      target: renderTarget,
    });
  }

  const typeNodes = [...includedTypeIds]
    .map((id) => nodeById.get(id))
    .filter((node): node is MoveTypeGraphNode => Boolean(node && isRenderableTypeNode(node)));
  const localNodes = sortTypeNodes(typeNodes.filter((node) => localIds.has(node.id)), selectedRenderableId);
  const dependencyNodes = sortTypeNodes(typeNodes.filter((node) => !localIds.has(node.id)), selectedRenderableId);
  const fieldFocusSourceNodes = fieldFocusLayout && selectedRenderableId
    ? sortTypeNodes(
      typeNodes.filter((node) =>
        fieldFocusEdges.length ? fieldFocusSourceIds.has(node.id) : node.id === selectedRenderableId,
      ),
      selectedRenderableId,
    )
    : localNodes;
  const fieldFocusTargetNodes = fieldFocusLayout && selectedRenderableId
    ? sortTypeNodes(
      typeNodes.filter((node) => fieldFocusTargetIds.has(node.id)),
      selectedRenderableId,
    )
    : dependencyNodes;
  const syntheticGenericNodes = [...includedGenericInstanceIds]
    .map((id) => genericInstances.get(id))
    .filter((instance): instance is GenericInstanceInfo => Boolean(instance))
    .sort((left, right) => left.label.localeCompare(right.label) || left.id.localeCompare(right.id));
  const functionNodes = [...includedFunctionIds].sort((left, right) =>
    functionLabel(left, functionIndex).localeCompare(functionLabel(right, functionIndex)),
  );
  const renderEdges = assignEdgeRoutes([...edgeGroups.values()].sort((left, right) => left.id.localeCompare(right.id)));
  const edgeEvidence = collectEdgeEvidence(allNormalizedEdges, localIds, packagePath);
  const metricsByType = typeNodeMetrics(edgeEvidence, functionIndex);
  const maxRows = Math.max(
    fieldFocusLayout ? Math.max(1, fieldFocusSourceNodes.length) : localNodes.length,
    syntheticFieldNodes.length,
    fieldFocusTargetNodes.length + syntheticGenericNodes.length,
    functionNodes.length,
    1,
  );
  const storageLayout = lens === "storage" && functionNodes.length === 0;
  const renderNodes = fieldFocusLayout && selectedRenderableId
    ? [
      ...fieldFocusSourceNodes.map((node, index) =>
        typeRenderNode(
          node,
          classifyTypeNode(node, packagePath),
          index,
          maxRows,
          metricsByType.get(node.id),
          null,
          storageLayout,
          "parent",
        ),
      ),
      ...syntheticFieldNodes,
      ...fieldFocusTargetNodes.map((node, index) =>
        typeRenderNode(
          node,
          classifyTypeNode(node, packagePath),
          index,
          maxRows,
          metricsByType.get(node.id),
          null,
          storageLayout,
          "target",
        ),
      ),
      ...syntheticGenericNodes.map((instance, index) =>
        syntheticGenericInstanceNode(
          instance,
          nodeById.get(instance.baseTypeId) ?? null,
          fieldFocusTargetNodes.length + index,
          maxRows,
          packagePath,
          storageLayout,
        ),
      ),
    ]
    : [
    ...functionNodes.map((id, index) => syntheticFunctionNode(id, index, maxRows, functionIndex)),
    ...localNodes.map((node, index) =>
      typeRenderNode(node, "local", index, maxRows, metricsByType.get(node.id), null, storageLayout, "parent"),
    ),
    ...dependencyNodes.map((node, index) =>
      typeRenderNode(
        node,
        classifyTypeNode(node, packagePath),
        index,
        maxRows,
        metricsByType.get(node.id),
        null,
        storageLayout,
        "target",
      ),
    ),
    ...syntheticGenericNodes.map((instance, index) =>
      syntheticGenericInstanceNode(
        instance,
        nodeById.get(instance.baseTypeId) ?? null,
        dependencyNodes.length + index,
        maxRows,
        packagePath,
        storageLayout,
      ),
    ),
  ];
  const limitedGraph = limitRenderGraph(renderNodes, renderEdges, selectedRenderableId);
  const limitedTypeNodes = limitedGraph.nodes
    .map((node) => node.node)
    .filter((node): node is MoveTypeGraphNode => Boolean(node));
  const limitedDependencyNodes = limitedTypeNodes.filter((node) => !localIds.has(node.id));
  const selectedLabel = selectedRenderableId
    ? typeDisplayName(nodeById.get(selectedRenderableId)!, classifyTypeNode(nodeById.get(selectedRenderableId)!, packagePath))
    : null;
  const capabilityCount = limitedTypeNodes.filter(isCapabilityLike).length;
  const resourceCount = limitedTypeNodes.filter(isResourceLike).length;
  const rawEdgeCount = limitedGraph.edges.reduce((total, edge) => total + edge.count, 0);
  const genericEdgeCount = limitedGraph.edges
    .filter((edge) => edge.category === "generic")
    .reduce((total, edge) => total + edge.count, 0);

  return {
    capabilityCount,
    edges: limitedGraph.edges,
    edgeEvidence,
    externalCount: limitedDependencyNodes.filter((node) => classifyTypeNode(node, packagePath) === "external").length,
    frameworkCount: limitedDependencyNodes.filter((node) => {
      const kind = classifyTypeNode(node, packagePath);
      return kind === "framework" || kind === "builtin";
    }).length,
    functionCount: limitedGraph.nodes.filter((node) => node.kind === "function").length,
    genericEdgeCount,
    hiddenEdgeCount: limitedGraph.hiddenEdgeCount,
    hiddenNodeCount: limitedGraph.hiddenNodeCount,
    localCount: limitedTypeNodes.filter((node) => localIds.has(node.id)).length,
    nodes: limitedGraph.nodes,
    rawEdgeCount,
    resourceCount,
    selectedLabel,
    selectedNode: limitedGraph.nodes.find((node) => node.id === selectedRenderableId) ?? null,
  };
}

function graphScopeIds({
  collapsedNodeIds,
  customQuery,
  edges,
  expandedNodeIds,
  nodes,
  packagePath,
  scope,
  selectedId,
  selectedNode,
}: {
  collapsedNodeIds: Set<string>;
  customQuery: string;
  edges: NormalizedEdge[];
  expandedNodeIds: Set<string>;
  nodes: MoveTypeGraphNode[];
  packagePath: string | null;
  scope: TypeGraphScope;
  selectedId: string | null;
  selectedNode: MoveTypeGraphNode | null;
}) {
  if (scope === "package") {
    return null;
  }

  if (scope === "custom") {
    const query = customQuery.trim();
    if (!query) {
      return selectedId ? oneHopNeighborhood(selectedId, edges) : null;
    }

    const matchingIds = new Set<string>();

    for (const node of nodes) {
      if (isRenderableTypeNode(node) && nodeMatchesQuery(node, query, packagePath)) {
        matchingIds.add(node.id);
      }
    }

    for (const edge of edges) {
      if (edgeMatchesQuery(edge, query)) {
        matchingIds.add(edge.source);
        matchingIds.add(edge.target);
      }
    }

    return expandIds(matchingIds, edges, 1);
  }

  if (!selectedId) {
    return null;
  }

  const expandedSeeds = new Set([selectedId, ...expandedNodeIds]);
  let ids: Set<string>;

  if (scope === "oneHop") {
    ids = expandIds(expandedSeeds, edges, 1);
    collapsedNodeIds.forEach((id) => removeNeighbors(ids, edges, id));
    return ids;
  }

  if (scope === "twoHop") {
    ids = expandIds(expandedSeeds, edges, 2);
    collapsedNodeIds.forEach((id) => removeNeighbors(ids, edges, id));
    return ids;
  }

  if (scope === "module" && selectedNode?.moduleName) {
    const ids = new Set<string>();

    for (const node of nodes) {
      if (
        isRenderableTypeNode(node)
        && node.packagePath === selectedNode.packagePath
        && node.moduleName === selectedNode.moduleName
      ) {
        ids.add(node.id);
      }
    }

    const moduleIds = expandIds(ids, edges, 1);
    collapsedNodeIds.forEach((id) => removeNeighbors(moduleIds, edges, id));
    return moduleIds;
  }

  ids = oneHopNeighborhood(selectedId, edges);
  collapsedNodeIds.forEach((id) => removeNeighbors(ids, edges, id));
  return ids;
}

function addRenderEdge(
  edgeGroups: Map<string, RenderEdge>,
  {
    category,
    edge,
    source,
    target,
  }: {
    category: RelationshipCategory;
    edge: MoveTypeGraphEdge;
    source: string;
    target: string;
  },
) {
  const groupKey = edgeGroupKey(source, target, edge, category);
  const current = edgeGroups.get(groupKey);

  if (current) {
    current.count += 1;
    return;
  }

  edgeGroups.set(groupKey, {
    category,
    count: 1,
    edge,
    id: groupKey,
    routeCount: 1,
    routeIndex: 0,
    source,
    target,
  });
}

function fieldEdgeSort(left: NormalizedEdge, right: NormalizedEdge) {
  return (
    fieldSortPriority(left) - fieldSortPriority(right)
    || (left.edge.fieldName ?? "").localeCompare(right.edge.fieldName ?? "")
    || left.target.localeCompare(right.target)
  );
}

function fieldSortPriority(edge: NormalizedEdge) {
  const name = edge.edge.fieldName ?? "";

  if (name === "id" || name === "uid") {
    return 0;
  }

  if (edge.targetNode && isResourceLike(edge.targetNode)) {
    return 1;
  }

  if (edge.targetNode?.kind === "builtin") {
    return 4;
  }

  if (isGenericContainer(edge.targetNode)) {
    return 2;
  }

  return 3;
}

function oneHopNeighborhood(selectedId: string, edges: NormalizedEdge[]) {
  return expandIds(new Set([selectedId]), edges, 1);
}

function expandIds(seedIds: Set<string>, edges: NormalizedEdge[], depth: number) {
  const ids = new Set(seedIds);
  let frontier = new Set(seedIds);

  for (let index = 0; index < depth; index += 1) {
    const next = new Set<string>();

    for (const edge of edges) {
      if (frontier.has(edge.source)) {
        next.add(edge.target);
      }
      if (frontier.has(edge.target)) {
        next.add(edge.source);
      }
    }

    next.forEach((id) => ids.add(id));
    frontier = next;
  }

  return ids;
}

function removeNeighbors(ids: Set<string>, edges: NormalizedEdge[], nodeId: string) {
  for (const edge of edges) {
    if (edge.source === nodeId) {
      ids.delete(edge.target);
    }
    if (edge.target === nodeId) {
      ids.delete(edge.source);
    }
  }
  ids.add(nodeId);
}

function nodeMatchesQuery(node: MoveTypeGraphNode, query: string, packagePath: string | null) {
  const lowerQuery = query.toLowerCase();
  const [prefix, rawValue] = lowerQuery.includes(":") ? lowerQuery.split(/:(.*)/, 2) : ["", lowerQuery];
  const value = rawValue ?? "";

  if (prefix === "ability") {
    return effectiveTypeAbilities(node).some((ability) => ability.toLowerCase() === value);
  }

  if (prefix === "origin") {
    return nodeOriginLabel(node, classifyTypeNode(node, packagePath)).toLowerCase().includes(value);
  }

  if (prefix === "kind") {
    return nodeRoleLabel(node, classifyTypeNode(node, packagePath)).toLowerCase().includes(value);
  }

  if (prefix === "generic") {
    return (
      node.qualifiedName.toLowerCase().includes(value)
      || node.name.toLowerCase().includes(value)
      || (node.typeParameters ?? []).some((parameter) => parameter.name.toLowerCase().includes(value))
    );
  }

  return (
    node.name.toLowerCase().includes(lowerQuery)
    || node.qualifiedName.toLowerCase().includes(lowerQuery)
    || (node.moduleName?.toLowerCase().includes(lowerQuery) ?? false)
    || effectiveTypeAbilities(node).some((ability) => ability.toLowerCase().includes(lowerQuery))
  );
}

function edgeMatchesQuery(edge: NormalizedEdge, query: string) {
  const lowerQuery = query.toLowerCase();
  const [prefix, rawValue] = lowerQuery.includes(":") ? lowerQuery.split(/:(.*)/, 2) : ["", lowerQuery];
  const value = rawValue ?? "";

  if (prefix === "field") {
    return edge.edge.fieldName?.toLowerCase().includes(value) ?? false;
  }

  if (prefix === "uses") {
    return edge.edge.functionName?.toLowerCase().includes(value) ?? edge.source.toLowerCase().includes(value);
  }

  return (
    edge.edge.relationship.toLowerCase().includes(lowerQuery)
    || (edge.edge.fieldName?.toLowerCase().includes(lowerQuery) ?? false)
    || (edge.edge.functionName?.toLowerCase().includes(lowerQuery) ?? false)
    || (edge.edge.typeExpression?.toLowerCase().includes(lowerQuery) ?? false)
    || (edge.edge.typeArgumentName?.toLowerCase().includes(lowerQuery) ?? false)
    || edge.source.toLowerCase().includes(lowerQuery)
    || edge.target.toLowerCase().includes(lowerQuery)
  );
}

function typeNodeMetrics(edges: RenderEdge[], functionIndex: Map<string, FunctionContext>) {
  const metrics = new Map<string, { entryFunctionCount: number; fieldCount: number; functionCount: number }>();

  const ensure = (typeId: string) => {
    const current = metrics.get(typeId);
    if (current) {
      return current;
    }

    const next = { entryFunctionCount: 0, fieldCount: 0, functionCount: 0 };
    metrics.set(typeId, next);
    return next;
  };

  for (const edge of edges) {
    if (edge.category === "field") {
      ensure(edge.source).fieldCount += edge.count;
    }

    if (edge.source.startsWith("function:")) {
      const metric = ensure(edge.target);
      metric.functionCount += 1;
      if (functionIndex.get(edge.source)?.isEntry) {
        metric.entryFunctionCount += 1;
      }
    }

    if (edge.target.startsWith("function:")) {
      const metric = ensure(edge.source);
      metric.functionCount += 1;
      if (functionIndex.get(edge.target)?.isEntry) {
        metric.entryFunctionCount += 1;
      }
    }
  }

  return metrics;
}

function exactGenericArgumentEdgeKeys(edges: NormalizedEdge[]) {
  const keys = new Set<string>();

  for (const edge of edges) {
    if (
      edge.edge.relationship === "genericArgument"
      && edge.edge.declaringTypeId
      && edge.edge.declaringFieldName
      && edge.source !== edge.edge.declaringTypeId
    ) {
      keys.add(genericArgumentShortcutKey(edge));
    }
  }

  return keys;
}

function isOwnerGenericShortcut(edge: NormalizedEdge, exactKeys: Set<string>) {
  return Boolean(
    edge.edge.relationship === "genericArgument"
    && edge.edge.declaringTypeId
    && edge.source === edge.edge.declaringTypeId
    && exactKeys.has(genericArgumentShortcutKey(edge)),
  );
}

function genericArgumentShortcutKey(edge: NormalizedEdge) {
  return [
    edge.edge.declaringTypeId ?? "",
    edge.edge.declaringFieldName ?? "",
    edge.edge.typeArgumentIndex ?? "",
    edge.target,
  ].join("|");
}

function genericInstanceForEdge(
  edge: NormalizedEdge,
  instances: Map<string, GenericInstanceInfo>,
) {
  const declaringTypeId = edge.edge.declaringTypeId;
  const declaringFieldName = edge.edge.declaringFieldName ?? edge.edge.fieldName;

  if (!declaringTypeId || !declaringFieldName) {
    return null;
  }

  if (edge.edge.relationship === "genericArgument") {
    return instances.get(genericInstanceId(edge.source, declaringTypeId, declaringFieldName))
      ?? genericInstanceForDeclaringField(instances, declaringTypeId, declaringFieldName);
  }

  if (isStorageFieldRelationship(edge.edge.relationship)) {
    return instances.get(genericInstanceId(edge.target, declaringTypeId, declaringFieldName)) ?? null;
  }

  return null;
}

function genericInstanceForDeclaringField(
  instances: Map<string, GenericInstanceInfo>,
  declaringTypeId: string,
  declaringFieldName: string,
) {
  for (const instance of instances.values()) {
    if (
      instance.declaringTypeId === declaringTypeId
      && instance.declaringFieldName === declaringFieldName
    ) {
      return instance;
    }
  }

  return null;
}

function normalizedEdgeForRender(
  normalized: NormalizedEdge,
  nodeById: Map<string, MoveTypeGraphNode>,
): MoveTypeGraphEdge {
  if (normalized.edge.relationship !== "genericArgument" || normalized.edge.typeArgumentName) {
    return normalized.edge;
  }

  const sourceNode = normalized.sourceNode ?? nodeById.get(normalized.source) ?? null;
  return {
    ...normalized.edge,
    typeArgumentName: genericArgumentNameForNode(
      sourceNode,
      normalized.edge.typeArgumentIndex ?? 0,
    ),
  };
}

function assignEdgeRoutes(edges: RenderEdge[]) {
  const groups = new Map<string, RenderEdge[]>();

  for (const edge of edges) {
    const key = [edge.source, edge.target, edge.category].join("|");
    groups.set(key, [...(groups.get(key) ?? []), edge]);
  }

  return edges.map((edge) => {
    const group = groups.get([edge.source, edge.target, edge.category].join("|")) ?? [edge];
    const routeIndex = group.findIndex((item) => item.id === edge.id);

    return {
      ...edge,
      routeCount: group.length,
      routeIndex: Math.max(0, routeIndex),
    };
  });
}

function genericInstanceKey(baseTypeId: string, declaringTypeId: string, declaringFieldName: string) {
  return [baseTypeId, declaringTypeId, declaringFieldName].join("|");
}

function genericInstanceId(baseTypeId: string, declaringTypeId: string, declaringFieldName: string) {
  return `generic-instance:${genericInstanceKey(baseTypeId, declaringTypeId, declaringFieldName)}`;
}

function isConcreteGenericContainer(
  node: MoveTypeGraphNode,
  localIds: Set<string>,
  packagePath: string | null,
) {
  if (localIds.has(node.id) || isLocalPackageType(node, packagePath)) {
    return node.qualifiedName.includes("<");
  }

  return isGenericContainer(node) || knownGenericParameterNames(node).length > 0;
}

function genericArgumentNameForNode(node: MoveTypeGraphNode | null, index: number) {
  return node?.typeParameters?.[index]?.name
    || knownGenericParameterNames(node)[index]
    || String(index);
}

function knownGenericParameterNames(node: MoveTypeGraphNode | null) {
  const moduleName = node?.moduleName?.toLowerCase() ?? "";
  const name = node?.name?.toLowerCase() ?? "";

  if (moduleName === "table" && name === "table") {
    return ["K", "V"];
  }

  if (moduleName === "vec_map" || moduleName === "vec_set") {
    return moduleName === "vec_map" ? ["K", "V"] : ["T"];
  }

  if (moduleName === "coin" && name === "coin") {
    return ["T"];
  }

  if (moduleName === "balance" && (name === "balance" || name === "supply")) {
    return ["T"];
  }

  if (name === "vector") {
    return ["T"];
  }

  return [];
}

function genericArgumentSort(label: string) {
  const normalized = label.toUpperCase();

  if (normalized === "K") {
    return 0;
  }

  if (normalized === "V") {
    return 1;
  }

  if (normalized === "T") {
    return 0;
  }

  const numeric = Number(label);
  return Number.isFinite(numeric) ? numeric : 20;
}

function genericInstanceInfo(
  edges: NormalizedEdge[],
  nodeById: Map<string, MoveTypeGraphNode>,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const fieldTargets = genericFieldTargets(edges, nodeById, localIds, packagePath);
  const grouped = new Map<string, GenericInstanceInfo>();

  for (const edge of edges) {
    if (edge.edge.relationship !== "genericArgument" || edge.sourceKind !== "type") {
      continue;
    }

    const declaringTypeId = edge.edge.declaringTypeId;
    const declaringFieldName = edge.edge.declaringFieldName;
    const baseTypeId = declaringTypeId && declaringFieldName
      ? fieldTargets.get(declaringFieldKey(declaringTypeId, declaringFieldName)) ?? edge.source
      : edge.source;
    const node = nodeById.get(baseTypeId);

    if (
      !node
      || !declaringTypeId
      || !declaringFieldName
      || !isConcreteGenericContainer(node, localIds, packagePath)
    ) {
      continue;
    }

    const key = genericInstanceKey(baseTypeId, declaringTypeId, declaringFieldName);
    const current = grouped.get(key) ?? {
      arguments: [],
      baseTypeId,
      declaringFieldName,
      declaringTypeId,
      id: genericInstanceId(baseTypeId, declaringTypeId, declaringFieldName),
      label: "",
      sourceLocation: null,
    };
    const argument = {
      label: edge.edge.typeArgumentName
        ?? genericArgumentNameForNode(node, edge.edge.typeArgumentIndex ?? current.arguments.length),
      value: edge.edge.typeExpression ?? compactEdgeEndpoint(edge.target),
    };

    if (
      !current.arguments.some((item) =>
        item.label === argument.label && item.value === argument.value,
      )
    ) {
      current.arguments.push(argument);
    }
    current.sourceLocation = current.sourceLocation ?? edgeSourceLocation(edge.edge);
    grouped.set(key, current);
  }

  const instances = new Map<string, GenericInstanceInfo>();

  for (const instance of grouped.values()) {
    const node = nodeById.get(instance.baseTypeId);

    if (!node || !instance.arguments.length) {
      continue;
    }

    const sortedArgs = [...instance.arguments].sort((left, right) =>
      genericArgumentSort(left.label) - genericArgumentSort(right.label)
      || left.label.localeCompare(right.label),
    );
    const base = node.name || typeDisplayName(node);
    const label = `${base}<${sortedArgs.map((argument) => compactTypeLabel(argument.value).split("::").at(-1) ?? compactTypeLabel(argument.value)).join(", ")}>`;

    instances.set(instance.id, {
      ...instance,
      arguments: sortedArgs,
      label,
    });
  }

  return instances;
}

function genericFieldTargets(
  edges: NormalizedEdge[],
  nodeById: Map<string, MoveTypeGraphNode>,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const targets = new Map<string, string>();

  for (const edge of edges) {
    const declaringTypeId = edge.edge.declaringTypeId;
    const declaringFieldName = edge.edge.declaringFieldName ?? edge.edge.fieldName;
    const targetNode = nodeById.get(edge.target);

    if (
      edge.sourceKind === "type"
      && edge.targetKind === "type"
      && declaringTypeId
      && declaringFieldName
      && isStorageFieldRelationship(edge.edge.relationship)
      && targetNode
      && isConcreteGenericContainer(targetNode, localIds, packagePath)
    ) {
      targets.set(declaringFieldKey(declaringTypeId, declaringFieldName), edge.target);
    }
  }

  return targets;
}

function declaringFieldKey(declaringTypeId: string, declaringFieldName: string) {
  return `${declaringTypeId}|${declaringFieldName}`;
}

function limitRenderGraph(
  nodes: RenderNode[],
  edges: RenderEdge[],
  selectedTypeId: string | null,
) {
  const nodeLimit = selectedTypeId ? MAX_FOCUSED_NODES : MAX_OVERVIEW_NODES;
  const totalEdgeCount = edges.reduce((total, edge) => total + edge.count, 0);

  if (nodes.length <= nodeLimit && totalEdgeCount <= MAX_RENDER_EDGES) {
    return {
      edges,
      hiddenEdgeCount: 0,
      hiddenNodeCount: 0,
      nodes: relayoutRenderNodes(nodes),
    };
  }

  const degree = nodeDegree(edges);
  const selectedDistances = selectedTypeId ? graphDistances(selectedTypeId, edges, 2) : new Map<string, number>();
  const selectedNode = selectedTypeId ? nodes.find((node) => node.id === selectedTypeId) ?? null : null;
  const prioritizedNodes = [...nodes].sort((left, right) =>
    nodeRenderPriority(left, selectedTypeId, selectedDistances, degree)
    - nodeRenderPriority(right, selectedTypeId, selectedDistances, degree)
    || left.label.localeCompare(right.label)
    || left.id.localeCompare(right.id),
  );
  const visibleIds = new Set<string>();

  if (selectedNode) {
    visibleIds.add(selectedNode.id);
  }

  for (const node of prioritizedNodes) {
    if (visibleIds.size >= nodeLimit) {
      break;
    }
    visibleIds.add(node.id);
  }

  const visibleCandidateEdges = edges.filter((edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target));
  const limitedEdges = prioritizedEdges(visibleCandidateEdges, selectedTypeId).slice(0, MAX_RENDER_EDGES);
  const edgeNodeIds = new Set<string>();

  for (const edge of limitedEdges) {
    edgeNodeIds.add(edge.source);
    edgeNodeIds.add(edge.target);
  }

  if (selectedNode) {
    edgeNodeIds.add(selectedNode.id);
  }

  const limitedNodes = relayoutRenderNodes(
    nodes.filter((node) => visibleIds.has(node.id) && (edgeNodeIds.has(node.id) || node.kind === "local")),
  );
  const visibleEdgeCount = limitedEdges.reduce((total, edge) => total + edge.count, 0);

  return {
    edges: limitedEdges,
    hiddenEdgeCount: Math.max(0, totalEdgeCount - visibleEdgeCount),
    hiddenNodeCount: Math.max(0, nodes.length - limitedNodes.length),
    nodes: limitedNodes,
  };
}

function nodeRenderPriority(
  node: RenderNode,
  selectedTypeId: string | null,
  selectedDistances: Map<string, number>,
  degree: Map<string, number>,
) {
  let score = 0;

  if (node.id === selectedTypeId) {
    score -= 100_000;
  }

  if (selectedTypeId) {
    const distance = selectedDistances.get(node.id);
    score += distance === undefined ? 50_000 : distance * 1_000;
  }

  if (node.kind === "local") {
    score -= 5_000;
  } else if (node.kind === "framework" || node.kind === "builtin") {
    score -= 1_500;
  } else if (node.kind === "external") {
    score -= 800;
  } else if (node.kind === "function") {
    score += selectedTypeId ? 250 : 2_500;
  }

  if (isResourceLike(node.node)) {
    score -= 600;
  }

  if (isCapabilityLike(node.node)) {
    score -= 500;
  }

  score -= (degree.get(node.id) ?? 0) * 25;

  return score;
}

function prioritizedEdges(edges: RenderEdge[], selectedTypeId: string | null) {
  return [...edges].sort((left, right) =>
    edgeRenderPriority(left, selectedTypeId) - edgeRenderPriority(right, selectedTypeId)
    || right.count - left.count
    || left.id.localeCompare(right.id),
  );
}

function edgeRenderPriority(edge: RenderEdge, selectedTypeId: string | null) {
  let score = 0;

  if (selectedTypeId && (edge.source === selectedTypeId || edge.target === selectedTypeId)) {
    score -= 10_000;
  }

  if (edge.category === "field") {
    score -= 700;
  } else if (edge.category === "capability") {
    score -= 650;
  } else if (edge.category === "input" || edge.category === "return") {
    score -= 500;
  } else if (edge.category === "generic") {
    score -= 300;
  }

  score -= Math.min(edge.count, 20) * 10;

  return score;
}

function nodeDegree(edges: RenderEdge[]) {
  const degree = new Map<string, number>();

  for (const edge of edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + edge.count);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + edge.count);
  }

  return degree;
}

function graphDistances(startId: string, edges: RenderEdge[], maxDepth: number) {
  const distances = new Map([[startId, 0]]);
  let frontier = new Set([startId]);

  for (let depth = 1; depth <= maxDepth; depth += 1) {
    const next = new Set<string>();

    for (const edge of edges) {
      if (frontier.has(edge.source) && !distances.has(edge.target)) {
        distances.set(edge.target, depth);
        next.add(edge.target);
      }
      if (frontier.has(edge.target) && !distances.has(edge.source)) {
        distances.set(edge.source, depth);
        next.add(edge.source);
      }
    }

    frontier = next;
  }

  return distances;
}

function relayoutRenderNodes(nodes: RenderNode[]) {
  const functionNodes = nodes.filter((node) => node.layoutRole === "function");
  const localNodes = nodes.filter((node) => node.layoutRole === "parent");
  const fieldNodes = nodes.filter((node) => node.layoutRole === "field");
  const dependencyNodes = nodes
    .filter((node) => node.layoutRole === "target")
    .sort((left, right) =>
      dependencyGroupOrder(left.groupLabel) - dependencyGroupOrder(right.groupLabel)
      || (left.groupLabel ?? "").localeCompare(right.groupLabel ?? "")
      || left.label.localeCompare(right.label)
      || left.id.localeCompare(right.id),
    );
  const dependencyNodesWithHeaders = dependencyNodes.map((node, index) => ({
    ...node,
    showGroupLabel: Boolean(
      node.groupLabel
      && (index === 0 || dependencyNodes[index - 1]?.groupLabel !== node.groupLabel),
    ),
  }));
  const maxRows = Math.max(functionNodes.length, localNodes.length, fieldNodes.length, dependencyNodes.length, 1);
  const storageLayout = functionNodes.length === 0;

  if (storageLayout && fieldNodes.length > 0 && localNodes.length === 1) {
    return relayoutStorageStarNodes(localNodes[0]!, fieldNodes, dependencyNodes);
  }

  return [
    ...functionNodes.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
    ...localNodes.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
    ...fieldNodes.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
    ...dependencyNodesWithHeaders.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
  ];
}

function relayoutStorageStarNodes(
  hubNode: RenderNode,
  fieldNodes: RenderNode[],
  dependencyNodes: RenderNode[],
) {
  const hub = {
    ...hubNode,
    showGroupLabel: false,
    x: 0,
    y: 0,
  };
  const hubCenter = {
    x: hub.x + TYPE_GRAPH_NODE_WIDTH / 2,
    y: hub.y + TYPE_GRAPH_NODE_HEIGHT / 2,
  };
  const fieldAngles = starFanAngles(fieldNodes.length);
  const rawFieldLayouts = fieldNodes.map((node, index) => {
    const angle = fieldAngles[index] ?? 0;
    const radians = degreesToRadians(angle);

    return {
      angle,
      centerX: hubCenter.x + TYPE_GRAPH_STAR_FIELD_OFFSET_X + Math.cos(radians) * TYPE_GRAPH_STAR_FIELD_ARC_X,
      centerY: hubCenter.y + Math.sin(radians) * TYPE_GRAPH_STAR_FIELD_ARC_Y,
      node,
    };
  });
  const fieldLayouts = spreadStarLayouts(rawFieldLayouts, TYPE_GRAPH_STAR_FIELD_MIN_GAP);
  const fieldAngleById = new Map(fieldLayouts.map((layout) => [layout.node.id, layout.angle]));
  const targetAngleById = new Map<string, { count: number; x: number; y: number }>();

  for (const layout of fieldLayouts) {
    const targetId = layout.node.fieldInfo?.targetTypeId;

    if (!targetId) {
      continue;
    }

    const radians = degreesToRadians(layout.angle);
    const current = targetAngleById.get(targetId) ?? { count: 0, x: 0, y: 0 };
    current.count += 1;
    current.x += Math.cos(radians);
    current.y += Math.sin(radians);
    targetAngleById.set(targetId, current);
  }

  const rawTargetLayouts = dependencyNodes.map((node, index) => {
    const vector = targetAngleById.get(node.id);
    const fallbackAngle = starFanAngles(dependencyNodes.length)[index] ?? 0;
    const angle = vector && vector.count > 0
      ? radiansToDegrees(Math.atan2(vector.y / vector.count, Math.max(0.16, vector.x / vector.count)))
      : fallbackAngle;
    const radians = degreesToRadians(angle);

    return {
      angle,
      centerX: hubCenter.x + TYPE_GRAPH_STAR_TARGET_OFFSET_X + Math.cos(radians) * TYPE_GRAPH_STAR_TARGET_ARC_X,
      centerY: hubCenter.y + Math.sin(radians) * TYPE_GRAPH_STAR_TARGET_ARC_Y,
      node,
    };
  });
  const targetLayouts = spreadStarTargetLayouts(rawTargetLayouts, TYPE_GRAPH_STAR_TARGET_MIN_GAP);
  const targetHeaderIds = starTargetGroupHeaderIds(targetLayouts);

  return [
    hub,
    ...fieldLayouts.map((layout) => ({
      ...layout.node,
      showGroupLabel: false,
      x: layout.centerX - TYPE_GRAPH_FIELD_NODE_WIDTH / 2,
      y: layout.centerY - TYPE_GRAPH_FIELD_NODE_HEIGHT / 2,
    })),
    ...targetLayouts.map((layout) => ({
      ...layout.node,
      showGroupLabel: targetHeaderIds.has(layout.node.id),
      x: layout.centerX - TYPE_GRAPH_NODE_WIDTH / 2,
      y: layout.centerY - TYPE_GRAPH_NODE_HEIGHT / 2,
    })),
  ].sort((left, right) => {
    if (left.id === hub.id) {
      return -1;
    }
    if (right.id === hub.id) {
      return 1;
    }

    const leftAngle = fieldAngleById.get(left.id);
    const rightAngle = fieldAngleById.get(right.id);

    if (leftAngle != null && rightAngle != null) {
      return leftAngle - rightAngle;
    }

    if (left.layoutRole === right.layoutRole) {
      return left.y - right.y || left.x - right.x;
    }

    return left.layoutRole === "field" ? -1 : 1;
  });
}

function starFanAngles(count: number) {
  if (count <= 0) {
    return [];
  }

  if (count === 1) {
    return [0];
  }

  const spread = count <= 4 ? 104 : count <= 7 ? 128 : 148;
  const step = spread / Math.max(1, count - 1);
  return Array.from({ length: count }, (_, index) => -spread / 2 + index * step);
}

function starTargetGroupHeaderIds(
  layouts: Array<{ centerY: number; node: RenderNode }>,
) {
  const ids = new Set<string>();
  const seenGroups = new Set<string>();

  for (const layout of [...layouts].sort((left, right) => left.centerY - right.centerY)) {
    const group = layout.node.groupLabel;

    if (!group || seenGroups.has(group)) {
      continue;
    }

    ids.add(layout.node.id);
    seenGroups.add(group);
  }

  return ids;
}

function spreadStarTargetLayouts<T extends { centerY: number; node: RenderNode }>(
  layouts: Array<T>,
  minGap: number,
) {
  if (layouts.length <= 1) {
    return layouts;
  }

  const sorted = [...layouts].sort((left, right) => left.centerY - right.centerY);
  const originalAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;

  for (let index = 1; index < sorted.length; index += 1) {
    const previous = sorted[index - 1]!;
    const current = sorted[index]!;
    const previousGroup = previous.node.groupLabel ?? "";
    const currentGroup = current.node.groupLabel ?? "";
    const requiredGap = minGap + (previousGroup !== currentGroup ? TYPE_GRAPH_STAR_TARGET_GROUP_GAP : 0);

    if (current.centerY < previous.centerY + requiredGap) {
      current.centerY = previous.centerY + requiredGap;
    }
  }

  const adjustedAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;
  const recenter = adjustedAverage - originalAverage;

  return sorted.map((layout) => ({
    ...layout,
    centerY: layout.centerY - recenter,
  }));
}

function spreadStarLayouts<T extends { centerY: number }>(
  layouts: Array<T>,
  minGap: number,
) {
  if (layouts.length <= 1) {
    return layouts;
  }

  const sorted = [...layouts].sort((left, right) => left.centerY - right.centerY);
  const originalAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;

  for (let index = 1; index < sorted.length; index += 1) {
    const previous = sorted[index - 1]!;
    const current = sorted[index]!;
    if (current.centerY < previous.centerY + minGap) {
      current.centerY = previous.centerY + minGap;
    }
  }

  const adjustedAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;
  const recenter = adjustedAverage - originalAverage;

  return sorted.map((layout) => ({
    ...layout,
    centerY: layout.centerY - recenter,
  }));
}

function degreesToRadians(degrees: number) {
  return degrees * (Math.PI / 180);
}

function radiansToDegrees(radians: number) {
  return radians * (180 / Math.PI);
}

function dependencyGroupOrder(groupLabel: string | null) {
  switch (groupLabel) {
    case "Local":
      return 0;
    case "Builtin":
      return 1;
    case "Sui Framework":
      return 2;
    case "External":
      return 3;
    case "Generic Containers":
      return 4;
    default:
      return 5;
  }
}

function repositionRenderNode(
  node: RenderNode,
  index: number,
  maxRows: number,
  storageLayout: boolean,
): RenderNode {
  return {
    ...node,
    x: node.layoutRole === "function"
      ? TYPE_GRAPH_FUNCTION_COLUMN_X
      : node.layoutRole === "parent"
        ? storageLayout ? TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X : TYPE_GRAPH_LOCAL_COLUMN_X
        : node.layoutRole === "field"
          ? TYPE_GRAPH_STORAGE_FIELD_COLUMN_X
        : storageLayout ? TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X : TYPE_GRAPH_DEPENDENCY_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

function normalizeEdge(
  edge: MoveTypeGraphEdge,
  nodeById: Map<string, MoveTypeGraphNode>,
  packagePath: string | null,
): NormalizedEdge | null {
  const sourceNode = nodeById.get(edge.source) ?? null;
  const targetNode = nodeById.get(edge.target) ?? null;
  const sourceIsFunction = sourceBelongsToPackage(edge.source, packagePath);
  const targetIsFunction = sourceBelongsToPackage(edge.target, packagePath);

  if (sourceIsFunction && targetNode && isRenderableTypeNode(targetNode)) {
    return {
      edge,
      source: edge.source,
      sourceKind: "function",
      sourceNode: null,
      target: edge.target,
      targetKind: "type",
      targetNode,
    };
  }

  if (sourceNode && targetNode && isRenderableTypeNode(sourceNode) && isRenderableTypeNode(targetNode)) {
    return {
      edge,
      source: edge.source,
      sourceKind: "type",
      sourceNode,
      target: edge.target,
      targetKind: "type",
      targetNode,
    };
  }

  if (sourceNode && isRenderableTypeNode(sourceNode) && targetIsFunction) {
    return {
      edge,
      source: edge.source,
      sourceKind: "type",
      sourceNode,
      target: edge.target,
      targetKind: "function",
      targetNode: null,
    };
  }

  return null;
}

function edgeMatchesLens(
  normalized: NormalizedEdge,
  lens: TypeGraphLens,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const relationship = normalized.edge.relationship;
  const sourceOrTargetExternal = edgeHasExternalType(normalized, localIds, packagePath);
  const sourceOrTargetCapability = Boolean(
    (normalized.sourceNode && isCapabilityLike(normalized.sourceNode))
      || (normalized.targetNode && isCapabilityLike(normalized.targetNode)),
  );

  switch (lens) {
    case "storage":
      return STORAGE_RELATIONSHIPS.has(relationship) && normalized.sourceKind === "type" && normalized.targetKind === "type";
    case "functions":
      return (
        normalized.sourceKind === "function"
        || normalized.targetKind === "function"
        || FUNCTION_RELATIONSHIPS.has(relationship)
      );
    case "capabilities":
      return sourceOrTargetCapability || relationship === "parameter" || relationship === "field";
    case "generics":
      return GENERIC_RELATIONSHIPS.has(relationship);
    case "external":
      return sourceOrTargetExternal;
  }
}

function isStorageFieldRelationship(relationship: string) {
  return relationship === "field" || relationship === "variantField" || relationship === "vectorElement";
}

function fieldNodeId(declaringTypeId: string, fieldName: string, targetId: string) {
  return `field-node:${declaringTypeId}:${fieldName}:${targetId}`;
}

function edgeTouchesFieldNode(edge: RenderEdge) {
  return edge.source.startsWith("field-node:") || edge.target.startsWith("field-node:");
}

function fieldNodeEndpoint(edge: RenderEdge | null) {
  if (!edge) {
    return null;
  }

  if (edge.source.startsWith("field-node:")) {
    return edge.source;
  }

  if (edge.target.startsWith("field-node:")) {
    return edge.target;
  }

  return null;
}

function edgeTouchesPackage(
  edge: NormalizedEdge,
  localIds: Set<string>,
  packagePath: string | null,
) {
  return (
    localIds.has(edge.source)
    || localIds.has(edge.target)
    || sourceBelongsToPackage(edge.source, packagePath)
    || sourceBelongsToPackage(edge.target, packagePath)
  );
}

function typeRenderNode(
  node: MoveTypeGraphNode,
  kind: TypeNodeKind,
  index: number,
  maxRows: number,
  metrics: {
    entryFunctionCount: number;
    fieldCount: number;
    functionCount: number;
  } | undefined,
  genericInstance: GenericInstanceInfo | null,
  storageLayout: boolean,
  layoutRole: TypeGraphLayoutRole,
): RenderNode {
  const roleLabel = genericInstance ? "generic instance" : nodeRoleLabel(node, kind);
  const fieldCount = metrics?.fieldCount ?? 0;
  const functionCount = metrics?.functionCount ?? 0;
  const sourceLocation = node.span ?? (kind === "local" ? sourceLocationFromMoveNode(node) : null);
  return {
    addressLabel: shortAddress(node.canonicalAddress ?? node.address),
    abilitiesKnown: typeAbilitiesKnown(node, kind),
    evidenceEdgeId: null,
    fieldInfo: null,
    entryFunctionCount: metrics?.entryFunctionCount ?? 0,
    fieldCount,
    functionContext: null,
    functionCount,
    groupLabel: dependencyGroupLabel(kind, node),
    genericArguments: genericInstance?.arguments ?? genericParameterChips(node),
    id: node.id,
    isGenericInstance: Boolean(genericInstance),
    isSynthetic: false,
    kind,
    label: genericInstance?.label ?? typeDisplayName(node, kind),
    layoutRole,
    metricLabel: typeNodeMetricLabel(kind, roleLabel, fieldCount, functionCount, storageLayout, Boolean(genericInstance)),
    node,
    originLabel: nodeOriginLabel(node, kind),
    riskTags: nodeRiskTags(node),
    roleLabel,
    selectTypeId: node.id,
    showGroupLabel: false,
    subtitle: nodeSubtitle(node, kind),
    tags: nodeTags(node),
    sourceLocation,
    x: kind === "local"
      ? storageLayout ? TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X : TYPE_GRAPH_LOCAL_COLUMN_X
      : storageLayout ? TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X : TYPE_GRAPH_DEPENDENCY_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

function syntheticFieldNode(
  edge: NormalizedEdge,
  index: number,
  maxRows: number,
  selectedTypeId: string,
  targetId: string,
  evidenceEdgeId: string,
  genericInstance: GenericInstanceInfo | null,
): RenderNode {
  const fieldName = edge.edge.fieldName ?? "field";
  const fieldType = resolvedFieldType(edge, genericInstance);
  const genericArguments = genericArgumentsFromTypeExpression(fieldType, edge.targetNode);
  const fieldTags = fieldSemanticTags(fieldName, fieldType, edge.targetNode);
  const sourceLocation = edgeSourceLocation(edge.edge);

  return {
    addressLabel: null,
    abilitiesKnown: true,
    evidenceEdgeId,
    fieldInfo: {
      baseType: fieldBaseType(edge),
      confidence: edge.edge.confidence || "syntactic",
      declaredIn: edge.sourceNode?.qualifiedName ?? compactEdgeEndpoint(selectedTypeId),
      declaringTypeId: edge.edge.declaringTypeId ?? selectedTypeId,
      fieldName,
      genericArguments,
      resolvedType: fieldType,
      sourceLocation,
      tags: fieldTags,
      targetTypeId: edge.target,
    },
    entryFunctionCount: 0,
    fieldCount: 0,
    functionContext: null,
    functionCount: 0,
    groupLabel: "Fields",
    genericArguments: [],
    id: fieldNodeId(selectedTypeId, fieldName, targetId),
    isGenericInstance: false,
    isSynthetic: true,
    kind: "field",
    label: fieldName,
    layoutRole: "field",
    metricLabel: compactEdgeEndpoint(targetId),
    node: null,
    originLabel: "field",
    riskTags: [],
    roleLabel: "field",
    selectTypeId: selectedTypeId,
    showGroupLabel: false,
    sourceLocation,
    subtitle: compactTypeLabel(fieldType),
    tags: fieldTags,
    x: TYPE_GRAPH_STORAGE_FIELD_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

function syntheticGenericInstanceNode(
  instance: GenericInstanceInfo,
  baseNode: MoveTypeGraphNode | null,
  index: number,
  maxRows: number,
  packagePath: string | null,
  storageLayout: boolean,
): RenderNode {
  const kind = baseNode ? classifyTypeNode(baseNode, packagePath) : "framework";
  const roleLabel = "generic instance";
  const sourceLocation = baseNode?.span ?? (baseNode && kind === "local" ? sourceLocationFromMoveNode(baseNode) : null);

  return {
    addressLabel: shortAddress(baseNode?.canonicalAddress ?? baseNode?.address),
    abilitiesKnown: baseNode ? typeAbilitiesKnown(baseNode, kind) : false,
    evidenceEdgeId: null,
    fieldInfo: null,
    entryFunctionCount: 0,
    fieldCount: 0,
    functionContext: null,
    functionCount: 0,
    groupLabel: "Generic Containers",
    genericArguments: instance.arguments,
    id: instance.id,
    isGenericInstance: true,
    isSynthetic: true,
    kind,
    label: instance.label,
    layoutRole: "target",
    metricLabel: "generic instance",
    node: baseNode,
    originLabel: baseNode ? nodeOriginLabel(baseNode, kind) : "generic container",
    riskTags: baseNode ? nodeRiskTags(baseNode) : ["generic container"],
    roleLabel,
    selectTypeId: instance.baseTypeId,
    showGroupLabel: false,
    sourceLocation,
    subtitle: baseNode ? nodeSubtitle(baseNode, kind) : instance.declaringTypeId,
    tags: ["generic container", ...instance.arguments.slice(0, 1).map((argument) => `${argument.label}: ${compactTypeLabel(argument.value)}`)],
    x: storageLayout ? TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X : TYPE_GRAPH_DEPENDENCY_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

function syntheticFunctionNode(
  id: string,
  index: number,
  maxRows: number,
  functionIndex: Map<string, FunctionContext>,
): RenderNode {
  const context = functionDetails(id, functionIndex);

  return {
    addressLabel: null,
    abilitiesKnown: true,
    evidenceEdgeId: null,
    fieldInfo: null,
    entryFunctionCount: context.isEntry ? 1 : 0,
    functionContext: context,
    fieldCount: 0,
    functionCount: 1,
    groupLabel: null,
    genericArguments: [],
    id,
    isGenericInstance: false,
    isSynthetic: false,
    kind: "function",
    label: context.label,
    layoutRole: "function",
    metricLabel: "function",
    node: null,
    originLabel: "function",
    riskTags: context.isEntry ? ["entry surface"] : [],
    roleLabel: context.isEntry ? "entry" : "function",
    selectTypeId: id,
    showGroupLabel: false,
    subtitle: context.moduleName,
    tags: [
      context.visibility,
      ...(context.isEntry ? ["entry"] : []),
      ...(context.isTransactionCallable ? ["tx"] : []),
    ],
    sourceLocation: null,
    x: TYPE_GRAPH_FUNCTION_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

function columnY(index: number, maxRows: number, gap: number) {
  const rows = Math.max(maxRows, 1);
  return Math.max(0, (rows - 1) * 8) + index * Math.max(gap, TYPE_GRAPH_NODE_HEIGHT);
}

function graphStats(edges: RenderEdge[]) {
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, number>();

  for (const edge of edges) {
    outgoing.set(edge.source, (outgoing.get(edge.source) ?? 0) + edge.count);
    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + edge.count);
  }

  return { incoming, outgoing };
}

function collectEdgeEvidence(
  edges: NormalizedEdge[],
  localIds: Set<string>,
  packagePath: string | null,
) {
  const groups = new Map<string, RenderEdge>();

  for (const edge of edges) {
    if (!edgeTouchesPackage(edge, localIds, packagePath)) {
      continue;
    }

    const category = relationshipCategory(edge, localIds, packagePath);
    const key = edgeGroupKey(edge.source, edge.target, edge.edge, category);
    const current = groups.get(key);

    if (current) {
      current.count += 1;
    } else {
      groups.set(key, {
        category,
        count: 1,
        edge: edge.edge,
        id: key,
        routeCount: 1,
        routeIndex: 0,
        source: edge.source,
        target: edge.target,
      });
    }
  }

  return [...groups.values()].sort((left, right) => left.id.localeCompare(right.id));
}

function selectedNeighborhoodIds(
  edges: RenderEdge[],
  selectedTypeId: string | null,
  activeEdge: RenderEdge | null,
) {
  const ids = new Set<string>();

  if (activeEdge) {
    const fieldId = fieldNodeEndpoint(activeEdge);

    if (fieldId) {
      for (const edge of edges) {
        if (edge.source === fieldId || edge.target === fieldId) {
          ids.add(edge.source);
          ids.add(edge.target);
        }
      }

      return ids;
    }

    ids.add(activeEdge.source);
    ids.add(activeEdge.target);
    return ids;
  }

  if (!selectedTypeId) {
    return ids;
  }

  ids.add(selectedTypeId);
  let frontier = new Set([selectedTypeId]);

  for (let depth = 0; depth < 2; depth += 1) {
    const next = new Set<string>();

    for (const edge of edges) {
      if (frontier.has(edge.source)) {
        next.add(edge.target);
      }
      if (frontier.has(edge.target)) {
        next.add(edge.source);
      }
    }

    next.forEach((id) => ids.add(id));
    frontier = next;
  }

  return ids;
}

function edgeGroupKey(
  source: string,
  target: string,
  edge: MoveTypeGraphEdge,
  category: RelationshipCategory,
) {
  return [
    source,
    target,
    category,
    edge.relationship,
    edge.fieldName ?? "",
    edge.variantName ?? "",
    edge.functionName ?? "",
    edge.parameterName ?? "",
    edge.typeArgumentIndex ?? "",
    edge.declaringTypeId ?? "",
    edge.declaringFieldName ?? "",
    edge.typeArgumentName ?? "",
    edge.isMutable ? "mut" : "imm",
    edge.isReference ? "ref" : "value",
  ].join("|");
}

function relationshipCategory(
  normalized: NormalizedEdge,
  localIds: Set<string>,
  packagePath: string | null,
): RelationshipCategory {
  const relationship = normalized.edge.relationship;
  const hasCapability =
    (normalized.sourceNode ? isCapabilityLike(normalized.sourceNode) : false)
    || (normalized.targetNode ? isCapabilityLike(normalized.targetNode) : false);

  if (hasCapability && (relationship === "field" || relationship === "parameter")) {
    return "capability";
  }

  if (relationship === "field" || relationship === "variantField") {
    return "field";
  }

  if (relationship === "parameter") {
    return "input";
  }

  if (relationship === "return") {
    return "return";
  }

  if (GENERIC_RELATIONSHIPS.has(relationship)) {
    return "generic";
  }

  if (relationship === "annotation" || relationship === "cast") {
    return "annotation";
  }

  if (relationship === "construction" || relationship === "destructuring") {
    return "mutation";
  }

  if (edgeHasExternalType(normalized, localIds, packagePath)) {
    return "external";
  }

  return "annotation";
}

function relationshipColor(category: RelationshipCategory) {
  switch (category) {
    case "field":
      return "#4ade80";
    case "input":
      return "#60a5fa";
    case "return":
      return FUNCTION_COLOR;
    case "generic":
      return GENERIC_COLOR;
    case "capability":
      return "#f87171";
    case "mutation":
      return "#fb7185";
    case "external":
      return EXTERNAL_COLOR;
    case "annotation":
      return "#94a3b8";
  }
}

function edgeLabel(edge: MoveTypeGraphEdge, count: number, category: RelationshipCategory) {
  const suffix = count > 1 ? ` x${count}` : "";

  if (edge.fieldName) {
    return `field(${edge.fieldName})${suffix}`;
  }

  if (edge.parameterName) {
    return `${relationshipLabel(edge.relationship)} (${edge.parameterName})${suffix}`;
  }

  if (edge.variantName) {
    return `${edge.variantName}${suffix}`;
  }

  if (edge.typeArgumentIndex != null) {
    return `generic(${edge.typeArgumentName ?? edge.typeArgumentIndex})${suffix}`;
  }

  if (category === "capability") {
    return `authorizes${suffix}`;
  }

  return `${relationshipLabel(edge.relationship)}${suffix}`;
}

function fieldCauseEdgeLabelIds(
  edges: RenderEdge[],
  selectedTypeId: string | null,
  selectedEdgeId: string | null,
) {
  const ids = new Set<string>();

  if (selectedEdgeId) {
    ids.add(selectedEdgeId);
  }

  if (!selectedTypeId) {
    return ids;
  }

  const fieldEdges = edges
    .filter(
      (edge) =>
        edge.edge.fieldName
        && edge.source === selectedTypeId,
    )
    .sort((left, right) =>
      fieldCausePriority(right, selectedTypeId) - fieldCausePriority(left, selectedTypeId)
      || left.edge.fieldName!.localeCompare(right.edge.fieldName!)
      || left.id.localeCompare(right.id),
    );

  const labelsByTarget = new Map<string, number>();

  for (const edge of fieldEdges) {
    if (ids.size >= MAX_FIELD_CAUSE_LABELS) {
      break;
    }

    const shownForTarget = labelsByTarget.get(edge.target) ?? 0;
    const targetLimit = 1;

    if (shownForTarget >= targetLimit) {
      continue;
    }

    ids.add(edge.id);
    labelsByTarget.set(edge.target, shownForTarget + 1);
  }

  return ids;
}

function prioritizedLabelEdgeIds(
  edges: RenderEdge[],
  selectedTypeId: string | null,
  limit: number,
) {
  const ids = new Set<string>();

  for (const edge of prioritizedEdges(edges, selectedTypeId).slice(0, limit)) {
    ids.add(edge.id);
  }

  return ids;
}

function fieldCausePriority(edge: RenderEdge, selectedTypeId: string) {
  let score = edge.count;

  if (edge.source === selectedTypeId) {
    score += 10;
  }

  if (edge.edge.fieldName === "id" || edge.edge.fieldName === "uid") {
    score += 6;
  }

  if (edge.category === "field") {
    score += 3;
  }

  return score;
}

function relationshipLabel(relationship: string) {
  switch (relationship) {
    case "annotation":
      return "annotation";
    case "callTypeArgument":
      return "type arg";
    case "construction":
      return "packs";
    case "destructuring":
      return "unpacks";
    case "genericArgument":
      return "generic";
    case "phantomTypeParameter":
      return "phantom";
    case "parameter":
      return "takes";
    case "return":
      return "returns";
    case "summaryUsage":
      return "summary";
    case "typeParameter":
      return "type param";
    case "variantField":
      return "variant";
    case "vectorElement":
      return "vector";
    default:
      return relationship;
  }
}

function nodeColor(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "function") {
    return FUNCTION_COLOR;
  }

  if (kind === "field") {
    return relationshipColor("field");
  }

  if (isCapabilityLike(node)) {
    return CAPABILITY_COLOR;
  }

  if (node?.kind === "enum") {
    return GENERIC_COLOR;
  }

  if (isGenericLike(node)) {
    return GENERIC_COLOR;
  }

  if (kind === "builtin") {
    return BUILTIN_COLOR;
  }

  if (kind === "framework") {
    return FRAMEWORK_COLOR;
  }

  if (kind === "external") {
    return EXTERNAL_COLOR;
  }

  return LOCAL_COLOR;
}

function nodeIcon(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "function") {
    return Workflow;
  }

  if (kind === "field") {
    return ListTree;
  }

  if (node?.kind === "enum") {
    return Hexagon;
  }

  if (isCapabilityLike(node)) {
    return ShieldAlert;
  }

  if (isResourceLike(node)) {
    return KeyRound;
  }

  if (kind === "builtin") {
    return Braces;
  }

  if (kind === "framework") {
    return Box;
  }

  if (kind === "external") {
    return FileCode2;
  }

  return Boxes;
}

function nodeKindLabel(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "function") {
    return "function";
  }

  if (kind === "field") {
    return "field";
  }

  if (node?.kind === "enum") {
    return "enum";
  }

  if (kind === "builtin") {
    return "builtin";
  }

  if (kind === "framework") {
    return "framework";
  }

  if (kind === "external") {
    return "external";
  }

  return "type";
}

function nodeRoleLabel(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (isCapabilityLike(node)) {
    return "capability";
  }

  if (isGenericContainer(node)) {
    return "generic container";
  }

  if (isEventLike(node)) {
    return "event";
  }

  if (isWitnessLike(node)) {
    return "witness";
  }

  if (isResourceLike(node)) {
    return "resource struct";
  }

  if (isGenericLike(node)) {
    return "generic";
  }

  return nodeKindLabel(kind, node);
}

function nodeOriginLabel(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (kind === "local") {
    return "local";
  }

  if (kind === "builtin") {
    return "primitive";
  }

  if (kind === "framework") {
    const address = node.address?.toLowerCase() ?? node.canonicalAddress?.toLowerCase();
    return address === "0x1" || address?.endsWith("0001") ? "move stdlib" : "sui framework";
  }

  return node.packageName ? "direct dependency" : "external";
}

function dependencyGroupLabel(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "local") {
    return "Local";
  }

  if (kind === "builtin") {
    return "Builtin";
  }

  if (isGenericContainer(node)) {
    return "Generic Containers";
  }

  if (kind === "framework") {
    return "Sui Framework";
  }

  if (kind === "external") {
    return "External";
  }

  return null;
}

function typeNodeMetricLabel(
  kind: TypeNodeKind,
  roleLabel: string,
  fieldCount: number,
  functionCount: number,
  storageLayout: boolean,
  isGenericInstance: boolean,
) {
  if (isGenericInstance) {
    return "generic instance";
  }

  if (storageLayout && kind !== "local") {
    if (kind === "builtin") {
      return "builtin";
    }

    if (kind === "framework" || kind === "external") {
      return "field target";
    }
  }

  if (fieldCount) {
    return pluralize(fieldCount, "field");
  }

  if (functionCount && !storageLayout) {
    return pluralize(functionCount, "fn");
  }

  return roleLabel;
}

function nodeRiskTags(node: MoveTypeGraphNode) {
  const tags: string[] = [];

  if (isCapabilityLike(node)) {
    tags.push("capability");
    tags.push("privileged");
  }

  if (isResourceLike(node)) {
    tags.push("resource");
    tags.push("key object");
  }

  if (
    /(vault|pool|escrow|balance|treasury|account|position)/i.test(node.name)
    && !/(config|receipt|event|witness|cap)$/i.test(node.name)
  ) {
    tags.push("value-holding");
  }

  if (isGenericLike(node)) {
    tags.push("generic container");
  }

  if (isEventLike(node)) {
    tags.push("event");
  }

  if (isWitnessLike(node)) {
    tags.push("witness");
  }

  if (isExternalDependencyType(node)) {
    tags.push("external");
    tags.push("trust boundary");
    if (/(asset|price|oracle|feed|object)/i.test(node.name)) {
      tags.push(node.name.toLowerCase().includes("asset") ? "asset metadata" : "external data");
    }
  }

  return [...new Set(tags)];
}

function nodeSubtitle(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (kind === "local") {
    return node.moduleName ? `${node.packageName ?? "local"}::${node.moduleName}` : "local package";
  }

  if (kind === "builtin") {
    return "Move builtin";
  }

  if (kind === "framework") {
    return node.moduleName ? `sui::${node.moduleName}` : "Sui framework";
  }

  return node.qualifiedName;
}

function nodeTags(node: MoveTypeGraphNode) {
  const riskTags = nodeRiskTags(node);
  const semanticTags = nodeSemanticTags(node);
  const abilities = effectiveTypeAbilities(node)
    .filter((ability) => !(ability === "key" && riskTags.includes("key object")));
  const tags = [...riskTags, ...semanticTags, ...abilities];

  if (isCapabilityLike(node) && !tags.includes("capability")) {
    tags.unshift("capability");
  } else if (isResourceLike(node) && !tags.includes("resource")) {
    tags.unshift("resource");
  }

  return [...new Set(tags)].slice(0, 3);
}

function nodeSemanticTags(node: MoveTypeGraphNode) {
  const tags: string[] = [];
  const moduleName = node.moduleName?.toLowerCase() ?? "";
  const name = node.name.toLowerCase();

  if (isFrameworkType(node) && moduleName === "object" && name === "uid") {
    tags.push("framework");
    tags.push("object identity");
  } else if (isFrameworkType(node) && moduleName === "object" && name === "id") {
    tags.push("framework");
    tags.push("object reference");
  } else if (isFrameworkType(node) && node.kind !== "builtin") {
    tags.push("framework");
  }

  if (isGenericContainer(node) && !tags.includes("generic container")) {
    tags.push("generic container");
  }

  return tags;
}

function typeAbilitiesKnown(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (node.abilities.length > 0) {
    return true;
  }

  if (node.kind === "builtin") {
    return Boolean(BUILTIN_TYPE_ABILITIES[node.name]);
  }

  const moduleName = node.moduleName ?? node.qualifiedName.split("::").slice(-2, -1)[0];
  if (moduleName && SUI_FRAMEWORK_TYPE_ABILITIES[`${moduleName}::${node.name}`]) {
    return true;
  }

  return kind === "local";
}

function abilitySummary(node: RenderNode) {
  const abilities = node.node ? effectiveTypeAbilities(node.node) : node.tags;

  if (abilities.length) {
    return abilities.join(", ");
  }

  return node.abilitiesKnown ? "no abilities" : "abilities unknown";
}

function effectiveTypeAbilities(node: MoveTypeGraphNode) {
  if (node.abilities.length) {
    return node.abilities;
  }

  const builtinAbilities = BUILTIN_TYPE_ABILITIES[node.name];
  if (node.kind === "builtin" && builtinAbilities) {
    return [...builtinAbilities];
  }

  const moduleName = node.moduleName ?? node.qualifiedName.split("::").slice(-2, -1)[0];
  const typeName = node.name;
  const frameworkAbilities = moduleName
    ? SUI_FRAMEWORK_TYPE_ABILITIES[`${moduleName}::${typeName}`]
    : undefined;

  return frameworkAbilities ? [...frameworkAbilities] : [];
}

function classifyTypeNode(node: MoveTypeGraphNode, packagePath: string | null): TypeNodeKind {
  if (isLocalPackageType(node, packagePath)) {
    return "local";
  }

  if (node.kind === "builtin") {
    return "builtin";
  }

  if (isFrameworkType(node)) {
    return "framework";
  }

  return "external";
}

function isLocalPackageType(node: MoveTypeGraphNode, packagePath: string | null) {
  return packagePath !== null && node.packagePath === packagePath && isRenderableTypeNode(node);
}

function isRenderableTypeNode(node: MoveTypeGraphNode) {
  return DISPLAY_TYPE_KINDS.has(node.kind);
}

function isFrameworkType(node: MoveTypeGraphNode) {
  const address = node.address?.toLowerCase();
  const canonicalAddress = node.canonicalAddress?.toLowerCase();
  const moduleName = node.moduleName?.toLowerCase();

  return (
    node.kind === "builtin"
    || (address ? FRAMEWORK_ADDRESSES.has(address) : false)
    || (canonicalAddress ? FRAMEWORK_ADDRESSES.has(canonicalAddress) : false)
    || (moduleName ? FRAMEWORK_MODULES.has(moduleName) : false)
  );
}

function isExternalDependencyType(node: MoveTypeGraphNode | null) {
  return Boolean(node && !isFrameworkType(node) && !isLocalPackageType(node, null) && node.isExternal);
}

function isCapabilityLike(node: MoveTypeGraphNode | null) {
  if (!node) {
    return false;
  }

  const name = node.name.toLowerCase();
  return name.includes("cap") || name.includes("admin") || name.includes("authority");
}

function isResourceLike(node: MoveTypeGraphNode | null) {
  return Boolean(node && effectiveTypeAbilities(node).includes("key"));
}

function isEventLike(node: MoveTypeGraphNode | null) {
  return Boolean(node?.name.toLowerCase().includes("event"));
}

function isWitnessLike(node: MoveTypeGraphNode | null) {
  const name = node?.name.toLowerCase() ?? "";
  return name.includes("witness") || name === "otw";
}

function isGenericLike(node: MoveTypeGraphNode | null) {
  if (!node) {
    return false;
  }

  return node.qualifiedName.includes("<") || ["coin", "balance", "table", "vec_map", "vec_set"].includes(node.moduleName?.toLowerCase() ?? "");
}

function isGenericContainer(node: MoveTypeGraphNode | null) {
  if (!node) {
    return false;
  }

  const moduleName = node.moduleName?.toLowerCase() ?? "";
  return (node.typeParameters?.length ?? 0) > 0 || ["table", "vector", "vec_map", "vec_set"].includes(moduleName);
}

function genericParameterChips(node: MoveTypeGraphNode) {
  const parameters = node.typeParameters?.length
    ? node.typeParameters
    : knownGenericParameterNames(node).map((name) => ({ abilities: [], isPhantom: false, name }));

  return parameters.map((parameter, index) => ({
    label: parameter.name || String(index),
    value: parameter.isPhantom ? "phantom" : "type",
  }));
}

function edgeHasExternalType(
  edge: NormalizedEdge,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const sourceExternal =
    edge.sourceKind === "type"
    && edge.sourceNode
    && !localIds.has(edge.source)
    && !isLocalPackageType(edge.sourceNode, packagePath)
    && !isFrameworkType(edge.sourceNode);
  const targetExternal =
    edge.targetKind === "type"
    && edge.targetNode
    && !localIds.has(edge.target)
    && !isLocalPackageType(edge.targetNode, packagePath)
    && !isFrameworkType(edge.targetNode);

  return Boolean(sourceExternal || targetExternal);
}

function sourceBelongsToPackage(sourceId: string, packagePath: string | null) {
  return packagePath !== null && sourceId.startsWith(`function:${packagePath}:`);
}

function sortTypeNodes(nodes: MoveTypeGraphNode[], selectedTypeId: string | null) {
  return [...nodes].sort((left, right) => {
    if (left.id === selectedTypeId) {
      return -1;
    }
    if (right.id === selectedTypeId) {
      return 1;
    }

    return (
      (left.moduleName ?? "").localeCompare(right.moduleName ?? "")
      || left.name.localeCompare(right.name)
      || left.qualifiedName.localeCompare(right.qualifiedName)
    );
  });
}

function typeDisplayName(node: MoveTypeGraphNode, kind?: TypeNodeKind) {
  const genericParameterNames = node.typeParameters?.length
    ? node.typeParameters.map((parameter) => parameter.name)
    : knownGenericParameterNames(node);

  if (kind === "local" || node.packagePath) {
    return genericParameterNames.length ? `${node.name}<${genericParameterNames.join(", ")}>` : node.name;
  }

  if (node.kind === "builtin") {
    return genericParameterNames.length ? `${node.name}<${genericParameterNames.join(", ")}>` : node.name;
  }

  if (node.moduleName) {
    const base = `${node.moduleName}::${node.name}`;
    return genericParameterNames.length
      ? `${base}<${genericParameterNames.join(", ")}>`
      : base;
  }

  return node.qualifiedName;
}

function compactTypeLabel(value: string) {
  const normalized = value.replace(/\s+/g, " ").trim();
  const parts = normalized.split("::");
  return parts.length > 2 ? parts.slice(-2).join("::") : normalized;
}

function resolvedFieldType(edge: NormalizedEdge, genericInstance: GenericInstanceInfo | null = null) {
  const expression = edge.edge.typeExpression?.trim();

  if (expression) {
    const simplified = simplifyMoveTypeExpression(expression, edge.sourceNode);

    if (genericInstance?.arguments.length && !simplified.includes("<")) {
      return `${simplified}<${genericInstance.arguments.map((argument) => simplifyMoveTypeExpression(argument.value, edge.sourceNode)).join(", ")}>`;
    }

    return simplified;
  }

  if (genericInstance?.arguments.length) {
    return genericInstance.label;
  }

  if (edge.targetNode) {
    return simplifyMoveTypeExpression(edge.targetNode.qualifiedName || edge.targetNode.name, edge.sourceNode);
  }

  return compactEdgeEndpoint(edge.target);
}

function fieldBaseType(edge: NormalizedEdge) {
  if (edge.targetNode) {
    return compactEdgeEndpoint(edge.targetNode.id);
  }

  return compactEdgeEndpoint(edge.target);
}

function genericArgumentsFromTypeExpression(
  expression: string,
  targetNode: MoveTypeGraphNode | null,
) {
  const parsed = parseGenericTypeExpression(expression);

  if (!parsed) {
    return [];
  }

  const labels = knownGenericParameterNames(targetNode);
  const fallbackLabels = labels.length
    ? labels
    : knownGenericParameterNamesFromTypeBase(parsed.base);
  return splitTopLevelTypeArguments(parsed.arguments).map((argument, index) => ({
    label: fallbackLabels[index] ?? String(index),
    value: simplifyMoveTypeExpression(argument, targetNode),
  }));
}

function knownGenericParameterNamesFromTypeBase(base: string) {
  const normalized = base.toLowerCase();

  if (normalized.endsWith("table::table") || normalized === "table") {
    return ["K", "V"];
  }

  if (normalized.endsWith("vec_map::vec_map")) {
    return ["K", "V"];
  }

  if (
    normalized.endsWith("vec_set::vec_set")
    || normalized.endsWith("coin::coin")
    || normalized.endsWith("balance::balance")
    || normalized.endsWith("balance::supply")
    || normalized === "coin"
    || normalized === "balance"
    || normalized === "vector"
  ) {
    return ["T"];
  }

  return [];
}

function fieldSemanticTags(
  fieldName: string,
  fieldType: string,
  targetNode: MoveTypeGraphNode | null,
) {
  const tags: string[] = [];
  const lowerName = fieldName.toLowerCase();
  const lowerType = fieldType.toLowerCase();

  if ((lowerName === "id" || lowerName === "uid") && lowerType.includes("uid")) {
    tags.push("key field");
  }

  if (lowerName.includes("config")) {
    tags.push("config field");
  }

  if (isExternalDependencyType(targetNode)) {
    tags.push("external field");
  }

  if (
    lowerType.includes("<")
    || lowerType.includes("table")
    || lowerType.includes("coin")
    || lowerType.includes("balance")
    || lowerType.includes("vector")
  ) {
    tags.push("container field");
  }

  if (/(asset|amount|balance|principal|total|saved|account|pool|margin|treasury|vault)/i.test(fieldName)) {
    tags.push("value field");
  }

  return [...new Set(tags)].slice(0, 2);
}

function simplifyMoveTypeExpression(
  value: string,
  declaringNode: MoveTypeGraphNode | null = null,
): string {
  const normalized = value.replace(/\s+/g, " ").trim();
  const parsed = parseGenericTypeExpression(normalized);

  if (!parsed) {
    return simplifyMoveTypeBase(normalized, declaringNode);
  }

  const base = simplifyMoveTypeBase(parsed.base, declaringNode);
  const args = splitTopLevelTypeArguments(parsed.arguments)
    .map((argument) => simplifyMoveTypeExpression(argument, declaringNode))
    .join(", ");

  return `${base}<${args}>`;
}

function simplifyMoveTypeBase(value: string, declaringNode: MoveTypeGraphNode | null) {
  const normalized = value.trim();
  const parts = normalized.split("::").filter(Boolean);

  if (parts.length < 2) {
    return normalized;
  }

  const name = parts[parts.length - 1] ?? normalized;
  const moduleName = parts[parts.length - 2] ?? "";
  const lowerModule = moduleName.toLowerCase();
  const lowerName = name.toLowerCase();
  const declaringModule = declaringNode?.moduleName?.toLowerCase();

  if (declaringModule && lowerModule === declaringModule) {
    return name;
  }

  if (
    (lowerModule === "table" && lowerName === "table")
    || (lowerModule === "coin" && lowerName === "coin")
    || (lowerModule === "balance" && (lowerName === "balance" || lowerName === "supply"))
    || lowerName === "vector"
  ) {
    return name;
  }

  return `${moduleName}::${name}`;
}

function parseGenericTypeExpression(value: string) {
  const trimmed = value.trim();
  const start = trimmed.indexOf("<");

  if (start < 0 || !trimmed.endsWith(">")) {
    return null;
  }

  let depth = 0;
  for (let index = start; index < trimmed.length; index += 1) {
    const char = trimmed[index];
    if (char === "<") {
      depth += 1;
    } else if (char === ">") {
      depth -= 1;
      if (depth === 0 && index !== trimmed.length - 1) {
        return null;
      }
    }
  }

  if (depth !== 0) {
    return null;
  }

  return {
    arguments: trimmed.slice(start + 1, -1),
    base: trimmed.slice(0, start),
  };
}

function splitTopLevelTypeArguments(value: string) {
  const args: string[] = [];
  let depth = 0;
  let start = 0;

  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if (char === "<") {
      depth += 1;
    } else if (char === ">") {
      depth -= 1;
    } else if (char === "," && depth === 0) {
      args.push(value.slice(start, index).trim());
      start = index + 1;
    }
  }

  const last = value.slice(start).trim();
  if (last) {
    args.push(last);
  }

  return args;
}

function importantLocalTypeId(graph: MoveTypeGraph, movePackage: MovePackage | null) {
  const packagePath = movePackage?.path ?? null;
  const localNodes = graph.nodes.filter((node) => isLocalPackageType(node, packagePath));

  if (!localNodes.length) {
    return null;
  }

  const degree = new Map<string, number>();
  for (const edge of graph.edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
  }

  return [...localNodes].sort((left, right) =>
    importantTypeScore(right, degree) - importantTypeScore(left, degree)
    || left.name.localeCompare(right.name),
  )[0]?.id ?? null;
}

function importantTypeScore(node: MoveTypeGraphNode, degree: Map<string, number>) {
  const name = node.name.toLowerCase();
  let score = degree.get(node.id) ?? 0;

  if (/(vault|pool|escrow|market|registry|admincap|cap|bucket|receipt)/.test(name)) {
    score += 100;
  }

  if (isResourceLike(node)) {
    score += 40;
  }

  if (isCapabilityLike(node)) {
    score += 35;
  }

  if (isGenericLike(node)) {
    score += 10;
  }

  return score;
}

function buildFunctionIndex(movePackage: MovePackage | null) {
  const index = new Map<string, FunctionContext>();

  if (!movePackage) {
    return index;
  }

  for (const moveModule of movePackage.modules) {
    for (const moveFunction of moveModule.functions) {
      const id = functionId(movePackage.path, moveModule.address, moveModule.name, moveFunction.name);
      index.set(id, {
        functionName: moveFunction.name,
        id,
        isEntry: moveFunction.isEntry,
        isTransactionCallable: moveFunction.isTransactionCallable,
        label: `${moveModule.name}::${moveFunction.name}`,
        moduleName: moveModule.name,
        signature: moveFunction.signature,
        visibility: moveFunction.visibility,
      });
    }
  }

  return index;
}

function functionId(
  packagePath: string,
  address: string | null,
  moduleName: string,
  functionName: string,
) {
  return `function:${packagePath}:${address ?? "_"}::${moduleName}::${functionName}`;
}

function functionDetails(id: string, functionIndex: Map<string, FunctionContext>) {
  const indexed = functionIndex.get(id);

  if (indexed) {
    return indexed;
  }

  const match = id.match(/^function:(.*):([^:]*)::([^:]+)::([^:]+)$/);
  const moduleName = match?.[3] ?? "function";
  const functionName = match?.[4] ?? functionLabelFromId(id);

  return {
    functionName,
    id,
    isEntry: false,
    isTransactionCallable: false,
    label: `${moduleName}::${functionName}`,
    moduleName,
    signature: `${moduleName}::${functionName}(...)`,
    visibility: "function",
  };
}

function functionLabel(id: string, functionIndex: Map<string, FunctionContext>) {
  return functionDetails(id, functionIndex).label;
}

function functionLabelFromId(id: string) {
  const parts = id.split("::");
  return parts[parts.length - 1] ?? id;
}

function pluralize(value: number, label: string) {
  return `${value} ${label}${value === 1 ? "" : "s"}`;
}
