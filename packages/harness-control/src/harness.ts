import { PeregrineAgentRuntime } from "@peregrine/agent-runtime";
import type {
  DeterministicToolSpec,
  EvidenceRef,
} from "@peregrine/agent-runtime";

import { DenyByDefaultApprovalGate } from "./approvals";
import { InMemoryEvidenceStore } from "./evidence-store";
import { DefaultApprovalPolicy } from "./policy";
import { InMemorySessionStore } from "./session-store";
import { HarnessToolRuntime } from "./tool-runtime";
import { InMemoryToolRegistry } from "./tool-registry";
import type {
  AssessmentSession,
  ContextPacketInput,
  CreateSessionRequest,
  PeregrineHarnessConfig,
  RunAgentTaskRequest,
  RunAgentTaskResult,
  RunToolRequest,
  RunToolResult,
} from "./types";
import { buildAgentContextPacket } from "./packet-builder";
import { createId } from "./ids";

export class PeregrineHarness {
  private readonly config: Required<PeregrineHarnessConfig>;

  constructor(config: PeregrineHarnessConfig) {
    const evidenceStore = config.evidenceStore ?? new InMemoryEvidenceStore();
    const policy = config.policy ?? new DefaultApprovalPolicy();
    const approvalGate =
      config.approvalGate ?? new DenyByDefaultApprovalGate();
    const sessionStore = config.sessionStore ?? new InMemorySessionStore();
    const toolRegistry = config.toolRegistry ?? new InMemoryToolRegistry();
    const toolRuntime =
      config.toolRuntime ??
      new HarnessToolRuntime({
        policy,
        approvalGate,
        evidenceStore,
        onToolRun: (toolRun, context) => {
          if (!context.sessionId) {
            return;
          }

          const session = sessionStore.get(context.sessionId);

          if (session) {
            sessionStore.update({
              ...session,
              toolRuns: [...session.toolRuns, toolRun],
              evidenceRefs: [
                ...session.evidenceRefs,
                ...toolRun.evidenceRefs,
              ],
            });
          }
        },
        onApprovalDecision: (request, decision) => {
          if (!request.sessionId) {
            return;
          }

          const session = sessionStore.get(request.sessionId);

          if (session) {
            sessionStore.update({
              ...session,
              approvals: [...session.approvals, decision],
            });
          }
        },
      });

    this.config = {
      model: config.model,
      policy,
      approvalGate,
      evidenceStore,
      sessionStore,
      toolRegistry,
      toolRuntime,
      maxAgentSteps: config.maxAgentSteps ?? 12,
    };
  }

  createSession(request: CreateSessionRequest): AssessmentSession {
    return this.config.sessionStore.create(request);
  }

  getSession(sessionId: string) {
    return this.config.sessionStore.get(sessionId);
  }

  registerTool(tool: DeterministicToolSpec) {
    this.config.toolRegistry.register(tool);
  }

  buildContextPacket(input: ContextPacketInput) {
    return {
      ...buildAgentContextPacket(input),
      approvalPolicy: this.config.policy.snapshot(),
    };
  }

  async runAgentTask(
    request: RunAgentTaskRequest,
  ): Promise<RunAgentTaskResult> {
    const session = this.requireSession(request.sessionId);
    this.updateSessionStatus(session, "running");

    const runtime = new PeregrineAgentRuntime({
      model: this.config.model,
      tools: this.config.toolRegistry.list(),
      toolGateway: this.config.toolRuntime,
      maxSteps: this.config.maxAgentSteps,
    });
    const result = await runtime.generate({
      ...request,
      sessionId: session.id,
    });
    const evidence = await this.config.evidenceStore.record({
      kind: "agentOutput",
      source: request.packet.task.role,
      summary: `Agent completed task ${request.packet.task.id}.`,
      raw: result.text,
    });
    const evidenceRef: EvidenceRef = {
      id: evidence.id,
      kind: evidence.kind,
      summary: evidence.summary,
      source: evidence.source,
    };

    this.updateSession(session, {
      status: "ready",
      evidenceRefs: [...session.evidenceRefs, evidenceRef],
    });

    return {
      ...result,
      evidenceRef,
    };
  }

  async runTool<Input = unknown, Output = unknown>(
    request: RunToolRequest<Input>,
  ): Promise<RunToolResult<Output>> {
    const session = this.requireSession(request.sessionId);
    const tool = this.config.toolRegistry.get(request.toolId);

    if (!tool) {
      throw new Error(`Tool ${request.toolId} is not registered.`);
    }

    return this.config.toolRuntime.runTool({
      tool: tool as DeterministicToolSpec<Input, Output>,
      input: request.input,
      toolCallId: request.toolCallId ?? createId("tool_call"),
      context: {
        sessionId: session.id,
        taskId: request.taskId,
        metadata: request.metadata,
        abortSignal: request.abortSignal,
      },
    });
  }

  private requireSession(sessionId: string) {
    const session = this.config.sessionStore.get(sessionId);

    if (!session) {
      throw new Error(`Assessment session ${sessionId} was not found.`);
    }

    return session;
  }

  private updateSessionStatus(
    session: AssessmentSession,
    status: AssessmentSession["status"],
  ) {
    this.updateSession(session, { status });
  }

  private updateSession(
    session: AssessmentSession,
    patch: Partial<AssessmentSession>,
  ) {
    this.config.sessionStore.update({
      ...session,
      ...patch,
    });
  }
}
