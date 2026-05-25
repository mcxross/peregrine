import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { readOnlyAction } from "../actions";
import { defineAgentTool } from "../define-tool";
import { toolSuccess } from "../executors";
import {
  findingAttachmentSchema,
  findingInputSchema,
} from "../schemas";
import type {
  AgentFindingRecord,
  AgentFindingSeverity,
  AgentToolRuntimeState,
} from "../types";

type FindingInput = {
  title: string;
  severity: AgentFindingSeverity;
  message: string;
  location?: string;
};

type FindingAttachmentInput = {
  findingId: string;
  payload: Record<string, unknown>;
};

export function createFindingsTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<FindingInput, AgentFindingRecord>({
      id: "rust.findings.emit",
      title: "Emit agent finding",
      description: "Record a structured finding in the current agent session.",
      inputSchema: findingInputSchema,
      action: readOnlyAction("Record a structured finding in the current session."),
      execute: async (input) => {
        const finding = state.session.emitFinding(input);

        return toolSuccess(finding, `Recorded finding ${finding.id}.`);
      },
    }),
    defineAgentTool<Record<string, never>, AgentFindingRecord[]>({
      id: "rust.findings.triage",
      title: "Triage session findings",
      description: "Return findings recorded during the current agent run ordered by severity.",
      inputSchema: {
        type: "object",
        properties: {},
        additionalProperties: false,
      },
      action: readOnlyAction("Read and prioritize findings recorded in the current session."),
      execute: async () => {
        const findings = state.session.triageFindings();

        return toolSuccess(
          findings,
          findings.length
            ? `Triaged ${findings.length} findings.`
            : "No findings were recorded in the current session.",
        );
      },
    }),
    defineAgentTool<FindingAttachmentInput, AgentFindingRecord>({
      id: "rust.findings.attach_trace",
      title: "Attach trace evidence",
      description: "Attach trace or operation evidence to an existing session finding.",
      inputSchema: findingAttachmentSchema,
      action: readOnlyAction("Attach trace evidence to a session finding."),
      execute: async (input) => {
        const finding = state.session.attachToFinding(input.findingId, "trace", input.payload);

        return toolSuccess(finding, `Attached trace evidence to ${finding.id}.`);
      },
    }),
    defineAgentTool<FindingAttachmentInput, AgentFindingRecord>({
      id: "rust.findings.attach_graph",
      title: "Attach graph evidence",
      description: "Attach graph evidence to an existing session finding.",
      inputSchema: findingAttachmentSchema,
      action: readOnlyAction("Attach graph evidence to a session finding."),
      execute: async (input) => {
        const finding = state.session.attachToFinding(input.findingId, "graph", input.payload);

        return toolSuccess(finding, `Attached graph evidence to ${finding.id}.`);
      },
    }),
    defineAgentTool<FindingAttachmentInput, AgentFindingRecord>({
      id: "rust.findings.attach_bytecode",
      title: "Attach bytecode evidence",
      description: "Attach bytecode evidence to an existing session finding.",
      inputSchema: findingAttachmentSchema,
      action: readOnlyAction("Attach bytecode evidence to a session finding."),
      execute: async (input) => {
        const finding = state.session.attachToFinding(input.findingId, "bytecode", input.payload);

        return toolSuccess(finding, `Attached bytecode evidence to ${finding.id}.`);
      },
    }),
    defineAgentTool<{
      findingId: string;
      patchId: string;
      summary: string;
      diffPreview?: string;
    }, AgentFindingRecord>({
      id: "rust.findings.link_patch",
      title: "Link patch proposal",
      description: "Attach a patch proposal reference to an existing session finding.",
      inputSchema: {
        type: "object",
        properties: {
          findingId: { type: "string" },
          patchId: { type: "string" },
          summary: { type: "string" },
          diffPreview: { type: "string" },
        },
        required: ["findingId", "patchId", "summary"],
        additionalProperties: false,
      },
      action: readOnlyAction("Link a patch proposal to a session finding."),
      execute: async (input) => {
        const finding = state.session.attachToFinding(input.findingId, "patch", {
          patchId: input.patchId,
          summary: input.summary,
          diffPreview: input.diffPreview ?? null,
        });

        return toolSuccess(finding, `Linked patch ${input.patchId} to ${finding.id}.`);
      },
    }),
  ];
}
