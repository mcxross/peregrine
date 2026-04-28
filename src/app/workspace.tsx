import { EmptyProjectScreen } from "@/features/empty-project/empty-project-screen";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";
import type {
  BuildLogRun,
  BuildLogSheetController,
} from "@/features/project-workspace/build-log-sheet";
import { ProjectWorkspace } from "@/features/project-workspace/project-workspace";
import type { WorkspaceTab } from "@/app/titlebar";

type WorkspaceProps = {
  activeWorkspaceTab: WorkspaceTab;
  activePackageManifestPath: string | null;
  buildLogSheet: BuildLogSheetController;
  isLeftPanelOpen: boolean;
  lastScannedAt: number | null;
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
  onCommandLog: (run: BuildLogRun) => void;
  packageTree: PackageTree | null;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
};

export function Workspace({
  activeWorkspaceTab,
  activePackageManifestPath,
  buildLogSheet,
  isLeftPanelOpen,
  lastScannedAt,
  onActivePackageManifestPathChange,
  onCommandLog,
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
        lastScannedAt={lastScannedAt}
        onActivePackageManifestPathChange={onActivePackageManifestPathChange}
        onCommandLog={onCommandLog}
        onProjectSelected={onProjectSelected}
        onWorkspaceTabChange={onWorkspaceTabChange}
        packageTree={packageTree}
      />
    );
  }

  return <EmptyProjectScreen onProjectSelected={onProjectSelected} />;
}
