import React from "react";
import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  Controls,
  Handle,
  MiniMap,
  Panel,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  BellRing,
  Bot,
  Boxes,
  BrainCircuit,
  Cable,
  GitBranch,
  Hammer,
  MemoryStick,
  Route,
  Split,
  Upload,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  createWorkflowNode,
} from "@peregrine/desktop-runtime";
import type {
  AgentStatus,
  AgentWorkflow,
  AgentWorkflowNodeData,
  AgentWorkflowNodeType,
} from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";

type AgentWorkflowCanvasProps = {
  onRunNode: (nodeId: string) => void;
  onWorkflowChange: (workflow: AgentWorkflow) => void;
  workflow: AgentWorkflow;
};

const palette: Array<{
  type: AgentWorkflowNodeType;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  label: string;
}> = [
  { type: "trigger", icon: BellRing, label: "Trigger" },
  { type: "agent", icon: Bot, label: "Agent" },
  { type: "tool", icon: Hammer, label: "Tool" },
  { type: "condition", icon: Split, label: "Condition" },
  { type: "model", icon: BrainCircuit, label: "Model" },
  { type: "memory", icon: MemoryStick, label: "Memory" },
  { type: "input", icon: Upload, label: "Input" },
  { type: "output", icon: Route, label: "Output" },
  { type: "integration", icon: Cable, label: "Integration" },
];

export function AgentWorkflowCanvas(props: AgentWorkflowCanvasProps) {
  return (
    <ReactFlowProvider>
      <AgentWorkflowCanvasInner {...props} />
    </ReactFlowProvider>
  );
}

function AgentWorkflowCanvasInner({
  onRunNode,
  onWorkflowChange,
  workflow,
}: AgentWorkflowCanvasProps) {
  const { screenToFlowPosition } = useReactFlow();
  const [nodes, setNodes] = React.useState(workflow.nodes);
  const [edges, setEdges] = React.useState(workflow.edges);
  const nodeTypes = React.useMemo(
    () => ({
      agentWorkflow: (props: NodeProps) => (
        <AgentWorkflowNodeComponent {...props} onRunNode={onRunNode} />
      ),
    }),
    [onRunNode],
  );

  React.useEffect(() => {
    setNodes(workflow.nodes);
    setEdges(workflow.edges);
  }, [workflow.id, workflow.nodes, workflow.edges]);

  const publish = React.useCallback(
    (nextNodes = nodes, nextEdges = edges) => {
      onWorkflowChange({
        ...workflow,
        nodes: nextNodes,
        edges: nextEdges,
        updatedAt: Date.now(),
        version: workflow.version + 1,
      });
    },
    [edges, nodes, onWorkflowChange, workflow],
  );

  const onNodesChange = React.useCallback(
    (changes: NodeChange[]) => {
      setNodes((currentNodes) => {
        const nextNodes = applyNodeChanges(changes, currentNodes) as typeof currentNodes;
        publish(nextNodes, edges);

        return nextNodes;
      });
    },
    [edges, publish],
  );
  const onEdgesChange = React.useCallback(
    (changes: EdgeChange[]) => {
      setEdges((currentEdges) => {
        const nextEdges = applyEdgeChanges(changes, currentEdges);
        publish(nodes, nextEdges);

        return nextEdges;
      });
    },
    [nodes, publish],
  );
  const onConnect = React.useCallback(
    (connection: Connection) => {
      setEdges((currentEdges) => {
        const nextEdges = addEdge(
          {
            ...connection,
            animated: true,
            type: "smoothstep",
          },
          currentEdges,
        );
        publish(nodes, nextEdges);

        return nextEdges;
      });
    },
    [nodes, publish],
  );
  const onDragOver = React.useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }, []);
  const onDrop = React.useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();
      const nodeType = event.dataTransfer.getData("application/peregrine-agent-node") as AgentWorkflowNodeType;

      if (!nodeType) {
        return;
      }

      const nextNode = createWorkflowNode(
        nodeType,
        screenToFlowPosition({ x: event.clientX, y: event.clientY }),
      );
      const nextNodes = [...nodes, nextNode];
      setNodes(nextNodes);
      publish(nextNodes, edges);
    },
    [edges, nodes, publish, screenToFlowPosition],
  );

  return (
    <div className="relative h-full min-h-0 overflow-hidden bg-[radial-gradient(circle_at_18px_18px,color-mix(in_oklch,var(--foreground)_10%,transparent)_1px,transparent_1px)] [background-size:24px_24px]">
      <ReactFlow
        colorMode="dark"
        edges={edges}
        fitView
        maxZoom={1.35}
        minZoom={0.35}
        nodeTypes={nodeTypes}
        nodes={nodes}
        onConnect={onConnect}
        onDragOver={onDragOver}
        onDrop={onDrop}
        onEdgesChange={onEdgesChange}
        onNodesChange={onNodesChange}
        proOptions={{ hideAttribution: true }}
      >
        <Background color="rgba(148,163,184,0.14)" gap={24} size={1} />
        <MiniMap
          className="!bottom-4 !right-4 !rounded-md !border !border-[color:var(--app-border)] !bg-[var(--app-panel)]"
          maskColor="rgba(0,0,0,0.32)"
          nodeColor={(node) => nodeColor((node.data as AgentWorkflowNodeData).nodeType)}
          pannable
          zoomable
        />
        <Controls
          className="!bottom-4 !left-4 !rounded-md !border !border-[color:var(--app-border)] !bg-[var(--app-panel)]"
          position="bottom-left"
        />
        <Panel position="top-left" className="m-3">
          <NodePalette />
        </Panel>
        <Panel position="top-right" className="m-3">
          <div className="flex items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-2 text-[11px] text-muted-foreground shadow-lg">
            <GitBranch className="size-3.5 text-primary" />
            <span className="font-medium text-foreground">v{workflow.version}</span>
            <span>{workflow.nodes.length} nodes</span>
            <span>{workflow.edges.length} edges</span>
          </div>
        </Panel>
      </ReactFlow>
      <div className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-[var(--app-window)] to-transparent" />
    </div>
  );

}

function AgentWorkflowNodeComponent({
  data: rawData,
  id,
  onRunNode,
  selected,
}: NodeProps & {
  onRunNode: (nodeId: string) => void;
}) {
  const data = rawData as AgentWorkflowNodeData;
  const Icon = iconForNodeType(data.nodeType);

  return (
    <div
      className={cn(
        "group w-[188px] rounded-md border bg-[var(--app-elevated)] shadow-[0_18px_42px_rgba(0,0,0,0.28),inset_0_1px_0_rgba(255,255,255,0.06)]",
        selected
          ? "border-primary/70 ring-2 ring-primary/20"
          : "border-[color:var(--app-border)]",
      )}
    >
      <Handle
        className="!size-2.5 !border !border-background !bg-muted-foreground"
        position={Position.Left}
        type="target"
      />
      <div className="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 border-b border-[color:var(--app-border)] px-2.5 py-2">
        <span
          className="grid size-7 place-items-center rounded-md"
          style={{ backgroundColor: `${nodeColor(data.nodeType)}22`, color: nodeColor(data.nodeType) }}
        >
          <Icon className="size-3.5" />
        </span>
        <span className="min-w-0">
          <span className="block truncate text-xs font-semibold text-foreground">
            {data.label}
          </span>
          <span className="block truncate text-[10px] uppercase tracking-wide text-muted-foreground">
            {nodeTypeLabel(data.nodeType)}
          </span>
        </span>
        <StatusDot status={data.status} />
      </div>
      <div className="space-y-2 px-2.5 py-2">
        <p className="line-clamp-2 min-h-8 text-[11px] leading-4 text-muted-foreground">
          {data.description || "Ready"}
        </p>
        {data.provider ? (
          <Badge className="max-w-full truncate rounded px-1.5 py-0.5 text-[10px]" variant="secondary">
            {data.provider.providerId} / {data.provider.modelId}
          </Badge>
        ) : null}
        <Button
          className="h-6 w-full text-[11px]"
          onClick={() => onRunNode(id)}
          size="sm"
          type="button"
          variant="secondary"
        >
          Run node
        </Button>
      </div>
      <Handle
        className="!size-2.5 !border !border-background !bg-primary"
        position={Position.Right}
        type="source"
      />
    </div>
  );
}

function NodePalette() {
  return (
    <div className="grid grid-cols-3 gap-1.5 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-1.5 shadow-lg">
      {palette.map((item) => {
        const Icon = item.icon;

        return (
          <button
            className="grid h-14 w-20 place-items-center gap-1 rounded border border-transparent text-[10px] text-muted-foreground hover:border-[color:var(--app-border)] hover:bg-[var(--app-subtle)] hover:text-foreground"
            draggable
            key={item.type}
            onDragStart={(event) => {
              event.dataTransfer.setData("application/peregrine-agent-node", item.type);
              event.dataTransfer.effectAllowed = "move";
            }}
            type="button"
          >
            <Icon className="size-4" style={{ color: nodeColor(item.type) }} />
            <span className="truncate">{item.label}</span>
          </button>
        );
      })}
    </div>
  );
}

function StatusDot({ status }: { status: AgentStatus }) {
  return (
    <span
      className={cn(
        "size-2 rounded-full",
        status === "idle" && "bg-muted-foreground",
        status === "active" && "bg-emerald-400",
        status === "running" && "animate-pulse bg-sky-400",
        status === "completed" && "bg-emerald-400",
        status === "blocked" && "bg-red-400",
        status === "needsApproval" && "bg-amber-400",
        status === "failed" && "bg-red-400",
      )}
    />
  );
}

function iconForNodeType(type: AgentWorkflowNodeType) {
  const icons = {
    trigger: BellRing,
    agent: Bot,
    tool: Hammer,
    condition: Split,
    model: BrainCircuit,
    memory: MemoryStick,
    input: Upload,
    output: Boxes,
    integration: Cable,
  };

  return icons[type];
}

function nodeTypeLabel(type: AgentWorkflowNodeType) {
  return type.replace(/([A-Z])/g, " $1");
}

function nodeColor(type: AgentWorkflowNodeType) {
  const colors: Record<AgentWorkflowNodeType, string> = {
    trigger: "#38bdf8",
    agent: "#a78bfa",
    tool: "#f59e0b",
    condition: "#fb7185",
    model: "#22c55e",
    memory: "#06b6d4",
    input: "#60a5fa",
    output: "#34d399",
    integration: "#f97316",
  };

  return colors[type];
}
