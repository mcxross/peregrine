import React from "react";

import {
  loadPackageTree,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { ProjectDropzone } from "@/features/empty-project/project-dropzone";
import { RecentProjects } from "@/features/empty-project/recent-projects";
import type { RecentProject } from "@/features/empty-project/types";

type EmptyProjectScreenProps = {
  recentProjects?: RecentProject[];
  onOpenProject?: () => void;
  onOpenRecentProject?: (project: RecentProject) => void;
  onClearRecentProjects?: () => void;
  onProjectSelected?: (packageTree: PackageTree) => void;
};

export function EmptyProjectScreen({
  recentProjects = [],
  onOpenProject,
  onOpenRecentProject,
  onClearRecentProjects,
  onProjectSelected,
}: EmptyProjectScreenProps) {
  const [loadError, setLoadError] = React.useState<string | null>(null);
  const [isLoading, setIsLoading] = React.useState(false);

  const handleOpenProject = onOpenProject ?? (async () => {
    setLoadError(null);
    setIsLoading(true);

    try {
      const packagePath = await openMovePackage();

      if (!packagePath) {
        return;
      }

      onProjectSelected?.(await loadPackageTree(packagePath));
    } catch (error) {
      setLoadError(getOpenPackageErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  });

  return (
    <div className="h-full min-h-0 overflow-auto bg-background">
      <div className="mx-auto flex min-h-full w-full max-w-3xl flex-col justify-center gap-6 px-6 py-10">
        <ProjectDropzone onOpenProject={handleOpenProject} isLoading={isLoading} />
        {loadError ? (
          <p className="rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {loadError}
          </p>
        ) : null}
        <RecentProjects
          projects={recentProjects}
          onClear={onClearRecentProjects}
          onOpenProject={onOpenRecentProject}
        />
      </div>
    </div>
  );
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
