import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { toolExecutionAction } from "../actions";
import { defineAgentTool } from "../define-tool";
import {
  runFormalCheck,
  runMoveBuild,
  runMoveTest,
  runMovyFuzzPackage,
  runStaticScan,
  toolSuccess,
} from "../executors";
import { projectPathSchema } from "../schemas";
import type { AgentToolRuntimeState } from "../types";

export function createValidationTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{ rootPath?: string; packagePath?: string }, unknown>({
      id: "rust.validation.run_suite",
      title: "Run validation suite",
      description:
        "Run build, tests, static analysis, and fuzzing for the active package in sequence.",
      inputSchema: projectPathSchema,
      action: toolExecutionAction("Run the standard Peregrine validation suite.", "medium"),
      execute: async (input) => {
        const build = await runMoveBuild(state, input);
        const tests = await runMoveTest(state, input);
        const analysis = await runStaticScan(state, input);
        const fuzz = await runMovyFuzzPackage(state, input);

        return toolSuccess(
          {
            build: build.output,
            tests: tests.output,
            analysis: analysis.report,
            fuzz: fuzz.output,
          },
          [
            build.summary,
            tests.summary,
            analysis.summary,
            fuzz.summary,
          ].join(" "),
        );
      },
    }),
    defineAgentTool<{
      moduleName: string;
      filePath?: string;
      rootPath?: string;
      packagePath?: string;
      timeoutSeconds?: number;
    }, unknown>({
      id: "rust.validation.assert_property",
      title: "Run formal verification",
      description: "Run bundled Sui Prover against a module in the active package.",
      inputSchema: {
        type: "object",
        properties: {
          moduleName: { type: "string", description: "Move module name." },
          filePath: { type: "string" },
          rootPath: { type: "string" },
          packagePath: { type: "string" },
          timeoutSeconds: { type: "integer", minimum: 5, maximum: 300 },
        },
        required: ["moduleName"],
        additionalProperties: false,
      },
      action: toolExecutionAction("Run formal verification for a Move module.", "medium"),
      execute: async (input) => {
        const result = await runFormalCheck(state, input);

        return toolSuccess(result.output, result.summary, [
          {
            kind: "proverResult",
            source: "rust.validation.assert_property",
            summary: result.summary,
            raw: result.output,
          },
        ]);
      },
    }),
  ];
}
