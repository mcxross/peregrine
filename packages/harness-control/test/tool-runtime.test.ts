import { describe, expect, test } from "bun:test";

import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import {
  DefaultApprovalPolicy,
  DenyByDefaultApprovalGate,
  HarnessToolRuntime,
  InMemoryEvidenceStore,
} from "../src";

describe("harness tool runtime", () => {
  test("executes allowed deterministic tools and records evidence", async () => {
    const evidenceStore = new InMemoryEvidenceStore();
    const runtime = new HarnessToolRuntime({
      policy: new DefaultApprovalPolicy(),
      approvalGate: new DenyByDefaultApprovalGate(),
      evidenceStore,
    });
    const tool: DeterministicToolSpec<{ target: string }, { ok: true }> = {
      id: "index.context.lookup",
      description: "Look up bounded context.",
      inputSchema: {
        type: "object",
        properties: {
          target: { type: "string" },
        },
        required: ["target"],
      },
      action: {
        actionClass: "toolExecution",
        reason: "Read indexed context.",
        risk: "low",
      },
      execute: async () => ({
        output: { ok: true },
        summary: "Context returned.",
      }),
    };
    const result = await runtime.runTool({
      tool,
      input: { target: "module::entry" },
      toolCallId: "call_1",
      context: {
        taskId: "task_1",
      },
    });

    expect(result.status).toBe("succeeded");
    expect(result.evidenceRefs).toHaveLength(1);
    expect(evidenceStore.list()[0].kind).toBe("toolOutput");
  });

  test("denies approval-required actions when no approval gate is connected", async () => {
    const evidenceStore = new InMemoryEvidenceStore();
    const runtime = new HarnessToolRuntime({
      policy: new DefaultApprovalPolicy(),
      approvalGate: new DenyByDefaultApprovalGate(),
      evidenceStore,
    });
    const tool: DeterministicToolSpec<{ path: string }, { written: true }> = {
      id: "draft.write",
      description: "Write a generated draft.",
      inputSchema: {
        type: "object",
        properties: {
          path: { type: "string" },
        },
        required: ["path"],
      },
      action: {
        actionClass: "generatedFileWrite",
        reason: "Create a draft artifact.",
        risk: "medium",
        files: ["draft.md"],
      },
      execute: async () => {
        throw new Error("Approval gate should run before execution.");
      },
    };
    const result = await runtime.runTool({
      tool,
      input: { path: "draft.md" },
      toolCallId: "call_2",
      context: {
        taskId: "task_1",
      },
    });

    expect(result.status).toBe("denied");
    expect(result.evidenceRefs[0].kind).toBe("humanApproval");
    expect(evidenceStore.list()).toHaveLength(1);
  });
});

