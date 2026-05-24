import { describe, expect, test } from "bun:test";

import {
  buildAgentPrompt,
  createAiSdkToolName,
  createAiSdkToolSet,
  type AgentContextPacket,
  type DeterministicToolSpec,
  type ToolGateway,
} from "../src";

describe("agent runtime tools", () => {
  test("normalizes deterministic tool ids into AI SDK tool names", () => {
    expect(createAiSdkToolName("sui.move.build")).toBe("sui_move_build");
    expect(createAiSdkToolName("123.analyzer")).toBe("tool_123_analyzer");
  });

  test("prompts include callable tool names separately from Peregrine ids", () => {
    const packet: AgentContextPacket = {
      task: {
        id: "task_1",
        role: "securityReview",
        title: "Audit",
        objective: "Run tools.",
      },
      developerIntent: "Use tools.",
      projectSummary: {
        id: "project_1",
        name: "demo",
        rootPath: "/tmp/demo",
        chain: "sui",
        modules: [],
      },
      securityProfile: "test",
      selectedCode: [],
      riskSignals: [],
      relevantGuides: [],
      currentFindings: [],
      recentToolResults: [],
      toolCapsules: [
        {
          callableName: "rust_static_scan_package",
          id: "rust.static.scan_package",
          description: "Run static analysis.",
          category: "staticAnalysis",
          actionClass: "toolExecution",
          risk: "low",
          whenToUse: ["Need source diagnostics."],
          whenNotToUse: ["Fresh evidence exists."],
          prerequisites: [],
          inputSchema: { type: "object" },
        },
      ],
      allowedActions: [],
      approvalPolicy: {
        mode: "localAi",
        networkAccess: "approvalRequired",
        sourceModification: "approvalRequired",
        dependencyModification: "approvalRequired",
        secretAccess: "forbidden",
      },
      outputContract: {
        format: "markdown",
        requiredEvidence: true,
        description: "Evidence-backed report.",
      },
    };
    const prompt = buildAgentPrompt(packet);

    expect(prompt).toContain("rust_static_scan_package");
    expect(prompt).toContain("Peregrine ID: rust.static.scan_package");
    expect(prompt).toContain("Use the callable name exactly");
    expect(prompt).toContain("Package Intent");
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
