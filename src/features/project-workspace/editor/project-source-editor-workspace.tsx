import React from "react";

import {
  isDirectoryPath,
  loadFilePreview,
  type MoveModule,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { EditorTabs } from "@/features/project-workspace/editor/editor-tabs";
import { ProjectFileTree } from "@/features/project-workspace/editor/project-file-tree";
import type { OpenFileTab } from "@/features/project-workspace/editor/types";
import { findModuleByPath } from "@/features/project-workspace/source-paths";

type ProjectSourceEditorWorkspaceProps = {
  activeMovePackage: MovePackage | null;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  packageTree: PackageTree;
};

export function ProjectSourceEditorWorkspace({
  activeMovePackage,
  onSelectModule,
  packageTree,
}: ProjectSourceEditorWorkspaceProps) {
  const [selectedPath, setSelectedPath] = React.useState<string | null>(null);
  const [activePath, setActivePath] = React.useState<string | null>(null);
  const [tabs, setTabs] = React.useState<OpenFileTab[]>([]);
  const rootPathRef = React.useRef(packageTree.rootPath);

  React.useEffect(() => {
    rootPathRef.current = packageTree.rootPath;
    setActivePath(null);
    setSelectedPath(null);
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

      void loadFilePreview(packageTree, path)
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
      }
    },
    [activeMovePackage, onSelectModule, packageTree],
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
    },
    [activePath, tabs],
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

  return (
    <section
      className="grid h-full min-h-0 bg-[var(--app-window)]"
      style={{ gridTemplateColumns: "280px minmax(0,1fr)" }}
    >
      <ProjectFileTree
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
        tabs={tabs}
        onActivateTab={(path) => {
          setActivePath(path);
          setSelectedPath(path);
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
