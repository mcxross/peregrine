import { describe, expect, test } from "bun:test";
import type { LanguageModel } from "ai";

import {
  AuditWorkflowRunner,
  InMemoryEvidenceStore,
  InMemorySessionStore,
  buildAttackHypothesisSet,
  buildAuditKnowledgeGraph,
  buildAuditReport,
  buildCanonicalProjectIndex,
  buildBytecodeReviewPacket,
  buildConfirmedFindingsSet,
  buildContractClassification,
  buildDynamicEvidencePacket,
  buildFunctionRiskMap,
  buildGraphEvidencePacket,
  buildInvariantRegistry,
  buildInvariantStressReport,
  buildRegressionTestPacket,
  buildRemediationPlan,
  buildSeverityRankedFindingList,
  buildStaticFindingsSet,
  buildTestPlanPacket,
  buildThreatModel,
  createAuditSessionPacket,
  PeregrineHarness,
} from "../src";

describe("audit workflow", () => {
  test("blocks analysis before immutable audit session packet exists", async () => {
    const sessionStore = new InMemorySessionStore();
    const evidenceStore = new InMemoryEvidenceStore();
    const runner = new AuditWorkflowRunner({ evidenceStore, sessionStore });
    const session = sessionStore.create({
      projectPath: "/tmp/demo",
      profile: { id: "full", title: "Full audit" },
    });

    await expect(runner.recordPacket({
      sessionId: session.id,
      stageId: "buildNormalize",
      artifactName: "projectIndex",
      packet: {} as never,
      summary: "should fail",
    })).rejects.toThrow("before an audit session");
  });

  test("rejects replacing the audit session packet", async () => {
    const sessionStore = new InMemorySessionStore();
    const evidenceStore = new InMemoryEvidenceStore();
    const runner = new AuditWorkflowRunner({ evidenceStore, sessionStore });
    const session = sessionStore.create({
      projectPath: "/tmp/demo",
      profile: { id: "full", title: "Full audit" },
    });
    const first = createSession("abc");
    const second = createSession("def");

    await runner.recordPacket({
      sessionId: session.id,
      stageId: "auditSession",
      artifactName: "auditSession",
      packet: first,
      summary: "created",
    });

    await expect(runner.recordPacket({
      sessionId: session.id,
      stageId: "auditSession",
      artifactName: "auditSession",
      packet: second,
      summary: "replace",
    })).rejects.toThrow("immutable");
  });

  test("builds full audit packets with hypotheses, tests, triage, report, and trace", () => {
    const auditSession = createSession("abc");
    const projectIndex = buildCanonicalProjectIndex({
      auditSession,
      movePackage: fixtureMovePackage(),
      buildOutput: { status: 0, stdout: "ok", stderr: "" },
      indexReport: {
        runId: "run_1",
        packageId: "pkg_1",
        dbPath: "/tmp/index.sqlite",
        status: "Indexed",
      },
      bytecodeView: fixtureBytecodeView(),
    });
    const knowledgeGraph = buildAuditKnowledgeGraph({
      auditSession,
      movePackage: fixtureMovePackage(),
      graphs: {
        callGraph: { nodes: [], edges: [] },
        typeGraph: { nodes: [], edges: [] },
        stateAccessGraph: { nodes: [], edges: [] },
      },
    }, projectIndex);
    const classification = buildContractClassification(auditSession, projectIndex, knowledgeGraph);
    const threatModel = buildThreatModel(auditSession, projectIndex, knowledgeGraph, classification);
    const riskMap = buildFunctionRiskMap(auditSession, projectIndex, knowledgeGraph);
    const invariants = buildInvariantRegistry(auditSession, threatModel, riskMap);
    const staticFindings = buildStaticFindingsSet(auditSession, {
      findings: [
        {
          ruleId: "unchecked_return",
          rulesetId: "unchecked_return",
          severity: "warning",
          message: "ignored return",
          file: "sources/vault.move",
        },
      ],
      diagnostics: [],
    }, riskMap);
    const graphEvidence = buildGraphEvidencePacket(auditSession, knowledgeGraph);
    const bytecodeReview = buildBytecodeReviewPacket(auditSession, fixtureBytecodeView());
    const hypotheses = buildAttackHypothesisSet({
      auditSession,
      riskMap,
      invariants,
      staticFindings,
      graphEvidence,
      bytecodeReview,
    });
    const testPlan = buildTestPlanPacket(auditSession, hypotheses, invariants);
    const dynamicEvidence = buildDynamicEvidencePacket(auditSession, testPlan, {
      status: 1,
      stdout: "",
      stderr: "withdraw unauthorized failed for vault::withdraw",
    });
    const stress = buildInvariantStressReport(auditSession, invariants, {
      status: null,
      stdout: "",
      stderr: "movy unavailable",
    });
    const confirmed = buildConfirmedFindingsSet({
      auditSession,
      hypotheses,
      staticFindings,
      graphEvidence,
      dynamicEvidence,
      invariantStress: stress,
    });
    const ranked = buildSeverityRankedFindingList(auditSession, confirmed);
    const remediation = buildRemediationPlan(auditSession, ranked);
    const regression = buildRegressionTestPacket(auditSession, ranked, testPlan);
    const report = buildAuditReport({
      auditSession,
      projectIndex,
      classification,
      threatModel,
      rankedFindings: ranked,
      invariants,
      testPlan,
      dynamicEvidence,
      remediationPlan: remediation,
    });

    expect(projectIndex.functions.map((fn) => fn.qualifiedName)).toContain("vault::withdraw");
    expect(classification.profiles).toContain("vault");
    expect(["critical", "high"]).toContain(riskMap.functions[0].risk);
    expect(invariants.invariants.length).toBeGreaterThan(0);
    expect(hypotheses.hypotheses.length).toBeGreaterThan(0);
    expect(testPlan.tests.length).toBeGreaterThan(0);
    expect(ranked.findings.length).toBeGreaterThan(0);
    expect(remediation.remediations.length).toBeGreaterThan(0);
    expect(regression.tests.length).toBeGreaterThan(0);
    expect(report.markdown).toContain("Peregrine Audit Report");
  });

  test("harness sessions carry audit state fields", () => {
    const harness = new PeregrineHarness({ model: {} as LanguageModel });
    const session = harness.createSession({
      projectPath: "/tmp/demo",
      profile: { id: "full", title: "Full audit" },
    });

    expect(session.auditStageRuns).toEqual([]);
    expect(session.auditPackets).toEqual({});
    expect(session.fixVerificationHistory).toEqual([]);
  });
});

function createSession(commit: string) {
  return createAuditSessionPacket({
    project: "demo",
    repoRoot: "/tmp/demo",
    commit,
    packageManifest: "Move.toml",
    targetModules: ["vault"],
    timestamp: "2026-05-24T00:00:00Z",
  });
}

function fixtureMovePackage() {
  return {
    name: "demo_vault",
    path: ".",
    manifestPath: "Move.toml",
    sourceFileCount: 1,
    modules: [
      {
        name: "vault",
        filePath: "sources/vault.move",
        attributes: [],
        structs: [
          {
            name: "Vault",
            abilities: ["key", "store"],
            fields: [
              { name: "id", typeName: "UID" },
              { name: "balance", typeName: "Balance<SUI>" },
            ],
          },
          {
            name: "AdminCap",
            abilities: ["key", "store"],
            fields: [{ name: "id", typeName: "UID" }],
          },
        ],
        functions: [
          {
            name: "deposit",
            visibility: "public",
            isEntry: true,
            isTransactionCallable: true,
            signature: "public entry fun deposit(vault: &mut Vault, coin: Coin<SUI>)",
            attributes: [],
          },
          {
            name: "withdraw",
            visibility: "public",
            isEntry: true,
            isTransactionCallable: true,
            signature: "public entry fun withdraw(vault: &mut Vault, amount: u64, ctx: &mut TxContext)",
            attributes: [],
          },
          {
            name: "set_fee",
            visibility: "public",
            isEntry: true,
            isTransactionCallable: true,
            signature: "public entry fun set_fee(config: &mut Config, fee_bps: u64)",
            attributes: [],
          },
        ],
      },
    ],
    surface: {
      capabilityFindings: [{ qualifiedName: "vault::AdminCap", protectedFunctions: ["vault::set_fee"] }],
      objectLifecycleMaps: [
        {
          qualifiedName: "vault::Vault",
          stages: [{ kind: "mutated" }],
          touchedBy: [
            {
              qualifiedName: "vault::withdraw",
              isTransactionCallable: true,
            },
          ],
        },
      ],
      externalCallFindings: [],
      adminControlFindings: [],
      entryFunctionCount: 3,
      capabilityCount: 1,
      sharedObjectCount: 1,
    },
  };
}

function fixtureBytecodeView() {
  return {
    moduleCount: 1,
    functionCount: 1,
    instructionCount: 2,
    modules: [
      {
        name: "vault",
        functions: [
          {
            name: "withdraw",
            instructionCount: 2,
            instructions: [
              { opcode: "Call", detail: "transfer::public_transfer", source: { startByte: 1 } },
              { opcode: "Abort", detail: "abort 0" },
            ],
          },
        ],
      },
    ],
  };
}
