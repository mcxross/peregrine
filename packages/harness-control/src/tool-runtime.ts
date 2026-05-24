import type {
  AgentRuntimeToolResult,
  DeterministicToolExecutionResult,
  DeterministicToolSpec,
  EvidenceCandidate,
  EvidenceRef,
  ToolGateway,
  ToolGatewayRequest,
} from "@peregrine/agent-runtime";

import { createId } from "./ids";
import { correlateFindings } from "./finding-engine";
import { compileToolEvidence } from "./reducers";
import type {
  ApprovalDecision,
  ApprovalRequest,
  HarnessToolRuntimeConfig,
} from "./types";
import type { ToolRunSummary } from "@peregrine/agent-runtime";

export class HarnessToolRuntime implements ToolGateway {
  private readonly config: HarnessToolRuntimeConfig;

  constructor(config: HarnessToolRuntimeConfig) {
    this.config = config;
  }

  async runTool<Input, Output>(
    request: ToolGatewayRequest<Input, Output>,
  ): Promise<AgentRuntimeToolResult<Output>> {
    const action = {
      ...request.tool.action,
      toolId: request.tool.id,
    };
    const policy = this.config.policy.evaluateAction(action, {
      sessionId: request.context.sessionId,
      taskId: request.context.taskId,
      toolId: request.tool.id,
    });

    if (policy.disposition === "forbidden") {
      const evidenceRefs = await this.recordEvidence([
        {
          kind: "diagnostic",
          source: request.tool.id,
          summary: policy.reason,
          raw: {
            action,
            policy,
          },
        },
      ]);

      return this.finalizeResult(
        {
          status: "denied",
          toolId: request.tool.id,
          toolCallId: request.toolCallId,
          action,
          summary: policy.reason,
          evidenceRefs,
          diagnostics: policy.diagnostics ?? [],
        },
        request,
      );
    }

    if (policy.disposition === "approvalRequired") {
      const approvalRequest = createApprovalRequest({
        sessionId: request.context.sessionId,
        taskId: request.context.taskId,
        action,
      });
      const decision = await this.config.approvalGate.requestApproval(
        approvalRequest,
      );
      const approvalEvidence = await this.recordApprovalEvidence(
        approvalRequest,
        decision,
      );
      this.config.onApprovalDecision?.(approvalRequest, decision);

      if (decision.status !== "approved") {
        return this.finalizeResult(
          {
            status: "denied",
            toolId: request.tool.id,
            toolCallId: request.toolCallId,
            action,
            summary:
              decision.rationale ??
              `${action.actionClass} was denied by the approval gate.`,
            evidenceRefs: approvalEvidence,
            diagnostics: [
              ...(policy.diagnostics ?? []),
              {
                level: "warning",
                source: request.tool.id,
                message: "Tool execution was blocked by approval policy.",
              },
            ],
          },
          request,
        );
      }
    }

    try {
      const execution = await request.tool.execute(request.input, {
        sessionId: request.context.sessionId,
        taskId: request.context.taskId,
        toolCallId: request.toolCallId,
        action,
        abortSignal: request.context.abortSignal,
        messages: request.context.messages,
        metadata: request.context.metadata,
      });
      const normalized = normalizeExecutionResult<Output>(execution);
      const status = normalized.status ?? "succeeded";
      const toolRunId = createId("tool_run");
      const fallbackSummary =
        normalized.summary ??
        `${request.tool.id} ${status === "succeeded" ? "completed" : "failed"}.`;
      const compiledEvidence = compileToolEvidence({
        tool: request.tool as DeterministicToolSpec,
        output: normalized.output,
        status,
        summary: fallbackSummary,
        toolRunId,
      });
      const correlatedFindings = correlateFindings({
        evidence: compiledEvidence.evidence,
        candidates: compiledEvidence.findingCandidates,
      });
      const summary = compiledEvidence.summary;
      const evidenceRefs = await this.recordEvidence([
        ...(normalized.evidence ?? []),
        ...compiledEvidence.evidence.map((item) => ({
          kind: item.kind,
          source: request.tool.id,
          summary: `${item.claim} ${item.observation}`,
          raw: item,
          metadata: {
            evidenceItemId: item.id,
            confidence: item.confidence,
            sourcePrecision: item.sourcePrecision,
            toolRunId,
          },
        })),
        {
          kind: status === "succeeded" ? "toolOutput" : "toolFailure",
          source: request.tool.id,
          summary,
          raw: toEvidenceRaw(normalized.output),
          metadata: {
            toolRunId,
            reducerId: request.tool.manifest?.reducerId,
          },
        },
      ]);

      return this.finalizeResult(
        {
          status,
          toolId: request.tool.id,
          toolCallId: request.toolCallId,
          action,
          summary,
          output: compiledEvidence.modelOutput as Output,
          evidenceRefs,
          securityEvidence: compiledEvidence.evidence,
          findingCandidates: correlatedFindings.findings,
          diagnostics: [
            ...(normalized.diagnostics ?? []),
            ...compiledEvidence.diagnostics,
          ],
        },
        request,
        toolRunId,
      );
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Tool execution failed.";
      const evidenceRefs = await this.recordEvidence([
        {
          kind: "toolFailure",
          source: request.tool.id,
          summary: message,
          raw: {
            toolId: request.tool.id,
            error: message,
          },
        },
      ]);

      return this.finalizeResult(
          {
            status: "failed",
            toolId: request.tool.id,
            toolCallId: request.toolCallId,
            action,
          summary: message,
          evidenceRefs,
          diagnostics: [
            {
              level: "error",
              source: request.tool.id,
              message,
            },
          ],
        },
        request,
      );
    }
  }

  private async recordApprovalEvidence(
    request: ApprovalRequest,
    decision: ApprovalDecision,
  ) {
    return this.recordEvidence([
      {
        kind: "humanApproval",
        source: "approval-gate",
        summary: `Approval ${decision.status} for ${request.action.actionClass}.`,
        raw: {
          request,
          decision,
        },
      },
    ]);
  }

  private async recordEvidence(candidates: EvidenceCandidate[]) {
    const records = await Promise.all(
      candidates.map((candidate) => this.config.evidenceStore.record(candidate)),
    );

    return records.map((record): EvidenceRef => ({
      id: record.id,
      kind: record.kind,
      summary: record.summary,
      source: record.source,
    }));
  }

  private finalizeResult<Input, Output>(
    result: AgentRuntimeToolResult<Output>,
    request: ToolGatewayRequest<Input, Output>,
    id = createId("tool_run"),
  ) {
    const toolRun: ToolRunSummary = {
      id,
      toolId: result.toolId,
      status: result.status,
      summary: result.summary,
      evidenceRefs: result.evidenceRefs,
      diagnostics: result.diagnostics,
    };

    this.config.onToolRun?.(toolRun, {
      sessionId: request.context.sessionId,
      taskId: request.context.taskId,
    });

    return result;
  }
}

function createApprovalRequest({
  sessionId,
  taskId,
  action,
}: {
  sessionId?: string;
  taskId: string;
  action: ApprovalRequest["action"];
}): ApprovalRequest {
  return {
    id: createId("approval"),
    sessionId,
    taskId,
    toolId: action.toolId,
    action,
    reason: action.reason,
    filesAffected: action.files ?? [],
    networkDomains: action.networkDomains ?? [],
    diffPreview: action.diffPreview,
    expectedChecks: action.expectedChecks ?? [],
    risk: action.risk,
    createdAt: new Date().toISOString(),
  };
}

function normalizeExecutionResult<Output>(
  value: Output | DeterministicToolExecutionResult<Output>,
): DeterministicToolExecutionResult<Output> {
  if (isStructuredExecutionResult<Output>(value)) {
    return value;
  }

  return {
    status: "succeeded",
    output: value,
  };
}

function isStructuredExecutionResult<Output>(
  value: Output | DeterministicToolExecutionResult<Output>,
): value is DeterministicToolExecutionResult<Output> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }

  return (
    "status" in value ||
    "evidence" in value ||
    "diagnostics" in value ||
    ("output" in value && "summary" in value)
  );
}

function toEvidenceRaw(value: unknown): unknown {
  if (value === undefined) {
    return "undefined";
  }

  try {
    return JSON.parse(JSON.stringify(value));
  } catch {
    return String(value);
  }
}
