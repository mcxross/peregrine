import type { FindingCandidate, ToolRunSummary } from "@peregrine/agent-runtime";

export interface EvaluationCase {
  id: string;
  vulnerabilityClass: string;
  expectedFindings: string[];
  benign?: boolean;
}

export interface EvaluationRun {
  caseId: string;
  mode: "modelOnly" | "toolAssisted";
  findings: FindingCandidate[];
  toolRuns: ToolRunSummary[];
  tokenEstimate?: number;
  durationMs?: number;
}

export interface EvaluationMetrics {
  caseCount: number;
  recall: number;
  falsePositiveRate: number;
  evidenceBackedFindingRate: number;
  averageToolCalls: number;
  averageTokenEstimate: number;
  averageDurationMs: number;
}

export function evaluateHarnessRuns(
  cases: EvaluationCase[],
  runs: EvaluationRun[],
): EvaluationMetrics {
  const caseById = new Map(cases.map((item) => [item.id, item]));
  let expected = 0;
  let matched = 0;
  let benignFindings = 0;
  let benignCases = 0;
  let evidenceBacked = 0;
  let findingCount = 0;
  let toolCalls = 0;
  let tokenEstimate = 0;
  let durationMs = 0;

  for (const run of runs) {
    const evaluationCase = caseById.get(run.caseId);
    if (!evaluationCase) {
      continue;
    }

    if (evaluationCase.benign) {
      benignCases += 1;
      benignFindings += run.findings.filter(
        (finding) => finding.status !== "falsePositive" && finding.status !== "accepted",
      ).length;
    }

    expected += evaluationCase.expectedFindings.length;
    matched += evaluationCase.expectedFindings.filter((needle) =>
      run.findings.some((finding) =>
        `${finding.title} ${finding.category}`.toLowerCase().includes(needle.toLowerCase()),
      ),
    ).length;
    evidenceBacked += run.findings.filter((finding) => finding.evidenceRefs.length > 0).length;
    findingCount += run.findings.length;
    toolCalls += run.toolRuns.length;
    tokenEstimate += run.tokenEstimate ?? 0;
    durationMs += run.durationMs ?? 0;
  }

  return {
    caseCount: runs.length,
    recall: expected ? matched / expected : 1,
    falsePositiveRate: benignCases ? benignFindings / benignCases : 0,
    evidenceBackedFindingRate: findingCount ? evidenceBacked / findingCount : 1,
    averageToolCalls: runs.length ? toolCalls / runs.length : 0,
    averageTokenEstimate: runs.length ? tokenEstimate / runs.length : 0,
    averageDurationMs: runs.length ? durationMs / runs.length : 0,
  };
}

export const SUI_MOVE_EVALUATION_CLASSES = [
  "accessControl",
  "capabilityLeakage",
  "sharedObjectMutation",
  "dynamicFields",
  "coinAccounting",
  "precisionLoss",
  "uncheckedReturns",
  "objectLifecycle",
  "sourceBytecodeMismatch",
] as const;
