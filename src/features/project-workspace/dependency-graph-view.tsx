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

import { Badge } from "@/components/ui/badge";
import type {
  PackageDependencyEdge,
  PackageDependencyGraph,
  PackageDependencyNode,
} from "@/features/empty-project/filesystem-tree";

type DependencyGraphViewProps = {
  className?: string;
  graph: PackageDependencyGraph;
  packageName?: string;
};

type PackageNodeData = PackageDependencyNode & {
  color: string;
  depth: number;
  directDependencies: number;
  focus: boolean;
  incomingPackages: number;
  outgoingPackages: number;
  role: string;
};

type LayoutNode = PackageDependencyNode & {
  depth: number;
  x: number;
  y: number;
};

const NODE_TYPES = {
  package: PackageGraphNode,
};

const ROOT_COLOR = "#0ea5e9";
const DIRECT_COLOR = "#10b981";
const TRANSITIVE_COLOR = "#8b5cf6";
const FRAMEWORK_COLOR = "#64748b";

export function DependencyGraphView({
  className = "h-72 rounded-md border",
  graph,
  packageName = "",
}: DependencyGraphViewProps) {
  const renderGraph = connectedPackageGraph(graph, packageName);

  if (!renderGraph.summaryPath) {
    return (
      <EmptyGraphState
        className={className}
        packageName={packageName || renderGraph.root || "active package"}
      />
    );
  }

  const layoutNodes = layoutGraph(renderGraph);
  const primaryEdges = primaryEdgeIds(renderGraph);
  const stats = graphStats(renderGraph);
  const flowNodes = layoutNodes.map<Node<PackageNodeData>>((node) => {
    const color = nodeColor(node);

    return {
      id: node.id,
      type: "package",
      position: { x: node.x, y: node.y },
      data: {
        ...node,
        color,
        directDependencies: stats.outgoing.get(node.id) ?? 0,
        focus: node.id === renderGraph.root,
        incomingPackages: stats.incoming.get(node.id) ?? 0,
        outgoingPackages: stats.outgoing.get(node.id) ?? 0,
        role: nodeRole(node, renderGraph.root),
      },
    };
  });
  const flowEdges = renderGraph.edges.map<Edge>((edge) => {
    const id = edgeId(edge);
    const isPrimary = primaryEdges.has(id);
    const isDirect = edge.source === renderGraph.root;
    const color = isDirect ? ROOT_COLOR : isPrimary ? DIRECT_COLOR : "#64748b";

    return {
      id,
      source: edge.source,
      target: edge.target,
      label: edgeLabel(edge, renderGraph, isPrimary),
      labelBgPadding: [6, 4],
      labelBgBorderRadius: 4,
      labelBgStyle: {
        fill: "color-mix(in oklch, var(--background) 88%, transparent)",
      },
      labelStyle: {
        fill: isPrimary ? color : "#94a3b8",
        fontSize: 10,
        fontWeight: 600,
      },
      markerEnd: {
        type: MarkerType.ArrowClosed,
        color,
      },
      style: {
        opacity: isPrimary ? 0.85 : 0.28,
        stroke: color,
        strokeDasharray: isPrimary ? undefined : "6 8",
        strokeWidth: isDirect ? 2.1 : isPrimary ? 1.7 : 1.2,
      },
      type: "smoothstep",
    };
  });
  const flowKey = `${renderGraph.root ?? "graph"}-${flowNodes.length}-${flowEdges.length}`;

  return (
    <div className={`${className} relative overflow-hidden border-[color:var(--app-border)] bg-[var(--app-surface)]`}>
      <ReactFlow
        key={flowKey}
        colorMode="dark"
        edges={flowEdges}
        edgesFocusable={false}
        fitView
        fitViewOptions={{ padding: 0.18 }}
        maxZoom={1.8}
        minZoom={0.25}
        nodes={flowNodes}
        nodesDraggable={false}
        nodesFocusable={false}
        nodeTypes={NODE_TYPES}
        proOptions={{ hideAttribution: true }}
      >
        <Background color="var(--border)" gap={18} size={1} />
        <Controls
          className="!bg-background/90 !shadow-none [&_button]:!border-border [&_button]:!bg-background [&_button]:!text-foreground"
          position="bottom-right"
          showInteractive={false}
        />
      </ReactFlow>
      <div className="pointer-events-none absolute left-3 top-3 rounded-md border border-[color:var(--app-border)] bg-background/80 px-2.5 py-2 text-[11px] leading-tight text-muted-foreground shadow-sm backdrop-blur">
        <div className="font-medium text-foreground">Immediate module dependencies</div>
        <div className="mt-1">source package uses target package</div>
        <div className="mt-1 flex items-center gap-3">
          <span className="inline-flex items-center gap-1">
            <span className="h-px w-5 bg-sky-400" />
            primary path
          </span>
          <span className="inline-flex items-center gap-1">
            <span className="h-px w-5 border-t border-dashed border-slate-500" />
            additional link
          </span>
        </div>
      </div>
    </div>
  );
}

export default DependencyGraphView;

function PackageGraphNode({ data }: NodeProps<Node<PackageNodeData>>) {
  const entryFunctionCount = data.entryFunctionCount ?? 0;
  const publicFunctionCount = data.publicFunctionCount ?? 0;

  return (
    <div
      className="group relative w-56 rounded-md border bg-[var(--app-elevated)] px-3 py-2.5 shadow-sm"
      style={{
        borderColor: data.color,
        boxShadow: data.focus
          ? `0 0 0 1px color-mix(in oklch, ${data.color} 34%, transparent)`
          : "none",
      }}
    >
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

      <div className="flex min-w-0 items-center gap-2">
        <span
          className="size-2.5 shrink-0 rounded-full"
          style={{ backgroundColor: data.color }}
        />
        <span className="min-w-0 truncate text-sm font-semibold text-card-foreground">
          {data.id}
        </span>
      </div>

      <div className="mt-1 flex min-w-0 items-center justify-between gap-2">
        <span className="truncate text-xs text-muted-foreground">{data.role}</span>
        {data.focus ? (
          <Badge className="rounded bg-muted px-1.5 py-0.5 text-[10px]" variant="secondary">
            active
          </Badge>
        ) : null}
      </div>

      <div className="mt-2 grid grid-cols-2 gap-1.5 text-[11px] text-muted-foreground">
        <span className="rounded bg-muted/70 px-1.5 py-1">
          {pluralize(data.moduleCount ?? 0, "module")}
        </span>
        <span className="rounded bg-muted/70 px-1.5 py-1">
          {pluralize(data.outgoingPackages, "dependency")}
        </span>
        {entryFunctionCount > 0 ? (
          <span className="rounded bg-rose-500/10 px-1.5 py-1 text-rose-300">
            {pluralize(entryFunctionCount, "entry")}
          </span>
        ) : null}
        {publicFunctionCount > 0 ? (
          <span className="rounded bg-sky-500/10 px-1.5 py-1 text-sky-300">
            {pluralize(publicFunctionCount, "public")}
          </span>
        ) : null}
      </div>

      <div className="mt-2 truncate text-[11px] text-muted-foreground">
        {shortAddress(data.address) ?? "unresolved address"}
      </div>

      <div className="pointer-events-none absolute left-1/2 top-full z-20 mt-2 hidden w-72 -translate-x-1/2 rounded-lg border bg-popover p-3 text-popover-foreground shadow-xl group-hover:block">
        <div className="text-sm font-semibold">{data.id}</div>
        <dl className="mt-2 space-y-1 text-xs">
          <MetadataRow label="Role" value={data.role} />
          <MetadataRow label="Address" value={data.address ?? "unresolved"} />
          <MetadataRow label="Modules" value={String(data.moduleCount ?? 0)} />
          <MetadataRow label="Entry functions" value={String(entryFunctionCount)} />
          <MetadataRow label="Public functions" value={String(publicFunctionCount)} />
          <MetadataRow label="Uses packages" value={String(data.outgoingPackages)} />
          <MetadataRow label="Used by packages" value={String(data.incomingPackages)} />
        </dl>
      </div>
    </div>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[7rem_minmax(0,1fr)] gap-3">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="truncate text-right">{value}</dd>
    </div>
  );
}

function EmptyGraphState({
  className,
  packageName,
}: {
  className: string;
  packageName: string;
}) {
  return (
    <div className={`${className} grid place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-6 text-center`}>
      <div className="max-w-md">
        <div className="text-sm font-semibold text-foreground">No dependency summary found</div>
        <p className="mt-2 text-sm text-muted-foreground">
          Peregrine needs package summary data for {packageName} before it can draw the real package dependency graph.
        </p>
      </div>
    </div>
  );
}

function connectedPackageGraph(
  graph: PackageDependencyGraph,
  packageName: string,
): PackageDependencyGraph {
  const knownNodes = new Set(graph.nodes.map((node) => node.id));
  const focus =
    packageName && knownNodes.has(packageName)
      ? packageName
      : graph.root && knownNodes.has(graph.root)
        ? graph.root
        : graph.nodes[0]?.id ?? null;

  if (!focus) {
    return graph;
  }

  const adjacency = new Map<string, Set<string>>();

  for (const node of graph.nodes) {
    adjacency.set(node.id, new Set());
  }

  for (const edge of graph.edges) {
    if (!knownNodes.has(edge.source) || !knownNodes.has(edge.target)) {
      continue;
    }

    adjacency.get(edge.source)?.add(edge.target);
    adjacency.get(edge.target)?.add(edge.source);
  }

  const connected = new Set<string>([focus]);
  const queue = [focus];

  while (queue.length) {
    const node = queue.shift()!;

    for (const next of adjacency.get(node) ?? []) {
      if (connected.has(next)) {
        continue;
      }

      connected.add(next);
      queue.push(next);
    }
  }

  return {
    ...graph,
    root: focus,
    nodes: graph.nodes
      .filter((node) => connected.has(node.id))
      .map((node) => ({
        ...node,
        isRoot: node.id === focus,
      })),
    edges: graph.edges.filter(
      (edge) => connected.has(edge.source) && connected.has(edge.target),
    ),
  };
}

function layoutGraph(graph: PackageDependencyGraph): LayoutNode[] {
  const levels = graphLevels(graph);
  const grouped = new Map<number, PackageDependencyNode[]>();

  for (const node of graph.nodes) {
    const depth = levels.get(node.id) ?? 0;
    const siblings = grouped.get(depth) ?? [];

    siblings.push(node);
    grouped.set(depth, siblings);
  }

  const columnWidth = 335;
  const rowHeight = 132;
  const laidOut: LayoutNode[] = [];

  for (const [depth, siblings] of [...grouped].sort((left, right) => left[0] - right[0])) {
    const sortedSiblings = siblings.sort((left, right) =>
      Number(right.isRoot) - Number(left.isRoot)
      || (right.moduleCount ?? 0) - (left.moduleCount ?? 0)
      || left.id.localeCompare(right.id),
    );
    const offset = ((sortedSiblings.length - 1) * rowHeight) / 2;

    sortedSiblings.forEach((node, index) => {
      laidOut.push({
        ...node,
        depth,
        x: depth * columnWidth,
        y: index * rowHeight - offset,
      });
    });
  }

  return laidOut;
}

function graphLevels(graph: PackageDependencyGraph) {
  const root = graph.root ?? graph.nodes[0]?.id ?? null;
  const levels = new Map<string, number>();

  if (!root) {
    return levels;
  }

  const outgoing = outgoingEdges(graph.edges);
  const queue = [root];

  levels.set(root, 0);

  while (queue.length) {
    const source = queue.shift()!;
    const sourceLevel = levels.get(source) ?? 0;
    const edges = [...(outgoing.get(source) ?? [])].sort(
      (left, right) => right.dependencyCount - left.dependencyCount || left.target.localeCompare(right.target),
    );

    for (const edge of edges) {
      if (levels.has(edge.target)) {
        continue;
      }

      levels.set(edge.target, sourceLevel + 1);
      queue.push(edge.target);
    }
  }

  const incoming = incomingEdges(graph.edges);

  for (const node of graph.nodes) {
    if (levels.has(node.id)) {
      continue;
    }

    const nearestIncoming = [...(incoming.get(node.id) ?? [])]
      .map((edge) => levels.get(edge.source))
      .filter((level): level is number => typeof level === "number")
      .sort((left, right) => left - right)[0];

    levels.set(node.id, typeof nearestIncoming === "number" ? nearestIncoming + 1 : 1);
  }

  return levels;
}

function primaryEdgeIds(graph: PackageDependencyGraph) {
  const root = graph.root ?? graph.nodes[0]?.id ?? null;
  const primary = new Set<string>();

  if (!root) {
    return primary;
  }

  const outgoing = outgoingEdges(graph.edges);
  const visited = new Set<string>([root]);
  const queue = [root];

  while (queue.length) {
    const source = queue.shift()!;
    const edges = [...(outgoing.get(source) ?? [])].sort(
      (left, right) => right.dependencyCount - left.dependencyCount || left.target.localeCompare(right.target),
    );

    for (const edge of edges) {
      const id = edgeId(edge);

      if (edge.source === root || !visited.has(edge.target)) {
        primary.add(id);
      }

      if (!visited.has(edge.target)) {
        visited.add(edge.target);
        queue.push(edge.target);
      }
    }
  }

  return primary;
}

function graphStats(graph: PackageDependencyGraph) {
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, number>();

  for (const node of graph.nodes) {
    incoming.set(node.id, 0);
    outgoing.set(node.id, 0);
  }

  for (const edge of graph.edges) {
    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + 1);
    outgoing.set(edge.source, (outgoing.get(edge.source) ?? 0) + 1);
  }

  return { incoming, outgoing };
}

function outgoingEdges(edges: PackageDependencyEdge[]) {
  const outgoing = new Map<string, PackageDependencyEdge[]>();

  for (const edge of edges) {
    const group = outgoing.get(edge.source) ?? [];

    group.push(edge);
    outgoing.set(edge.source, group);
  }

  return outgoing;
}

function incomingEdges(edges: PackageDependencyEdge[]) {
  const incoming = new Map<string, PackageDependencyEdge[]>();

  for (const edge of edges) {
    const group = incoming.get(edge.target) ?? [];

    group.push(edge);
    incoming.set(edge.target, group);
  }

  return incoming;
}

function nodeColor(node: PackageDependencyNode & { depth: number }) {
  if (node.isRoot || node.depth === 0) {
    return ROOT_COLOR;
  }

  if (isFrameworkPackage(node.id)) {
    return FRAMEWORK_COLOR;
  }

  if (node.depth === 1) {
    return DIRECT_COLOR;
  }

  return TRANSITIVE_COLOR;
}

function nodeRole(node: PackageDependencyNode & { depth: number }, root: string | null) {
  if (node.id === root || node.isRoot) {
    return "active package";
  }

  if (node.depth === 1) {
    return "direct dependency";
  }

  if (isFrameworkPackage(node.id)) {
    return "framework dependency";
  }

  return "transitive dependency";
}

function edgeLabel(
  edge: PackageDependencyEdge,
  graph: PackageDependencyGraph,
  isPrimary: boolean,
) {
  const count = edge.dependencyCount ?? 0;

  if (!isPrimary && graph.edges.length > 10) {
    return undefined;
  }

  if (edge.source !== graph.root && count < 3 && graph.edges.length > 8) {
    return undefined;
  }

  return count === 1 ? "1 link" : `${count} links`;
}

function edgeId(edge: PackageDependencyEdge) {
  return `${edge.source}->${edge.target}`;
}

function shortAddress(address: string | null | undefined) {
  if (!address) {
    return null;
  }

  if (address.length <= 18) {
    return address;
  }

  return `${address.slice(0, 8)}...${address.slice(-6)}`;
}

function pluralize(count: number, label: string) {
  if (count === 1) {
    return `1 ${label}`;
  }

  return `${count} ${label}s`;
}

function isFrameworkPackage(id: string) {
  return id === "std" || id === "sui";
}
