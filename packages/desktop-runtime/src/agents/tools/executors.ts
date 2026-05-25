import type {
  DeterministicToolExecutionContext,
  DeterministicToolExecutionResult,
} from "@peregrine/agent-runtime";

import {
  analyzeMovePackage,
  buildMovePackage,
  loadMoveBytecodeView,
  loadMoveGraphs,
  loadMoveStateAccessGraph,
  loadPackageTree,
  runFormalVerification,
  runMovyFuzz,
  runSecurityCommand,
  type CommandOutput,
  type MovePackage,
  type PackageTree,
} from "../../project/filesystem-tree";

import type { AgentToolProjectContext, AgentToolRuntimeState } from "./types";

export function resolveProjectPaths(
  context: AgentToolProjectContext,
  input?: { rootPath?: string; packagePath?: string },
) {
  return {
    rootPath: input?.rootPath?.trim() || context.rootPath,
    packagePath: input?.packagePath?.trim() || context.packagePath || ".",
  };
}

export async function resolvePackageTree(
  context: AgentToolProjectContext,
  input?: { rootPath?: string },
): Promise<PackageTree> {
  if (context.packageTree && (!input?.rootPath || input.rootPath === context.rootPath)) {
    return context.packageTree;
  }

  const rootPath = input?.rootPath?.trim() || context.rootPath;
  return loadPackageTree(rootPath);
}

export async function resolveActiveMovePackage(
  context: AgentToolProjectContext,
  input?: { rootPath?: string; packagePath?: string },
): Promise<{ packageTree: PackageTree; movePackage: MovePackage }> {
  const paths = resolveProjectPaths(context, input);
  const packageTree = await resolvePackageTree(context, { rootPath: paths.rootPath });
  const movePackage =
    packageTree.movePackages.find((candidate) => candidate.path === paths.packagePath)
    ?? packageTree.movePackages.find(
      (candidate) => candidate.manifestPath === context.manifestPath,
    )
    ?? packageTree.movePackages[0];

  if (!movePackage) {
    throw new Error("No Move package is available in the active project.");
  }

  return { packageTree, movePackage };
}

export function requireIndexPackageId(state: AgentToolRuntimeState) {
  if (!state.indexPackageId) {
    throw new Error(
      "No active package index. Run rust.index.package before indexer-backed tools.",
    );
  }

  return state.indexPackageId;
}

export function toolSuccess<Output>(
  output: Output,
  summary: string,
  evidence?: DeterministicToolExecutionResult<Output>["evidence"],
): DeterministicToolExecutionResult<Output> {
  return {
    status: "succeeded",
    output,
    summary,
    evidence,
  };
}

export function toolFailure(message: string): DeterministicToolExecutionResult<never> {
  return {
    status: "failed",
    summary: message,
    diagnostics: [
      {
        level: "error",
        source: "agents.tools",
        message,
      },
    ],
  };
}

export function summarizeCommandOutput(output: CommandOutput, label: string) {
  const status = output.status === 0 ? "passed" : "failed";

  return `${label} ${status} (exit ${output.status ?? "unknown"}).`;
}

export async function runMoveBuild(
  state: AgentToolRuntimeState,
  input?: { rootPath?: string; packagePath?: string },
) {
  const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
  const output = await buildMovePackage(packageTree, movePackage.path);

  return {
    output,
    movePackage,
    packageTree,
    summary: summarizeCommandOutput(output, "sui move build"),
  };
}

export async function runMoveTest(
  state: AgentToolRuntimeState,
  input?: { rootPath?: string; packagePath?: string },
) {
  const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
  const output = await runSecurityCommand(packageTree, movePackage.path, "move-test");

  return {
    output,
    movePackage,
    packageTree,
    summary: summarizeCommandOutput(output, "sui move test"),
  };
}

export async function runStaticScan(
  state: AgentToolRuntimeState,
  input?: { rootPath?: string; packagePath?: string },
) {
  const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
  const report = await analyzeMovePackage(packageTree, movePackage.path);

  return {
    report,
    movePackage,
    packageTree,
    summary: `Static analysis returned ${report.findings.length} findings and ${report.diagnostics.length} diagnostics.`,
  };
}

export async function runMovyFuzzPackage(
  state: AgentToolRuntimeState,
  input?: { rootPath?: string; packagePath?: string },
) {
  const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
  const output = await runMovyFuzz(packageTree, movePackage.path);

  return {
    output,
    movePackage,
    packageTree,
    summary: summarizeCommandOutput(output, "movy fuzz"),
  };
}

export async function runFormalCheck(
  state: AgentToolRuntimeState,
  input: {
    moduleName: string;
    filePath?: string;
    rootPath?: string;
    packagePath?: string;
    timeoutSeconds?: number;
  },
) {
  const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
  const module =
    movePackage.modules.find((candidate) => candidate.name === input.moduleName)
    ?? movePackage.modules[0];

  if (!module) {
    throw new Error(`Module ${input.moduleName} was not found in the active package.`);
  }

  const output = await runFormalVerification(
    packageTree,
    movePackage.path,
    input.filePath ?? module.filePath,
    input.moduleName,
    { timeoutSeconds: input.timeoutSeconds ?? 45 },
  );

  return {
    output,
    module,
    movePackage,
    packageTree,
    summary: summarizeCommandOutput(output, "sui-prover"),
  };
}

export async function loadProjectGraphs(
  state: AgentToolRuntimeState,
  input?: { rootPath?: string; packagePath?: string },
) {
  const paths = resolveProjectPaths(state.context, input);
  const graphs = await loadMoveGraphs(paths.rootPath, paths.packagePath);

  return {
    graphs,
    summary: `Loaded graphs with ${graphs.callGraph.nodes.length} call nodes and ${graphs.typeGraph.nodes.length} type nodes.`,
  };
}

export async function loadBytecodePackage(
  state: AgentToolRuntimeState,
  input?: { rootPath?: string; packagePath?: string },
) {
  const { movePackage, packageTree } = await resolveActiveMovePackage(state.context, input);
  const view = await loadMoveBytecodeView(packageTree, movePackage);

  return {
    view,
    movePackage,
    packageTree,
    summary: `Loaded bytecode view for ${view.moduleCount} modules and ${view.functionCount} functions.`,
  };
}

export async function loadStateAccessGraph(
  state: AgentToolRuntimeState,
  input: {
    moduleName: string;
    functionName: string;
    moduleAddress?: string;
    rootPath?: string;
    packagePath?: string;
  },
) {
  const paths = resolveProjectPaths(state.context, input);
  const graph = await loadMoveStateAccessGraph(
    paths.rootPath,
    paths.packagePath,
    input.moduleAddress ?? null,
    input.moduleName,
    input.functionName,
  );

  return {
    graph,
    summary: `Loaded state access graph with ${graph.nodes.length} nodes and ${graph.edges.length} edges.`,
  };
}

export function readMetadataContext(
  context: DeterministicToolExecutionContext,
): Record<string, unknown> {
  return context.metadata ?? {};
}
