import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ContextLevel = "Level0" | "Level1" | "Level2" | "Level3" | "Level4";

export type ContextBudget = {
  maxTokensEstimate: number;
  level: ContextLevel;
  maxOperations: number;
  maxCallDepth: number;
  maxCallees: number;
  maxCallers: number;
  maxRelatedTypes: number;
  maxSourceExcerptLines: number;
  includeSource: boolean;
  includeFullSource: boolean;
  includeOperationRawJson: boolean;
  includeRawSummaryJson: boolean;
  includeDiagnostics: boolean;
  includeSemanticTags: boolean;
  includeRelatedTypes: boolean;
  includeCallers: boolean;
  includeCallees: boolean;
  includeReachableGraph: boolean;
  materializeDependencySummaries: boolean;
};

export const defaultContextBudget = (level: ContextLevel = "Level1"): ContextBudget => ({
  maxTokensEstimate: level === "Level0" ? 150 : 500,
  level,
  maxOperations: 25,
  maxCallDepth: 1,
  maxCallees: 8,
  maxCallers: 4,
  maxRelatedTypes: 6,
  maxSourceExcerptLines: 24,
  includeSource: false,
  includeFullSource: false,
  includeOperationRawJson: false,
  includeRawSummaryJson: false,
  includeDiagnostics: true,
  includeSemanticTags: true,
  includeRelatedTypes: true,
  includeCallers: false,
  includeCallees: true,
  includeReachableGraph: false,
  materializeDependencySummaries: false,
});

export type IndexReport = {
  runId: string;
  packageId: string;
  packageName: string;
  dbPath: string;
  status: string;
  summaryArtifactCount: number;
  moduleCount: number;
  functionCount: number;
  typeCount: number;
  operationCount: number;
  diagnosticCount: number;
};

export type PackageOverview = {
  id: string;
  name: string;
  rootPath: string;
  status: string;
  indexedAt: number;
  modules: number;
  functions: number;
  types: number;
  summaryArtifacts: number;
  pointerOnlySummaries: number;
};

export type SymbolResult = {
  id: string;
  kind: string;
  fullName: string;
  visibility: string;
  isEntry: boolean;
};

export type ModuleSummaryCard = {
  artifactId: string;
  packageAlias: string;
  moduleName: string;
  summaryPath: string;
  contentHash: string;
  role: string;
  materializedStatus: string;
  card: unknown | null;
  estimatedTokens: number;
  budgetTokens: number;
  trimmed: boolean;
  trimReasons: string[];
};

export type IndexEventPayload = {
  runId: string;
  message: string;
  packageId: string | null;
};

export function indexPackage(rootPath: string) {
  return invoke<IndexReport>("index_package", { rootPath });
}

export function reindexPackage(packageId: string) {
  return invoke<IndexReport>("reindex_package", { packageId });
}

export function cancelIndex(runId: string) {
  return invoke<boolean>("cancel_index", { runId });
}

export function getPackageOverview(packageId: string) {
  return invoke<PackageOverview>("get_package_overview", { packageId });
}

export function getFunctionContext(functionId: string, budget = defaultContextBudget()) {
  return invoke<unknown>("get_function_context", { functionId, budget });
}

export function getFunctionBody(functionId: string, budget = defaultContextBudget("Level2")) {
  return invoke<unknown>("get_function_body", { functionId, budget });
}

export function getFunctionOperations(functionId: string, budget = defaultContextBudget("Level2")) {
  return invoke<unknown[]>("get_function_operations", { functionId, budget });
}

export function getFunctionCallers(functionId: string, budget = defaultContextBudget()) {
  return invoke<string[]>("get_function_callers", { functionId, budget });
}

export function getFunctionCallees(functionId: string, budget = defaultContextBudget()) {
  return invoke<string[]>("get_function_callees", { functionId, budget });
}

export function getReachableCallees(
  functionId: string,
  depth: number,
  budget = defaultContextBudget("Level3"),
) {
  return invoke<string[]>("get_reachable_callees", { functionId, depth, budget });
}

export function getFunctionFieldReads(functionId: string) {
  return invoke<string[]>("get_function_field_reads", { functionId });
}

export function getFunctionFieldWrites(functionId: string) {
  return invoke<string[]>("get_function_field_writes", { functionId });
}

export function getContextPack(targetId: string, budget = defaultContextBudget()) {
  return invoke<unknown>("get_context_pack", { targetId, budget });
}

export function searchIndexedSymbols(
  packageId: string,
  query: string,
  budget = defaultContextBudget(),
) {
  return invoke<SymbolResult[]>("search_symbols", { packageId, query, budget });
}

export function getOperationsByTag(
  packageId: string,
  tag: string,
  budget = defaultContextBudget("Level2"),
) {
  return invoke<unknown[]>("get_operations_by_tag", { packageId, tag, budget });
}

export function getFunctionsByTag(
  packageId: string,
  tag: string,
  budget = defaultContextBudget(),
) {
  return invoke<SymbolResult[]>("get_functions_by_tag", { packageId, tag, budget });
}

export function getPublicEntryFunctions(packageId: string) {
  return invoke<SymbolResult[]>("get_public_entry_functions", { packageId });
}

export function materializeSummaryModule(
  packageAlias: string,
  moduleName: string,
  budget = defaultContextBudget(),
) {
  return invoke<ModuleSummaryCard>("materialize_summary_module", {
    packageAlias,
    moduleName,
    budget,
  });
}

export function materializeSummarySymbol(
  packageAlias: string,
  moduleName: string,
  symbolName: string,
  budget = defaultContextBudget("Level0"),
) {
  return invoke<ModuleSummaryCard>("materialize_summary_symbol", {
    packageAlias,
    moduleName,
    symbolName,
    budget,
  });
}

export function listenToIndexProgress(
  handler: (event: IndexEventPayload) => void,
) {
  return listen<IndexEventPayload>("index_progress", (event) => handler(event.payload));
}
