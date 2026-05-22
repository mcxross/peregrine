import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { readOnlyAction, toolExecutionAction } from "@/features/agents/tools/actions";
import { defineAgentTool } from "@/features/agents/tools/define-tool";
import {
  requireIndexPackageId,
  resolveActiveMovePackage,
  runStaticScan,
  toolFailure,
  toolSuccess,
} from "@/features/agents/tools/executors";
import {
  functionTargetSchema,
  moduleTargetSchema,
  projectPathSchema,
} from "@/features/agents/tools/schemas";
import { mapAnalysisSeverity } from "@/features/agents/tools/severity";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";
import {
  defaultContextBudget,
  getFunctionBody,
  getFunctionContext,
  getPublicEntryFunctions,
} from "@/features/project-workspace/indexer/sui-indexer-client";

export function createStaticTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.static.scan_package",
      title: "Run static analysis",
      description:
        "Run Peregrine static analyzers against the active Move package and return findings and diagnostics.",
      inputSchema: projectPathSchema,
      action: toolExecutionAction("Run bundled static analyzers on the active package."),
      execute: async (input) => {
        const result = await runStaticScan(state, input);

        for (const finding of result.report.findings.slice(0, 12)) {
          state.session.emitFinding({
            title: finding.ruleId,
            severity: mapAnalysisSeverity(finding.severity),
            message: finding.message,
            location: finding.file,
            attachments: { source: "rust.static.scan_package", finding },
          });
        }

        return toolSuccess(result.report, result.summary, [
          {
            kind: "toolOutput",
            source: "rust.static.scan_package",
            summary: result.summary,
            raw: result.report,
          },
        ]);
      },
    }),
    defineAgentTool<{ functionId: string }, unknown>({
      id: "rust.static.inspect_function",
      title: "Inspect indexed function",
      description:
        "Load indexed function context and optional body excerpts for a specific function id.",
      inputSchema: functionTargetSchema,
      action: readOnlyAction("Inspect indexed function context and body excerpts."),
      execute: async (input) => {
        const context = await getFunctionContext(input.functionId, defaultContextBudget("Level2"));
        const body = await getFunctionBody(input.functionId, defaultContextBudget("Level2"));

        return toolSuccess(
          { context, body },
          `Loaded indexed context for function ${input.functionId}.`,
        );
      },
    }),
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.static.find_capabilities",
      title: "Find capability surfaces",
      description:
        "List public entry functions and capability-like object surfaces for the active package.",
      inputSchema: projectPathSchema,
      action: readOnlyAction("Read capability and entry-function surfaces from index and package scan."),
      execute: async (input) => {
        const { movePackage } = await resolveActiveMovePackage(state.context, input);
        const entries = state.indexPackageId
          ? await getPublicEntryFunctions(requireIndexPackageId(state))
          : [];

        return toolSuccess(
          {
            entryFunctions: entries,
            capabilityFindings: movePackage.surface.capabilityFindings,
            capabilityStructs: movePackage.surface.capabilityStructs,
            adminControlFindings: movePackage.surface.adminControlFindings,
          },
          `Found ${entries.length} entry functions and ${movePackage.surface.capabilityFindings.length} capability findings.`,
        );
      },
    }),
    defineAgentTool<{ moduleName: string; rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.static.list_modules",
      title: "List package modules",
      description: "List parseable Move modules and signatures from the active package tree.",
      inputSchema: moduleTargetSchema,
      action: readOnlyAction("Read module signatures from the loaded package tree."),
      execute: async (input) => {
        const { movePackage } = await resolveActiveMovePackage(state.context, input);
        const modules = input.moduleName
          ? movePackage.modules.filter((module) => module.name === input.moduleName)
          : movePackage.modules;

        if (!modules.length) {
          return toolFailure(`Module ${input.moduleName} was not found in the active package.`);
        }

        return toolSuccess(
          modules,
          `Loaded ${modules.length} module signature${modules.length === 1 ? "" : "s"}.`,
        );
      },
    }),
  ];
}
