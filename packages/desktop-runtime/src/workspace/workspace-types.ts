export const workspaceTabs = [
  "Explore",
  "Agents",
  "Execution",
  "Attack Surface",
  "Tests",
  "Fuzzing",
  "Formal",
  "Audit",
  "CI",
] as const;

export type WorkspaceTab = (typeof workspaceTabs)[number];
export type WorkspaceMode = "security" | "editor";

export type FormalVerificationTarget = {
  packageName: string;
  packagePath: string;
  filePath: string;
  moduleName: string;
};
