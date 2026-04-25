import { invoke } from "@tauri-apps/api/core";

export type PackageTree = {
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
  modules: MoveModule[];
};

export type MoveModule = {
  name: string;
  address: string | null;
  filePath: string;
  functions: MoveFunctionSignature[];
};

export type MoveFunctionSignature = {
  name: string;
  visibility: string;
  isEntry: boolean;
  signature: string;
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
  isRoot: boolean;
};

export type PackageDependencyEdge = {
  source: string;
  target: string;
  dependencyCount: number;
};

export type CommandOutput = {
  status: number | null;
  stdout: string;
  stderr: string;
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
