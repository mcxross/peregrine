import { invoke } from "@tauri-apps/api/core";

export type PackageTree = {
  rootPath: string;
  rootName: string;
  paths: string[];
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

export async function readPackageTextFile(
  packageTree: PackageTree,
  relativePath: string,
) {
  return invoke<string>("read_package_text_file", {
    rootPath: packageTree.rootPath,
    relativePath,
  });
}
