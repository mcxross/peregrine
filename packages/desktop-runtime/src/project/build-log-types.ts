import type { CommandOutput } from "./filesystem-tree";

export type BuildRunState = "running" | "success" | "error";

export type BuildLogRun = {
  canRerun?: boolean;
  command: string;
  error: string | null;
  emptyText?: string;
  finishedAt: Date | null;
  id: number;
  metadata?: { label: string; value: string }[];
  note?: string | null;
  output: CommandOutput | null;
  packageName: string;
  packagePath: string;
  runningText?: string;
  startedAt: Date;
  state: BuildRunState;
  title?: string;
  workingDirectory: string;
};

export type BuildLogUpdateOptions = {
  open?: boolean;
  reset?: boolean;
};
