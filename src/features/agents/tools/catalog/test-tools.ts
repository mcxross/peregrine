import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { generatedFileAction } from "@/features/agents/tools/actions";
import { defineAgentTool } from "@/features/agents/tools/define-tool";
import { resolveActiveMovePackage, toolSuccess } from "@/features/agents/tools/executors";
import type { AgentFindingSeverity } from "@/features/agents/tools/types";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";

export function createTestTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{
      title: string;
      severity: AgentFindingSeverity;
      message: string;
      moduleName: string;
      functionName?: string;
    }, unknown>({
      id: "rust.test.generate_case",
      title: "Generate regression test case",
      description:
        "Generate a structured Move test-case draft for a finding without writing project files.",
      inputSchema: {
        type: "object",
        properties: {
          title: { type: "string" },
          severity: {
            type: "string",
            enum: ["critical", "high", "medium", "low", "info"],
          },
          message: { type: "string" },
          moduleName: { type: "string" },
          functionName: { type: "string" },
        },
        required: ["title", "severity", "message", "moduleName"],
        additionalProperties: false,
      },
      action: generatedFileAction("Generate a regression test-case draft."),
      execute: async (input) => {
        const { movePackage } = await resolveActiveMovePackage(state.context);
        const module =
          movePackage.modules.find((candidate) => candidate.name === input.moduleName)
          ?? movePackage.modules[0];

        const testCase = {
          moduleName: module?.name ?? input.moduleName,
          functionName: input.functionName ?? null,
          title: input.title,
          severity: input.severity,
          expectation: input.message,
          template: [
            "#[test_only]",
            "fun regression_case() {",
            "    // TODO: reproduce the reported condition with deterministic setup.",
            "    abort 0",
            "}",
          ].join("\n"),
        };

        return toolSuccess(testCase, `Generated regression test draft for ${testCase.moduleName}.`);
      },
    }),
  ];
}
