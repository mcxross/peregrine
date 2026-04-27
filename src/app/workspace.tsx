import { EmptyProjectScreen } from "@/features/empty-project/empty-project-screen";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";
import type { BuildLogSheetController } from "@/features/project-workspace/build-log-sheet";
import { ProjectWorkspace } from "@/features/project-workspace/project-workspace";
import type { WorkspaceTab } from "@/app/titlebar";

type WorkspaceProps = {
  activeWorkspaceTab: WorkspaceTab;
  activePackageManifestPath: string | null;
  buildLogSheet: BuildLogSheetController;
  isLeftPanelOpen: boolean;
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
  packageTree: PackageTree | null;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
};

export function Workspace({
  activeWorkspaceTab,
  activePackageManifestPath,
  buildLogSheet,
  isLeftPanelOpen,
  onActivePackageManifestPathChange,
  onWorkspaceTabChange,
  packageTree,
  onProjectSelected,
}: WorkspaceProps) {
  if (packageTree) {
    return (
      <ProjectWorkspace
        activeWorkspaceTab={activeWorkspaceTab}
        activePackageManifestPath={activePackageManifestPath}
        buildLogSheet={buildLogSheet}
        isLeftPanelOpen={isLeftPanelOpen}
        onActivePackageManifestPathChange={onActivePackageManifestPathChange}
        onWorkspaceTabChange={onWorkspaceTabChange}
        packageTree={packageTree}
      />
    );
  }

  return <EmptyProjectScreen onProjectSelected={onProjectSelected} />;
}
