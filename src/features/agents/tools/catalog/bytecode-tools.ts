import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { readOnlyAction } from "@/features/agents/tools/actions";
import { defineAgentTool } from "@/features/agents/tools/define-tool";
import { loadBytecodePackage, toolFailure, toolSuccess } from "@/features/agents/tools/executors";
import { moduleTargetSchema } from "@/features/agents/tools/schemas";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";

type ModuleTargetInput = {
  moduleName: string;
  functionName?: string;
  rootPath?: string;
  packagePath?: string;
};

export function createBytecodeTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<ModuleTargetInput, unknown>({
      id: "rust.bytecode.disassemble",
      title: "Disassemble package bytecode",
      description: "Load the compiled Move bytecode view for the active package.",
      inputSchema: moduleTargetSchema,
      action: readOnlyAction("Read compiled Move bytecode and disassembly."),
      execute: async (input) => {
        const result = await loadBytecodePackage(state, input);
        const modules = input.moduleName
          ? result.view.modules.filter((module) => module.name === input.moduleName)
          : result.view.modules;

        return toolSuccess(
          {
            package: {
              packageName: result.view.packageName,
              moduleCount: result.view.moduleCount,
              functionCount: result.view.functionCount,
            },
            modules,
          },
          result.summary,
        );
      },
    }),
    defineAgentTool<ModuleTargetInput, unknown>({
      id: "rust.bytecode.cfg",
      title: "Read bytecode control flow",
      description: "Read bytecode basic blocks and control-flow edges for one function.",
      inputSchema: moduleTargetSchema,
      action: readOnlyAction("Read bytecode control-flow graphs."),
      execute: async (input) => {
        const result = await loadBytecodePackage(state, input);
        const module = result.view.modules.find((candidate) => candidate.name === input.moduleName);

        if (!module) {
          return toolFailure(`Bytecode module ${input.moduleName} was not found.`);
        }

        const functions = input.functionName
          ? module.functions.filter((candidate) => candidate.name === input.functionName)
          : module.functions;

        return toolSuccess(
          functions.map((candidate) => ({
            moduleName: module.name,
            functionName: candidate.name,
            controlFlow: candidate.controlFlow,
            instructionCount: candidate.instructionCount,
          })),
          `Loaded bytecode CFG for ${functions.length} function(s) in ${module.name}.`,
        );
      },
    }),
    defineAgentTool<ModuleTargetInput, unknown>({
      id: "rust.bytecode.stack_effects",
      title: "Read bytecode stack effects",
      description: "Read instruction-level stack behavior for one bytecode function.",
      inputSchema: moduleTargetSchema,
      action: readOnlyAction("Read bytecode stack effects."),
      execute: async (input) => {
        const result = await loadBytecodePackage(state, input);
        const module = result.view.modules.find((candidate) => candidate.name === input.moduleName);

        if (!module) {
          return toolFailure(`Bytecode module ${input.moduleName} was not found.`);
        }

        const functions = input.functionName
          ? module.functions.filter((candidate) => candidate.name === input.functionName)
          : module.functions.slice(0, 3);

        return toolSuccess(
          functions.map((candidate) => ({
            moduleName: module.name,
            functionName: candidate.name,
            localCount: candidate.localCount,
            returnCount: candidate.returnCount,
            instructions: candidate.instructions,
          })),
          `Loaded stack effects for ${functions.length} bytecode function(s).`,
        );
      },
    }),
    defineAgentTool<ModuleTargetInput, unknown>({
      id: "rust.bytecode.source_map",
      title: "Read bytecode source map",
      description: "Read source-map spans attached to bytecode instructions.",
      inputSchema: moduleTargetSchema,
      action: readOnlyAction("Read bytecode source-map spans."),
      execute: async (input) => {
        const result = await loadBytecodePackage(state, input);
        const module = result.view.modules.find((candidate) => candidate.name === input.moduleName);

        if (!module) {
          return toolFailure(`Bytecode module ${input.moduleName} was not found.`);
        }

        const mappedInstructions = module.functions.flatMap((candidate) =>
          candidate.instructions
            .filter((instruction) => instruction.source)
            .map((instruction) => ({
              functionName: candidate.name,
              offset: instruction.offset,
              opcode: instruction.opcode,
              source: instruction.source,
            })),
        );

        return toolSuccess(
          {
            moduleName: module.name,
            sourceMapPath: module.sourceMapPath,
            sourcePath: module.sourcePath,
            mappedInstructions,
          },
          `Loaded ${mappedInstructions.length} mapped bytecode instructions.`,
        );
      },
    }),
  ];
}
