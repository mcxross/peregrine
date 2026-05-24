import type {
  EvidenceConfidence,
  FindingCandidate,
  SecurityEvidenceItem,
} from "@peregrine/agent-runtime";

import { createId } from "./ids";

export interface FindingCorrelationInput {
  evidence: SecurityEvidenceItem[];
  candidates?: FindingCandidate[];
}

export interface FindingCorrelationResult {
  findings: FindingCandidate[];
}

export function correlateFindings(
  input: FindingCorrelationInput,
): FindingCorrelationResult {
  const byTitle = new Map<string, FindingCandidate>();

  for (const candidate of input.candidates ?? []) {
    byTitle.set(candidate.title, normalizeCandidate(candidate, input.evidence));
  }

  for (const evidence of input.evidence) {
    if (!evidenceSuggestsFinding(evidence)) {
      continue;
    }

    const title = titleForEvidence(evidence);
    const existing = byTitle.get(title);
    if (existing) {
      byTitle.set(title, mergeEvidence(existing, evidence));
    } else {
      byTitle.set(title, candidateFromEvidence(title, evidence));
    }
  }

  return {
    findings: Array.from(byTitle.values()).sort(compareFindings),
  };
}

export function classifyFindingStatus(
  evidence: SecurityEvidenceItem[],
): FindingCandidate["status"] {
  if (evidence.some((item) => item.confidence === "confirmed")) {
    return "confirmed";
  }

  if (
    evidence.some((item) => item.confidence === "high")
    && evidence.some((item) => item.kind === "testResult" || item.kind === "proverResult")
  ) {
    return "likely";
  }

  if (evidence.some((item) => item.confidence === "high" || item.confidence === "medium")) {
    return "likely";
  }

  return "hypothesis";
}

function normalizeCandidate(
  candidate: FindingCandidate,
  evidence: SecurityEvidenceItem[],
): FindingCandidate {
  const relevantEvidence = evidence.filter((item) => candidate.evidenceRefs.includes(item.id));
  const status = candidate.status === "confirmed"
    ? candidate.status
    : classifyFindingStatus(relevantEvidence);

  return {
    ...candidate,
    status,
    confidence: maxConfidence([
      candidate.confidence,
      ...relevantEvidence.map((item) => item.confidence),
    ]),
    validationPlan: candidate.validationPlan.commands.length
      ? candidate.validationPlan
      : defaultValidationPlan(status),
  };
}

function mergeEvidence(
  candidate: FindingCandidate,
  evidence: SecurityEvidenceItem,
): FindingCandidate {
  const evidenceRefs = Array.from(new Set([...candidate.evidenceRefs, evidence.id]));
  const confidence = maxConfidence([candidate.confidence, evidence.confidence]);
  const status = confidence === "confirmed" ? "confirmed" : candidate.status;

  return {
    ...candidate,
    evidenceRefs,
    confidence,
    status,
    affectedSymbols: Array.from(
      new Set([...candidate.affectedSymbols, ...evidence.symbolRefs]),
    ),
  };
}

function candidateFromEvidence(
  title: string,
  evidence: SecurityEvidenceItem,
): FindingCandidate {
  const status = classifyFindingStatus([evidence]);

  return {
    id: createId("finding"),
    title,
    category: evidence.kind,
    severity: severityForEvidence(evidence),
    confidence: evidence.confidence,
    status,
    affectedSymbols: evidence.symbolRefs,
    evidenceRefs: [evidence.id],
    validationPlan: defaultValidationPlan(status),
    metadata: {
      sourcePrecision: evidence.sourcePrecision,
      toolRunId: evidence.toolRunId,
    },
  };
}

function evidenceSuggestsFinding(evidence: SecurityEvidenceItem) {
  return (
    evidence.kind === "fuzzCounterexample"
    || evidence.kind === "proverResult"
    || evidence.kind === "staticFinding"
    || evidence.kind === "graphSignal"
  );
}

function titleForEvidence(evidence: SecurityEvidenceItem) {
  if (evidence.kind === "fuzzCounterexample") return "Fuzz counterexample";
  if (evidence.kind === "proverResult") return "Formal verification signal";
  return evidence.claim;
}

function defaultValidationPlan(status: FindingCandidate["status"]) {
  return {
    commands: ["sui move test", "peregrine analyze"],
    expectedEvidence:
      status === "hypothesis"
        ? ["Additional deterministic evidence confirms or rejects the hypothesis."]
        : ["The finding is absent or explicitly mitigated after the patch."],
    required: status !== "hypothesis",
  };
}

function severityForEvidence(evidence: SecurityEvidenceItem): FindingCandidate["severity"] {
  if (evidence.kind === "fuzzCounterexample") return "high";
  if (evidence.kind === "proverResult" && evidence.confidence === "confirmed") return "medium";
  if (evidence.confidence === "confirmed" || evidence.confidence === "high") return "medium";
  return "low";
}

function maxConfidence(values: EvidenceConfidence[]): EvidenceConfidence {
  const order: EvidenceConfidence[] = ["unknown", "low", "medium", "high", "confirmed"];
  return values.reduce((best, value) =>
    order.indexOf(value) > order.indexOf(best) ? value : best,
  "unknown");
}

function compareFindings(left: FindingCandidate, right: FindingCandidate) {
  const severityOrder = ["critical", "high", "medium", "low", "info"];
  const statusOrder = ["confirmed", "likely", "needsValidation", "hypothesis", "falsePositive", "fixed", "accepted"];

  return (
    severityOrder.indexOf(left.severity) - severityOrder.indexOf(right.severity)
    || statusOrder.indexOf(left.status) - statusOrder.indexOf(right.status)
    || left.title.localeCompare(right.title)
  );
}
