import React from "react";
import { EmptyProjectScreen } from "@/features/empty-project/empty-project-screen";
import type { SuiNetworkSelection } from "@peregrine/desktop-runtime";
import type {
  FormalVerificationTarget,
  WorkspaceMode,
  WorkspaceTab,
} from "@peregrine/desktop-runtime";
import type { PackageTree } from "@peregrine/desktop-runtime";
import type {
  BuildLogRun,
  BuildLogSheetController,
  BuildLogUpdateOptions,
} from "@/features/project-workspace/build-log-sheet";
import type { PackageLoadAssessment } from "@peregrine/desktop-runtime";
import type { AuditReportExport } from "@peregrine/desktop-runtime";

const ProjectWorkspace = React.lazy(() =>
  import("@/features/project-workspace/project-workspace").then((module) => ({
    default: module.ProjectWorkspace,
  })),
);

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
  onToggleMode: () => void;
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
  onAuditReportExportReady?: (report: AuditReportExport | null) => void;
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
  onToggleMode,
  onActivePackageManifestPathChange,
  onAuditReportExportReady,
  onCommandLog,
  onFormalVerificationTargetChange,
  onWorkspaceTabChange,
  packageTree,
  onProjectSelected,
}: WorkspaceProps) {
  if (packageTree) {
    return (
      <React.Suspense fallback={<WorkspaceLoadingState />}>
        <ProjectWorkspace
          activeWorkspaceTab={activeWorkspaceTab}
          activePackageManifestPath={activePackageManifestPath}
          buildLogSheet={buildLogSheet}
          isDependencyGraphLoading={isDependencyGraphLoading}
          isLeftPanelOpen={isLeftPanelOpen}
          lastScannedAt={lastScannedAt}
          loadAssessment={loadAssessment}
          mode={mode}
          network={network}
          onToggleMode={onToggleMode}
          onActivePackageManifestPathChange={onActivePackageManifestPathChange}
          onAuditReportExportReady={onAuditReportExportReady}
          onCommandLog={onCommandLog}
          onFormalVerificationTargetChange={onFormalVerificationTargetChange}
          onProjectSelected={onProjectSelected}
          onWorkspaceTabChange={onWorkspaceTabChange}
          packageTree={packageTree}
        />
      </React.Suspense>
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

function WorkspaceLoadingState() {
  return (
    <div className="grid h-full min-h-0 place-items-center bg-[var(--app-window)] px-6 text-sm text-muted-foreground">
      Loading workspace...
    </div>
  );
}
