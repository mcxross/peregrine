import React from "react";

import {
  isDirectoryPath,
  loadFilePreview,
  saveTextFile,
  type MoveModule,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import type { CodeEditorJumpRequest } from "@/features/project-workspace/editor/code-editor";
import { EditorTabs } from "@/features/project-workspace/editor/editor-tabs";
import type {
  MoveAnalyzerResolvedLocation,
  MoveAnalyzerWorkspaceEdit,
} from "@/features/project-workspace/editor/lsp/types";
import { useMoveAnalyzer } from "@/features/project-workspace/editor/lsp/use-move-analyzer";
import { applyMoveAnalyzerTextEdits } from "@/features/project-workspace/editor/lsp/workspace-edit";
import { ProjectFileTree } from "@/features/project-workspace/editor/project-file-tree";
import type { OpenFileTab } from "@/features/project-workspace/editor/types";
import { findModuleByPath } from "@/features/project-workspace/source-paths";

type ProjectSourceEditorWorkspaceProps = {
  activeMovePackage: MovePackage | null;
  onBackToSecurity?: () => void;
  onClearSelectedModule: () => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  packageTree: PackageTree;
};

export function ProjectSourceEditorWorkspace({
  activeMovePackage,
  onBackToSecurity,
  onClearSelectedModule,
  onSelectModule,
  packageTree,
}: ProjectSourceEditorWorkspaceProps) {
  const [selectedPath, setSelectedPath] = React.useState<string | null>(null);
  const [activePath, setActivePath] = React.useState<string | null>(null);
  const [jumpRequestByPath, setJumpRequestByPath] = React.useState<Record<string, CodeEditorJumpRequest>>({});
  const [tabs, setTabs] = React.useState<OpenFileTab[]>([]);
  const rootPathRef = React.useRef(packageTree.rootPath);
  const moveAnalyzer = useMoveAnalyzer({
    rootPath: packageTree.rootPath,
    tabs,
  });

  React.useEffect(() => {
    rootPathRef.current = packageTree.rootPath;
    setActivePath(null);
    setSelectedPath(null);
    setJumpRequestByPath({});
    setTabs([]);
  }, [packageTree.rootPath]);

  const openFile = React.useCallback(
    (path: string) => {
      if (isDirectoryPath(path)) {
        return;
      }

      setSelectedPath(path);
      setActivePath(path);
      const rootPathAtRequest = packageTree.rootPath;
      setTabs((current) => {
        if (current.some((tab) => tab.path === path)) {
          return current;
        }

        return [...current, createOpenFileTab(path)];
      });

      void loadFilePreview(packageTree, path, { includeHighlightedHtml: false })
        .then((preview) => {
          if (rootPathRef.current !== rootPathAtRequest) {
            return;
          }

          setTabs((current) =>
            current.map((tab) =>
              tab.path === path
                ? {
                    ...tab,
                    error: null,
                    preview,
                    status: "loaded",
                  }
                : tab,
            ),
          );
        })
        .catch((error: unknown) => {
          if (rootPathRef.current !== rootPathAtRequest) {
            return;
          }

          setTabs((current) =>
            current.map((tab) =>
              tab.path === path
                ? {
                    ...tab,
                    error:
                      error instanceof Error
                        ? error.message
                        : "Could not load this file.",
                    status: "error",
                  }
                : tab,
            ),
          );
        });

      const selectedMoveModule = findModuleByPath(
        packageTree.movePackages,
        path,
        activeMovePackage,
      );

      if (selectedMoveModule) {
        onSelectModule(
          selectedMoveModule.movePackage,
          selectedMoveModule.moveModule,
        );
      } else {
        onClearSelectedModule();
      }
    },
    [activeMovePackage, onClearSelectedModule, onSelectModule, packageTree],
  );

  const selectOpenModule = React.useCallback(
    (path: string) => {
      const selectedMoveModule = findModuleByPath(
        packageTree.movePackages,
        path,
        activeMovePackage,
      );

      if (selectedMoveModule) {
        onSelectModule(
          selectedMoveModule.movePackage,
          selectedMoveModule.moveModule,
        );
      } else {
        onClearSelectedModule();
      }
    },
    [activeMovePackage, onClearSelectedModule, onSelectModule, packageTree.movePackages],
  );

  const closeTab = React.useCallback(
    (path: string) => {
      const nextTabs = tabs.filter((tab) => tab.path !== path);

      setTabs(nextTabs);

      if (activePath !== path) {
        return;
      }

      const closedIndex = tabs.findIndex((tab) => tab.path === path);
      const nextActivePath =
        nextTabs[Math.max(0, closedIndex - 1)]?.path ??
        nextTabs[0]?.path ??
        null;

      setActivePath(nextActivePath);
      setSelectedPath(nextActivePath);
      if (nextActivePath) {
        selectOpenModule(nextActivePath);
      } else {
        onClearSelectedModule();
      }
    },
    [activePath, onClearSelectedModule, selectOpenModule, tabs],
  );

  const updateTabSource = React.useCallback((path: string, source: string) => {
    setTabs((current) =>
      current.map((tab) =>
        tab.path === path
          ? {
              ...tab,
              editedSource: source,
              isDirty:
                tab.preview?.kind === "text"
                  ? source !== tab.preview.source
                  : true,
            }
          : tab,
      ),
    );
  }, []);

  const openMoveAnalyzerLocation = React.useCallback((location: MoveAnalyzerResolvedLocation) => {
    setJumpRequestByPath((current) => ({
      ...current,
      [location.path]: {
        line: location.range.start.line + 1,
        token: Date.now(),
      },
    }));
    openFile(location.path);
  }, [openFile]);

  const applyMoveAnalyzerWorkspaceEdit = React.useCallback(async (workspaceEdit: MoveAnalyzerWorkspaceEdit) => {
    const rootPathAtRequest = packageTree.rootPath;
    const nextSourcesByPath = new Map<string, string>();
    const openTabsByPath = new Map(tabs.map((tab) => [tab.path, tab] as const));

    for (const [path, edits] of Object.entries(workspaceEdit.editsByPath)) {
      const openTab = openTabsByPath.get(path);
      const openSource = openTab?.preview?.kind === "text"
        ? openTab.editedSource ?? openTab.preview.source
        : null;
      const preview = openSource != null
        ? null
        : await loadFilePreview(packageTree, path, { includeHighlightedHtml: false });
      const source = openSource
        ?? (preview?.kind === "text" || preview?.kind === "markdown" ? preview.source : null);

      if (source == null) {
        continue;
      }

      const nextSource = applyMoveAnalyzerTextEdits(source, edits);

      nextSourcesByPath.set(path, nextSource);

      if (!openTab && rootPathRef.current === rootPathAtRequest) {
        await saveTextFile(packageTree, path, nextSource, { includeHighlightedHtml: false });
      }
    }

    if (rootPathRef.current !== rootPathAtRequest || !nextSourcesByPath.size) {
      return;
    }

    setTabs((current) =>
      current.map((tab) => {
        const nextSource = nextSourcesByPath.get(tab.path);

        if (nextSource == null || tab.preview?.kind !== "text") {
          return tab;
        }

        return {
          ...tab,
          editedSource: nextSource,
          isDirty: nextSource !== tab.preview.source,
        };
      }),
    );
  }, [packageTree, tabs]);

  const activeJumpRequest = activePath ? jumpRequestByPath[activePath] ?? null : null;

  return (
    <section
      className="grid h-full min-h-0 bg-[var(--app-window)]"
      style={{ gridTemplateColumns: "280px minmax(0,1fr)" }}
    >
      <ProjectFileTree
        onBackToSecurity={onBackToSecurity}
        packageTree={packageTree}
        selectedPath={selectedPath}
        onSelectPath={(path) => {
          if (!path) {
            return;
          }

          if (isDirectoryPath(path)) {
            setSelectedPath(path);
            return;
          }

          if (path !== activePath) {
            openFile(path);
          }
        }}
      />
      <EditorTabs
        activePath={activePath}
        diagnosticsByPath={moveAnalyzer.diagnosticsByPath}
        jumpRequest={activeJumpRequest}
        moveAnalyzerLspFeatures={moveAnalyzer.lspFeatures}
        moveAnalyzerStatus={moveAnalyzer.status}
        onMoveAnalyzerLocation={openMoveAnalyzerLocation}
        onMoveAnalyzerWorkspaceEdit={applyMoveAnalyzerWorkspaceEdit}
        tabs={tabs}
        onActivateTab={(path) => {
          setActivePath(path);
          setSelectedPath(path);
          selectOpenModule(path);
        }}
        onCloseTab={closeTab}
        onUpdateTabSource={updateTabSource}
      />
    </section>
  );
}

function createOpenFileTab(path: string): OpenFileTab {
  return {
    editedSource: null,
    error: null,
    isDirty: false,
    isSaving: false,
    path,
    preview: null,
    status: "loading",
  };
}
