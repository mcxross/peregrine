import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { readOnlyAction, toolExecutionAction } from "@/features/agents/tools/actions";
import { defineAgentTool } from "@/features/agents/tools/define-tool";
import {
  loadStateAccessGraph,
  runMoveTest,
  runMovyFuzzPackage,
  toolSuccess,
} from "@/features/agents/tools/executors";
import {
  functionTargetSchema,
  projectPathSchema,
} from "@/features/agents/tools/schemas";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";
import {
  defaultContextBudget,
  getFunctionOperations,
} from "@/features/project-workspace/indexer/sui-indexer-client";

type StateGraphInput = {
  moduleName: string;
  functionName: string;
  moduleAddress?: string;
  rootPath?: string;
  packagePath?: string;
};

export function createDynamicTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.dynamic.run_test",
      title: "Run Move tests",
      description: "Execute `sui move test` for the active package and return command output.",
      inputSchema: projectPathSchema,
      action: toolExecutionAction("Run Move tests for the active package."),
      execute: async (input) => {
        const result = await runMoveTest(state, input);

        return toolSuccess(result.output, result.summary, [
          {
            kind: "testResult",
            source: "rust.dynamic.run_test",
            summary: result.summary,
            raw: result.output,
          },
        ]);
      },
    }),
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.dynamic.fuzz_function",
      title: "Run Movy fuzzing",
      description: "Run Movy public-function fuzzing against the active package.",
      inputSchema: projectPathSchema,
      action: toolExecutionAction("Run Movy fuzzing for the active package.", "medium"),
      execute: async (input) => {
        const result = await runMovyFuzzPackage(state, input);

        return toolSuccess(result.output, result.summary, [
          {
            kind: "fuzzCounterexample",
            source: "rust.dynamic.fuzz_function",
            summary: result.summary,
            raw: result.output,
          },
        ]);
      },
    }),
    defineAgentTool<{ functionId: string }, unknown>({
      id: "rust.dynamic.trace_execution",
      title: "Trace indexed operations",
      description: "Read indexed operation traces for a function from the SQLite index.",
      inputSchema: functionTargetSchema,
      action: readOnlyAction("Read indexed operation traces for a function."),
      execute: async (input) => {
        const operations = await getFunctionOperations(
          input.functionId,
          defaultContextBudget("Level2"),
        );

        return toolSuccess(
          operations,
          `Loaded ${operations.length} indexed operations for ${input.functionId}.`,
        );
      },
    }),
    defineAgentTool<StateGraphInput, unknown>({
      id: "rust.dynamic.state_diff",
      title: "Compare state access paths",
      description:
        "Load a function state access graph to compare storage-touch paths across execution.",
      inputSchema: {
        type: "object",
        properties: {
          moduleName: { type: "string" },
          functionName: { type: "string" },
          moduleAddress: { type: "string" },
          rootPath: { type: "string" },
          packagePath: { type: "string" },
        },
        required: ["moduleName", "functionName"],
        additionalProperties: false,
      },
      action: readOnlyAction("Read state access evidence for dynamic comparison."),
      execute: async (input) => {
        const result = await loadStateAccessGraph(state, input);

        return toolSuccess(
          {
            unresolvedAccesses: result.graph.unresolvedAccesses,
            edges: result.graph.edges,
            nodes: result.graph.nodes,
          },
          result.summary,
        );
      },
    }),
  ];
}
