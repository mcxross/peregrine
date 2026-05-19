import { describe, expect, test } from "bun:test";

import type { DeterministicToolSpec } from "@peregrine/agent-runtime";
import type { LanguageModel } from "ai";

import { PeregrineHarness } from "../src";

describe("peregrine harness", () => {
  test("records direct tool runs on the assessment session", async () => {
    const harness = new PeregrineHarness({
      model: {} as LanguageModel,
    });
    const session = harness.createSession({
      projectPath: "/tmp/project",
      profile: {
        id: "generic-smart-contract",
        title: "Generic smart contract",
      },
    });
    const tool: DeterministicToolSpec<{ query: string }, { found: true }> = {
      id: "index.lookup",
      description: "Read indexed context.",
      inputSchema: {
        type: "object",
        properties: {
          query: { type: "string" },
        },
        required: ["query"],
      },
      action: {
        actionClass: "toolExecution",
        reason: "Lookup indexed context.",
        risk: "low",
      },
      execute: async () => ({
        output: { found: true },
        summary: "Lookup completed.",
      }),
    };

    harness.registerTool(tool);

    const result = await harness.runTool({
      sessionId: session.id,
      taskId: "task_1",
      toolId: "index.lookup",
      input: { query: "module::entry" },
    });
    const updated = harness.getSession(session.id);

    expect(result.status).toBe("succeeded");
    expect(updated?.toolRuns).toHaveLength(1);
    expect(updated?.toolRuns[0].toolId).toBe("index.lookup");
    expect(updated?.evidenceRefs).toHaveLength(1);
  });
});

