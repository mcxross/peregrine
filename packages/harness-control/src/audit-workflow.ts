import type { EvidenceRef } from "@peregrine/agent-runtime";

import {
  AUDIT_STAGE_SEQUENCE,
  AUDIT_TRACE_FILENAMES,
  type AuditPacketBundle,
  type AuditStageId,
  type AuditStageRun,
  type AuditTraceArtifactName,
} from "./audit-types";
import { buildAuditTrace } from "./audit-builders";
import { createId } from "./ids";
import type {
  AssessmentSession,
  EvidenceStore,
  SessionStore,
} from "./types";

export interface AuditWorkflowRunnerConfig {
  evidenceStore: EvidenceStore;
  sessionStore: SessionStore;
  now?: () => Date;
}

export interface AuditRecordPacketRequest<Name extends AuditTraceArtifactName> {
  artifactName: Name;
  packet: NonNullable<AuditPacketBundle[Name]>;
  sessionId: string;
  stageId: AuditStageId;
  summary: string;
  source?: string;
  status?: AuditStageRun["status"];
}

export class AuditWorkflowRunner {
  private readonly config: AuditWorkflowRunnerConfig;

  constructor(config: AuditWorkflowRunnerConfig) {
    this.config = config;
  }

  async recordPacket<Name extends AuditTraceArtifactName>(
    request: AuditRecordPacketRequest<Name>,
  ): Promise<EvidenceRef> {
    const session = this.requireSession(request.sessionId);
    this.assertStageAllowed(session, request.stageId);

    if (request.artifactName === "auditSession") {
      const nextId = (request.packet as { id?: string }).id;
      if (session.auditSession && session.auditSession.id !== nextId) {
        throw new Error("Audit session packet is immutable once recorded.");
      }
    } else if (!session.auditSession) {
      throw new Error("Audit analysis cannot run before an audit session packet exists.");
    }

    const evidence = await this.config.evidenceStore.record({
      kind: "toolOutput",
      source: request.source ?? `audit.${request.stageId}`,
      summary: request.summary,
      raw: request.packet,
      metadata: {
        auditStageId: request.stageId,
        auditArtifactName: request.artifactName,
        filename: AUDIT_TRACE_FILENAMES[request.artifactName],
      },
    });
    const evidenceRef: EvidenceRef = {
      id: evidence.id,
      kind: evidence.kind,
      summary: evidence.summary,
      source: evidence.source,
    };
    const now = this.nowIso();
    const stageRun: AuditStageRun = {
      id: createId("audit_stage"),
      stageId: request.stageId,
      status: request.status ?? "succeeded",
      startedAt: now,
      completedAt: now,
      summary: request.summary,
      artifactName: request.artifactName,
      filename: AUDIT_TRACE_FILENAMES[request.artifactName],
      evidenceRef,
    };
    const packetRefs = {
      ...(session.auditPacketRefs ?? {}),
      [request.artifactName]: evidenceRef,
    };
    const packetBundle = {
      ...(session.auditPackets ?? {}),
      [request.artifactName]: request.packet,
    };
    const auditSession =
      request.artifactName === "auditSession"
        ? packetBundle.auditSession
        : session.auditSession;

    this.config.sessionStore.update({
      ...session,
      auditSession,
      auditPackets: packetBundle,
      auditPacketRefs: packetRefs,
      auditStageRuns: [...(session.auditStageRuns ?? []), stageRun],
    });

    return evidenceRef;
  }

  async recordTrace(sessionId: string): Promise<EvidenceRef> {
    const session = this.requireSession(sessionId);

    if (!session.auditSession) {
      throw new Error("Audit trace cannot be created before an audit session packet exists.");
    }

    const trace = buildAuditTrace({
      auditSession: session.auditSession,
      packets: session.auditPackets ?? {},
      stageRuns: session.auditStageRuns ?? [],
      findingSource:
        session.auditPackets?.severityRanking
        ?? session.auditPackets?.confirmedFindings,
      generatedAt: this.nowIso(),
    });

    return this.recordPacket({
      sessionId,
      stageId: "auditTrace",
      artifactName: "auditTrace",
      packet: trace,
      summary: "Generated machine-readable audit trace.",
      source: "audit.trace",
    });
  }

  canResume(sessionId: string, stageId: AuditStageId) {
    const session = this.requireSession(sessionId);
    const completed = new Set(
      (session.auditStageRuns ?? [])
        .filter((run) => run.status === "succeeded" || run.status === "skipped")
        .map((run) => run.stageId),
    );
    const index = AUDIT_STAGE_SEQUENCE.indexOf(stageId);

    return AUDIT_STAGE_SEQUENCE.slice(0, Math.max(0, index)).every((previous) =>
      completed.has(previous),
    );
  }

  private assertStageAllowed(session: AssessmentSession, stageId: AuditStageId) {
    if (stageId === "auditSession") return;
    if (!session.auditSession) {
      throw new Error("Audit analysis cannot run before an audit session packet exists.");
    }
    if (!this.canResume(session.id, stageId)) {
      throw new Error(`Audit stage ${stageId} cannot run before required previous stages.`);
    }
  }

  private requireSession(sessionId: string) {
    const session = this.config.sessionStore.get(sessionId);
    if (!session) {
      throw new Error(`Assessment session ${sessionId} was not found.`);
    }
    return session;
  }

  private nowIso() {
    return (this.config.now ?? (() => new Date()))().toISOString();
  }
}
