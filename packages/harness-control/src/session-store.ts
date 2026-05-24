import { createId } from "./ids";
import type {
  AssessmentSession,
  CreateSessionRequest,
  SessionStore,
} from "./types";

export class InMemorySessionStore implements SessionStore {
  private readonly sessions = new Map<string, AssessmentSession>();

  create(request: CreateSessionRequest): AssessmentSession {
    const now = new Date().toISOString();
    const session: AssessmentSession = {
      id: createId("session"),
      projectPath: request.projectPath,
      targetChain: request.targetChain,
      profile: request.profile,
      status: "created",
      createdAt: now,
      updatedAt: now,
      findings: [],
      toolRuns: [],
      approvals: [],
      evidenceRefs: [],
      auditStageRuns: [],
      auditPackets: {},
      auditPacketRefs: {},
      confirmedFindings: [],
      regressionRefs: [],
      fixVerificationHistory: [],
      metadata: request.metadata,
    };

    this.sessions.set(session.id, session);

    return session;
  }

  get(sessionId: string) {
    return this.sessions.get(sessionId);
  }

  update(session: AssessmentSession) {
    this.sessions.set(session.id, {
      ...session,
      updatedAt: new Date().toISOString(),
    });
  }

  list() {
    return Array.from(this.sessions.values());
  }
}
