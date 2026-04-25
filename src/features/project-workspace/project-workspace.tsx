import React from "react";

import {
  buildMovePackage,
  isDirectoryPath,
  loadFilePreview,
  saveTextFile,
  type FilePreview,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { EditorTabs } from "@/features/project-workspace/editor-tabs";
import {
  MovePackagePanel,
  type PackageBuildStatus,
} from "@/features/project-workspace/move-package-panel";
import { ProjectFileTree } from "@/features/project-workspace/project-file-tree";
import { Button } from "@/components/ui/button";
import { Package, PanelLeftOpen, PanelRightClose, PanelRightOpen } from "lucide-react";
import type { SelectedMoveModule } from "@/features/project-workspace/module-signature-screen";

type ProjectWorkspaceProps = {
  packageTree: PackageTree;
};

export type OpenFileTab = {
  path: string;
  preview: FilePreview | null;
  editedSource: string | null;
  error: string | null;
  isDirty: boolean;
  isSaving: boolean;
  status: "idle" | "loading" | "loaded" | "error";
};

export function ProjectWorkspace({ packageTree }: ProjectWorkspaceProps) {
  const [selectedPath, setSelectedPath] = React.useState<string | null>(null);
  const [tabs, setTabs] = React.useState<OpenFileTab[]>([]);
  const [activePath, setActivePath] = React.useState<string | null>(null);
  const [selectedModule, setSelectedModule] =
    React.useState<SelectedMoveModule | null>(null);
  const [isPackagePanelCollapsed, setIsPackagePanelCollapsed] = React.useState(false);
  const [isFileTreeCollapsed, setIsFileTreeCollapsed] = React.useState(true);
  const [buildStatuses, setBuildStatuses] = React.useState<
    Record<string, PackageBuildStatus>
  >({});
  const loadingPathsRef = React.useRef<Set<string>>(new Set());
  const previewCacheRef = React.useRef<Map<string, FilePreview>>(new Map());
  const rootPackageName =
    packageTree.dependencyGraph.root ?? packageTree.movePackages[0]?.name ?? null;

  const loadPreview = React.useCallback(
    (path: string, options: { force?: boolean } = {}) => {
      const cachedPreview = previewCacheRef.current.get(path);

      if (cachedPreview && !options.force) {
        setTabs((currentTabs) =>
          updateTab(ensureTab(currentTabs, path), path, {
            error: null,
            preview: cachedPreview,
            status: "loaded",
          }),
        );
        return;
      }

      if (loadingPathsRef.current.has(path) && !options.force) {
        return;
      }

      loadingPathsRef.current.add(path);
      setTabs((currentTabs) =>
        updateTab(ensureTab(currentTabs, path), path, {
          status: "loading",
          error: null,
        }),
      );

      withTimeout(loadFilePreview(packageTree, path), 15_000)
        .then((preview) => {
          previewCacheRef.current.set(path, preview);
          setTabs((currentTabs) =>
            updateTab(currentTabs, path, {
              editedSource: preview.kind === "text" ? preview.source : null,
              isDirty: false,
              isSaving: false,
              preview,
              status: "loaded",
              error: null,
            }),
          );
        })
        .catch((error: unknown) => {
          setTabs((currentTabs) =>
            updateTab(currentTabs, path, {
              error: error instanceof Error ? error.message : String(error),
              status: "error",
            }),
          );
        })
        .finally(() => {
          loadingPathsRef.current.delete(path);
        });
    },
    [packageTree],
  );

  const openFile = React.useCallback(
    (path: string) => {
      setSelectedModule(null);
      setSelectedPath(path);
      setTabs((currentTabs) =>
        ensureTab(currentTabs, path),
      );
      setActivePath(path);
      loadPreview(path);
    },
    [loadPreview],
  );

  const selectPath = React.useCallback(
    (path: string | null) => {
      setSelectedPath(path);

      if (!path || isDirectoryPath(path)) {
        return;
      }

      openFile(path);
    },
    [openFile],
  );

  const activateTab = React.useCallback(
    (path: string) => {
      setActivePath(path);

      const tab = tabs.find((currentTab) => currentTab.path === path);

      if (tab?.status === "error") {
        loadPreview(path, { force: true });
      }
    },
    [loadPreview, tabs],
  );

  React.useEffect(() => {
    previewCacheRef.current.clear();
    loadingPathsRef.current.clear();
    setSelectedModule(null);
    setSelectedPath(null);
    setTabs([]);
    setActivePath(null);
    setBuildStatuses({});
  }, [packageTree.rootPath]);

  const closeTab = (path: string) => {
    setTabs((currentTabs) => {
      const tabIndex = currentTabs.findIndex((tab) => tab.path === path);
      const nextTabs = currentTabs.filter((tab) => tab.path !== path);

      if (activePath === path) {
        const nextActiveTab = nextTabs[tabIndex] ?? nextTabs[tabIndex - 1] ?? null;
        setActivePath(nextActiveTab?.path ?? null);
      }

      return nextTabs;
    });
  };

  const updateTabSource = React.useCallback((path: string, source: string) => {
    setTabs((currentTabs) =>
      currentTabs.map((tab) => {
        if (tab.path !== path) {
          return tab;
        }

        const originalSource =
          tab.preview?.kind === "text" ? tab.preview.source : null;

        return {
          ...tab,
          editedSource: source,
          isDirty: originalSource !== null && source !== originalSource,
        };
      }),
    );
  }, []);

  const saveTab = React.useCallback(
    (path: string) => {
      const tab = tabs.find((currentTab) => currentTab.path === path);

      if (!tab || tab.preview?.kind !== "text") {
        return;
      }

      const contents = tab.editedSource ?? tab.preview.source;

      setTabs((currentTabs) =>
        updateTab(currentTabs, path, {
          error: null,
          isSaving: true,
        }),
      );

      withTimeout(saveTextFile(packageTree, path, contents), 15_000)
        .then((preview) => {
          previewCacheRef.current.set(path, preview);
          setTabs((currentTabs) =>
            updateTab(currentTabs, path, {
              editedSource: preview.kind === "text" ? preview.source : null,
              error: null,
              isDirty: false,
              isSaving: false,
              preview,
              status: "loaded",
            }),
          );
        })
        .catch((error: unknown) => {
          setTabs((currentTabs) =>
            updateTab(currentTabs, path, {
              error: error instanceof Error ? error.message : String(error),
              isSaving: false,
            }),
          );
        });
    },
    [packageTree, tabs],
  );

  const buildPackage = React.useCallback(
    (movePackage: { name: string; path: string }) => {
      setBuildStatuses((currentStatuses) => ({
        ...currentStatuses,
        [movePackage.path]: {
          message: "Running sui move build...",
          state: "running",
        },
      }));

      withTimeout(buildMovePackage(packageTree, movePackage.path), 120_000)
        .then((output) => {
          const didSucceed = output.status === 0;
          const outputText = didSucceed
            ? output.stdout || "Build completed."
            : output.stderr || output.stdout || "Build failed.";

          setBuildStatuses((currentStatuses) => ({
            ...currentStatuses,
            [movePackage.path]: {
              message: summarizeCommandOutput(outputText),
              state: didSucceed ? "success" : "error",
            },
          }));
        })
        .catch((error: unknown) => {
          setBuildStatuses((currentStatuses) => ({
            ...currentStatuses,
            [movePackage.path]: {
              message: error instanceof Error ? error.message : String(error),
              state: "error",
            },
          }));
        });
    },
    [packageTree],
  );

  const selectModule = React.useCallback(
    (movePackage: SelectedMoveModule["movePackage"], moveModule: SelectedMoveModule["moveModule"]) => {
      setSelectedModule({ moveModule, movePackage });
      setSelectedPath(moveModule.filePath);
      setActivePath(null);
    },
    [],
  );

  return (
    <div
      className={workspaceGridClass(isPackagePanelCollapsed, isFileTreeCollapsed)}
    >
      {isPackagePanelCollapsed ? (
        <aside className="grid min-h-0 grid-rows-[auto_1fr] border-r bg-sidebar text-sidebar-foreground">
          <div className="flex h-10 items-center justify-center border-b">
            <Button
              aria-label="Show package panel"
              onClick={() => setIsPackagePanelCollapsed(false)}
              size="icon-xs"
              type="button"
              variant="ghost"
            >
              <PanelLeftOpen aria-hidden="true" />
            </Button>
          </div>
          <div className="flex justify-center pt-3 text-muted-foreground">
            <Package className="size-4" aria-hidden="true" />
          </div>
        </aside>
      ) : (
        <MovePackagePanel
          activePath={activePath}
          buildStatuses={buildStatuses}
          packages={packageTree.movePackages}
          rootPackage={rootPackageName}
          selectedModulePath={selectedModule?.moveModule.filePath ?? null}
          onBuildPackage={buildPackage}
          onCollapse={() => setIsPackagePanelCollapsed(true)}
          onOpenFile={openFile}
          onSelectModule={selectModule}
        />
      )}
      <EditorTabs
        activePath={activePath}
        onActivateTab={activateTab}
        onCloseTab={closeTab}
        onSaveTab={saveTab}
        onUpdateTabSource={updateTabSource}
        dependencyGraph={packageTree.dependencyGraph}
        packageName={packageTree.rootName}
        selectedModule={selectedModule}
        tabs={tabs}
      />
      <aside className="grid min-h-0 grid-rows-[auto_1fr] border-l bg-sidebar text-sidebar-foreground">
        <div className="flex h-10 items-center justify-center border-b">
          <Button
            aria-label={isFileTreeCollapsed ? "Show file tree" : "Hide file tree"}
            onClick={() => setIsFileTreeCollapsed((isCollapsed) => !isCollapsed)}
            size="icon-xs"
            type="button"
            variant="ghost"
          >
            {isFileTreeCollapsed ? (
              <PanelRightOpen aria-hidden="true" />
            ) : (
              <PanelRightClose aria-hidden="true" />
            )}
          </Button>
        </div>
        {isFileTreeCollapsed ? null : (
          <ProjectFileTree
            packageTree={packageTree}
            selectedPath={selectedPath}
            side="right"
            onSelectPath={selectPath}
          />
        )}
      </aside>
    </div>
  );
}

function updateTab(
  tabs: OpenFileTab[],
  path: string,
  patch: Partial<OpenFileTab>,
) {
  return tabs.map((tab) => (tab.path === path ? { ...tab, ...patch } : tab));
}

function ensureTab(tabs: OpenFileTab[], path: string) {
  if (tabs.some((tab) => tab.path === path)) {
    return tabs;
  }

  return [
    ...tabs,
    {
      path,
      preview: null,
      editedSource: null,
      error: null,
      isDirty: false,
      isSaving: false,
      status: "idle" as const,
    },
  ];
}

function withTimeout<TValue>(promise: Promise<TValue>, timeoutMs: number) {
  return new Promise<TValue>((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("File preview timed out."));
    }, timeoutMs);

    promise
      .then(resolve)
      .catch(reject)
      .finally(() => window.clearTimeout(timeout));
  });
}

function summarizeCommandOutput(output: string) {
  const lines = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);

  return lines.at(-1) ?? "Command finished.";
}

function workspaceGridClass(
  isPackagePanelCollapsed: boolean,
  isFileTreeCollapsed: boolean,
) {
  if (isPackagePanelCollapsed && isFileTreeCollapsed) {
    return "grid h-full min-h-0 grid-cols-[44px_minmax(0,1fr)_44px] bg-background";
  }

  if (isPackagePanelCollapsed) {
    return "grid h-full min-h-0 grid-cols-[44px_minmax(0,1fr)_300px] bg-background";
  }

  if (isFileTreeCollapsed) {
    return "grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)_44px] bg-background";
  }

  return "grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)_300px] bg-background";
}
