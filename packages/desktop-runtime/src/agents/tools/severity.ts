import type { AnalysisSeverity } from "../../project/filesystem-tree";
import type { AgentFindingSeverity } from "./types";

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
