import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { generatedFileAction, readOnlyAction } from "../actions";
import { defineAgentTool } from "../define-tool";
import { toolSuccess } from "../executors";
import type { AgentToolRuntimeState } from "../types";

function renderMarkdownReport(state: AgentToolRuntimeState, title: string) {
  const findings = state.session.triageFindings();

  const lines = [
    `# ${title}`,
    "",
    "## Findings",
    findings.length
      ? findings
        .map(
          (finding) =>
            `- [${finding.severity}] ${finding.title}: ${finding.message}${finding.location ? ` (${finding.location})` : ""}`,
        )
        .join("\n")
      : "- No structured findings were recorded in this session.",
    "",
    "## Next actions",
    "- Review tool evidence attached to each finding.",
    "- Re-run validation tools for any unresolved high-severity items.",
  ];

  return lines.join("\n");
}

export function createReportTools(state: AgentToolRuntimeState): DeterministicToolSpec[] {
  return [
    defineAgentTool<{ title?: string }, unknown>({
      id: "rust.report.synthesize",
      title: "Synthesize session evidence",
      description: "Summarize findings and tool evidence recorded in the current agent session.",
      inputSchema: {
        type: "object",
        properties: {
          title: { type: "string" },
        },
        additionalProperties: false,
      },
      action: readOnlyAction("Synthesize evidence recorded in the current session."),
      execute: async (input) => {
        const findings = state.session.triageFindings();
        const summary = {
          title: input?.title ?? "Peregrine agent session",
          findingCount: findings.length,
          findings,
        };

        return toolSuccess(summary, `Synthesized ${findings.length} session findings.`);
      },
    }),
    defineAgentTool<{ title?: string }, { format: string; content: string }>({
      id: "rust.report.generate",
      title: "Generate audit report",
      description: "Generate a markdown audit report from session findings.",
      inputSchema: {
        type: "object",
        properties: {
          title: { type: "string" },
        },
        additionalProperties: false,
      },
      action: generatedFileAction("Generate a markdown audit report draft."),
      execute: async (input) => {
        const markdown = renderMarkdownReport(state, input?.title ?? "Peregrine audit report");

        return toolSuccess(
          { format: "markdown", content: markdown },
          "Generated markdown audit report draft.",
        );
      },
    }),
    defineAgentTool<{ title?: string }, { markdown: string }>({
      id: "rust.report.export_markdown",
      title: "Export markdown report",
      description: "Export the current session findings as markdown text.",
      inputSchema: {
        type: "object",
        properties: {
          title: { type: "string" },
        },
        additionalProperties: false,
      },
      action: generatedFileAction("Export a markdown report for the current session."),
      execute: async (input) => {
        const markdown = renderMarkdownReport(state, input?.title ?? "Peregrine export");

        return toolSuccess({ markdown }, "Exported markdown report.");
      },
    }),
  ];
}
