import React from "react";
import { FolderOpen, Package, PackagePlus, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SuiNetworkSelector } from "@/app/sui-network-selector";
import {
  suiGraphQlUrlForSelection,
  suiNetworkLabel,
  type SuiNetworkSelection,
} from "@peregrine/desktop-runtime";
import {
  createMoveProject,
  displayMovePackageName,
  importMovePackageById,
  loadPackageTree,
  moveProjectPathExists,
  type MovePackage,
  type PackageTree,
} from "@peregrine/desktop-runtime";
import { ProjectDropzone } from "@/features/empty-project/project-dropzone";
import { RecentProjects } from "@/features/empty-project/recent-projects";
import {
  activeManifestPathForRecentProject,
  clearRecentProjects,
  loadRecentProjects,
  rememberRecentProject,
  removeRecentProject,
} from "@peregrine/desktop-runtime";
import type { RecentProject } from "@peregrine/desktop-runtime";

type EmptyProjectScreenProps = {
  recentProjects?: RecentProject[];
  onOpenProject?: () => void;
  onOpenRecentProject?: (project: RecentProject) => void;
  onClearRecentProjects?: () => void;
  onRemoveRecentProject?: (project: RecentProject) => void;
  onProjectSelected?: (packageTree: PackageTree) => void;
  network: SuiNetworkSelection;
  onNetworkChange: (network: SuiNetworkSelection) => void;
};

export function EmptyProjectScreen({
  recentProjects,
  onOpenProject,
  onOpenRecentProject,
  onClearRecentProjects,
  onRemoveRecentProject,
  onProjectSelected,
  network,
  onNetworkChange,
}: EmptyProjectScreenProps) {
  const [storedRecentProjects, setStoredRecentProjects] = React.useState<RecentProject[]>(() => loadRecentProjects());
  const [loadError, setLoadError] = React.useState<string | null>(null);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isCreateProjectOpen, setIsCreateProjectOpen] = React.useState(false);
  const [isImportPackageOpen, setIsImportPackageOpen] = React.useState(false);
  const [pendingPackageTree, setPendingPackageTree] = React.useState<PackageTree | null>(null);
  const visibleRecentProjects = recentProjects ?? storedRecentProjects;
  const graphQlUrl = suiGraphQlUrlForSelection(network);
  const networkLabel = suiNetworkLabel(network);
  const selectProject = React.useCallback(
    (packageTree: PackageTree) => {
      const nextRecentProjects = rememberRecentProject(loadRecentProjects(), packageTree);

      setStoredRecentProjects(nextRecentProjects);
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
  const handleImportPackage = React.useCallback(
    async (packageId: string, saveRootPath: string, generateBuildable: boolean) => {
      setLoadError(null);
      setPendingPackageTree(null);
      setIsLoading(true);

      try {
        if (!graphQlUrl) {
          throw new Error(`${networkLabel} does not have a GraphQL endpoint configured.`);
        }

        const packageTree = await importMovePackageById(
          packageId,
          network.id,
          graphQlUrl,
          saveRootPath,
          generateBuildable,
        );

        setIsImportPackageOpen(false);
        selectProject(withActivePackage(packageTree, packageTree.movePackages[0] ?? null));
      } catch (error) {
        setLoadError(getOpenPackageErrorMessage(error));
      } finally {
        setIsLoading(false);
      }
    },
    [graphQlUrl, network.id, networkLabel, selectProject],
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
  const handleRemoveRecentProject = React.useCallback(
    (project: RecentProject) => {
      setStoredRecentProjects((currentProjects) => {
        const nextProjects = removeRecentProject(
          recentProjects ?? currentProjects,
          project.id,
        );

        return recentProjects ? currentProjects : nextProjects;
      });
      onRemoveRecentProject?.(project);
    },
    [onRemoveRecentProject, recentProjects],
  );

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
          onCreateProject={() => setIsCreateProjectOpen(true)}
          onImportPackage={() => setIsImportPackageOpen(true)}
          onOpenProject={handleOpenProject}
          isLoading={isLoading}
        />
        <Dialog
          open={isCreateProjectOpen}
          onOpenChange={(isOpen) => {
            if (!isLoading) {
              setIsCreateProjectOpen(isOpen);
            }
          }}
        >
          <CreateMoveProjectDialog
            isLoading={isLoading}
            onCancel={() => setIsCreateProjectOpen(false)}
            onCreateProject={handleCreateProject}
          />
        </Dialog>
        <Dialog
          open={isImportPackageOpen}
          onOpenChange={(isOpen) => {
            if (!isLoading) {
              setIsImportPackageOpen(isOpen);
            }
          }}
        >
          <ImportMovePackageDialog
            graphQlUrl={graphQlUrl}
            isLoading={isLoading}
            network={network}
            networkLabel={networkLabel}
            onCancel={() => setIsImportPackageOpen(false)}
            onImportPackage={handleImportPackage}
            onNetworkChange={onNetworkChange}
          />
        </Dialog>
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
            onRemoveProject={handleRemoveRecentProject}
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

function CreateMoveProjectDialog({
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
  const [projectPathExists, setProjectPathExists] = React.useState(false);
  const [projectPathCheckError, setProjectPathCheckError] = React.useState<string | null>(null);
  const [isCheckingProjectPath, setIsCheckingProjectPath] = React.useState(false);
  const trimmedProjectName = projectName.trim();
  const projectNameError = trimmedProjectName && !isValidMoveProjectName(trimmedProjectName)
    ? "Use a Move package name: letters, numbers, and underscores; start with a letter or underscore."
    : null;
  const canCreate = Boolean(
    parentPath
      && trimmedProjectName
      && !projectNameError
      && !projectPathExists
      && !projectPathCheckError
      && !isCheckingProjectPath
      && !isLoading,
  );

  React.useEffect(() => {
    let isCurrent = true;

    if (!parentPath || !trimmedProjectName || projectNameError) {
      setProjectPathExists(false);
      setProjectPathCheckError(null);
      setIsCheckingProjectPath(false);
      return () => {
        isCurrent = false;
      };
    }

    setIsCheckingProjectPath(true);
    setProjectPathCheckError(null);

    void moveProjectPathExists(parentPath, trimmedProjectName)
      .then((exists) => {
        if (!isCurrent) {
          return;
        }

        setProjectPathExists(exists);
      })
      .catch((error) => {
        if (!isCurrent) {
          return;
        }

        setProjectPathExists(false);
        setProjectPathCheckError(getOpenPackageErrorMessage(error));
      })
      .finally(() => {
        if (isCurrent) {
          setIsCheckingProjectPath(false);
        }
      });

    return () => {
      isCurrent = false;
    };
  }, [parentPath, projectNameError, trimmedProjectName]);

  return (
    <DialogContent
      onInteractOutside={(event) => {
        event.preventDefault();
      }}
    >
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
        <DialogHeader>
          <DialogTitle>Create a New Package</DialogTitle>
          <DialogDescription>
            Choose a parent folder and package name.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
          <div className="grid gap-2">
            <label className="text-sm font-medium" htmlFor="new-move-project-name">
              Package Name
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

          <div className="flex items-end">
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
          <div className="grid gap-2">
            <p className="truncate rounded-md border bg-[var(--app-surface)] px-3 py-2 font-mono text-xs text-muted-foreground">
              {parentPath}/{trimmedProjectName || "<package_name>"}
            </p>
            {projectPathExists ? (
              <p className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-300">
                A file or folder named `{trimmedProjectName}` already exists in that directory.
              </p>
            ) : null}
            {projectPathCheckError ? (
              <p className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                {projectPathCheckError}
              </p>
            ) : null}
          </div>
        ) : null}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onCancel} disabled={isLoading}>
            Cancel
          </Button>
          <Button type="submit" disabled={!canCreate}>
            <Plus aria-hidden="true" />
            {isLoading ? "Creating..." : "Create Package"}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  );
}

function ImportMovePackageDialog({
  graphQlUrl,
  isLoading,
  network,
  networkLabel,
  onCancel,
  onImportPackage,
  onNetworkChange,
}: {
  graphQlUrl: string | null;
  isLoading: boolean;
  network: SuiNetworkSelection;
  networkLabel: string;
  onCancel: () => void;
  onImportPackage: (packageId: string, saveRootPath: string, generateBuildable: boolean) => void;
  onNetworkChange: (network: SuiNetworkSelection) => void;
}) {
  const [packageId, setPackageId] = React.useState("");
  const [saveRootPath, setSaveRootPath] = React.useState<string | null>(null);
  const [generateBuildable, setGenerateBuildable] = React.useState(false);
  const trimmedPackageId = packageId.trim();
  const packageIdError = trimmedPackageId && !isValidSuiPackageId(trimmedPackageId)
    ? "Use a Sui package ID: 0x followed by up to 64 hex characters."
    : null;
  const endpointError = graphQlUrl ? null : `${networkLabel} does not have a GraphQL endpoint configured.`;
  const canImport = Boolean(
    trimmedPackageId
      && !packageIdError
      && graphQlUrl
      && saveRootPath
      && !isLoading,
  );

  return (
    <DialogContent
      onInteractOutside={(event) => {
        event.preventDefault();
      }}
    >
      <form
        className="grid gap-4"
        onSubmit={(event) => {
          event.preventDefault();

          if (!canImport || !saveRootPath) {
            return;
          }

          onImportPackage(trimmedPackageId, saveRootPath, generateBuildable);
        }}
      >
        <DialogHeader>
          <DialogTitle>Import Package ID</DialogTitle>
          <DialogDescription>
            Fetch and decompile an on-chain package from {networkLabel}.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="import-package-id">
            Package ID
          </label>
          <Input
            autoComplete="off"
            autoFocus
            id="import-package-id"
            onChange={(event) => setPackageId(event.target.value)}
            placeholder="0x2"
            spellCheck={false}
            type="text"
            value={packageId}
            aria-invalid={Boolean(packageIdError)}
          />
          {packageIdError ? (
            <p className="text-xs text-destructive">{packageIdError}</p>
          ) : null}
        </div>

        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="import-package-network">
            Network
          </label>
          <SuiNetworkSelector
            align="start"
            buttonId="import-package-network"
            className="w-full"
            contentClassName="w-96"
            network={network}
            onNetworkChange={onNetworkChange}
            size="default"
          />
        </div>

        <div className="grid gap-2">
          <p className="truncate rounded-md border bg-[var(--app-surface)] px-3 py-2 font-mono text-xs text-muted-foreground">
            {graphQlUrl ?? "No GraphQL endpoint configured"}
          </p>
          {endpointError ? (
            <p className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-300">
              {endpointError}
            </p>
          ) : null}
        </div>

        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="import-package-save-location">
            Save location
          </label>
          <div className="flex min-w-0 gap-2">
            <p
              className="min-w-0 flex-1 truncate rounded-md border bg-[var(--app-surface)] px-3 py-2 font-mono text-xs text-muted-foreground"
              id="import-package-save-location"
            >
              {saveRootPath ?? "No folder selected"}
            </p>
            <Button
              className="shrink-0"
              disabled={isLoading}
              onClick={() => {
                void chooseImportSaveDirectory().then((selectedPath) => {
                  if (selectedPath) {
                    setSaveRootPath(selectedPath);
                  }
                });
              }}
              type="button"
              variant="outline"
            >
              <FolderOpen aria-hidden="true" />
              Choose
            </Button>
          </div>
        </div>

        <label className="flex items-start gap-3 rounded-md border bg-[var(--app-surface)] px-3 py-3">
          <input
            checked={generateBuildable}
            className="mt-1 size-4 accent-emerald-400"
            disabled={isLoading}
            onChange={(event) => setGenerateBuildable(event.target.checked)}
            type="checkbox"
          />
          <span className="grid gap-1">
            <span className="text-sm font-medium">Generate buildable package</span>
            <span className="text-xs leading-5 text-muted-foreground">
              Resolve dependencies, rewrite package IDs, and run Sui build verification.
            </span>
          </span>
        </label>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onCancel} disabled={isLoading}>
            Cancel
          </Button>
          <Button type="submit" disabled={!canImport}>
            <PackagePlus aria-hidden="true" />
            {isLoading ? "Importing..." : "Import Package"}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
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
                      <h2 className="truncate text-base font-semibold">
                        {displayMovePackageName(movePackage.name)}
                      </h2>
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

async function chooseImportSaveDirectory(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");

  const selectedPath = await open({
    directory: true,
    multiple: false,
    recursive: false,
    title: "Choose Import Save Location",
  });

  return typeof selectedPath === "string" ? selectedPath : null;
}

function isValidMoveProjectName(projectName: string) {
  return /^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(projectName);
}

function isValidSuiPackageId(packageId: string) {
  return /^0x[0-9a-fA-F]{1,64}$/.test(packageId);
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
