import type { DeterministicToolSpec, JsonSchemaDefinition } from "@peregrine/agent-runtime";
import {
  AUDIT_STAGE_SEQUENCE,
  buildAttackHypothesisSet,
  buildAuditKnowledgeGraph,
  buildAuditReport,
  buildAuditTrace,
  buildBytecodeReviewPacket,
  buildCanonicalProjectIndex,
  buildConfirmedFindingsSet,
  buildContractClassification,
  buildDynamicEvidencePacket,
  buildFixVerificationPacket,
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
  createId,
  type AuditPacketBundle,
  type AuditStageId,
  type AuditStageRun,
  type AuditTraceArtifactName,
} from "@peregrine/harness-control";

import { toolExecutionAction } from "../actions";
import { defineAgentTool } from "../define-tool";
import {
  loadBytecodePackage,
  loadProjectGraphs,
  resolveActiveMovePackage,
  runMoveBuild,
  runMoveTest,
  runMovyFuzzPackage,
  runStaticScan,
  toolFailure,
  toolSuccess,
} from "../executors";
import { projectPathProperties } from "../schemas";
import type { AgentToolRuntimeState } from "../types";
import {
  createIndexRunId,
  getPackageOverview,
  indexPackage,
} from "../../../indexer/sui-indexer-client";

const auditStageSchema: JsonSchemaDefinition = {
  type: "object",
  properties: {
    rootPath: projectPathProperties.rootPath,
    packagePath: projectPathProperties.packagePath,
    commit: {
      type: "string",
      description: "Optional git commit hash to record in the audit session packet.",
    },
    changedFiles: {
      type: "array",
      items: { type: "string" },
      description: "Changed files for fix verification.",
    },
    skipDynamic: {
      type: "boolean",
      description: "If true, dynamic and fuzz commands are represented as blocked evidence gaps.",
    },
  },
  additionalProperties: false,
};

type AuditToolInput = {
  rootPath?: string;
  packagePath?: string;
  commit?: string;
  changedFiles?: string[];
  skipDynamic?: boolean;
};

export function createAuditTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    auditTool("rust.audit.create_session", "Create audit session packet", "auditSession", "auditSession", async (input) => {
      const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
      const packet = createAuditSessionPacket({
        project: movePackage.name,
        repoRoot: packageTree.rootPath,
        commit: input.commit,
        packageManifest: movePackage.manifestPath,
        targetModules: movePackage.modules.map((module) => module.name),
        dependencyGraph: packageTree.dependencyGraph,
        enabledTools: defaultAuditToolIds(),
        toolVersions: {
          peregrine: "0.1.0",
        },
        metadata: {
          packagePath: movePackage.path,
          sourceFileCount: movePackage.sourceFileCount,
        },
      });

      state.audit.sessionPacket = packet;
      state.audit.packets.auditSession = packet;
      markStage(state, "auditSession", "auditSession", "Created immutable audit session packet.");

      return packet;
    }),
    auditTool("rust.audit.build_index", "Build and normalize project", "buildNormalize", "projectIndex", async (input) => {
      const auditSession = requireAuditSession(state);
      const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
      const buildResult = await safeRun(() => runMoveBuild(state, input), "sui move build unavailable");
      const indexReport = await safeRun(async () => {
        const report = await indexPackage(packageTree.rootPath, createIndexRunId());
        state.indexPackageId = report.packageId;
        return report;
      }, "index package unavailable");
      const packageOverview = indexReport.ok
        ? await safeRun(() => getPackageOverview(indexReport.value.packageId), "package overview unavailable")
        : { ok: false as const, error: "package overview unavailable because indexing failed" };
      const bytecodeView = await safeRun(() => loadBytecodePackage(state, input), "bytecode view unavailable");
      const packet = buildCanonicalProjectIndex({
        auditSession,
        packageTree,
        movePackage,
        buildOutput: buildResult.ok ? buildResult.value.output : { status: null, stderr: buildResult.error, stdout: "" },
        indexReport: indexReport.ok ? indexReport.value : null,
        packageOverview: packageOverview.ok ? packageOverview.value : null,
        bytecodeView: bytecodeView.ok ? bytecodeView.value.view : null,
      });

      state.audit.packets.projectIndex = packet;
      markStage(state, "buildNormalize", "projectIndex", "Built canonical project index.");

      return packet;
    }),
    auditTool("rust.audit.knowledge_graph", "Build audit knowledge graph", "semanticGraphs", "knowledgeGraph", async (input) => {
      const auditSession = requireAuditSession(state);
      const projectIndex = requirePacket(state, "projectIndex");
      const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
      const graphs = await safeRun(() => loadProjectGraphs(state, input), "graph load unavailable");
      const packet = buildAuditKnowledgeGraph({
        auditSession,
        packageTree,
        movePackage,
        graphs: graphs.ok ? graphs.value.graphs : null,
      }, projectIndex);

      state.audit.packets.knowledgeGraph = packet;
      markStage(state, "semanticGraphs", "knowledgeGraph", "Built audit knowledge graph.");

      return packet;
    }),
    auditTool("rust.audit.classify", "Classify contract surface", "classification", "classification", async () => {
      const packet = buildContractClassification(
        requireAuditSession(state),
        requirePacket(state, "projectIndex"),
        requirePacket(state, "knowledgeGraph"),
      );
      state.audit.packets.classification = packet;
      markStage(state, "classification", "classification", "Classified contract surface.");
      return packet;
    }),
    auditTool("rust.audit.threat_model", "Generate threat model", "threatModel", "threatModel", async () => {
      const packet = buildThreatModel(
        requireAuditSession(state),
        requirePacket(state, "projectIndex"),
        requirePacket(state, "knowledgeGraph"),
        requirePacket(state, "classification"),
      );
      state.audit.packets.threatModel = packet;
      markStage(state, "threatModel", "threatModel", "Generated threat model packet.");
      return packet;
    }),
    auditTool("rust.audit.function_risk_map", "Rank function risk", "functionRiskMap", "functionRiskMap", async () => {
      const packet = buildFunctionRiskMap(
        requireAuditSession(state),
        requirePacket(state, "projectIndex"),
        requirePacket(state, "knowledgeGraph"),
      );
      state.audit.packets.functionRiskMap = packet;
      markStage(state, "functionRiskMap", "functionRiskMap", "Generated function risk map.");
      return packet;
    }),
    auditTool("rust.audit.invariants", "Build invariant registry", "invariants", "invariants", async () => {
      const packet = buildInvariantRegistry(
        requireAuditSession(state),
        requirePacket(state, "threatModel"),
        requirePacket(state, "functionRiskMap"),
      );
      state.audit.packets.invariants = packet;
      markStage(state, "invariants", "invariants", "Generated invariant registry.");
      return packet;
    }),
    auditTool("rust.audit.static_analysis", "Run full static audit checks", "staticAnalysis", "staticFindings", async (input) => {
      const auditSession = requireAuditSession(state);
      const scan = await safeRun(() => runStaticScan(state, input), "static analysis unavailable");
      const packet = buildStaticFindingsSet(
        auditSession,
        scan.ok ? scan.value.report : { findings: [], diagnostics: [{ message: scan.error }] },
        requirePacket(state, "functionRiskMap"),
      );
      state.audit.packets.staticFindings = packet;
      markStage(state, "staticAnalysis", "staticFindings", "Generated static findings set.");
      return packet;
    }),
    auditTool("rust.audit.graph_analysis", "Run graph security analysis", "graphAnalysis", "graphEvidence", async () => {
      const packet = buildGraphEvidencePacket(
        requireAuditSession(state),
        requirePacket(state, "knowledgeGraph"),
      );
      state.audit.packets.graphEvidence = packet;
      markStage(state, "graphAnalysis", "graphEvidence", "Generated graph evidence packet.");
      return packet;
    }),
    auditTool("rust.audit.bytecode_review", "Review compiled bytecode", "bytecodeReview", "bytecodeReview", async (input) => {
      const auditSession = requireAuditSession(state);
      const bytecode = await safeRun(() => loadBytecodePackage(state, input), "bytecode review unavailable");
      const packet = buildBytecodeReviewPacket(
        auditSession,
        bytecode.ok ? bytecode.value.view : null,
      );
      state.audit.packets.bytecodeReview = packet;
      markStage(state, "bytecodeReview", "bytecodeReview", "Generated bytecode review packet.");
      return packet;
    }),
    auditTool("rust.audit.attack_hypotheses", "Generate attack hypotheses", "attackHypotheses", "attackHypotheses", async () => {
      const packet = buildAttackHypothesisSet({
        auditSession: requireAuditSession(state),
        riskMap: requirePacket(state, "functionRiskMap"),
        invariants: requirePacket(state, "invariants"),
        staticFindings: requirePacket(state, "staticFindings"),
        graphEvidence: requirePacket(state, "graphEvidence"),
        bytecodeReview: requirePacket(state, "bytecodeReview"),
      });
      state.audit.packets.attackHypotheses = packet;
      markStage(state, "attackHypotheses", "attackHypotheses", "Generated attack hypothesis set.");
      return packet;
    }),
    auditTool("rust.audit.test_plan", "Generate targeted test plan", "targetedTests", "testPlan", async () => {
      const packet = buildTestPlanPacket(
        requireAuditSession(state),
        requirePacket(state, "attackHypotheses"),
        requirePacket(state, "invariants"),
      );
      state.audit.packets.testPlan = packet;
      markStage(state, "targetedTests", "testPlan", "Generated targeted test plan.");
      return packet;
    }),
    auditTool("rust.audit.dynamic_analysis", "Run dynamic analysis", "dynamicAnalysis", "dynamicResults", async (input) => {
      const dynamic = input.skipDynamic
        ? { ok: false as const, error: "dynamic analysis skipped by request" }
        : await safeRun(() => runMoveTest(state, input), "dynamic test execution unavailable");
      const packet = buildDynamicEvidencePacket(
        requireAuditSession(state),
        requirePacket(state, "testPlan"),
        dynamic.ok ? dynamic.value.output : { status: null, stderr: dynamic.error, stdout: "" },
      );
      state.audit.packets.dynamicResults = packet;
      markStage(state, "dynamicAnalysis", "dynamicResults", "Generated dynamic evidence packet.");
      return packet;
    }),
    auditTool("rust.audit.invariant_stress", "Run fuzz and invariant stress", "invariantStress", "invariantStress", async (input) => {
      const fuzz = input.skipDynamic
        ? { ok: false as const, error: "fuzzing skipped by request" }
        : await safeRun(() => runMovyFuzzPackage(state, input), "fuzzing unavailable");
      const packet = buildInvariantStressReport(
        requireAuditSession(state),
        requirePacket(state, "invariants"),
        fuzz.ok ? fuzz.value.output : { status: null, stderr: fuzz.error, stdout: "" },
      );
      state.audit.packets.invariantStress = packet;
      markStage(state, "invariantStress", "invariantStress", "Generated invariant stress report.");
      return packet;
    }),
    auditTool("rust.audit.confirm_findings", "Confirm exploitability", "exploitConfirmation", "confirmedFindings", async () => {
      const packet = buildConfirmedFindingsSet({
        auditSession: requireAuditSession(state),
        hypotheses: requirePacket(state, "attackHypotheses"),
        staticFindings: requirePacket(state, "staticFindings"),
        graphEvidence: requirePacket(state, "graphEvidence"),
        dynamicEvidence: requirePacket(state, "dynamicResults"),
        invariantStress: requirePacket(state, "invariantStress"),
      });
      state.audit.packets.confirmedFindings = packet;
      markStage(state, "exploitConfirmation", "confirmedFindings", "Generated confirmed findings set.");
      return packet;
    }),
    auditTool("rust.audit.severity_ranking", "Rank severity", "severityRanking", "severityRanking", async () => {
      const packet = buildSeverityRankedFindingList(
        requireAuditSession(state),
        requirePacket(state, "confirmedFindings"),
      );
      state.audit.packets.severityRanking = packet;
      markStage(state, "severityRanking", "severityRanking", "Generated severity-ranked findings.");
      return packet;
    }),
    auditTool("rust.audit.remediation", "Generate remediation plan", "remediation", "remediationPlan", async () => {
      const packet = buildRemediationPlan(
        requireAuditSession(state),
        requirePacket(state, "severityRanking"),
      );
      state.audit.packets.remediationPlan = packet;
      markStage(state, "remediation", "remediationPlan", "Generated remediation plan.");
      return packet;
    }),
    auditTool("rust.audit.regression_tests", "Generate regression tests", "regressionTests", "regressionTests", async () => {
      const packet = buildRegressionTestPacket(
        requireAuditSession(state),
        requirePacket(state, "severityRanking"),
        requirePacket(state, "testPlan"),
      );
      state.audit.packets.regressionTests = packet;
      markStage(state, "regressionTests", "regressionTests", "Generated regression test packet.");
      return packet;
    }),
    auditTool("rust.audit.report", "Generate audit report", "auditReport", "auditReport", async () => {
      const packet = buildAuditReport({
        auditSession: requireAuditSession(state),
        projectIndex: requirePacket(state, "projectIndex"),
        classification: requirePacket(state, "classification"),
        threatModel: requirePacket(state, "threatModel"),
        rankedFindings: requirePacket(state, "severityRanking"),
        invariants: requirePacket(state, "invariants"),
        testPlan: requirePacket(state, "testPlan"),
        dynamicEvidence: requirePacket(state, "dynamicResults"),
        remediationPlan: requirePacket(state, "remediationPlan"),
      });
      state.audit.packets.auditReport = packet;
      markStage(state, "auditReport", "auditReport", "Generated human audit report.");
      return packet;
    }),
    auditTool("rust.audit.trace", "Export audit trace", "auditTrace", "auditTrace", async () => {
      const packet = buildAuditTrace({
        auditSession: requireAuditSession(state),
        packets: state.audit.packets,
        stageRuns: state.audit.stageRuns,
        findingSource: state.audit.packets.severityRanking ?? state.audit.packets.confirmedFindings,
      });
      state.audit.packets.auditTrace = packet;
      markStage(state, "auditTrace", "auditTrace", "Generated audit trace.");
      return packet;
    }),
    auditTool("rust.audit.fix_verification", "Run fix verification planning", "fixVerification", "fixVerification", async (input) => {
      const packet = buildFixVerificationPacket({
        auditSession: requireAuditSession(state),
        changedFiles: input.changedFiles ?? [],
        rankedFindings: requirePacket(state, "severityRanking"),
        dynamicEvidence: state.audit.packets.dynamicResults,
      });
      state.audit.packets.fixVerification = packet;
      markStage(state, "fixVerification", "fixVerification", "Generated fix verification packet.");
      return packet;
    }),
    defineAgentTool<AuditToolInput, unknown>({
      id: "rust.audit.run_full",
      title: "Run full audit workflow",
      description:
        "Run the complete evidence-gated Peregrine audit workflow and return the machine-readable audit trace.",
      inputSchema: auditStageSchema,
      action: toolExecutionAction("Run the complete local audit workflow.", "medium"),
      execute: async (input) => {
        for (const stage of AUDIT_STAGE_SEQUENCE) {
          if (stage === "auditTrace" || stage === "fixVerification") {
            continue;
          }
          const tool = createAuditTools(state).find((candidate) => candidate.id === toolIdForStage(stage));
          if (!tool) {
            return toolFailure(`No audit tool is registered for stage ${stage}.`);
          }
          const result = await tool.execute(input, {
            taskId: "audit-full",
            toolCallId: createId("audit_call"),
            action: tool.action,
          });
          if (executionFailed(result)) {
            return result;
          }
        }

        const traceTool = createAuditTools(state).find((candidate) => candidate.id === "rust.audit.trace");
        const trace = traceTool
          ? await traceTool.execute(input, {
              taskId: "audit-full",
              toolCallId: createId("audit_call"),
              action: traceTool.action,
            })
          : null;

        if (trace && executionFailed(trace)) {
          return trace;
        }

        return toolSuccess(
          executionOutput(trace),
          "Full audit workflow completed and emitted audit trace.",
        );
      },
    }) as DeterministicToolSpec,
  ];
}

function executionFailed(value: unknown) {
  return (
    value
    && typeof value === "object"
    && !Array.isArray(value)
    && "status" in value
    && (value as { status?: unknown }).status === "failed"
  );
}

function executionOutput(value: unknown) {
  if (value && typeof value === "object" && !Array.isArray(value) && "output" in value) {
    return (value as { output?: unknown }).output;
  }

  return value;
}

function auditTool<Name extends AuditTraceArtifactName>(
  id: string,
  title: string,
  stageId: AuditStageId,
  artifactName: Name,
  run: (input: AuditToolInput) => Promise<NonNullable<AuditPacketBundle[Name]>>,
): DeterministicToolSpec {
  return defineAgentTool<AuditToolInput, unknown>({
    id,
    title,
    description: `${title} for the ordered Peregrine audit workflow.`,
    inputSchema: auditStageSchema,
    action: toolExecutionAction(`${title} for the active package.`, "medium"),
    execute: async (input) => {
      try {
        const packet = await run(input ?? {});

        return toolSuccess(
          {
            artifactName,
            stageId,
            packet,
          },
          `${title} completed.`,
          [
            {
              kind: "toolOutput",
              source: id,
              summary: `${title} completed.`,
              raw: packet,
              metadata: { artifactName, stageId },
            },
          ],
        );
      } catch (error) {
        return toolFailure(error instanceof Error ? error.message : `${title} failed.`);
      }
    },
  }) as DeterministicToolSpec;
}

function requireAuditSession(state: AgentToolRuntimeState) {
  const packet = state.audit.sessionPacket ?? state.audit.packets.auditSession;
  if (!packet) {
    throw new Error("Run rust.audit.create_session before audit analysis stages.");
  }
  return packet;
}

function requirePacket<Name extends keyof AuditPacketBundle>(
  state: AgentToolRuntimeState,
  name: Name,
): NonNullable<AuditPacketBundle[Name]> {
  const packet = state.audit.packets[name];
  if (!packet) {
    throw new Error(`Audit packet ${String(name)} is unavailable. Run prerequisite stages first.`);
  }
  return packet as NonNullable<AuditPacketBundle[Name]>;
}

function markStage(
  state: AgentToolRuntimeState,
  stageId: AuditStageId,
  artifactName: AuditTraceArtifactName,
  summary: string,
) {
  const now = new Date().toISOString();
  const run: AuditStageRun = {
    id: createId("audit_stage"),
    stageId,
    status: "succeeded",
    startedAt: now,
    completedAt: now,
    artifactName,
    summary,
  };

  state.audit.stageRuns.push(run);
  state.audit.completedStages.add(stageId);
}

async function safeRun<T>(
  run: () => Promise<T>,
  fallback: string,
): Promise<{ ok: true; value: T } | { ok: false; error: string }> {
  try {
    return { ok: true, value: await run() };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : fallback,
    };
  }
}

function toolIdForStage(stageId: AuditStageId) {
  switch (stageId) {
    case "auditSession":
      return "rust.audit.create_session";
    case "buildNormalize":
      return "rust.audit.build_index";
    case "semanticGraphs":
      return "rust.audit.knowledge_graph";
    case "classification":
      return "rust.audit.classify";
    case "threatModel":
      return "rust.audit.threat_model";
    case "functionRiskMap":
      return "rust.audit.function_risk_map";
    case "invariants":
      return "rust.audit.invariants";
    case "staticAnalysis":
      return "rust.audit.static_analysis";
    case "graphAnalysis":
      return "rust.audit.graph_analysis";
    case "bytecodeReview":
      return "rust.audit.bytecode_review";
    case "attackHypotheses":
      return "rust.audit.attack_hypotheses";
    case "targetedTests":
      return "rust.audit.test_plan";
    case "dynamicAnalysis":
      return "rust.audit.dynamic_analysis";
    case "invariantStress":
      return "rust.audit.invariant_stress";
    case "exploitConfirmation":
      return "rust.audit.confirm_findings";
    case "severityRanking":
      return "rust.audit.severity_ranking";
    case "remediation":
      return "rust.audit.remediation";
    case "regressionTests":
      return "rust.audit.regression_tests";
    case "auditReport":
      return "rust.audit.report";
    case "auditTrace":
      return "rust.audit.trace";
    case "fixVerification":
      return "rust.audit.fix_verification";
  }
}

function defaultAuditToolIds() {
  return [
    "rust.index.package",
    "rust.validation.run_suite",
    "rust.static.scan_package",
    "rust.graph.call_graph.read",
    "rust.graph.object_lifecycle",
    "rust.graph.capability_flow",
    "rust.bytecode.disassemble",
    "rust.dynamic.run_test",
    "rust.dynamic.fuzz_function",
    "rust.report.generate",
  ];
}
