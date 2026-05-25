import type {
  MoveModule,
  MovePackage,
} from "./filesystem-tree";

export type MoveModuleMatch = {
  moveModule: MoveModule;
  movePackage: MovePackage;
};

export function findSourceModule(
  movePackages: MovePackage[],
  location: { filePath: string },
): MoveModuleMatch | null {
  for (const movePackage of movePackages) {
    const moveModule = movePackage.modules.find((module) =>
      sameSourcePath(module.filePath, location.filePath, movePackage.path),
    );

    if (moveModule) {
      return { moveModule, movePackage };
    }
  }

  return null;
}

export function findModuleByPath(
  movePackages: MovePackage[],
  path: string,
  preferredPackage: MovePackage | null,
): MoveModuleMatch | null {
  const packages = preferredPackage
    ? [
        preferredPackage,
        ...movePackages.filter(
          (movePackage) =>
            movePackage.manifestPath !== preferredPackage.manifestPath,
        ),
      ]
    : movePackages;

  for (const movePackage of packages) {
    const moveModule = movePackage.modules.find((module) =>
      sameSourcePath(module.filePath, path, movePackage.path),
    );

    if (moveModule) {
      return { moveModule, movePackage };
    }
  }

  return null;
}

export function sameSourcePath(
  moduleFilePath: string,
  requestedFilePath: string,
  packagePath: string,
) {
  const normalizedModulePath = normalizeFilePath(moduleFilePath);
  const normalizedRequestedPath = normalizeFilePath(requestedFilePath);
  const normalizedPackagePath = normalizeFilePath(packagePath);
  const requestedRelativeToPackage =
    normalizedPackagePath &&
    normalizedRequestedPath.startsWith(`${normalizedPackagePath}/`)
      ? normalizedRequestedPath.slice(normalizedPackagePath.length + 1)
      : normalizedRequestedPath;

  return (
    normalizedModulePath === normalizedRequestedPath ||
    normalizedModulePath === requestedRelativeToPackage ||
    normalizedModulePath.endsWith(`/${requestedRelativeToPackage}`) ||
    requestedRelativeToPackage.endsWith(`/${normalizedModulePath}`)
  );
}

function normalizeFilePath(filePath: string | null | undefined) {
  return (filePath ?? "")
    .replace(/\\/g, "/")
    .replace(/^\.\//, "")
    .replace(/\/$/, "");
}
