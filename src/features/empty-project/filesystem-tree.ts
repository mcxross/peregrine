import { invoke } from "@tauri-apps/api/core";

export type PackageTree = {
  activePackageManifestPath?: string | null;
  rootPath: string;
  rootName: string;
  paths: string[];
  movePackages: MovePackage[];
  dependencyGraph: PackageDependencyGraph;
};

export type MovePackage = {
  name: string;
  path: string;
  manifestPath: string;
  surface: MovePackageSurface;
  modules: MoveModule[];
};

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
  objectOwnershipFindings: ObjectOwnershipFinding[];
  adminControlFindings: AdminControlFinding[];
  externalCallFindings: ExternalCallFinding[];
  publicPackageRelationships: PublicPackageRelationship[];
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
  structs: MoveStructSignature[];
  functions: MoveFunctionSignature[];
};

export type MoveStructSignature = {
  name: string;
  abilities: string[];
  fields: MoveStructField[];
  signature: string;
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

export type CommandOutput = {
  status: number | null;
  stdout: string;
  stderr: string;
};

export type SecurityCommandKind =
  | "move-coverage"
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

export type SuiCliStatus = {
  installed: boolean;
  version: string | null;
  installHint: string | null;
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
) {
  return invoke<FilePreview>("load_file_preview", {
    rootPath: packageTree.rootPath,
    relativePath,
  });
}

export async function saveTextFile(
  packageTree: PackageTree,
  relativePath: string,
  contents: string,
) {
  return invoke<FilePreview>("save_text_file", {
    rootPath: packageTree.rootPath,
    relativePath,
    contents,
  });
}

export async function buildMovePackage(
  packageTree: PackageTree,
  packagePath: string,
) {
  return invoke<CommandOutput>("build_move_package", {
    rootPath: packageTree.rootPath,
    packagePath,
  });
}

export async function runSecurityCommand(
  packageTree: PackageTree,
  packagePath: string,
  commandKind: SecurityCommandKind,
) {
  return invoke<CommandOutput>("run_security_command", {
    rootPath: packageTree.rootPath,
    packagePath,
    commandKind,
  });
}

export async function runSecurityScript(
  packageTree: PackageTree,
  packagePath: string,
  scriptPath: string,
) {
  return invoke<CommandOutput>("run_security_script", {
    rootPath: packageTree.rootPath,
    packagePath,
    scriptPath,
  });
}

export async function checkSuiCli() {
  return invoke<SuiCliStatus>("check_sui_cli");
}
