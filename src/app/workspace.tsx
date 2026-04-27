import { EmptyProjectScreen } from "@/features/empty-project/empty-project-screen";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";
import { ProjectWorkspace } from "@/features/project-workspace/project-workspace";
import type { WorkspaceTab } from "@/app/titlebar";

type WorkspaceProps = {
  activeWorkspaceTab: WorkspaceTab;
  isLeftPanelOpen: boolean;
  packageTree: PackageTree | null;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
};

export function Workspace({
  activeWorkspaceTab,
  isLeftPanelOpen,
  onWorkspaceTabChange,
  packageTree,
  onProjectSelected,
}: WorkspaceProps) {
  if (packageTree) {
    return (
      <ProjectWorkspace
        activeWorkspaceTab={activeWorkspaceTab}
        isLeftPanelOpen={isLeftPanelOpen}
        onWorkspaceTabChange={onWorkspaceTabChange}
        packageTree={packageTree}
      />
    );
  }

  return <EmptyProjectScreen onProjectSelected={onProjectSelected} />;
}
