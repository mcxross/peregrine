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

import type {
  PackageDependencyGraph,
  PackageDependencyNode,
} from "@/features/empty-project/filesystem-tree";

type DependencyGraphViewProps = {
  className?: string;
  graph: PackageDependencyGraph;
};

type PackageNodeData = PackageDependencyNode & {
  color: string;
};

const NODE_TYPES = {
  package: PackageGraphNode,
};

const PALETTE = [
  "#60a5fa",
  "#34d399",
  "#f59e0b",
  "#f472b6",
  "#a78bfa",
  "#22d3ee",
  "#fb7185",
  "#84cc16",
];

export function DependencyGraphView({
  className = "h-72 rounded-md border",
  graph,
}: DependencyGraphViewProps) {
  if (!graph.nodes.length) {
    return (
      <div className="flex h-full min-h-48 items-center justify-center rounded-md border bg-background/40 px-4 text-center text-sm text-muted-foreground">
        Build summaries are needed to construct package dependencies.
      </div>
    );
  }

  const levels = graphLevels(graph);
  const nodes = graph.nodes.map<Node<PackageNodeData>>((node, index) => {
    const level = levels.get(node.id) ?? 0;
    const siblingIndex = graph.nodes
      .filter((candidate) => (levels.get(candidate.id) ?? 0) === level)
      .findIndex((candidate) => candidate.id === node.id);

    return {
      id: node.id,
      type: "package",
      position: {
        x: level * 270,
        y: siblingIndex * 150,
      },
      data: {
        ...node,
        color: node.isRoot ? "#f97316" : PALETTE[index % PALETTE.length],
      },
    };
  });
  const edges = graph.edges.map<Edge>((edge, index) => ({
    id: `${edge.source}->${edge.target}`,
    source: edge.source,
    target: edge.target,
    markerEnd: {
      type: MarkerType.ArrowClosed,
      color: edge.source === graph.root ? PALETTE[index % PALETTE.length] : "#64748b",
    },
    style: {
      opacity: edge.source === graph.root ? 0.95 : 0.42,
      stroke: edge.source === graph.root ? PALETTE[index % PALETTE.length] : "#64748b",
      strokeWidth: Math.min(3, 1.2 + edge.dependencyCount / 16),
    },
    type: "smoothstep",
  }));

  return (
    <div className={`${className} overflow-hidden bg-background`}>
      <ReactFlow
        colorMode="dark"
        defaultEdges={edges}
        defaultNodes={nodes}
        edgesFocusable={false}
        fitView
        fitViewOptions={{ padding: 0.22 }}
        maxZoom={1.7}
        minZoom={0.25}
        nodeTypes={NODE_TYPES}
        nodesDraggable={false}
        nodesFocusable={false}
        proOptions={{ hideAttribution: true }}
      >
        <Background color="var(--border)" gap={18} size={1} />
        <Controls
          className="!bg-background/90 !shadow-none [&_button]:!border-border [&_button]:!bg-background [&_button]:!text-foreground"
          position="bottom-right"
          showInteractive={false}
        />
      </ReactFlow>
    </div>
  );
}

export default DependencyGraphView;

function PackageGraphNode({ data }: NodeProps<Node<PackageNodeData>>) {
  return (
    <div
      className="group relative w-44 rounded-lg border bg-card px-3 py-2 shadow-sm"
      style={{
        borderColor: data.color,
        boxShadow: `0 0 0 1px color-mix(in oklch, ${data.color} 32%, transparent)`,
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
      <div className="flex items-center gap-2">
        <span
          className="size-2.5 rounded-full"
          style={{ backgroundColor: data.color }}
        />
        <span className="min-w-0 truncate text-sm font-semibold text-card-foreground">
          {data.id}
        </span>
      </div>
      <div className="mt-1 flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
        <span>{data.moduleCount} modules</span>
        {data.isRoot ? <span className="text-orange-300">root</span> : null}
      </div>
      <div className="pointer-events-none absolute left-1/2 top-full z-20 mt-2 hidden w-64 -translate-x-1/2 rounded-lg border bg-popover p-3 text-popover-foreground shadow-xl group-hover:block">
        <div className="text-sm font-semibold">{data.id}</div>
        <dl className="mt-2 space-y-1 text-xs">
          <div className="flex justify-between gap-3">
            <dt className="text-muted-foreground">Address</dt>
            <dd className="truncate text-right">{data.address ?? "unresolved"}</dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt className="text-muted-foreground">Modules</dt>
            <dd>{data.moduleCount}</dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt className="text-muted-foreground">Role</dt>
            <dd>{data.isRoot ? "root package" : "dependency"}</dd>
          </div>
        </dl>
      </div>
    </div>
  );
}

function graphLevels(graph: PackageDependencyGraph) {
  const levels = new Map<string, number>();
  const queue = graph.root ? [graph.root] : graph.nodes.slice(0, 1).map((node) => node.id);

  if (!queue.length) {
    return levels;
  }

  levels.set(queue[0], 0);

  while (queue.length) {
    const source = queue.shift()!;
    const sourceLevel = levels.get(source) ?? 0;

    for (const edge of graph.edges) {
      if (edge.source !== source || levels.has(edge.target)) {
        continue;
      }

      levels.set(edge.target, sourceLevel + 1);
      queue.push(edge.target);
    }
  }

  for (const node of graph.nodes) {
    if (!levels.has(node.id)) {
      levels.set(node.id, 0);
    }
  }

  return levels;
}
