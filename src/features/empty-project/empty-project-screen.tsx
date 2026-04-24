import { ProjectDropzone } from "@/features/empty-project/project-dropzone";
import { RecentProjects } from "@/features/empty-project/recent-projects";
import type { RecentProject } from "@/features/empty-project/types";

type EmptyProjectScreenProps = {
  recentProjects?: RecentProject[];
  onOpenProject?: () => void;
  onOpenRecentProject?: (project: RecentProject) => void;
  onClearRecentProjects?: () => void;
};

export function EmptyProjectScreen({
  recentProjects = [],
  onOpenProject,
  onOpenRecentProject,
  onClearRecentProjects,
}: EmptyProjectScreenProps) {
  const handleOpenProject = onOpenProject ?? openMovePackage;

  return (
    <div className="h-full min-h-0 overflow-auto bg-background">
      <div className="mx-auto flex min-h-full w-full max-w-3xl flex-col justify-center gap-6 px-6 py-10">
        <ProjectDropzone onOpenProject={handleOpenProject} />
        <RecentProjects
          projects={recentProjects}
          onClear={onClearRecentProjects}
          onOpenProject={onOpenRecentProject}
        />
      </div>
    </div>
  );
}

async function openMovePackage() {
  const { open } = await import("@tauri-apps/plugin-dialog");

  await open({
    directory: true,
    multiple: false,
    title: "Open Move Package",
  });
}
