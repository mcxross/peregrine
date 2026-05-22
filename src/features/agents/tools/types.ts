import type { PackageTree } from "@/features/empty-project/filesystem-tree";

export type AgentToolProjectContext = {
  rootPath: string;
  packagePath: string;
  packageName: string;
  manifestPath: string;
  packageTree?: PackageTree | null;
};

export type AgentToolRuntimeState = {
  context: AgentToolProjectContext;
  indexPackageId: string | null;
  session: AgentToolSessionStore;
};

export type AgentFindingSeverity = "critical" | "high" | "medium" | "low" | "info";

export type AgentFindingRecord = {
  id: string;
  title: string;
  severity: AgentFindingSeverity;
  status: "open" | "fixed" | "accepted" | "falsePositive" | "needsReview";
  message: string;
  location?: string;
  evidenceRefs: string[];
  attachments: Record<string, unknown>;
  createdAt: number;
};

export class AgentToolSessionStore {
  private findings: AgentFindingRecord[] = [];
  private nextFindingId = 1;

  listFindings() {
    return [...this.findings];
  }

  emitFinding(input: {
    title: string;
    severity: AgentFindingSeverity;
    message: string;
    location?: string;
    evidenceRefs?: string[];
    attachments?: Record<string, unknown>;
  }) {
    const finding: AgentFindingRecord = {
      id: `finding_${this.nextFindingId++}`,
      title: input.title,
      severity: input.severity,
      status: "open",
      message: input.message,
      location: input.location,
      evidenceRefs: input.evidenceRefs ?? [],
      attachments: input.attachments ?? {},
      createdAt: Date.now(),
    };

    this.findings.push(finding);
    return finding;
  }

  attachToFinding(
    findingId: string,
    attachmentKey: string,
    payload: unknown,
  ) {
    const finding = this.findings.find((candidate) => candidate.id === findingId);

    if (!finding) {
      throw new Error(`Finding ${findingId} was not found in the agent session.`);
    }

    finding.attachments[attachmentKey] = payload;
    return finding;
  }

  triageFindings() {
    const order: Record<AgentFindingSeverity, number> = {
      critical: 0,
      high: 1,
      medium: 2,
      low: 3,
      info: 4,
    };

    return [...this.findings].sort(
      (left, right) => order[left.severity] - order[right.severity],
    );
  }
}

export function createAgentToolRuntimeState(
  context: AgentToolProjectContext,
): AgentToolRuntimeState {
  return {
    context,
    indexPackageId: null,
    session: new AgentToolSessionStore(),
  };
}
