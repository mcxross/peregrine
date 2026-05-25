import { open } from "@tauri-apps/plugin-dialog";
import React from "react";
import { BarChart3, FolderOpen, Loader2, Play, TerminalSquare } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  displayMovePackageName,
  loadProjectMetadata,
  projectPackageConfig,
  projectPackageConfigKey,
  saveProjectMetadata,
  type MovePackage,
  type PackageTree,
  type ProjectMetadata,
} from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";

type ProjectConfigDialogProps = {
  activeMovePackage: MovePackage | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  packageTree: PackageTree;
};

type ProjectConfigCategory = "tests" | "coverage" | "commands";

const projectConfigCategories: {
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  id: ProjectConfigCategory;
  label: string;
}[] = [
  {
    id: "tests",
    label: "Tests",
    icon: Play,
  },
  {
    id: "coverage",
    label: "Coverage",
    icon: BarChart3,
  },
  {
    id: "commands",
    label: "Commands",
    icon: TerminalSquare,
  },
];

export function ProjectConfigDialog({
  activeMovePackage,
  open: isOpen,
  onOpenChange,
  packageTree,
}: ProjectConfigDialogProps) {
  const [activeCategory, setActiveCategory] = React.useState<ProjectConfigCategory>("tests");
  const [draftMoveCoverageScriptPath, setDraftMoveCoverageScriptPath] = React.useState("");
  const [draftMoveTestScriptPath, setDraftMoveTestScriptPath] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [isLoading, setIsLoading] = React.useState(false);
  const [isPicking, setIsPicking] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);
  const packageKey = activeMovePackage ? projectPackageConfigKey(activeMovePackage) : null;
  const activePackageName = displayMovePackageName(activeMovePackage?.name ?? packageTree.rootName);

  React.useEffect(() => {
    if (!isOpen || !activeMovePackage || !packageKey) {
      return;
    }

    let isCancelled = false;

    setIsLoading(true);
    setError(null);

    void loadProjectMetadata(packageTree.rootPath)
      .then((metadata) => {
        if (isCancelled) {
          return;
        }

        const packageConfig = projectPackageConfig(metadata, activeMovePackage);

        setDraftMoveTestScriptPath(packageConfig?.commands?.moveTestScriptPath ?? "");
        setDraftMoveCoverageScriptPath(packageConfig?.commands?.moveCoverageScriptPath ?? "");
      })
      .catch((loadError) => {
        if (!isCancelled) {
          setError(getProjectConfigError(loadError, "Could not load project configuration."));
        }
      })
      .finally(() => {
        if (!isCancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      isCancelled = true;
    };
  }, [activeMovePackage, isOpen, packageKey, packageTree.rootPath]);

  const chooseMoveTestScript = React.useCallback(async () => {
    if (!activeMovePackage || isPicking) {
      return;
    }

    setIsPicking(true);
    setError(null);

    try {
      const packageDirectory = absolutePackagePath(packageTree, activeMovePackage.path);
      const selectedPath = await open({
        defaultPath: packageDirectory,
        directory: false,
        multiple: false,
        title: "Choose Move test script",
      });

      if (!selectedPath || Array.isArray(selectedPath)) {
        return;
      }

      const relativePath = packageRelativePath(packageDirectory, selectedPath);

      if (!relativePath) {
        setError("Choose a script inside the selected Move package.");
        return;
      }

      setDraftMoveTestScriptPath(relativePath);
    } catch (pickError) {
      setError(getProjectConfigError(pickError, "Could not choose script."));
    } finally {
      setIsPicking(false);
    }
  }, [activeMovePackage, isPicking, packageTree]);

  const chooseMoveCoverageScript = React.useCallback(async () => {
    if (!activeMovePackage || isPicking) {
      return;
    }

    setIsPicking(true);
    setError(null);

    try {
      const packageDirectory = absolutePackagePath(packageTree, activeMovePackage.path);
      const selectedPath = await open({
        defaultPath: packageDirectory,
        directory: false,
        multiple: false,
        title: "Choose Move coverage script",
      });

      if (!selectedPath || Array.isArray(selectedPath)) {
        return;
      }

      const relativePath = packageRelativePath(packageDirectory, selectedPath);

      if (!relativePath) {
        setError("Choose a script inside the selected Move package.");
        return;
      }

      setDraftMoveCoverageScriptPath(relativePath);
    } catch (pickError) {
      setError(getProjectConfigError(pickError, "Could not choose script."));
    } finally {
      setIsPicking(false);
    }
  }, [activeMovePackage, isPicking, packageTree]);

  const saveConfig = React.useCallback(async () => {
    if (!activeMovePackage || !packageKey || isSaving) {
      return;
    }

    setIsSaving(true);
    setError(null);

    try {
      const metadata = await loadProjectMetadata(packageTree.rootPath);
      const normalizedCoverageScriptPath = normalizeScriptPath(draftMoveCoverageScriptPath);
      const normalizedScriptPath = normalizeScriptPath(draftMoveTestScriptPath);
      const nextMetadata: ProjectMetadata = {
        ...metadata,
        packageConfigs: {
          ...(metadata.packageConfigs ?? {}),
          [packageKey]: {
            ...(metadata.packageConfigs?.[packageKey] ?? {}),
            commands: {
              ...(metadata.packageConfigs?.[packageKey]?.commands ?? {}),
              moveCoverageScriptPath: normalizedCoverageScriptPath,
              moveTestScriptPath: normalizedScriptPath,
            },
          },
        },
      };

      await saveProjectMetadata(packageTree.rootPath, nextMetadata);
      setDraftMoveCoverageScriptPath(normalizedCoverageScriptPath ?? "");
      setDraftMoveTestScriptPath(normalizedScriptPath ?? "");
      onOpenChange(false);
    } catch (saveError) {
      setError(getProjectConfigError(saveError, "Could not save project configuration."));
    } finally {
      setIsSaving(false);
    }
  }, [
    activeMovePackage,
    draftMoveCoverageScriptPath,
    draftMoveTestScriptPath,
    isSaving,
    onOpenChange,
    packageKey,
    packageTree.rootPath,
  ]);

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-5xl gap-0 overflow-hidden p-0 sm:p-0">
        <div className="grid max-h-[calc(100vh-2rem)] min-h-[min(680px,calc(100vh-2rem))] grid-rows-[auto_minmax(0,1fr)_auto]">
          <DialogHeader className="border-b border-[color:var(--app-border)] px-5 py-4 sm:px-6">
            <DialogTitle>Project Configuration</DialogTitle>
            <DialogDescription className="sr-only">
              Project-specific configuration for {activePackageName}.
            </DialogDescription>
          </DialogHeader>

          <div className="grid min-h-0 md:grid-cols-[240px_minmax(0,1fr)]">
            <aside className="min-h-0 border-b border-[color:var(--app-border)] bg-[var(--app-panel)] p-3 md:border-r md:border-b-0">
              <div className="grid gap-1">
                {projectConfigCategories.map((category) => {
                  const Icon = category.icon;
                  const isActive = category.id === activeCategory;

                  return (
                    <button
                      className={cn(
                        "grid grid-cols-[auto_minmax(0,1fr)] items-center gap-3 rounded-md px-3 py-2.5 text-left text-sm transition-colors",
                        isActive
                          ? "bg-primary/15 text-foreground"
                          : "text-muted-foreground hover:bg-accent hover:text-foreground",
                      )}
                      key={category.id}
                      onClick={() => setActiveCategory(category.id)}
                      type="button"
                    >
                      <Icon className="size-4" aria-hidden="true" />
                      <span className="block min-w-0 truncate font-medium">{category.label}</span>
                    </button>
                  );
                })}
              </div>
            </aside>

            <div className="min-h-0 overflow-auto px-5 py-5 sm:px-6">
              <section className="mx-auto grid max-w-3xl gap-5">
                {activeCategory === "tests" ? (
                  <TestsConfigSection
                    activeMovePackage={activeMovePackage}
                    draftMoveTestScriptPath={draftMoveTestScriptPath}
                    isLoading={isLoading}
                    isPicking={isPicking}
                    isSaving={isSaving}
                    onChooseMoveTestScript={() => void chooseMoveTestScript()}
                    onDraftMoveTestScriptPathChange={setDraftMoveTestScriptPath}
                  />
                ) : null}

                {activeCategory === "coverage" ? (
                  <CoverageConfigSection
                    activeMovePackage={activeMovePackage}
                    draftMoveCoverageScriptPath={draftMoveCoverageScriptPath}
                    isLoading={isLoading}
                    isPicking={isPicking}
                    isSaving={isSaving}
                    onChooseMoveCoverageScript={() => void chooseMoveCoverageScript()}
                    onDraftMoveCoverageScriptPathChange={setDraftMoveCoverageScriptPath}
                  />
                ) : null}

                {activeCategory === "commands" ? (
                  <CommandsConfigSection
                    activeMovePackage={activeMovePackage}
                    packageKey={packageKey}
                    packageTree={packageTree}
                  />
                ) : null}

                {error ? (
                  <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                    {error}
                  </p>
                ) : null}
              </section>
            </div>
          </div>

          <DialogFooter className="border-t border-[color:var(--app-border)] px-5 py-4 sm:px-6">
            <Button
              disabled={isSaving}
              onClick={() => onOpenChange(false)}
              type="button"
              variant="outline"
            >
              Cancel
            </Button>
            <Button
              disabled={isLoading || isSaving || !activeMovePackage}
              onClick={() => void saveConfig()}
              type="button"
            >
              {isSaving ? <Loader2 className="animate-spin" aria-hidden="true" /> : null}
              Save
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function TestsConfigSection({
  activeMovePackage,
  draftMoveTestScriptPath,
  isLoading,
  isPicking,
  isSaving,
  onChooseMoveTestScript,
  onDraftMoveTestScriptPathChange,
}: {
  activeMovePackage: MovePackage | null;
  draftMoveTestScriptPath: string;
  isLoading: boolean;
  isPicking: boolean;
  isSaving: boolean;
  onChooseMoveTestScript: () => void;
  onDraftMoveTestScriptPathChange: (path: string) => void;
}) {
  return (
    <div className="overflow-hidden rounded-lg border border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <ConfigRow
        description={
          <>
            Overrides <span className="font-mono">sui move test</span>.
          </>
        }
        label="Test script"
      >
        <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
          <Input
            autoComplete="off"
            disabled={isLoading || isSaving || !activeMovePackage}
            onChange={(event) => onDraftMoveTestScriptPathChange(event.target.value)}
            placeholder="scripts/test.sh"
            value={draftMoveTestScriptPath}
          />
          <Button
            disabled={isLoading || isPicking || isSaving || !activeMovePackage}
            onClick={onChooseMoveTestScript}
            type="button"
            variant="outline"
          >
            {isPicking ? (
              <Loader2 className="animate-spin" aria-hidden="true" />
            ) : (
              <FolderOpen aria-hidden="true" />
            )}
            Browse
          </Button>
        </div>
      </ConfigRow>
    </div>
  );
}

function CoverageConfigSection({
  activeMovePackage,
  draftMoveCoverageScriptPath,
  isLoading,
  isPicking,
  isSaving,
  onChooseMoveCoverageScript,
  onDraftMoveCoverageScriptPathChange,
}: {
  activeMovePackage: MovePackage | null;
  draftMoveCoverageScriptPath: string;
  isLoading: boolean;
  isPicking: boolean;
  isSaving: boolean;
  onChooseMoveCoverageScript: () => void;
  onDraftMoveCoverageScriptPathChange: (path: string) => void;
}) {
  return (
    <div className="overflow-hidden rounded-lg border border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <ConfigRow
        description="Runs the full coverage flow."
        label="Coverage script"
      >
        <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
          <Input
            autoComplete="off"
            disabled={isLoading || isSaving || !activeMovePackage}
            onChange={(event) => onDraftMoveCoverageScriptPathChange(event.target.value)}
            placeholder="scripts/coverage.sh"
            value={draftMoveCoverageScriptPath}
          />
          <Button
            disabled={isLoading || isPicking || isSaving || !activeMovePackage}
            onClick={onChooseMoveCoverageScript}
            type="button"
            variant="outline"
          >
            {isPicking ? (
              <Loader2 className="animate-spin" aria-hidden="true" />
            ) : (
              <FolderOpen aria-hidden="true" />
            )}
            Browse
          </Button>
        </div>
      </ConfigRow>
      <ConfigRow description="Used when no coverage script is set." label="Default">
        <ReadOnlyValue value="sui move test --coverage; sui move coverage summary" />
      </ConfigRow>
    </div>
  );
}

function CommandsConfigSection({
  activeMovePackage,
  packageKey,
  packageTree,
}: {
  activeMovePackage: MovePackage | null;
  packageKey: string | null;
  packageTree: PackageTree;
}) {
  return (
    <div className="overflow-hidden rounded-lg border border-[color:var(--app-border)] bg-[var(--app-panel)]">
      <ConfigRow label="Package">
        <ReadOnlyValue value={displayMovePackageName(activeMovePackage?.name ?? packageTree.rootName)} />
      </ConfigRow>
      <ConfigRow label="Path">
        <ReadOnlyValue value={activeMovePackage?.path || "."} />
      </ConfigRow>
      <ConfigRow label="Config key">
        <ReadOnlyValue value={packageKey ?? "No active Move package"} />
      </ConfigRow>
    </div>
  );
}

function ConfigRow({
  children,
  description,
  label,
}: {
  children: React.ReactNode;
  description?: React.ReactNode;
  label: string;
}) {
  return (
    <div className="grid gap-3 border-b border-[color:var(--app-border)] px-4 py-4 last:border-b-0 lg:grid-cols-[190px_minmax(0,1fr)] lg:items-start">
      <div className="min-w-0">
        <div className="text-sm font-medium">{label}</div>
        {description ? (
          <p className="mt-1 text-xs leading-5 text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

function ReadOnlyValue({ value }: { value: string }) {
  return (
    <div className="min-h-9 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2 text-sm text-muted-foreground">
      {value}
    </div>
  );
}

function normalizeScriptPath(scriptPath: string) {
  return scriptPath.trim().replace(/^\/+/, "") || null;
}

function absolutePackagePath(packageTree: PackageTree, packagePath: string) {
  const rootPath = packageTree.rootPath.replace(/\/+$/, "");
  const relativePath = packagePath.replace(/^\/+|\/+$/g, "");

  return relativePath ? `${rootPath}/${relativePath}` : rootPath;
}

function packageRelativePath(packageDirectory: string, selectedPath: string) {
  const normalizedPackageDirectory = packageDirectory.replace(/\/+$/, "");
  const normalizedSelectedPath = selectedPath.replace(/\/+$/, "");
  const prefix = `${normalizedPackageDirectory}/`;

  if (!normalizedSelectedPath.startsWith(prefix)) {
    return null;
  }

  return normalizedSelectedPath.slice(prefix.length);
}

function getProjectConfigError(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : fallback;
}
