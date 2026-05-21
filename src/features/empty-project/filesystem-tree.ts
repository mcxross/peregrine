import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type PackageTree = {
  activePackageManifestPath?: string | null;
  rootPath: string;
  rootName: string;
  isDetailed: boolean;
  paths: string[];
  movePackages: MovePackage[];
  dependencyGraph: PackageDependencyGraph;
  callGraph: MoveCallGraph;
  typeGraph: MoveTypeGraph;
  stateAccessGraph: MoveStateAccessGraph;
  loadReport: ProjectLoadReport;
};

export type ProjectLoadStageStatus =
  | "failed"
  | "passed"
  | "running"
  | "skipped"
  | "warning";

export type ProjectLoadDiagnostic = {
  level: string;
  source: string;
  message: string;
  packageManifestPath: string | null;
};

export type ProjectLoadStageReport = {
  id: string;
  label: string;
  status: ProjectLoadStageStatus;
  diagnostics: ProjectLoadDiagnostic[];
  durationMs: number;
};

export type PackageLoadCapabilities = {
  packageName: string;
  packagePath: string;
  manifestPath: string;
  hasManifest: boolean;
  hasSourceFiles: boolean;
  hasParseableModules: boolean;
  hasPackageSummaries: boolean;
  canShowDependencyGraph: boolean;
  canShowCallGraph: boolean;
  canShowTypeGraph: boolean;
  canShowBytecode: boolean;
  canRunStaticAnalysis: boolean;
};

export type ProjectLoadReport = {
  stages: ProjectLoadStageReport[];
  capabilities: Record<string, PackageLoadCapabilities>;
  analysisReports: Record<string, AnalysisReport>;
};

export type ProjectMetadata = {
  agents?: unknown;
  version: number;
  builds: Record<string, ProjectBuildMetadata>;
  packageConfigs: Record<string, ProjectPackageConfig>;
};

export type ProjectBuildMetadata = {
  lastSuccessfulBuildAt?: number | null;
};

export type ProjectPackageConfig = {
  commands?: ProjectCommandConfig;
};

export type ProjectCommandConfig = {
  moveCoverageScriptPath?: string | null;
  moveTestScriptPath?: string | null;
};

export function defaultProjectMetadata(): ProjectMetadata {
  return {
    agents: undefined,
    builds: {},
    packageConfigs: {},
    version: 1,
  };
}

export function projectPackageConfigKey(movePackage: Pick<MovePackage, "manifestPath" | "path">) {
  return movePackage.manifestPath || movePackage.path || ".";
}

export function projectPackageConfig(
  metadata: ProjectMetadata,
  movePackage: Pick<MovePackage, "manifestPath" | "path">,
) {
  const candidateKeys = [
    projectPackageConfigKey(movePackage),
    movePackage.manifestPath,
    movePackage.path,
    ".",
  ].filter((key): key is string => Boolean(key));

  for (const key of candidateKeys) {
    const config = metadata.packageConfigs?.[key];

    if (config) {
      return config;
    }
  }

  return null;
}

export function projectMoveTestScriptPath(
  metadata: ProjectMetadata,
  movePackage: Pick<MovePackage, "manifestPath" | "path">,
) {
  return projectPackageConfig(metadata, movePackage)?.commands?.moveTestScriptPath?.trim() || null;
}

export function projectMoveCoverageScriptPath(
  metadata: ProjectMetadata,
  movePackage: Pick<MovePackage, "manifestPath" | "path">,
) {
  return projectPackageConfig(metadata, movePackage)?.commands?.moveCoverageScriptPath?.trim() || null;
}

export type MoveProjectGraphs = {
  callGraph: MoveCallGraph;
  typeGraph: MoveTypeGraph;
  stateAccessGraph: MoveStateAccessGraph;
};

export type MovePackage = {
  name: string;
  path: string;
  manifestPath: string;
  hasSourceFiles: boolean;
  hasSourceModules: boolean;
  sourceFileCount: number;
  surface: MovePackageSurface;
  modules: MoveModule[];
};

export function moveSourceUnavailableMessage(
  movePackage: Pick<MovePackage, "hasSourceModules" | "name" | "path" | "sourceFileCount">,
) {
  if (movePackage.hasSourceModules) {
    return null;
  }

  const packagePath = movePackage.path || ".";

  if (movePackage.sourceFileCount === 0) {
    return `Move package ${displayMovePackageName(movePackage.name)} (${packagePath}) contains a Move.toml manifest but no Move source files under sources/. Dependency graph, call graph, type graph, and bytecode views require parseable source modules.`;
  }

  return `Move package ${displayMovePackageName(movePackage.name)} (${packagePath}) contains ${movePackage.sourceFileCount} Move source ${movePackage.sourceFileCount === 1 ? "file" : "files"}, but no parseable Move modules were found. The source may be commented out or invalid. Dependency graph, call graph, type graph, and bytecode views require parseable source modules.`;
}

export function displayMovePackageName(packageName: string) {
  const normalizedName = packageName.replace(/^onchain_/, "");
  const hexPrefix = normalizedName.startsWith("0x") || normalizedName.startsWith("0X")
    ? normalizedName.slice(0, 2)
    : "";
  const hex = hexPrefix ? normalizedName.slice(2) : normalizedName;

  if (hex.length > 18 && /^[0-9a-fA-F]+$/.test(hex)) {
    return `${hexPrefix}${hex.slice(0, 8)}…${hex.slice(-6)}`;
  }

  return normalizedName;
}

export type MovePackageSurface = {
  entryFunctionCount: number;
  capabilityCount: number;
  sharedObjectCount: number;
  addressOwnedObjectCount: number;
  immutableObjectCount: number;
  wrappedObjectCount: number;
  partyObjectCount: number;
  adminControlCount: number;
  externalCallCount: number;
  publicPackageRelationshipCount: number;
  capabilityStructs: string[];
  capabilityFindings: CapabilityFinding[];
  sharedObjectStructs: string[];
  objectLifecycleMaps: ObjectLifecycleMap[];
  objectOwnershipFindings: ObjectOwnershipFinding[];
  adminControlFindings: AdminControlFinding[];
  externalCallFindings: ExternalCallFinding[];
  publicPackageRelationships: PublicPackageRelationship[];
};

export type ObjectLifecycleStageKind =
  | "created"
  | "owned"
  | "mutated"
  | "transferred"
  | "shared"
  | "wrapped"
  | "immutable"
  | "party"
  | "deleted";

export type ObjectLifecycleMap = {
  typeName: string;
  moduleName: string;
  qualifiedName: string;
  filePath: string;
  abilities: string[];
  isCapabilityLike: boolean;
  stages: ObjectLifecycleStage[];
  touchedBy: ObjectLifecycleFunctionRef[];
  risks: ObjectLifecycleRisk[];
};

export type ObjectLifecycleStage = {
  kind: ObjectLifecycleStageKind;
  functions: ObjectLifecycleFunctionRef[];
  evidence: string[];
};

export type ObjectLifecycleFunctionRef = {
  moduleName: string;
  functionName: string;
  qualifiedName: string;
  filePath: string;
  visibility: string;
  isEntry: boolean;
  isTransactionCallable: boolean;
  direct: boolean;
  callPath: string[];
  evidence: string[];
};

export type ObjectLifecycleRisk = {
  kind: string;
  severity: "high" | "medium" | "low" | string;
  message: string;
  evidence: string[];
  functions: ObjectLifecycleFunctionRef[];
};

export type CapabilityFinding = {
  typeName: string;
  moduleName: string;
  qualifiedName: string;
  confidence: "high" | "medium" | "low";
  evidence: string[];
  protectedFunctions: string[];
};

export type ObjectOwnershipFinding = {
  typeName: string;
  moduleName: string;
  qualifiedName: string;
  ownershipKind: "shared" | "addressOwned" | "immutable" | "wrapped" | "party";
  confidence: "high" | "medium" | "low";
  evidence: string[];
  relatedFunctions: string[];
  wrappedTypes: string[];
};

export type AdminControlFinding = {
  functionName: string;
  moduleName: string;
  qualifiedName: string;
  confidence: "high" | "medium" | "low";
  evidence: string[];
  guardingTypes: string[];
};

export type ExternalCallFinding = {
  callerModule: string;
  callerFunction: string;
  targetModule: string;
  targetFunction: string;
  target: string;
};

export type PublicPackageRelationship = {
  sourceModule: string;
  sourceFunction: string;
  targetModule: string;
  targetFunction: string;
};

export type MoveModule = {
  name: string;
  address: string | null;
  filePath: string;
  attributes: string[];
  structs: MoveStructSignature[];
  functions: MoveFunctionSignature[];
};

export type MoveStructSignature = {
  name: string;
  abilities: string[];
  fields: MoveStructField[];
  signature: string;
  attributes: string[];
};

export type MoveStructField = {
  name: string;
  typeName: string;
};

export type MoveFunctionSignature = {
  name: string;
  visibility: string;
  isEntry: boolean;
  isTransactionCallable: boolean;
  signature: string;
  body: string | null;
  attributes: string[];
};

export type PackageDependencyGraph = {
  root: string | null;
  nodes: PackageDependencyNode[];
  edges: PackageDependencyEdge[];
  summaryPath: string | null;
};

export type PackageDependencyNode = {
  id: string;
  address: string | null;
  moduleCount: number;
  publicFunctionCount: number;
  entryFunctionCount: number;
  isRoot: boolean;
};

export type PackageDependencyEdge = {
  source: string;
  target: string;
  dependencyCount: number;
  dependencyKind: string;
};

export type MoveSourceSpan = {
  filePath: string;
  startLine: number;
  endLine: number;
  startByte: number;
  endByte: number;
};

export type MoveCallGraph = {
  nodes: MoveCallGraphNode[];
  edges: MoveCallGraphEdge[];
  unresolvedCalls: MoveUnresolvedCall[];
};

export type MoveCallGraphNode = {
  id: string;
  packageName: string | null;
  packagePath: string | null;
  address: string | null;
  moduleName: string;
  functionName: string;
  qualifiedName: string;
  filePath: string | null;
  visibility: string;
  isEntry: boolean;
  isTransactionCallable: boolean;
  attributes: string[];
  signature: string | null;
  span: MoveSourceSpan | null;
  isExternal: boolean;
  source: string;
};

export type MoveCallGraphEdge = {
  source: string;
  target: string;
  callKind: string;
  confidence: string;
  callCount: number;
  rawTarget: string;
  typeArguments: string[];
  sourceSpans: MoveSourceSpan[];
  isExternal: boolean;
  isResolved: boolean;
};

export type MoveUnresolvedCall = {
  source: string;
  rawTarget: string;
  callKind: string;
  filePath: string;
  spans: MoveSourceSpan[];
  reason: string;
};

export type MoveTypeGraph = {
  nodes: MoveTypeGraphNode[];
  edges: MoveTypeGraphEdge[];
  unresolvedTypes: MoveUnresolvedType[];
};

export type MoveTypeGraphNode = {
  id: string;
  kind: string;
  packageName: string | null;
  packagePath: string | null;
  address: string | null;
  canonicalAddress: string | null;
  moduleName: string | null;
  name: string;
  qualifiedName: string;
  filePath: string | null;
  abilities: string[];
  typeParameters: MoveTypeParameter[];
  attributes: string[];
  span: MoveSourceSpan | null;
  source: string;
  isExternal: boolean;
};

export type MoveTypeParameter = {
  name: string;
  abilities: string[];
  isPhantom: boolean;
};

export type MoveTypeGraphEdge = {
  source: string;
  target: string;
  relationship: string;
  fieldName: string | null;
  variantName: string | null;
  functionName: string | null;
  parameterName: string | null;
  typeArgumentIndex: number | null;
  isMutable: boolean;
  isReference: boolean;
  typeExpression: string | null;
  declaringTypeId: string | null;
  declaringFieldName: string | null;
  typeArgumentName: string | null;
  sourceSpans: MoveSourceSpan[];
  confidence: string;
  evidence: string[];
};

export type MoveUnresolvedType = {
  source: string;
  rawType: string;
  context: string;
  filePath: string;
  spans: MoveSourceSpan[];
  reason: string;
};

export type MoveStateAccessGraph = {
  nodes: MoveStateAccessGraphNode[];
  edges: MoveStateAccessGraphEdge[];
  unresolvedAccesses: MoveUnresolvedStateAccess[];
};

export type MoveBytecodePackageView = {
  packageName: string;
  packagePath: string;
  buildPath: string;
  moduleCount: number;
  functionCount: number;
  instructionCount: number;
  structCount: number;
  constantCount: number;
  dependencyCount: number;
  sourceMapCount: number;
  modules: MoveBytecodeModuleView[];
};

export type MoveBytecodeModuleView = {
  name: string;
  address: string;
  packageName: string;
  isDependency: boolean;
  bytecodePath: string;
  sourceMapPath: string | null;
  sourcePath: string | null;
  byteSize: number;
  version: number;
  functionCount: number;
  instructionCount: number;
  structCount: number;
  constantCount: number;
  importCount: number;
  friendCount: number;
  functions: MoveBytecodeFunctionView[];
  imports: string[];
  disassembly: string;
};

export type MoveBytecodeFunctionView = {
  name: string;
  visibility: string;
  isEntry: boolean;
  parameters: string[];
  returns: string[];
  typeParameterCount: number;
  instructionCount: number;
  localCount: number;
  returnCount: number;
  acquires: string[];
  instructions: MoveBytecodeInstructionView[];
  controlFlow: MoveBytecodeControlFlowView;
};

export type MoveBytecodeInstructionView = {
  offset: number;
  opcode: string;
  detail: string;
  call: MoveBytecodeCallView | null;
  source: MoveBytecodeSourceSpan | null;
};

export type MoveBytecodeCallView = {
  handleIndex: number;
  moduleAddress: string;
  moduleName: string;
  functionName: string;
  qualifiedName: string;
  genericTypeArguments: string[];
};

export type MoveBytecodeControlFlowView = {
  blocks: MoveBytecodeBasicBlockView[];
  edges: MoveBytecodeControlFlowEdgeView[];
};

export type MoveBytecodeBasicBlockView = {
  id: string;
  label: string;
  startOffset: number;
  endOffset: number;
  instructionOffsets: number[];
};

export type MoveBytecodeControlFlowEdgeView = {
  source: string;
  target: string;
  sourceOffset: number;
  targetOffset: number;
  kind: string;
};

export type MoveBytecodeSourceSpan = {
  startByte: number;
  endByte: number;
};

export type MoveStateAccessGraphNode = {
  id: string;
  kind: "function" | "stateType" | "field" | string;
  packageName: string | null;
  packagePath: string | null;
  address: string | null;
  moduleName: string | null;
  name: string;
  qualifiedName: string;
  filePath: string | null;
  abilities: string[];
  span: MoveSourceSpan | null;
  isExternal: boolean;
  source: string;
};

export type MoveStateAccessGraphEdge = {
  source: string;
  target: string;
  accessKind: string;
  fieldName: string | null;
  viaFunction: string | null;
  sourceSpans: MoveSourceSpan[];
  confidence: string;
  evidence: string[];
};

export type MoveUnresolvedStateAccess = {
  source: string;
  rawTarget: string;
  accessKind: string;
  filePath: string;
  spans: MoveSourceSpan[];
  reason: string;
};

export type AnalysisSeverity = "info" | "warning" | "error";

export type AnalysisSpan = {
  startLine: number;
  endLine: number;
};

export type AnalysisMetric = {
  name: string;
  value: number;
  threshold: number | null;
};

export type AnalysisFinding = {
  ruleId: string;
  rulesetId: string;
  severity: AnalysisSeverity;
  message: string;
  file: string;
  span: AnalysisSpan | null;
  metric: AnalysisMetric | null;
};

export type AnalysisRuleMetric = {
  rulesetId: string;
  ruleId: string;
  target: string;
  file: string | null;
  span: AnalysisSpan | null;
  metric: AnalysisMetric;
};

export type AnalysisDiagnostic = {
  level: string;
  source: string;
  message: string;
};

export type AnalysisReport = {
  findings: AnalysisFinding[];
  metrics: AnalysisRuleMetric[];
  loadedRulesets: string[];
  loadedPlugins: string[];
  diagnostics: AnalysisDiagnostic[];
};

export type AnalysisRuleConfigValueKind = "boolean" | "integer" | "string" | "severity";

export type AnalysisRuleConfigProperty = {
  key: string;
  valueKind: AnalysisRuleConfigValueKind;
  description: string;
  defaultValue: string | null;
};

export type AnalysisRuleMetadata = {
  id: string;
  name: string;
  description: string;
  active: boolean;
  defaultSeverity: AnalysisSeverity;
  configuredSeverity: AnalysisSeverity | null;
  configSchema: AnalysisRuleConfigProperty[];
};

export type AnalysisRuleSetMetadata = {
  id: string;
  name: string;
  description: string;
  bundled: boolean;
  pluginId: string | null;
  active: boolean;
  rules: AnalysisRuleMetadata[];
};

export type AnalysisRuleCatalog = {
  rulesets: AnalysisRuleSetMetadata[];
  loadedPlugins: string[];
  diagnostics: AnalysisDiagnostic[];
};

export type AnalyzerPluginManifest = {
  schemaVersion: number;
  pluginId: string;
  version: string;
  name?: string | null;
  description?: string | null;
  rulesets: {
    id: string;
    name?: string | null;
    description?: string | null;
    rules: {
      id: string;
      name?: string | null;
      description?: string | null;
      defaultSeverity?: AnalysisSeverity | null;
      configKeys: string[];
    }[];
  }[];
};

export type InstalledAnalyzerPlugin = {
  pluginId: string;
  version: string;
  kind: string;
  runtime: "wasm" | "native";
  path: string;
  checksum: string;
  enabled: boolean;
  installedAtUnixMs: number;
  name?: string | null;
  description?: string | null;
  manifest: AnalyzerPluginManifest;
};

export type InstalledPlugin = Omit<InstalledAnalyzerPlugin, "manifest"> & {
  manifest: unknown;
};

export type AnalysisRuleConfigPatch = {
  rulesetId: string;
  ruleId?: string | null;
  active?: boolean | null;
  severity?: AnalysisSeverity | null;
  threshold?: number | null;
  entryThreshold?: number | null;
};

export type CommandOutput = {
  status: number | null;
  stdout: string;
  stderr: string;
};

export type CommandOutputStreamOptions = {
  streamId?: number | string;
  onOutput?: (output: CommandOutput) => void;
};

export type CommandScriptOutputStreamOptions = CommandOutputStreamOptions & {
  args?: string[];
};

type CommandOutputChunk = {
  streamId: string;
  stream: "stderr" | "stdout";
  chunk: string;
};

const COMMAND_OUTPUT_EVENT = "command-output";
const PROJECT_METADATA_CHANGED_EVENT = "project-metadata-changed";
const SUI_ADAPTER_SETTINGS_CHANGED_EVENT = "sui-adapter-settings-changed";

export type SecurityCommandKind =
  | "move-coverage"
  | "move-coverage-summary"
  | "move-fuzz"
  | "move-test"
  | "publish-dry-run-localnet"
  | "publish-dry-run-devnet"
  | "publish-dry-run-testnet"
  | "publish-dry-run-mainnet"
  | "publish-localnet"
  | "publish-devnet"
  | "publish-testnet"
  | "publish-mainnet";

export type SuiAdapterStatus = {
  installed: boolean;
  version: string | null;
  installHint: string | null;
  activeSource: SuiAdapterSource | null;
  preferredSource: SuiAdapterSource;
  resolvedPath: string | null;
  bundled: SuiAdapterSourceStatus;
  system: SuiAdapterSourceStatus;
};

export type SuiAdapterSource = "bundled" | "system";

export type SuiAdapterSettings = {
  cliPath?: string | null;
  source: SuiAdapterSource;
};

export type SuiAdapterSourceStatus = {
  source: SuiAdapterSource;
  available: boolean;
  version: string | null;
  path: string | null;
  error: string | null;
};

export type SuiKeyConfigStatus = "missing" | "loaded" | "invalid";

export type SuiKeyDiagnosticLevel = "error" | "warning" | "info";

export type SuiKeyDiagnostic = {
  level: SuiKeyDiagnosticLevel;
  message: string;
  path?: string | null;
};

export type SuiKeyAccount = {
  address: string;
  alias?: string | null;
  canExportPrivateKey: boolean;
  canRemove: boolean;
  flag: number;
  isActive: boolean;
  isExternal: boolean;
  keyScheme: string;
  peerId?: string | null;
  publicBase64Key: string;
};

export type SuiKeyState = {
  accounts: SuiKeyAccount[];
  activeAddress?: string | null;
  activeEnv?: string | null;
  aliasesPath: string;
  clientConfigPath: string;
  configDir: string;
  configStatus: SuiKeyConfigStatus;
  diagnostics: SuiKeyDiagnostic[];
  externalKeystorePath: string;
  keystorePath: string;
  supportedSchemes: string[];
  supportedWordLengths: string[];
};

export type SuiGenerateKeyRequest = {
  alias?: string | null;
  derivationPath?: string | null;
  keyScheme: string;
  revealRecoveryPhrase: boolean;
  wordLength?: string | null;
};

export type SuiGenerateKeyResponse = {
  generated: SuiKeyAccount;
  keyScheme: string;
  recoveryPhrase?: string | null;
  state: SuiKeyState;
};

export type SuiImportKeyRequest = {
  alias?: string | null;
  derivationPath?: string | null;
  inputString: string;
  keyScheme: string;
};

export type SuiImportKeyResponse = {
  imported: SuiKeyAccount;
  state: SuiKeyState;
};

export type SuiExportPrivateKeyResponse = {
  account: SuiKeyAccount;
  exportedPrivateKey: string;
};

export type SuiWalletSummary = {
  activeAddress?: string | null;
  activeAlias?: string | null;
  balance?: SuiBalanceSummary | null;
  balanceError?: string | null;
};

export type SuiBalanceSummary = {
  coinType: string;
  totalBalanceMist: string;
};

export type FilePreview =
  | {
      kind: "text";
      path: string;
      language: string;
      source: string;
      highlightedHtml: string;
    }
  | {
      kind: "markdown";
      path: string;
      source: string;
      html: string;
    }
  | {
      kind: "image";
      path: string;
      mime: string;
      dataUrl: string;
    }
  | {
      kind: "video";
      path: string;
      mime: string;
      dataUrl: string;
    }
  | {
      kind: "unsupported";
      path: string;
      reason: string;
      size: number;
    };

export async function loadPackageTree(rootPath: string): Promise<PackageTree> {
  return invoke<PackageTree>("load_package_tree", { rootPath });
}

export async function loadPackageTreeDetails(rootPath: string): Promise<PackageTree> {
  return invoke<PackageTree>("load_package_tree_details", { rootPath });
}

export async function loadMoveGraphs(
  rootPath: string,
  packagePath?: string,
): Promise<MoveProjectGraphs> {
  return invoke<MoveProjectGraphs>("load_move_graphs", {
    packagePath: packagePath ?? null,
    rootPath,
  });
}


export async function loadMoveStateAccessGraph(
  rootPath: string,
  packagePath: string,
  moduleAddress: string | null,
  moduleName: string,
  functionName: string,
): Promise<MoveStateAccessGraph> {
  return invoke<MoveStateAccessGraph>("load_move_state_access_graph", {
    functionName,
    moduleAddress,
    moduleName,
    packagePath,
    rootPath,
  });
}

export async function createMoveProject(
  parentPath: string,
  projectName: string,
): Promise<PackageTree> {
  return invoke<PackageTree>("create_move_project", {
    parentPath,
    projectName,
  });
}

export async function importMovePackageById(
  packageId: string,
  networkId: string,
  graphQlUrl: string,
  saveRootPath: string | null,
  generateBuildable: boolean,
): Promise<PackageTree> {
  return invoke<PackageTree>("import_move_package_by_id", {
    generateBuildable,
    graphQlUrl,
    networkId,
    packageId,
    saveRootPath,
  });
}

export async function moveProjectPathExists(
  parentPath: string,
  projectName: string,
): Promise<boolean> {
  return invoke<boolean>("move_project_path_exists", {
    parentPath,
    projectName,
  });
}

export async function loadMoveBytecodeView(
  packageTree: PackageTree,
  movePackage: MovePackage,
): Promise<MoveBytecodePackageView> {
  return invoke<MoveBytecodePackageView>("load_move_bytecode_view", {
    rootPath: packageTree.rootPath,
    packagePath: movePackage.path,
    packageName: movePackage.name,
  });
}

export function isDirectoryPath(path: string) {
  return path.endsWith("/");
}

export function resolvePackagePath(packageTree: PackageTree, relativePath: string) {
  const normalizedRelativePath = relativePath.replace(/\/$/, "");

  return `${packageTree.rootPath}/${normalizedRelativePath}`;
}

export async function loadFilePreview(
  packageTree: PackageTree,
  relativePath: string,
  options?: { includeHighlightedHtml?: boolean },
) {
  return invoke<FilePreview>("load_file_preview", {
    includeHighlightedHtml: options?.includeHighlightedHtml ?? true,
    rootPath: packageTree.rootPath,
    relativePath,
  });
}

export async function saveTextFile(
  packageTree: PackageTree,
  relativePath: string,
  contents: string,
  options?: { includeHighlightedHtml?: boolean },
) {
  return invoke<FilePreview>("save_text_file", {
    rootPath: packageTree.rootPath,
    relativePath,
    contents,
    includeHighlightedHtml: options?.includeHighlightedHtml ?? true,
  });
}

export async function buildMovePackage(
  packageTree: PackageTree,
  packagePath: string,
  options?: CommandOutputStreamOptions,
) {
  return invokeCommandOutput("build_move_package", {
    rootPath: packageTree.rootPath,
    packagePath,
  }, options);
}

export async function runSecurityCommand(
  packageTree: PackageTree,
  packagePath: string,
  commandKind: SecurityCommandKind,
  options?: CommandOutputStreamOptions,
) {
  return invokeCommandOutput("run_security_command", {
    rootPath: packageTree.rootPath,
    packagePath,
    commandKind,
  }, options);
}

export async function runMovyFuzz(
  packageTree: PackageTree,
  packagePath: string,
  options?: CommandOutputStreamOptions,
) {
  return invokeCommandOutput("run_movy_fuzz", {
    rootPath: packageTree.rootPath,
    packagePath,
  }, options);
}

export async function runFormalVerification(
  packageTree: PackageTree,
  packagePath: string,
  filePath: string,
  moduleName: string,
  options?: CommandOutputStreamOptions & { timeoutSeconds?: number },
) {
  return invokeCommandOutput("run_formal_verification", {
    rootPath: packageTree.rootPath,
    packagePath,
    filePath,
    moduleName,
    timeoutSeconds: options?.timeoutSeconds ?? null,
  }, options);
}

export async function runSecurityScript(
  packageTree: PackageTree,
  packagePath: string,
  scriptPath: string,
  options?: CommandScriptOutputStreamOptions,
) {
  return invokeCommandOutput("run_security_script", {
    rootPath: packageTree.rootPath,
    packagePath,
    scriptArgs: options?.args ?? [],
    scriptPath,
  }, options);
}

export async function analyzeMovePackage(
  packageTree: PackageTree,
  packagePath: string,
) {
  return invoke<AnalysisReport>("analyze_move_package", {
    rootPath: packageTree.rootPath,
    packagePath,
  });
}

export async function listAnalyzerPlugins() {
  return invoke<InstalledAnalyzerPlugin[]>("list_analyzer_plugins");
}

export async function installAnalyzerPlugin(pluginPath: string) {
  return invoke<InstalledAnalyzerPlugin>("install_analyzer_plugin", { pluginPath });
}

export async function removeAnalyzerPlugin(pluginId: string) {
  return invoke<InstalledAnalyzerPlugin[]>("remove_analyzer_plugin", { pluginId });
}

export async function setAnalyzerPluginEnabled(pluginId: string, enabled: boolean) {
  return invoke<InstalledAnalyzerPlugin[]>("set_analyzer_plugin_enabled", {
    enabled,
    pluginId,
  });
}

export async function listPlugins(kind?: string | null) {
  return invoke<InstalledPlugin[]>("list_plugins", { kind });
}

export async function removePlugin(kind: string, pluginId: string) {
  return invoke<InstalledPlugin[]>("remove_plugin", { kind, pluginId });
}

export async function setPluginEnabled(kind: string, pluginId: string, enabled: boolean) {
  return invoke<InstalledPlugin[]>("set_plugin_enabled", {
    enabled,
    kind,
    pluginId,
  });
}

export async function listAnalyzerRuleCatalog(
  packageTree?: PackageTree | null,
  packagePath?: string | null,
) {
  return invoke<AnalysisRuleCatalog>("list_analyzer_rule_catalog", {
    packagePath: packagePath ?? null,
    rootPath: packageTree?.rootPath ?? null,
  });
}

export async function saveAnalysisRuleConfig(
  packageTree: PackageTree,
  packagePath: string,
  patch: AnalysisRuleConfigPatch,
) {
  return invoke("save_analysis_rule_config", {
    packagePath,
    patch,
    rootPath: packageTree.rootPath,
  });
}

export async function checkSuiAdapter() {
  return invoke<SuiAdapterStatus>("check_sui_adapter");
}

export async function getSuiAdapterSettings() {
  return invoke<SuiAdapterSettings>("get_sui_adapter_settings");
}

export async function saveSuiAdapterSettings(settings: SuiAdapterSettings) {
  return invoke<SuiAdapterSettings>("save_sui_adapter_settings", { settings });
}

export async function loadSuiKeyState() {
  return invoke<SuiKeyState>("load_sui_key_state");
}

export async function loadSuiWalletSummary(graphQlUrl: string | null) {
  return invoke<SuiWalletSummary>("load_sui_wallet_summary", {
    request: {
      graphQlUrl,
    },
  });
}

export async function generateSuiKey(request: SuiGenerateKeyRequest) {
  return invoke<SuiGenerateKeyResponse>("generate_sui_key", { request });
}

export async function importSuiKey(request: SuiImportKeyRequest) {
  return invoke<SuiImportKeyResponse>("import_sui_key", { request });
}

export async function renameSuiKeyAlias(aliasOrAddress: string, newAlias: string) {
  return invoke<SuiKeyState>("rename_sui_key_alias", {
    request: {
      aliasOrAddress,
      newAlias,
    },
  });
}

export async function setActiveSuiAddress(aliasOrAddress: string) {
  return invoke<SuiKeyState>("set_active_sui_address", {
    request: {
      aliasOrAddress,
    },
  });
}

export async function removeSuiKey(aliasOrAddress: string, confirmation: string) {
  return invoke<SuiKeyState>("remove_sui_key", {
    request: {
      aliasOrAddress,
      confirmation,
    },
  });
}

export async function exportSuiPrivateKey(aliasOrAddress: string, confirmation: string) {
  return invoke<SuiExportPrivateKeyResponse>("export_sui_private_key", {
    request: {
      aliasOrAddress,
      confirmation,
    },
  });
}

export async function loadProjectMetadata(rootPath: string) {
  return invoke<ProjectMetadata>("load_project_metadata", { rootPath });
}

export async function saveProjectMetadata(rootPath: string, metadata: ProjectMetadata) {
  const savedMetadata = await invoke<ProjectMetadata>("save_project_metadata", { rootPath, metadata });

  window.dispatchEvent(new CustomEvent<ProjectMetadataChangedDetail>(
    PROJECT_METADATA_CHANGED_EVENT,
    {
      detail: {
        metadata: savedMetadata,
        rootPath,
      },
    },
  ));

  return savedMetadata;
}

export type ProjectMetadataChangedDetail = {
  metadata: ProjectMetadata;
  rootPath: string;
};

export function listenProjectMetadataChanged(
  onProjectMetadataChanged: (detail: ProjectMetadataChangedDetail) => void,
) {
  const listener = (event: Event) => {
    onProjectMetadataChanged((event as CustomEvent<ProjectMetadataChangedDetail>).detail);
  };

  window.addEventListener(PROJECT_METADATA_CHANGED_EVENT, listener);

  return () => window.removeEventListener(PROJECT_METADATA_CHANGED_EVENT, listener);
}

export async function listenSuiAdapterSettingsChanged(
  onSettingsChanged: (settings: SuiAdapterSettings) => void,
) {
  return listen<SuiAdapterSettings>(SUI_ADAPTER_SETTINGS_CHANGED_EVENT, (event) => {
    onSettingsChanged(event.payload);
  });
}

async function invokeCommandOutput(
  command: string,
  args: Record<string, unknown>,
  options?: CommandOutputStreamOptions,
) {
  const streamId = options?.streamId == null ? null : String(options.streamId);
  const output: CommandOutput = { status: null, stderr: "", stdout: "" };
  const onOutput = options?.onOutput;
  let scheduledOutputFlush: ReturnType<typeof globalThis.setTimeout> | number | null = null;
  let scheduledOutputFlushKind: "animation-frame" | "timeout" | null = null;
  let hasPendingOutput = false;
  const flushOutput = () => {
    scheduledOutputFlush = null;
    scheduledOutputFlushKind = null;

    if (!hasPendingOutput) {
      return;
    }

    hasPendingOutput = false;
    onOutput?.({ ...output });
  };
  const scheduleOutputFlush = () => {
    if (!onOutput) {
      return;
    }

    hasPendingOutput = true;

    if (scheduledOutputFlush != null) {
      return;
    }

    scheduledOutputFlush = typeof window !== "undefined" && "requestAnimationFrame" in window
      ? window.requestAnimationFrame(flushOutput)
      : globalThis.setTimeout(flushOutput, 16);
    scheduledOutputFlushKind = typeof window !== "undefined" && "requestAnimationFrame" in window
      ? "animation-frame"
      : "timeout";
  };
  const unlisten = streamId && onOutput
    ? await listen<CommandOutputChunk>(COMMAND_OUTPUT_EVENT, (event) => {
        if (event.payload.streamId !== streamId) {
          return;
        }

        if (event.payload.stream === "stdout") {
          output.stdout += event.payload.chunk;
        } else {
          output.stderr += event.payload.chunk;
        }

        scheduleOutputFlush();
      })
    : null;

  try {
    return await invoke<CommandOutput>(command, {
      ...args,
      streamId,
    });
  } finally {
    if (scheduledOutputFlush != null) {
      if (scheduledOutputFlushKind === "animation-frame" && typeof window !== "undefined") {
        window.cancelAnimationFrame(Number(scheduledOutputFlush));
      } else {
        globalThis.clearTimeout(scheduledOutputFlush);
      }
      scheduledOutputFlush = null;
      scheduledOutputFlushKind = null;
    }
    flushOutput();
    unlisten?.();
  }
}
