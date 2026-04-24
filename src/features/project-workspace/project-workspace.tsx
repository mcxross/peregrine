import React from "react";

import {
  isDirectoryPath,
  readPackageTextFile,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { FileViewer } from "@/features/project-workspace/file-viewer";
import { ProjectFileTree } from "@/features/project-workspace/project-file-tree";

type ProjectWorkspaceProps = {
  packageTree: PackageTree;
};

type FileState = {
  content: string | null;
  error: string | null;
  isLoading: boolean;
};

export function ProjectWorkspace({ packageTree }: ProjectWorkspaceProps) {
  const [selectedPath, setSelectedPath] = React.useState<string | null>(null);
  const [fileState, setFileState] = React.useState<FileState>({
    content: null,
    error: null,
    isLoading: false,
  });

  React.useEffect(() => {
    let isStale = false;

    if (!selectedPath || isDirectoryPath(selectedPath)) {
      setFileState({ content: null, error: null, isLoading: false });
      return () => {
        isStale = true;
      };
    }

    setFileState({ content: null, error: null, isLoading: true });

    readPackageTextFile(packageTree, selectedPath)
      .then((content) => {
        if (!isStale) {
          setFileState({ content, error: null, isLoading: false });
        }
      })
      .catch((error: unknown) => {
        if (!isStale) {
          setFileState({
            content: null,
            error: error instanceof Error ? error.message : "Could not read file.",
            isLoading: false,
          });
        }
      });

    return () => {
      isStale = true;
    };
  }, [packageTree, selectedPath]);

  return (
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] bg-background">
      <ProjectFileTree
        packageTree={packageTree}
        selectedPath={selectedPath}
        onSelectPath={setSelectedPath}
      />
      <FileViewer
        content={fileState.content}
        error={fileState.error}
        isLoading={fileState.isLoading}
        packageTree={packageTree}
        selectedPath={selectedPath}
      />
    </div>
  );
}
