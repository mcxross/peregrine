import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { generatedFileAction, readOnlyAction } from "@/features/agents/tools/actions";
import { defineAgentTool } from "@/features/agents/tools/define-tool";
import {
  resolveActiveMovePackage,
  toolFailure,
  toolSuccess,
} from "@/features/agents/tools/executors";
import type { AgentFindingSeverity, AgentToolRuntimeState } from "@/features/agents/tools/types";
import { loadFilePreview } from "@/features/empty-project/filesystem-tree";

export function createPatchTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{
      title: string;
      severity: AgentFindingSeverity;
      message: string;
      filePath?: string;
      proposedChange: string;
    }, unknown>({
      id: "rust.patch.suggest",
      title: "Suggest patch plan",
      description:
        "Create a structured patch proposal for a confirmed finding without modifying source files.",
      inputSchema: {
        type: "object",
        properties: {
          title: { type: "string" },
          severity: {
            type: "string",
            enum: ["critical", "high", "medium", "low", "info"],
          },
          message: { type: "string" },
          filePath: { type: "string" },
          proposedChange: { type: "string" },
        },
        required: ["title", "severity", "message", "proposedChange"],
        additionalProperties: false,
      },
      action: readOnlyAction("Suggest a minimal patch plan without applying changes."),
      execute: async (input) => {
        const patchId = `patch_${Date.now()}`;
        const proposal = {
          patchId,
          title: input.title,
          severity: input.severity,
          message: input.message,
          filePath: input.filePath ?? null,
          proposedChange: input.proposedChange,
          status: "draft",
        };

        return toolSuccess(proposal, `Prepared patch proposal ${patchId}.`);
      },
    }),
    defineAgentTool<{ filePath: string }, unknown>({
      id: "rust.patch.apply_preview",
      title: "Preview patch target",
      description: "Load the current source preview for a file targeted by a patch proposal.",
      inputSchema: {
        type: "object",
        properties: {
          filePath: { type: "string" },
        },
        required: ["filePath"],
        additionalProperties: false,
      },
      action: generatedFileAction("Preview a patch target file without writing changes."),
      execute: async (input) => {
        const { packageTree } = await resolveActiveMovePackage(state.context);
        const preview = await loadFilePreview(packageTree, input.filePath, {
          includeHighlightedHtml: false,
        });

        if (preview.kind === "unsupported") {
          return toolFailure(`File ${input.filePath} is not previewable in the active project.`);
        }

        return toolSuccess(
          preview,
          `Loaded preview for ${input.filePath}. No source changes were applied.`,
        );
      },
    }),
  ];
}
