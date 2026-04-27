import React from "react";
import { Package } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  loadPackageTree,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { ProjectDropzone } from "@/features/empty-project/project-dropzone";
import { RecentProjects } from "@/features/empty-project/recent-projects";
import {
  activeManifestPathForRecentProject,
  clearRecentProjects,
  loadRecentProjects,
  rememberRecentProject,
} from "@/features/empty-project/recent-project-store";
import type { RecentProject } from "@/features/empty-project/types";

type EmptyProjectScreenProps = {
  recentProjects?: RecentProject[];
  onOpenProject?: () => void;
  onOpenRecentProject?: (project: RecentProject) => void;
  onClearRecentProjects?: () => void;
  onProjectSelected?: (packageTree: PackageTree) => void;
};

export function EmptyProjectScreen({
  recentProjects,
  onOpenProject,
  onOpenRecentProject,
  onClearRecentProjects,
  onProjectSelected,
}: EmptyProjectScreenProps) {
  const [storedRecentProjects, setStoredRecentProjects] = React.useState<RecentProject[]>(() => loadRecentProjects());
  const [loadError, setLoadError] = React.useState<string | null>(null);
  const [isLoading, setIsLoading] = React.useState(false);
  const [pendingPackageTree, setPendingPackageTree] = React.useState<PackageTree | null>(null);
  const visibleRecentProjects = recentProjects ?? storedRecentProjects;
  const selectProject = React.useCallback(
    (packageTree: PackageTree) => {
      setStoredRecentProjects((projects) => rememberRecentProject(projects, packageTree));
      onProjectSelected?.(packageTree);
    },
    [onProjectSelected],
  );

  const handleOpenProject = onOpenProject ?? (async () => {
    setLoadError(null);
    setPendingPackageTree(null);
    setIsLoading(true);

    try {
      const packagePath = await openMovePackage();

      if (!packagePath) {
        return;
      }

      const packageTree = await loadPackageTree(packagePath);

      if (packageTree.movePackages.length > 1) {
        setPendingPackageTree(packageTree);
        return;
      }

      selectProject(withActivePackage(packageTree, packageTree.movePackages[0] ?? null));
    } catch (error) {
      setLoadError(getOpenPackageErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  });
  const handleOpenRecentProject = onOpenRecentProject ?? (async (project: RecentProject) => {
    setLoadError(null);
    setPendingPackageTree(null);
    setIsLoading(true);

    try {
      const packageTree = await loadPackageTree(project.rootPath);
      const activePackageManifestPath = activeManifestPathForRecentProject(packageTree, project);

      if (packageTree.movePackages.length > 1 && !activePackageManifestPath) {
        setPendingPackageTree(packageTree);
        return;
      }

      selectProject({
        ...packageTree,
        activePackageManifestPath,
      });
    } catch (error) {
      setLoadError(getOpenPackageErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  });
  const handleClearRecentProjects = React.useCallback(() => {
    clearRecentProjects();
    setStoredRecentProjects([]);
    onClearRecentProjects?.();
  }, [onClearRecentProjects]);

  if (pendingPackageTree) {
    return (
      <PackageLoadSelection
        packageTree={pendingPackageTree}
        onCancel={() => setPendingPackageTree(null)}
        onSelectPackage={(movePackage) => {
          selectProject(withActivePackage(pendingPackageTree, movePackage));
        }}
      />
    );
  }

  return (
    <div className="grid h-full min-h-0 place-items-center overflow-hidden bg-background px-6 py-5">
      <div className="flex max-h-full w-full max-w-[660px] flex-col items-stretch gap-4">
        <ProjectDropzone onOpenProject={handleOpenProject} isLoading={isLoading} />
        {loadError ? (
          <p className="rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {loadError}
          </p>
        ) : null}
        <ScrollArea className="max-h-[240px] min-h-0">
          <RecentProjects
            projects={visibleRecentProjects}
            onClear={handleClearRecentProjects}
            onOpenProject={handleOpenRecentProject}
          />
        </ScrollArea>
      </div>
    </div>
  );
}

function PackageLoadSelection({
  onCancel,
  onSelectPackage,
  packageTree,
}: {
  onCancel: () => void;
  onSelectPackage: (movePackage: MovePackage) => void;
  packageTree: PackageTree;
}) {
  const packages = orderedPackages(packageTree);

  return (
    <div className="grid h-full min-h-0 bg-background px-6 py-6">
      <div className="mx-auto grid h-full min-h-0 w-full max-w-4xl grid-rows-[auto_minmax(0,1fr)] gap-5">
        <header className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="text-sm font-medium text-muted-foreground">Multiple Move packages found</p>
            <h1 className="mt-1 text-2xl font-semibold tracking-tight">Select the active package</h1>
            <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
              Peregrine will focus the workspace, module surface, and security context on the package you select.
            </p>
          </div>
          <Button type="button" variant="outline" onClick={onCancel}>
            Cancel
          </Button>
        </header>

        <ScrollArea className="min-h-0">
          <div className="grid gap-3 pb-2 sm:grid-cols-2">
            {packages.map((movePackage) => (
              <button
                className="group min-w-0 rounded-md text-left"
                key={movePackage.manifestPath}
                onClick={() => onSelectPackage(movePackage)}
                type="button"
              >
                <Card className="h-full min-w-0 gap-0 rounded-md p-4 transition group-hover:border-primary/60 group-hover:bg-[var(--app-subtle)]">
                  <div className="flex min-w-0 items-start gap-3">
                    <Package className="mt-0.5 size-5 shrink-0 text-muted-foreground group-hover:text-primary" aria-hidden="true" />
                    <div className="min-w-0">
                      <h2 className="truncate text-base font-semibold">{movePackage.name}</h2>
                      <p className="mt-1 truncate text-sm text-muted-foreground">
                        {movePackage.path || "."}
                      </p>
                      <p className="mt-3 text-sm text-muted-foreground">
                        {moduleCountLabel(movePackage.modules.length)}
                      </p>
                    </div>
                  </div>
                </Card>
              </button>
            ))}
          </div>
        </ScrollArea>
      </div>
    </div>
  );
}

function orderedPackages(packageTree: PackageTree) {
  const rootPackage = packageTree.dependencyGraph.root;

  return [...packageTree.movePackages].sort((left, right) => {
    const leftIsRoot = left.name === rootPackage;
    const rightIsRoot = right.name === rootPackage;

    return Number(rightIsRoot) - Number(leftIsRoot)
      || left.name.localeCompare(right.name)
      || left.path.localeCompare(right.path);
  });
}

function withActivePackage(packageTree: PackageTree, movePackage: MovePackage | null): PackageTree {
  return {
    ...packageTree,
    activePackageManifestPath: movePackage?.manifestPath ?? null,
  };
}

function moduleCountLabel(count: number) {
  if (count === 1) {
    return "1 module";
  }

  return `${count} modules`;
}

async function openMovePackage(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");

  const selectedPath = await open({
    directory: true,
    multiple: false,
    recursive: true,
    title: "Open Move Package",
  });

  return typeof selectedPath === "string" ? selectedPath : null;
}

function getOpenPackageErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  try {
    return JSON.stringify(error);
  } catch {
    return "Could not open package.";
  }
}
