import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { readOnlyAction, toolExecutionAction } from "../actions";
import { defineAgentTool } from "../define-tool";
import { toolFailure, toolSuccess } from "../executors";
import {
  contextLookupSchema,
  projectPathSchema,
  symbolQuerySchema,
} from "../schemas";
import type { AgentToolRuntimeState } from "../types";
import {
  createIndexRunId,
  defaultContextBudget,
  getContextPack,
  getPackageOverview,
  indexPackage,
  searchIndexedSymbols,
  type ContextLevel,
  type IndexReport,
} from "../../../indexer/sui-indexer-client";

export function createIndexTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{ rootPath?: string }, IndexReport>({
      id: "rust.index.package",
      title: "Index Move package",
      description:
        "Build the Peregrine SQLite index for the active Move package, including summaries, operations, and graph facts.",
      inputSchema: projectPathSchema,
      action: toolExecutionAction("Index the active Move package for agent queries.", "medium"),
      execute: async (input) => {
        const rootPath = input?.rootPath?.trim() || state.context.rootPath;
        const report = await indexPackage(rootPath, createIndexRunId());
        state.indexPackageId = report.packageId;

        return toolSuccess(
          report,
          `Indexed ${report.packageName} with ${report.functionCount} functions.`,
          [
            {
              kind: "toolOutput",
              source: "rust.index.package",
              summary: `Indexed package ${report.packageId}.`,
              raw: report,
            },
          ],
        );
      },
    }),
    defineAgentTool<{ query: string }, unknown>({
      id: "rust.index.read_symbols",
      title: "Search indexed symbols",
      description: "Search the active package index for functions, modules, and types.",
      inputSchema: symbolQuerySchema,
      action: readOnlyAction("Read indexed symbol names for follow-up context lookups."),
      execute: async (input) => {
        const packageId = state.indexPackageId;

        if (!packageId) {
          return toolFailure(
            "No active package index. Run rust.index.package before rust.index.read_symbols.",
          );
        }

        const symbols = await searchIndexedSymbols(packageId, input.query);

        return toolSuccess(
          symbols,
          `Found ${symbols.length} indexed symbols for "${input.query}".`,
        );
      },
    }),
    defineAgentTool<{ targetId: string; level?: ContextLevel }, unknown>({
      id: "index.context.lookup",
      title: "Load indexed context pack",
      description:
        "Materialize a bounded Peregrine context pack for an indexed function, module, or type target.",
      inputSchema: contextLookupSchema,
      action: readOnlyAction("Read bounded indexed context for the selected target."),
      execute: async (input) => {
        const budget = defaultContextBudget(input.level ?? "Level1");
        const pack = await getContextPack(input.targetId, budget);

        return toolSuccess(pack, `Loaded context pack for ${input.targetId}.`);
      },
    }),
    defineAgentTool<Record<string, never>, unknown>({
      id: "rust.index.package_overview",
      title: "Read package overview",
      description:
        "Read high-level indexed package metadata for package intent discovery and analysis planning.",
      inputSchema: {
        type: "object",
        properties: {},
        additionalProperties: false,
      },
      action: readOnlyAction("Read indexed package overview metadata."),
      execute: async () => {
        const packageId = state.indexPackageId;

        if (!packageId) {
          return toolFailure(
            "No active package index. Run rust.index.package before reading the overview.",
          );
        }

        const overview = await getPackageOverview(packageId);

        return toolSuccess(
          overview,
          `Package ${overview.name} has ${overview.functions} functions and ${overview.modules} modules.`,
        );
      },
    }),
  ];
}
