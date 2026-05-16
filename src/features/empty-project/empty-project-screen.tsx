import React from "react";
import { FolderOpen, Package, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  createMoveProject,
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
  const [isCreateProjectOpen, setIsCreateProjectOpen] = React.useState(false);
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
  const handleCreateProject = React.useCallback(
    async (input: CreateMoveProjectInput) => {
      setLoadError(null);
      setPendingPackageTree(null);
      setIsLoading(true);

      try {
        const packageTree = await createMoveProject(input.parentPath, input.projectName);
        const activePackage = packageTree.movePackages.find(
          (movePackage) => movePackage.name === input.projectName,
        ) ?? packageTree.movePackages[0] ?? null;

        setIsCreateProjectOpen(false);
        selectProject(withActivePackage(packageTree, activePackage));
      } catch (error) {
        setLoadError(getOpenPackageErrorMessage(error));
      } finally {
        setIsLoading(false);
      }
    },
    [selectProject],
  );
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
    <div className="grid h-full min-h-0 place-items-center overflow-auto bg-background px-6 py-5">
      <div className="flex max-h-full w-full max-w-[660px] flex-col items-stretch gap-4">
        <ProjectDropzone
          onCreateProject={() => setIsCreateProjectOpen((isOpen) => !isOpen)}
          onOpenProject={handleOpenProject}
          isLoading={isLoading}
        />
        {isCreateProjectOpen ? (
          <CreateMoveProjectForm
            isLoading={isLoading}
            onCancel={() => setIsCreateProjectOpen(false)}
            onCreateProject={handleCreateProject}
          />
        ) : null}
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

type CreateMoveProjectInput = {
  parentPath: string;
  projectName: string;
};

function CreateMoveProjectForm({
  isLoading,
  onCancel,
  onCreateProject,
}: {
  isLoading: boolean;
  onCancel: () => void;
  onCreateProject: (input: CreateMoveProjectInput) => void;
}) {
  const [parentPath, setParentPath] = React.useState("");
  const [projectName, setProjectName] = React.useState("");
  const trimmedProjectName = projectName.trim();
  const projectNameError = trimmedProjectName && !isValidMoveProjectName(trimmedProjectName)
    ? "Use a Move package name: letters, numbers, and underscores; start with a letter or underscore."
    : null;
  const canCreate = Boolean(parentPath && trimmedProjectName && !projectNameError && !isLoading);

  return (
    <Card className="rounded-md p-4 shadow-none">
      <form
        className="grid gap-4"
        onSubmit={(event) => {
          event.preventDefault();

          if (!canCreate) {
            return;
          }

          onCreateProject({ parentPath, projectName: trimmedProjectName });
        }}
      >
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h2 className="text-base font-semibold tracking-tight">Create a Move package</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              Choose a parent folder and package name. Peregrine opens the new package after creation.
            </p>
          </div>
          <Button type="button" variant="ghost" onClick={onCancel} disabled={isLoading}>
            Cancel
          </Button>
        </div>

        <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
          <div className="grid gap-2">
            <label className="text-sm font-medium" htmlFor="new-move-project-name">
              Package name
            </label>
            <Input
              autoComplete="off"
              id="new-move-project-name"
              onChange={(event) => setProjectName(event.target.value)}
              placeholder="my_package"
              type="text"
              value={projectName}
              aria-invalid={Boolean(projectNameError)}
            />
            {projectNameError ? (
              <p className="text-xs text-destructive">{projectNameError}</p>
            ) : null}
          </div>

          <div className="grid gap-2">
            <span className="text-sm font-medium">Parent directory</span>
            <Button
              className="justify-start"
              type="button"
              variant="outline"
              onClick={() => {
                void chooseProjectParentDirectory().then((selectedPath) => {
                  if (selectedPath) {
                    setParentPath(selectedPath);
                  }
                });
              }}
              disabled={isLoading}
            >
              <FolderOpen aria-hidden="true" />
              Choose Folder
            </Button>
          </div>
        </div>

        {parentPath ? (
          <p className="truncate rounded-md border bg-[var(--app-surface)] px-3 py-2 font-mono text-xs text-muted-foreground">
            {parentPath}/{trimmedProjectName || "<package_name>"}
          </p>
        ) : (
          <p className="rounded-md border border-dashed px-3 py-2 text-sm text-muted-foreground">
            Choose the directory where the new package folder should be created.
          </p>
        )}

        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="outline" onClick={onCancel} disabled={isLoading}>
            Close
          </Button>
          <Button type="submit" disabled={!canCreate}>
            <Plus aria-hidden="true" />
            {isLoading ? "Creating..." : "Create Package"}
          </Button>
        </div>
      </form>
    </Card>
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

async function chooseProjectParentDirectory(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");

  const selectedPath = await open({
    directory: true,
    multiple: false,
    recursive: false,
    title: "Choose Project Directory",
  });

  return typeof selectedPath === "string" ? selectedPath : null;
}

function isValidMoveProjectName(projectName: string) {
  return /^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(projectName);
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
