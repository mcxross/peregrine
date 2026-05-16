export const workspaceTabs = [
  "Overview",
  "Explore",
  "Execution",
  "Bytecode",
  "Attack Surface",
  "Tests",
  "Fuzzing",
  "Formal",
  "Audit",
  "CI",
] as const;

export type WorkspaceTab = (typeof workspaceTabs)[number];
export type WorkspaceMode = "security" | "editor";
