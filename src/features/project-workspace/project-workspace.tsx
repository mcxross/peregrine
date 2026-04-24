import React from "react";

import {
  isDirectoryPath,
  loadFilePreview,
  saveTextFile,
  type FilePreview,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { EditorTabs } from "@/features/project-workspace/editor-tabs";
import { ProjectFileTree } from "@/features/project-workspace/project-file-tree";

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
  const loadingPathsRef = React.useRef<Set<string>>(new Set());
  const previewCacheRef = React.useRef<Map<string, FilePreview>>(new Map());

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
    setSelectedPath(null);
    setTabs([]);
    setActivePath(null);
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

  return (
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] bg-background">
      <ProjectFileTree
        packageTree={packageTree}
        selectedPath={selectedPath}
        onSelectPath={selectPath}
      />
      <EditorTabs
        activePath={activePath}
        onActivateTab={activateTab}
        onCloseTab={closeTab}
        onSaveTab={saveTab}
        onUpdateTabSource={updateTabSource}
        packageName={packageTree.rootName}
        tabs={tabs}
      />
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
