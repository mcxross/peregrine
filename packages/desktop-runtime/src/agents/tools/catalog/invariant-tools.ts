import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { readOnlyAction } from "../actions";
import { defineAgentTool } from "../define-tool";
import { requireIndexPackageId, toolSuccess } from "../executors";
import type { AgentToolRuntimeState } from "../types";
import {
  defaultContextBudget,
  getFunctionsByTag,
  getOperationsByTag,
} from "../../../indexer/sui-indexer-client";

export function createInvariantTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{ tag: string }, unknown>({
      id: "rust.invariant.infer",
      title: "Infer invariant candidates",
      description:
        "Infer candidate invariants from indexed semantic tags and storage-touch operations.",
      inputSchema: {
        type: "object",
        properties: {
          tag: {
            type: "string",
            description: "Semantic tag to inspect, such as storage_write or abort.",
          },
        },
        required: ["tag"],
        additionalProperties: false,
      },
      action: readOnlyAction("Infer invariant candidates from indexed semantic evidence."),
      execute: async (input) => {
        const packageId = requireIndexPackageId(state);
        const operations = await getOperationsByTag(
          packageId,
          input.tag,
          defaultContextBudget("Level2"),
        );

        return toolSuccess(
          {
            tag: input.tag,
            operations,
            hypotheses: operations.slice(0, 12).map((operation) => ({
              statement: `Operation tagged ${input.tag} should preserve package safety assumptions.`,
              evidence: operation,
            })),
          },
          `Inferred ${operations.length} invariant candidates from tag ${input.tag}.`,
        );
      },
    }),
    defineAgentTool<{ tag: string }, unknown>({
      id: "rust.invariant.check",
      title: "Check invariant coverage",
      description: "List functions tagged with a semantic invariant marker from the index.",
      inputSchema: {
        type: "object",
        properties: {
          tag: { type: "string" },
        },
        required: ["tag"],
        additionalProperties: false,
      },
      action: readOnlyAction("Check indexed functions associated with an invariant tag."),
      execute: async (input) => {
        const packageId = requireIndexPackageId(state);
        const functions = await getFunctionsByTag(packageId, input.tag, defaultContextBudget());

        return toolSuccess(
          functions,
          `Found ${functions.length} functions tagged with ${input.tag}.`,
        );
      },
    }),
  ];
}
