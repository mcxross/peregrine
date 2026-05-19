import { describe, expect, test } from "bun:test";

import {
  createAiSdkToolName,
  createAiSdkToolSet,
  type DeterministicToolSpec,
  type ToolGateway,
} from "../src";

describe("agent runtime tools", () => {
  test("normalizes deterministic tool ids into AI SDK tool names", () => {
    expect(createAiSdkToolName("sui.move.build")).toBe("sui_move_build");
    expect(createAiSdkToolName("123.analyzer")).toBe("tool_123_analyzer");
  });

  test("routes AI SDK tool execution through the Peregrine tool gateway", async () => {
    const spec: DeterministicToolSpec<{ packagePath: string }, { ok: true }> = {
      id: "sui.move.build",
      description: "Build a package through Peregrine.",
      inputSchema: {
        type: "object",
        properties: {
          packagePath: { type: "string" },
        },
        required: ["packagePath"],
      },
      action: {
        actionClass: "toolExecution",
        reason: "Validate build status.",
        risk: "low",
      },
      execute: async () => ({ ok: true }),
    };
    const gateway: ToolGateway = {
      async runTool(request) {
        expect(request.tool.id).toBe("sui.move.build");
        expect(request.toolCallId).toBe("call_1");

        return {
          status: "succeeded",
          toolId: request.tool.id,
          toolCallId: request.toolCallId,
          action: request.tool.action,
          summary: "Build completed.",
          output: { ok: true },
          evidenceRefs: [],
          diagnostics: [],
        };
      },
    };
    const toolSet = createAiSdkToolSet({
      specs: [spec],
      gateway,
      context: {
        taskId: "task_1",
      },
    });
    const result = await toolSet.tools.sui_move_build.execute?.(
      { packagePath: "." },
      {
        toolCallId: "call_1",
        messages: [],
      },
    );

    expect(result?.status).toBe("succeeded");
  });
});

