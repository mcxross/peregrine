import { describe, expect, test } from "bun:test";
import type { DeterministicToolSpec, EvidenceCandidate } from "@peregrine/agent-runtime";

import {
  ContentAddressedEvidenceStore,
  compileToolEvidence,
  evaluateHarnessRuns,
  routeTools,
} from "../src";
import type { EvidencePersistenceAdapter, EvidenceRecord } from "../src";

describe("harness core", () => {
  test("routes staged tools and hides target-specific tools without a target", () => {
    const tools = [
      tool("rust.index.package", "index"),
      tool("rust.static.scan_package", "staticAnalysis"),
      tool("rust.graph.call_graph", "graph", ["functionId"]),
      tool("rust.report.generate", "report"),
    ];

    const plan = routeTools(tools, {
      role: "securityReview",
      activeToolIds: tools.map((item) => item.id),
    });

    expect(plan.tools.map((item) => item.id)).toEqual([
      "rust.index.package",
      "rust.static.scan_package",
    ]);
    expect(plan.capsules[0].whenNotToUse.length).toBeGreaterThan(0);
  });

  test("compiles static analysis into compact evidence and finding candidates", () => {
    const result = compileToolEvidence({
      tool: tool("rust.static.scan_package", "staticAnalysis"),
      status: "succeeded",
      summary: "Static analysis returned 1 finding.",
      toolRunId: "tool_run_1",
      output: {
        findings: [
          {
            ruleId: "unchecked_return",
            rulesetId: "unchecked_return",
            severity: "warning",
            message: "Return value is ignored.",
            file: "sources/vault.move",
            span: { startLine: 10, endLine: 11 },
          },
        ],
        diagnostics: [],
      },
    });

    expect(result.modelOutput.compact).toMatchObject({
      reducer: "staticAnalysis",
      findingCount: 1,
    });
    expect(result.evidence[0].kind).toBe("staticFinding");
    expect(result.findingCandidates[0].status).toBe("likely");
  });

  test("stores raw evidence by content hash", async () => {
    const persistence = new MemoryEvidencePersistence();
    const store = new ContentAddressedEvidenceStore(persistence);
    const record = await store.record({
      kind: "toolOutput",
      source: "test",
      summary: "stored",
      raw: { value: 1 },
    } satisfies EvidenceCandidate);

    expect(record.rawPath).toContain("peregrine-evidence://sha256/");
    expect(await store.rawContent(record.contentHash)).toBe('{"value":1}');
  });

  test("scores tool-assisted evaluation runs", () => {
    const metrics = evaluateHarnessRuns(
      [
        {
          id: "access-control-vulnerable",
          vulnerabilityClass: "accessControl",
          expectedFindings: ["access"],
        },
      ],
      [
        {
          caseId: "access-control-vulnerable",
          mode: "toolAssisted",
          findings: [
            {
              id: "finding_1",
              title: "Access control missing",
              category: "accessControl",
              severity: "high",
              confidence: "confirmed",
              status: "confirmed",
              affectedSymbols: ["vault::withdraw"],
              evidenceRefs: ["evidence_1"],
              validationPlan: {
                commands: ["sui move test"],
                expectedEvidence: ["test fails before patch"],
                required: true,
              },
            },
          ],
          toolRuns: [
            {
              id: "run_1",
              toolId: "rust.static.scan_package",
              status: "succeeded",
              summary: "ok",
              evidenceRefs: [],
            },
          ],
        },
      ],
    );

    expect(metrics.recall).toBe(1);
    expect(metrics.evidenceBackedFindingRate).toBe(1);
    expect(metrics.averageToolCalls).toBe(1);
  });
});

function tool(
  id: string,
  category: string,
  required: string[] = [],
): DeterministicToolSpec {
  return {
    id,
    description: id,
    inputSchema: {
      type: "object",
      properties: Object.fromEntries(required.map((key) => [key, { type: "string" }])),
      required,
    },
    manifest: {
      id,
      version: "1",
      category,
      description: id,
      whenToUse: [`Use ${id}.`],
      whenNotToUse: ["Skip when fresh evidence exists."],
      prerequisites: [],
      inputSchema: {
        type: "object",
        properties: Object.fromEntries(required.map((key) => [key, { type: "string" }])),
        required,
      },
      cost: {
        risk: "low",
        outputBudgetTokens: 500,
      },
      actionClass: "readOnly",
      sideEffects: [],
      reducerId: category === "staticAnalysis" ? "staticAnalysis" : "generic",
    },
    action: {
      actionClass: "readOnly",
      reason: id,
      risk: "low",
    },
    execute: () => ({}),
  };
}

class MemoryEvidencePersistence implements EvidencePersistenceAdapter {
  records: EvidenceRecord[] = [];
  blobs = new Map<string, string>();

  async readRecords() {
    return this.records;
  }

  async writeRecords(records: EvidenceRecord[]) {
    this.records = records;
  }

  async readBlob(contentHash: string) {
    return this.blobs.get(contentHash);
  }

  async writeBlob(contentHash: string, value: string) {
    this.blobs.set(contentHash, value);
  }
}
