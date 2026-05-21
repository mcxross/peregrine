import {
  displayMovePackageName,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import type { RecentProject } from "@/features/empty-project/types";

const RECENT_PROJECTS_STORAGE_KEY = "peregrine.recentProjects.v1";
const MAX_RECENT_PROJECTS = 8;

export function loadRecentProjects(): RecentProject[] {
  if (typeof window === "undefined") {
    return [];
  }

  try {
    const rawProjects = window.localStorage.getItem(RECENT_PROJECTS_STORAGE_KEY);

    if (!rawProjects) {
      return [];
    }

    const parsed = JSON.parse(rawProjects);

    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed
      .map(toRecentProject)
      .filter((project): project is RecentProject => Boolean(project))
      .sort((left, right) => right.lastOpenedAt - left.lastOpenedAt)
      .slice(0, MAX_RECENT_PROJECTS);
  } catch {
    return [];
  }
}

export function saveRecentProjects(projects: RecentProject[]) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(
    RECENT_PROJECTS_STORAGE_KEY,
    JSON.stringify(projects.slice(0, MAX_RECENT_PROJECTS)),
  );
}

export function clearRecentProjects() {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.removeItem(RECENT_PROJECTS_STORAGE_KEY);
}

export function removeRecentProject(projects: RecentProject[], projectId: string) {
  const nextProjects = projects.filter((project) => project.id !== projectId);

  if (nextProjects.length === 0) {
    clearRecentProjects();
  } else {
    saveRecentProjects(nextProjects);
  }

  return nextProjects;
}

export function rememberRecentProject(
  projects: RecentProject[],
  packageTree: PackageTree,
) {
  const recentProject = recentProjectFromPackageTree(packageTree);
  const nextProjects = [
    recentProject,
    ...projects.filter((project) => project.id !== recentProject.id),
  ].slice(0, MAX_RECENT_PROJECTS);

  saveRecentProjects(nextProjects);

  return nextProjects;
}

export function recentProjectFromPackageTree(packageTree: PackageTree): RecentProject {
  const activePackage = activeMovePackage(packageTree);
  const packagePath = activePackage ? resolvePackagePath(packageTree, activePackage) : packageTree.rootPath;
  const id = `${packageTree.rootPath}::${activePackage?.manifestPath ?? "root"}`;

  return {
    activePackageManifestPath: activePackage?.manifestPath ?? null,
    id,
    lastOpenedAt: Date.now(),
    moduleCount: activePackage?.modules.length ?? 0,
    name: displayMovePackageName(activePackage?.name ?? packageTree.rootName),
    packageCount: packageTree.movePackages.length,
    packagePath,
    rootPath: packageTree.rootPath,
  };
}

export function activeManifestPathForRecentProject(
  packageTree: PackageTree,
  project: RecentProject,
) {
  if (
    project.activePackageManifestPath &&
    packageTree.movePackages.some(
      (movePackage) => movePackage.manifestPath === project.activePackageManifestPath,
    )
  ) {
    return project.activePackageManifestPath;
  }

  const matchingPackage = packageTree.movePackages.find((movePackage) => {
    const packagePath = resolvePackagePath(packageTree, movePackage);

    return displayMovePackageName(movePackage.name) === project.name && packagePath === project.packagePath;
  });

  return matchingPackage?.manifestPath ?? packageTree.movePackages[0]?.manifestPath ?? null;
}

function activeMovePackage(packageTree: PackageTree): MovePackage | null {
  if (packageTree.activePackageManifestPath) {
    return packageTree.movePackages.find(
      (movePackage) => movePackage.manifestPath === packageTree.activePackageManifestPath,
    ) ?? null;
  }

  return packageTree.movePackages[0] ?? null;
}

function resolvePackagePath(packageTree: PackageTree, movePackage: MovePackage) {
  if (!movePackage.path || movePackage.path === ".") {
    return packageTree.rootPath;
  }

  if (movePackage.path.startsWith("/")) {
    return movePackage.path;
  }

  return `${packageTree.rootPath}/${movePackage.path}`;
}

function toRecentProject(value: unknown): RecentProject | null {
  if (!value || typeof value !== "object") {
    return null;
  }

  const candidate = value as Record<string, unknown>;

  if (
    typeof candidate.id !== "string" ||
    typeof candidate.name !== "string" ||
    typeof candidate.rootPath !== "string" ||
    typeof candidate.packagePath !== "string"
  ) {
    return null;
  }

  return {
    activePackageManifestPath:
      typeof candidate.activePackageManifestPath === "string"
        ? candidate.activePackageManifestPath
        : null,
    id: candidate.id,
    lastOpenedAt:
      typeof candidate.lastOpenedAt === "number"
        ? candidate.lastOpenedAt
        : Date.now(),
    moduleCount:
      typeof candidate.moduleCount === "number"
        ? candidate.moduleCount
        : 0,
    name: candidate.name,
    packageCount:
      typeof candidate.packageCount === "number"
        ? candidate.packageCount
        : 0,
    packagePath: candidate.packagePath,
    rootPath: candidate.rootPath,
  };
}
