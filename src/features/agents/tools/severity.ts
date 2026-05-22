import type { AnalysisSeverity } from "@/features/empty-project/filesystem-tree";
import type { AgentFindingSeverity } from "@/features/agents/tools/types";

export function mapAnalysisSeverity(severity: AnalysisSeverity): AgentFindingSeverity {
  switch (severity) {
    case "error":
      return "high";
    case "warning":
      return "medium";
    case "info":
    default:
      return "info";
  }
}
