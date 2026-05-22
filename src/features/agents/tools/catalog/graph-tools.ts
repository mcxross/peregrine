import type { DeterministicToolSpec, JsonSchemaDefinition } from "@peregrine/agent-runtime";

import { readOnlyAction } from "@/features/agents/tools/actions";
import { defineAgentTool } from "@/features/agents/tools/define-tool";
import {
  loadProjectGraphs,
  loadStateAccessGraph,
  requireIndexPackageId,
  resolveActiveMovePackage,
  toolSuccess,
} from "@/features/agents/tools/executors";
import { projectPathProperties, projectPathSchema } from "@/features/agents/tools/schemas";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";
import {
  defaultContextBudget,
  getCallGraph,
  getReachableCallees,
} from "@/features/project-workspace/indexer/sui-indexer-client";

const graphDepthSchema: JsonSchemaDefinition = {
  type: "object",
  properties: {
    functionId: {
      type: "string",
      description: "Indexed function id from rust.index.read_symbols or search results.",
    },
    depth: {
      type: "integer",
      minimum: 1,
      maximum: 6,
      description: "Traversal depth for graph expansion.",
    },
    rootPath: projectPathProperties.rootPath,
    packagePath: projectPathProperties.packagePath,
  },
  required: ["functionId"],
  additionalProperties: false,
};

const stateGraphSchema: JsonSchemaDefinition = {
  type: "object",
  properties: {
    moduleName: {
      type: "string",
      description: "Move module name.",
    },
    functionName: {
      type: "string",
      description: "Function name inside the module.",
    },
    moduleAddress: { type: "string" },
    rootPath: projectPathProperties.rootPath,
    packagePath: projectPathProperties.packagePath,
  },
  required: ["moduleName", "functionName"],
  additionalProperties: false,
};

export function createGraphTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.graph.call_graph.read",
      title: "Read package call graph",
      description: "Load the project call graph for the active Move package.",
      inputSchema: projectPathSchema,
      action: readOnlyAction("Read the package call graph."),
      execute: async (input) => {
        const result = await loadProjectGraphs(state, input);

        return toolSuccess(result.graphs.callGraph, result.summary);
      },
    }),
    defineAgentTool<{ functionId: string; depth?: number }, unknown>({
      id: "rust.graph.call_graph",
      title: "Expand indexed call graph",
      description: "Expand an indexed function call graph to a bounded depth.",
      inputSchema: graphDepthSchema,
      action: readOnlyAction("Read an indexed function call graph."),
      execute: async (input) => {
        const graph = await getCallGraph(
          input.functionId,
          input.depth ?? 2,
          defaultContextBudget("Level3"),
        );

        return toolSuccess(
          graph,
          `Expanded call graph for ${input.functionId} with ${graph.nodes.length} nodes.`,
        );
      },
    }),
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.graph.object_lifecycle",
      title: "Read object lifecycle maps",
      description:
        "Read object lifecycle, ownership, and capability surfaces discovered during package load.",
      inputSchema: projectPathSchema,
      action: readOnlyAction("Read object lifecycle maps from the loaded package surface."),
      execute: async (input) => {
        const { movePackage } = await resolveActiveMovePackage(state.context, input);

        return toolSuccess(
          {
            objectLifecycleMaps: movePackage.surface.objectLifecycleMaps,
            objectOwnershipFindings: movePackage.surface.objectOwnershipFindings,
            sharedObjectStructs: movePackage.surface.sharedObjectStructs,
          },
          `Loaded ${movePackage.surface.objectLifecycleMaps.length} object lifecycle maps.`,
        );
      },
    }),
    defineAgentTool<{
      moduleName: string;
      functionName: string;
      moduleAddress?: string;
      rootPath?: string;
      packagePath?: string;
    }, unknown>({
      id: "rust.graph.cfg",
      title: "Read state access graph",
      description:
        "Load a function-level state access graph for control-flow and storage-touch reasoning.",
      inputSchema: stateGraphSchema,
      action: readOnlyAction("Read a function state access graph."),
      execute: async (input) => {
        const result = await loadStateAccessGraph(state, input);

        return toolSuccess(result.graph, result.summary);
      },
    }),
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.graph.capability_flow",
      title: "Read capability flow",
      description: "Read capability findings and external call relationships for the active package.",
      inputSchema: projectPathSchema,
      action: readOnlyAction("Read capability flow evidence from the package surface."),
      execute: async (input) => {
        const { movePackage } = await resolveActiveMovePackage(state.context, input);

        return toolSuccess(
          {
            capabilityFindings: movePackage.surface.capabilityFindings,
            externalCallFindings: movePackage.surface.externalCallFindings,
            publicPackageRelationships: movePackage.surface.publicPackageRelationships,
          },
          `Loaded ${movePackage.surface.capabilityFindings.length} capability findings.`,
        );
      },
    }),
    defineAgentTool<{ functionId: string; depth?: number }, unknown>({
      id: "rust.graph.finding_impact",
      title: "Expand reachable callees",
      description: "Expand reachable callees from an indexed function to estimate blast radius.",
      inputSchema: graphDepthSchema,
      action: readOnlyAction("Read reachable callee expansion for impact analysis."),
      execute: async (input) => {
        const callees = await getReachableCallees(
          input.functionId,
          input.depth ?? 3,
          defaultContextBudget("Level3"),
        );

        return toolSuccess(
          callees,
          `Expanded ${callees.length} reachable callees from ${input.functionId}.`,
        );
      },
    }),
    defineAgentTool<{ functionId: string; depth?: number }, unknown>({
      id: "rust.graph.path_query",
      title: "Query indexed call graph",
      description: "Alias for bounded indexed call-graph expansion used during path queries.",
      inputSchema: graphDepthSchema,
      action: readOnlyAction("Query indexed call graph paths."),
      execute: async (input) => {
        requireIndexPackageId(state);
        const graph = await getCallGraph(
          input.functionId,
          input.depth ?? 2,
          defaultContextBudget("Level3"),
        );

        return toolSuccess(graph, `Resolved call-graph path query for ${input.functionId}.`);
      },
    }),
  ];
}
