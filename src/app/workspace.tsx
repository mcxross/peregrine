import { EmptyProjectScreen } from "@/features/empty-project/empty-project-screen";
import type { SuiNetworkSelection } from "@/app/sui-network";
import type {
  FormalVerificationTarget,
  WorkspaceMode,
  WorkspaceTab,
} from "@/app/workspace-types";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";
import type {
  BuildLogRun,
  BuildLogSheetController,
  BuildLogUpdateOptions,
} from "@/features/project-workspace/build-log-sheet";
import type { PackageLoadAssessment } from "@/features/project-workspace/package-load-assessment";
import { ProjectWorkspace } from "@/features/project-workspace/project-workspace";

type WorkspaceProps = {
  activeWorkspaceTab: WorkspaceTab;
  activePackageManifestPath: string | null;
  buildLogSheet: BuildLogSheetController;
  isLeftPanelOpen: boolean;
  isDependencyGraphLoading?: boolean;
  lastScannedAt: number | null;
  loadAssessment: PackageLoadAssessment | null;
  mode: WorkspaceMode;
  network: SuiNetworkSelection;
  onNetworkChange: (network: SuiNetworkSelection) => void;
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  onFormalVerificationTargetChange: (target: FormalVerificationTarget | null) => void;
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
  mode,
  network,
  onNetworkChange,
  onActivePackageManifestPathChange,
  onCommandLog,
  onFormalVerificationTargetChange,
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
        mode={mode}
        onActivePackageManifestPathChange={onActivePackageManifestPathChange}
        onCommandLog={onCommandLog}
        onFormalVerificationTargetChange={onFormalVerificationTargetChange}
        onProjectSelected={onProjectSelected}
        onWorkspaceTabChange={onWorkspaceTabChange}
        packageTree={packageTree}
      />
    );
  }

  return (
    <EmptyProjectScreen
      network={network}
      onNetworkChange={onNetworkChange}
      onProjectSelected={onProjectSelected}
    />
  );
}
