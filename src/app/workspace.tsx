import { EmptyProjectScreen } from "@/features/empty-project/empty-project-screen";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";
import type {
  BuildLogRun,
  BuildLogSheetController,
  BuildLogUpdateOptions,
} from "@/features/project-workspace/build-log-sheet";
import type { PackageLoadAssessment } from "@/features/project-workspace/package-load-assessment";
import { ProjectWorkspace } from "@/features/project-workspace/project-workspace";
import type { WorkspaceTab } from "@/app/titlebar";

type WorkspaceProps = {
  activeWorkspaceTab: WorkspaceTab;
  activePackageManifestPath: string | null;
  buildLogSheet: BuildLogSheetController;
  isLeftPanelOpen: boolean;
  isDependencyGraphLoading?: boolean;
  lastScannedAt: number | null;
  loadAssessment: PackageLoadAssessment | null;
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  packageTree: PackageTree | null;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
};

export function Workspace({
  activeWorkspaceTab,
  activePackageManifestPath,
  buildLogSheet,
  isDependencyGraphLoading = false,
  isLeftPanelOpen,
  lastScannedAt,
  loadAssessment,
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
        isDependencyGraphLoading={isDependencyGraphLoading}
        isLeftPanelOpen={isLeftPanelOpen}
        lastScannedAt={lastScannedAt}
        loadAssessment={loadAssessment}
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
