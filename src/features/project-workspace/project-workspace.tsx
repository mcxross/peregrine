import React from "react";
import {
  Boxes,
  Binary,
  Bot,
  Check,
  ChevronDown,
  Code2,
  FileCode2,
  Gauge,
  GitBranch,
  Link2,
  Network,
  PanelLeftOpen,
  PanelRightClose,
  ScanEye,
  ShieldAlert,
  ShieldCheck,
  Workflow,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type {
  FormalVerificationTarget,
  WorkspaceMode,
  WorkspaceTab,
} from "@/app/workspace-types";
import type { SuiNetworkSelection } from "@/app/sui-network";
import {
  displayMovePackageName,
  type MoveModule,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import {
  BuildLogSheet,
  type BuildLogRun,
  type BuildLogSheetController,
  type BuildLogUpdateOptions,
} from "@/features/project-workspace/build-log-sheet";
import type { AuditReportExport } from "@/features/agents/types";
import { assessmentSidebarItems } from "@/features/project-workspace/package-load-assessment-cards";
import type { PackageLoadAssessment } from "@/features/project-workspace/package-load-assessment";
import type { SelectedMoveModule } from "@/features/project-workspace/module-signature-screen";
import type { ExploreGraphMode } from "@/features/project-workspace/dependency-graph-screen";
import { findSourceModule } from "@/features/project-workspace/source-paths";
import type { TypeGraphSourceLocation } from "@/features/project-workspace/type-graph-view";
import type { SurfaceDetailKind } from "@/features/project-workspace/surface-detail-screen";
import {
  workspaceSidebarWidth,
  workspaceStatusBarHeight,
} from "@/layout/window-chrome";
import { cn } from "@/lib/utils";

const AgentsScreen = React.lazy(() =>
  import("@/features/agents/agents-screen").then((module) => ({
    default: module.AgentsScreen,
  })),
);
const BytecodeViewScreen = React.lazy(() =>
  import("@/features/project-workspace/bytecode-view-screen").then((module) => ({
    default: module.BytecodeViewScreen,
  })),
);
const DependencyGraphScreen = React.lazy(() =>
  import("@/features/project-workspace/dependency-graph-screen").then((module) => ({
    default: module.DependencyGraphScreen,
  })),
);
const ExecutionBuilderScreen = React.lazy(() =>
  import("@/features/project-workspace/execution-builder-screen").then((module) => ({
    default: module.ExecutionBuilderScreen,
  })),
);
const MovePackagesOverviewScreen = React.lazy(() =>
  import("@/features/project-workspace/move-packages-overview-screen").then((module) => ({
    default: module.MovePackagesOverviewScreen,
  })),
);
const ProjectSourceEditorWorkspace = React.lazy(() =>
  import("@/features/project-workspace/editor/project-source-editor-workspace").then((module) => ({
    default: module.ProjectSourceEditorWorkspace,
  })),
);
const SurfaceDetailScreen = React.lazy(() =>
  import("@/features/project-workspace/surface-detail-screen").then((module) => ({
    default: module.SurfaceDetailScreen,
  })),
);

type ProjectWorkspaceProps = {
  activeWorkspaceTab: WorkspaceTab;
  activePackageManifestPath: string | null;
  buildLogSheet: BuildLogSheetController;
  isDependencyGraphLoading?: boolean;
  isLeftPanelOpen: boolean;
  lastScannedAt: number | null;
  loadAssessment: PackageLoadAssessment | null;
  mode: WorkspaceMode;
  network: SuiNetworkSelection;
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
  onAuditReportExportReady?: (report: AuditReportExport | null) => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  onFormalVerificationTargetChange: (target: FormalVerificationTarget | null) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  onToggleMode: () => void;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
  packageTree: PackageTree;
};

export type SourceJumpRequest = TypeGraphSourceLocation & {
  token: number;
};

type ExploreTab = "code" | "bytecode" | ExploreGraphMode;

type WorkspaceErrorBoundaryProps = {
  children: React.ReactNode;
  resetKey: string;
};

type WorkspaceErrorBoundaryState = {
  error: Error | null;
  info: React.ErrorInfo | null;
};

class WorkspaceErrorBoundary extends React.Component<WorkspaceErrorBoundaryProps, WorkspaceErrorBoundaryState> {
  state: WorkspaceErrorBoundaryState = {
    error: null,
    info: null,
  };

  static getDerivedStateFromError(error: Error): WorkspaceErrorBoundaryState {
    return { error, info: null };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("[ProjectWorkspace] render error boundary caught crash", {
      componentStack: info.componentStack,
      error,
      message: error.message,
      stack: error.stack,
    });
    this.setState({ info });
  }

  componentDidUpdate(previousProps: WorkspaceErrorBoundaryProps) {
    if (previousProps.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null, info: null });
    }
  }

  render() {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <div className="grid h-full min-h-0 place-items-center bg-[var(--app-window)] px-6">
        <div className="max-w-xl rounded-md border border-red-500/25 bg-red-500/10 p-4 text-sm text-red-100">
          <div className="font-semibold">Workspace render error</div>
          <p className="mt-2 text-xs leading-5 text-red-100/80">
            {this.state.error.message || "An unknown render error occurred."}
          </p>
          {this.state.info?.componentStack ? (
            <pre className="mt-3 max-h-44 overflow-auto whitespace-pre-wrap rounded border border-red-500/20 bg-black/25 p-2 text-[10px] leading-4 text-red-100/70">
              {this.state.info.componentStack}
            </pre>
          ) : null}
        </div>
      </div>
    );
  }
}

export function ProjectWorkspace({
  activePackageManifestPath,
  activeWorkspaceTab,
  buildLogSheet,
  isDependencyGraphLoading = false,
  isLeftPanelOpen,
  lastScannedAt,
  loadAssessment,
  mode,
  network,
  onActivePackageManifestPathChange,
  onAuditReportExportReady,
  onCommandLog,
  onFormalVerificationTargetChange,
  onProjectSelected,
  onToggleMode,
  onWorkspaceTabChange,
  packageTree,
}: ProjectWorkspaceProps) {
  const [isRightPanelOpen, setIsRightPanelOpen] = React.useState(true);
  const [activeExploreTab, setActiveExploreTab] = React.useState<ExploreTab>("code");
  const [selectedModule, setSelectedModule] = React.useState<SelectedMoveModule | null>(null);
  const [activeSurfaceDetail, setActiveSurfaceDetail] = React.useState<SurfaceDetailKind | null>(null);
  const [sourceJumpRequest, setSourceJumpRequest] = React.useState<SourceJumpRequest | null>(null);
  const activeMovePackage =
    packageTree.movePackages.length === 1
      ? packageTree.movePackages[0]
      : packageTree.movePackages.find(
          (movePackage) => movePackage.manifestPath === activePackageManifestPath,
        ) ?? null;
  const packageName = activeMovePackage?.name || packageTree.rootName || packageTree.movePackages[0]?.name || "savings_personal";
  const isEditorMode = mode === "editor";
  const hasInspectorColumn =
    activeWorkspaceTab !== "Agents"
    && !(activeWorkspaceTab === "Explore" && activeExploreTab === "bytecode");
  const workspaceColumns = isEditorMode
    ? "minmax(0, 1fr)"
    : isLeftPanelOpen
    ? hasInspectorColumn
      ? `${workspaceSidebarWidth}px minmax(0, 1fr) ${isRightPanelOpen ? "clamp(360px, 24vw, 400px)" : "44px"}`
      : `${workspaceSidebarWidth}px minmax(0, 1fr)`
    : hasInspectorColumn
      ? `minmax(0, 1fr) ${isRightPanelOpen ? "clamp(360px, 24vw, 400px)" : "44px"}`
      : "minmax(0, 1fr)";

  React.useEffect(() => {
    setSelectedModule(null);
    setActiveSurfaceDetail(null);
    onFormalVerificationTargetChange(null);
  }, [activePackageManifestPath, onFormalVerificationTargetChange, packageTree.rootPath]);

  React.useEffect(() => {
    onFormalVerificationTargetChange(
      selectedModule
        ? {
            filePath: selectedModule.moveModule.filePath,
            moduleName: selectedModule.moveModule.name,
            packageName: selectedModule.movePackage.name,
            packagePath: selectedModule.movePackage.path || ".",
          }
        : null,
    );
  }, [onFormalVerificationTargetChange, selectedModule]);

  React.useEffect(() => {
    const firstModule = activeMovePackage?.modules[0] ?? null;

    setSelectedModule(
      firstModule && activeMovePackage
        ? { moveModule: firstModule, movePackage: activeMovePackage }
        : null,
    );
  }, [activeMovePackage]);

  React.useEffect(() => {
    if (
      activeWorkspaceTab === "Agents"
      || activeWorkspaceTab === "Execution"
      || (activeWorkspaceTab === "Explore" && activeExploreTab !== "code")
    ) {
      setIsRightPanelOpen(false);
    }
  }, [activeExploreTab, activeWorkspaceTab]);

  React.useEffect(() => {
    const onError = (event: ErrorEvent) => {
      if (isResizeObserverLoopWarning(event.message)) {
        event.preventDefault();
        return;
      }

      console.error("[ProjectWorkspace] uncaught browser error", {
        colno: event.colno,
        error: event.error,
        filename: event.filename,
        lineno: event.lineno,
        message: event.message,
      });
    };
    const onUnhandledRejection = (event: PromiseRejectionEvent) => {
      console.error("[ProjectWorkspace] unhandled promise rejection", {
        reason: event.reason,
      });
    };

    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onUnhandledRejection);

    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onUnhandledRejection);
    };
  }, []);

  const openSourceLocation = React.useCallback(
    (location: TypeGraphSourceLocation) => {
      if (import.meta.env.DEV) {
        console.info("[ProjectWorkspace] source jump requested", {
          activePackageManifestPath,
          location,
          packages: packageTree.movePackages.map((movePackage) => ({
            manifestPath: movePackage.manifestPath,
            moduleCount: movePackage.modules.length,
            name: movePackage.name,
            path: movePackage.path,
          })),
        });
      }
      const match = findSourceModule(packageTree.movePackages, location);

      if (!match) {
        console.warn("[ProjectWorkspace] source jump has no matching module", {
          location,
          modulePaths: packageTree.movePackages.flatMap((movePackage) =>
            movePackage.modules.map((moveModule) => ({
              filePath: moveModule.filePath,
              module: moveModule.name,
              packagePath: movePackage.path,
            })),
          ),
        });
        return;
      }

      if (import.meta.env.DEV) {
        console.info("[ProjectWorkspace] source jump matched module", {
          filePath: match.moveModule.filePath,
          line: location.line,
          module: match.moveModule.name,
          package: match.movePackage.name,
        });
      }
      setActiveSurfaceDetail(null);
      setSelectedModule({ moveModule: match.moveModule, movePackage: match.movePackage });
      onActivePackageManifestPathChange(match.movePackage.manifestPath);
      setSourceJumpRequest({
        filePath: match.moveModule.filePath,
        line: location.line,
        token: Date.now(),
      });
      setActiveExploreTab("code");
      onWorkspaceTabChange("Explore");
    },
    [activePackageManifestPath, onActivePackageManifestPathChange, onWorkspaceTabChange, packageTree.movePackages],
  );

  return (
    <div
      className="relative grid h-full min-h-0 overflow-hidden bg-[var(--app-window)] text-foreground"
      style={{
        gridTemplateColumns: workspaceColumns,
      }}
    >
      {!isEditorMode && isLeftPanelOpen ? (
        <SecuritySidebar
          activeMovePackage={activeMovePackage}
          activeSurfaceDetail={activeSurfaceDetail}
          activeWorkspaceTab={activeWorkspaceTab}
          loadAssessment={loadAssessment}
          packageTree={packageTree}
          onSelectSurfaceDetail={(detail) => {
            setActiveSurfaceDetail(detail);
            setSelectedModule(null);
            onWorkspaceTabChange("Explore");
          }}
          onSelectPackage={(movePackage) => {
            onActivePackageManifestPathChange(movePackage.manifestPath);
            setActiveSurfaceDetail(null);
            setSelectedModule(null);
          }}
          onWorkspaceTabChange={(tab) => {
            setActiveSurfaceDetail(null);
            onWorkspaceTabChange(tab);
          }}
        />
      ) : null}

      <main
        className="relative grid min-h-0 overflow-hidden border-r border-[color:var(--app-border)] bg-[var(--app-window)]"
        style={{ gridTemplateRows: `minmax(0, 1fr) ${workspaceStatusBarHeight}px` }}
      >
        <WorkspaceErrorBoundary
          resetKey={`${mode}:${packageTree.rootPath}:${activeWorkspaceTab}:${activePackageManifestPath ?? ""}:${sourceJumpRequest?.token ?? "no-source-jump"}`}
        >
          <React.Suspense fallback={<WorkspacePanelLoadingState />}>
            <WorkspaceMainPanel
              activeWorkspaceTab={activeWorkspaceTab}
              activeExploreTab={activeExploreTab}
              activeSurfaceDetail={activeSurfaceDetail}
              activeMovePackage={activeMovePackage}
              isDependencyGraphLoading={isDependencyGraphLoading}
              mode={mode}
              network={network}
              packageTree={packageTree}
              packageName={packageName}
              selectedModule={selectedModule}
              onAuditReportExportReady={onAuditReportExportReady}
              onCommandLog={onCommandLog}
              onProjectSelected={onProjectSelected}
              onToggleMode={onToggleMode}
              onExploreTabChange={setActiveExploreTab}
              onClearSelectedModule={() => setSelectedModule(null)}
              onOpenSourceLocation={openSourceLocation}
              onSelectModule={(movePackage, moveModule) => {
                setActiveSurfaceDetail(null);
                setSelectedModule({ moveModule, movePackage });
              }}
              sourceJumpRequest={sourceJumpRequest}
            />
          </React.Suspense>
        </WorkspaceErrorBoundary>
        <BuildLogSheet
          bottomInset={workspaceStatusBarHeight}
          isOpen={buildLogSheet.isOpen}
          onClose={buildLogSheet.onClose}
          onRerun={buildLogSheet.onRerun}
          runs={buildLogSheet.runs}
        />
        <footer className="flex items-center justify-end overflow-hidden border-t border-[color:var(--app-border)] bg-[var(--app-chrome)] px-4 text-[11px] leading-5 text-muted-foreground">
          <LastScannedStatus scannedAt={lastScannedAt} />
        </footer>
      </main>

      {!isEditorMode && hasInspectorColumn ? (
        isRightPanelOpen ? (
          <InspectorPanel
            onCollapse={() => setIsRightPanelOpen(false)}
          />
        ) : (
          <CollapsedPanelRail
            label="Open inspector"
            side="right"
            onOpen={() => setIsRightPanelOpen(true)}
          />
        )
      ) : null}
    </div>
  );
}

function WorkspacePanelLoadingState() {
  return (
    <div className="grid h-full min-h-0 place-items-center bg-[var(--app-window)] px-6 text-sm text-muted-foreground">
      Loading view...
    </div>
  );
}

function WorkspaceMainPanel({
  activeWorkspaceTab,
  activeExploreTab,
  activeSurfaceDetail,
  activeMovePackage,
  isDependencyGraphLoading,
  mode,
  network,
  onClearSelectedModule,
  onAuditReportExportReady,
  onExploreTabChange,
  onOpenSourceLocation,
  onSelectModule,
  onToggleMode,
  packageTree,
  packageName,
  selectedModule,
  sourceJumpRequest,
  onProjectSelected,
  onCommandLog,
}: {
  activeWorkspaceTab: WorkspaceTab;
  activeExploreTab: ExploreTab;
  activeSurfaceDetail: SurfaceDetailKind | null;
  activeMovePackage: MovePackage | null;
  isDependencyGraphLoading: boolean;
  mode: WorkspaceMode;
  network: SuiNetworkSelection;
  onClearSelectedModule: () => void;
  onAuditReportExportReady?: (report: AuditReportExport | null) => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  onExploreTabChange: (tab: ExploreTab) => void;
  onOpenSourceLocation: (location: TypeGraphSourceLocation) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  onToggleMode: () => void;
  packageTree: PackageTree;
  packageName: string;
  selectedModule: SelectedMoveModule | null;
  sourceJumpRequest: SourceJumpRequest | null;
}) {
  if (mode === "editor") {
    return (
      <ProjectSourceEditorWorkspace
        activeMovePackage={activeMovePackage}
        onBackToSecurity={onToggleMode}
        onClearSelectedModule={onClearSelectedModule}
        onSelectModule={onSelectModule}
        packageTree={packageTree}
      />
    );
  }

  if (activeSurfaceDetail) {
    return <SurfaceDetailScreen detail={activeSurfaceDetail} movePackage={activeMovePackage} />;
  }

  if (activeWorkspaceTab === "Execution") {
    return (
      <ExecutionBuilderScreen
        activeMovePackage={activeMovePackage}
        network={network}
        onCommandLog={onCommandLog}
        onProjectSelected={onProjectSelected}
        packageTree={packageTree}
      />
    );
  }

  if (activeWorkspaceTab === "Agents") {
    return (
      <AgentsScreen
        activeMovePackage={activeMovePackage}
        onAuditReportExportReady={onAuditReportExportReady}
        packageTree={packageTree}
        projectRootPath={packageTree.rootPath}
      />
    );
  }

  return (
    <ExploreMainPanel
      activeExploreTab={activeExploreTab}
      activeMovePackage={activeMovePackage}
      isDependencyGraphLoading={isDependencyGraphLoading}
      onClearSelectedModule={onClearSelectedModule}
      onExploreTabChange={onExploreTabChange}
      onOpenSourceLocation={onOpenSourceLocation}
      onProjectSelected={onProjectSelected}
      onSelectModule={onSelectModule}
      onToggleMode={onToggleMode}
      packageName={packageName}
      packageTree={packageTree}
      selectedModule={selectedModule}
      sourceJumpRequest={sourceJumpRequest}
    />
  );
}

function ExploreMainPanel({
  activeExploreTab,
  activeMovePackage,
  isDependencyGraphLoading,
  onClearSelectedModule,
  onExploreTabChange,
  onOpenSourceLocation,
  onProjectSelected,
  onSelectModule,
  onToggleMode,
  packageName,
  packageTree,
  selectedModule,
  sourceJumpRequest,
}: {
  activeExploreTab: ExploreTab;
  activeMovePackage: MovePackage | null;
  isDependencyGraphLoading: boolean;
  onClearSelectedModule: () => void;
  onExploreTabChange: (tab: ExploreTab) => void;
  onOpenSourceLocation: (location: TypeGraphSourceLocation) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  onToggleMode: () => void;
  packageName: string;
  packageTree: PackageTree;
  selectedModule: SelectedMoveModule | null;
  sourceJumpRequest: SourceJumpRequest | null;
}) {
  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <div className="row-start-1 grid min-w-0 grid-cols-[1fr_auto] items-center gap-3 px-5 pb-2 pt-2">
        <ExploreTabSwitch activeTab={activeExploreTab} onTabChange={onExploreTabChange} />
        <WorkspaceModeSwitch mode="security" onToggleMode={onToggleMode} />
      </div>

      <div className="row-start-2 min-h-0 overflow-hidden">
        {activeExploreTab === "code" ? (
          <MovePackagesOverviewScreen
            activeMovePackage={activeMovePackage}
            packageTree={packageTree}
            onClearSelectedModule={onClearSelectedModule}
            onSelectModule={onSelectModule}
            selectedModule={selectedModule}
            sourceJumpRequest={sourceJumpRequest}
          />
        ) : activeExploreTab === "bytecode" ? (
          <BytecodeViewScreen
            activeMovePackage={activeMovePackage}
            packageTree={packageTree}
          />
        ) : (
          <DependencyGraphScreen
            activeMovePackage={activeMovePackage}
            callGraph={packageTree.callGraph}
            graph={packageTree.dependencyGraph}
            graphMode={activeExploreTab}
            isDependencyGraphLoading={isDependencyGraphLoading}
            onMoveGraphsLoaded={(graphs) =>
              onProjectSelected({
                ...packageTree,
                callGraph: graphs.callGraph,
                typeGraph: graphs.typeGraph,
                stateAccessGraph: graphs.stateAccessGraph,
              })
            }
            onOpenSourceLocation={onOpenSourceLocation}
            packageName={packageName}
            rootPath={packageTree.rootPath}
            typeGraph={packageTree.typeGraph}
          />
        )}
      </div>
    </section>
  );
}

function WorkspaceModeSwitch({
  mode,
  onToggleMode,
}: {
  mode: WorkspaceMode;
  onToggleMode: () => void;
}) {
  const isSecurityMode = mode === "security";
  const Icon = isSecurityMode ? Code2 : ShieldCheck;
  const label = isSecurityMode ? "Editor" : "Security";

  return (
    <Button
      aria-label={isSecurityMode ? "Open editor workspace" : "Back to security workspace"}
      className="h-8 shrink-0 gap-1.5 rounded bg-[var(--app-elevated)] px-3 text-xs font-medium text-foreground shadow-sm hover:bg-[var(--app-elevated)] hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/60"
      onClick={onToggleMode}
      title={label}
      type="button"
      variant="ghost"
    >
      <Icon className="size-3.5" aria-hidden="true" />
      <span>{label}</span>
    </Button>
  );
}

const exploreTabs: Array<{
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  id: ExploreTab;
  label: string;
}> = [
  { id: "code", icon: FileCode2, label: "Code" },
  { id: "bytecode", icon: Binary, label: "Bytecode" },
  { id: "dependencies", icon: Network, label: "Dependency" },
  { id: "calls", icon: Workflow, label: "Call" },
  { id: "types", icon: Boxes, label: "Type" },
];

function ExploreTabSwitch({
  activeTab,
  onTabChange,
}: {
  activeTab: ExploreTab;
  onTabChange: (tab: ExploreTab) => void;
}) {
  return (
    <div
      aria-label="Explore sections"
      className="inline-flex h-10 w-fit shrink-0 justify-self-start overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-1 shadow-sm"
    >
      {exploreTabs.map((tab) => {
        const Icon = tab.icon;
        const active = tab.id === activeTab;

        return (
          <button
            aria-pressed={active}
            className={cn(
              "inline-flex h-full min-w-0 items-center justify-center gap-1.5 rounded px-3 text-xs font-medium leading-none text-muted-foreground transition hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
              active && "bg-[var(--app-elevated)] text-foreground shadow-sm",
            )}
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            type="button"
          >
            <Icon className="size-3.5 shrink-0" aria-hidden="true" />
            <span className="truncate">{tab.label}</span>
          </button>
        );
      })}
    </div>
  );
}

function SecuritySidebar({
  activeMovePackage,
  activeSurfaceDetail,
  activeWorkspaceTab,
  loadAssessment,
  packageTree,
  onSelectPackage,
  onSelectSurfaceDetail,
  onWorkspaceTabChange,
}: {
  activeMovePackage: MovePackage | null;
  activeSurfaceDetail: SurfaceDetailKind | null;
  activeWorkspaceTab: WorkspaceTab;
  loadAssessment: PackageLoadAssessment | null;
  packageTree: PackageTree;
  onSelectPackage: (movePackage: MovePackage) => void;
  onSelectSurfaceDetail: (detail: SurfaceDetailKind) => void;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
}) {
  const surfaceItems = packageSurfaceItems(activeMovePackage);
  const validationItems = assessmentSidebarItems(loadAssessment);

  return (
    <aside className="flex min-h-0 flex-col border-r border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground">
      <ScrollArea className="min-h-0 flex-1">
        <div className="px-3 pb-3 pt-2.5">
        <ProjectSwitcher
          activeMovePackage={activeMovePackage}
          packageTree={packageTree}
          onSelectPackage={onSelectPackage}
        />
        <SidebarSection title="Security Workspace">
          <SidebarItem
            active={activeWorkspaceTab === "Agents" && !activeSurfaceDetail}
            icon={Bot}
            label="Agents"
            onClick={() => onWorkspaceTabChange("Agents")}
          />
          <SidebarItem
            active={activeWorkspaceTab === "Explore" && !activeSurfaceDetail}
            icon={Gauge}
            label="Explore"
            onClick={() => onWorkspaceTabChange("Explore")}
          />
          <SidebarItem
            active={activeWorkspaceTab === "Execution" && !activeSurfaceDetail}
            icon={Workflow}
            label="Execution"
            onClick={() => onWorkspaceTabChange("Execution")}
          />
        </SidebarSection>

        <SidebarSection title="Surface Analysis">
          {surfaceItems.map((item) => (
            <SidebarItem
              badge={item.value}
              icon={item.icon}
              key={item.label}
              label={item.label}
              active={activeSurfaceDetail === item.detail}
              onClick={() => onSelectSurfaceDetail(item.detail)}
              tone={item.tone}
            />
          ))}
        </SidebarSection>

        <SidebarSection title="Validation">
          {validationItems.map((item) => (
            <SidebarItem
              badge={item.badge}
              icon={item.icon}
              key={item.label}
              label={item.label}
              tone={item.tone}
            />
          ))}
        </SidebarSection>

        </div>
      </ScrollArea>
    </aside>
  );
}

function LastScannedStatus({ scannedAt }: { scannedAt: number | null }) {
  const [now, setNow] = React.useState(() => Date.now());

  React.useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 15_000);

    return () => window.clearInterval(interval);
  }, []);

  React.useEffect(() => {
    setNow(Date.now());
  }, [scannedAt]);

  return (
    <span className="min-w-0 truncate text-right leading-5">
      Last scanned: {formatRelativeScanTime(scannedAt, now)}
    </span>
  );
}

function formatRelativeScanTime(scannedAt: number | null, now: number) {
  if (!scannedAt) {
    return "not yet";
  }

  const elapsed = Math.max(0, now - scannedAt);
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;

  if (elapsed < minute) {
    return "just now";
  }

  if (elapsed < hour) {
    const minutes = Math.floor(elapsed / minute);
    return `${minutes} ${minutes === 1 ? "minute" : "minutes"} ago`;
  }

  if (elapsed < day) {
    const hours = Math.floor(elapsed / hour);
    return `${hours} ${hours === 1 ? "hour" : "hours"} ago`;
  }

  const days = Math.floor(elapsed / day);
  return `${days} ${days === 1 ? "day" : "days"} ago`;
}

function packageSurfaceItems(movePackage: MovePackage | null) {
  const surface = movePackage?.surface;

  return [
    {
      icon: GitBranch,
      detail: "entry-functions" as const,
      label: "Entry Functions",
      value: String(surface?.entryFunctionCount ?? 0),
      tone: "danger",
    },
    {
      icon: Boxes,
      detail: "objects" as const,
      label: "Objects",
      value: String(
        (surface?.capabilityCount ?? 0) +
        (surface?.sharedObjectCount ?? 0) +
        (surface?.addressOwnedObjectCount ?? 0) +
        (surface?.immutableObjectCount ?? 0) +
        (surface?.wrappedObjectCount ?? 0) +
        (surface?.partyObjectCount ?? 0),
      ),
      tone: "yellow",
    },
    {
      icon: ShieldAlert,
      detail: "admin-controls" as const,
      label: "Admin Controls",
      value: String(surface?.adminControlCount ?? 0),
      tone: "warning",
    },
    {
      icon: Network,
      detail: "external-calls" as const,
      label: "External Calls",
      value: String(surface?.externalCallCount ?? 0),
      tone: "muted",
    },
    {
      icon: Link2,
      detail: "package-internals" as const,
      label: "Package Internals",
      value: String(surface?.publicPackageRelationshipCount ?? 0),
      tone: "muted",
    },
  ];
}

function ProjectSwitcher({
  activeMovePackage,
  onSelectPackage,
  packageTree,
}: {
  activeMovePackage: MovePackage | null;
  onSelectPackage: (movePackage: MovePackage) => void;
  packageTree: PackageTree;
}) {
  const [isOpen, setIsOpen] = React.useState(false);
  const packages = packageTree.movePackages;
  const hasMultiplePackages = packages.length > 1;
  const name = displayMovePackageName(activeMovePackage?.name ?? packageTree.rootName);
  const path = activeMovePackage
    ? packagePathLabel(activeMovePackage, packageTree)
    : compactPath(packageTree.rootPath);

  return (
    <div className="relative mb-2">
      <Button
        aria-expanded={isOpen}
        aria-haspopup="menu"
        className={cn(
          "relative grid h-9 w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2.5 pl-3 text-left text-muted-foreground shadow-[inset_0_1px_0_rgba(255,255,255,0.055),0_0_0_1px_rgba(255,255,255,0.018)] hover:border-primary/35 hover:bg-[var(--app-subtle)] hover:text-foreground",
          isOpen && "border-primary/45 bg-[var(--app-subtle)]",
        )}
        disabled={!hasMultiplePackages}
        onClick={() => setIsOpen((open) => !open)}
        title={`${name} - ${path}`}
        type="button"
        variant="ghost"
      >
        <span
          aria-hidden="true"
          className="pointer-events-none absolute inset-y-1 left-1 w-px rounded-full bg-primary/50"
        />
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="min-w-0 truncate text-sm font-semibold leading-5 text-foreground">
            {name}
          </span>
          <span className="min-w-0 truncate text-[11px] font-normal leading-5 text-muted-foreground">
            {path}
          </span>
        </span>
        <ChevronDown
          className={cn(
            "size-4 shrink-0 text-muted-foreground transition-transform",
            isOpen && "rotate-180",
          )}
          aria-hidden="true"
        />
      </Button>

      {isOpen && hasMultiplePackages ? (
        <div
          className="absolute left-0 right-0 top-[calc(100%+6px)] z-30 overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] p-1 shadow-xl"
          role="menu"
        >
          {packages.map((movePackage) => {
            const isActive = movePackage.manifestPath === activeMovePackage?.manifestPath;

            return (
              <button
                className={cn(
                  "grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded px-2.5 py-2 text-left hover:bg-[var(--app-subtle)]",
                  isActive && "bg-[var(--app-subtle)] text-foreground",
                )}
                key={movePackage.manifestPath}
                onClick={() => {
                  onSelectPackage(movePackage);
                  setIsOpen(false);
                }}
                role="menuitem"
                type="button"
              >
                <span className="min-w-0">
                  <span className="block truncate text-sm font-semibold">
                    {displayMovePackageName(movePackage.name)}
                  </span>
                  <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
                    {packagePathLabel(movePackage, packageTree)}
                  </span>
                </span>
                {isActive ? (
                  <Check className="size-3.5 shrink-0 text-primary" aria-hidden="true" />
                ) : null}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function SidebarSection({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="py-2.5 first:pt-0">
      <h2 className="mb-2 px-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {title}
      </h2>
      <div className="space-y-0.5">{children}</div>
      <Separator className="mt-2.5" />
    </section>
  );
}

function SidebarItem({
  active,
  badge,
  icon: Icon,
  label,
  onClick,
  tone = "muted",
}: {
  active?: boolean;
  badge?: string;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  label: string;
  onClick?: () => void;
  tone?: string;
}) {
  return (
    <Button
      className={cn(
        "h-7 w-full justify-start gap-2 px-2 text-left text-[13px] font-normal text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
        active && "bg-[var(--app-subtle)] text-foreground",
      )}
      onClick={onClick}
      type="button"
      variant="ghost"
    >
      <Icon className="size-3.5 shrink-0" aria-hidden="true" />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {badge ? <MetricBadge tone={tone}>{badge === "check" ? <Check className="size-3" /> : badge}</MetricBadge> : null}
    </Button>
  );
}

function MetricBadge({
  children,
  tone,
}: {
  children: React.ReactNode;
  tone: string;
}) {
  return (
    <Badge
      variant="secondary"
      className={cn(
        "min-w-6 rounded-full px-1.5 py-0.5 text-[11px] font-semibold",
        tone === "success" && "bg-emerald-500/15 text-emerald-400",
        tone === "danger" && "bg-red-500/15 text-red-400",
        tone === "warning" && "bg-amber-500/15 text-amber-400",
        tone === "yellow" && "bg-yellow-500/15 text-yellow-400",
        tone === "violet" && "bg-violet-500/15 text-violet-400",
        tone === "muted" && "bg-muted text-muted-foreground",
      )}
    >
      {children}
    </Badge>
  );
}

function InspectorPanel({
  className,
  onCollapse,
}: {
  className?: string;
  onCollapse: () => void;
}) {
  return (
    <aside className={cn("grid min-h-0 grid-rows-[auto_1fr] overflow-hidden border-l border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground", className)}>
      <header className="grid grid-cols-[1fr_auto] items-center border-b border-[color:var(--app-border)] pl-4 pr-2">
        <Tabs className="h-11 min-w-0" value="inspector">
          <TabsList className="h-11 w-full rounded-none p-0" variant="line">
            <TabsTrigger
              className="h-full flex-1 data-[state=active]:after:bg-chart-1"
              value="inspector"
            >
              Inspector
            </TabsTrigger>
            <TabsTrigger className="h-full flex-1" value="activity">
              Activity
            </TabsTrigger>
          </TabsList>
        </Tabs>
        <Button
          aria-label="Collapse inspector"
          className="size-7 text-muted-foreground"
          onClick={onCollapse}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <PanelRightClose className="size-4" aria-hidden="true" />
        </Button>
      </header>

      <div aria-hidden="true" className="min-h-0" />
    </aside>
  );
}

function CollapsedPanelRail({
  label,
  onOpen,
  side,
}: {
  label: string;
  onOpen: () => void;
  side: "left" | "right";
}) {
  const Icon = side === "left" ? PanelLeftOpen : ScanEye;

  return (
    <aside
      className={cn(
        "grid min-h-0 grid-rows-[auto_1fr] bg-[var(--app-panel)] text-foreground",
        side === "left"
          ? "border-r border-[color:var(--app-border)]"
          : "border-l border-[color:var(--app-border)]",
      )}
    >
      <div
        className={cn(
          "flex min-h-12 flex-col items-center gap-3 px-1.5 py-2",
          side === "left" && "border-b border-[color:var(--app-border)]",
        )}
      >
        <Button
          aria-label={label}
          className="size-8 text-muted-foreground hover:text-foreground"
          onClick={onOpen}
          size="icon-sm"
          type="button"
          variant="ghost"
        >
          <Icon className="size-4" aria-hidden="true" />
        </Button>
      </div>
    </aside>
  );
}

function isResizeObserverLoopWarning(message: string) {
  return (
    message === "ResizeObserver loop completed with undelivered notifications." ||
    message === "ResizeObserver loop limit exceeded"
  );
}

function compactPath(path: string) {
  if (path.startsWith("/Users/")) {
    const [, , , ...rest] = path.split("/");
    return rest.length ? `~/${rest.join("/")}` : path;
  }

  return path;
}

function packagePathLabel(movePackage: MovePackage, packageTree: PackageTree) {
  if (!movePackage.path || movePackage.path === ".") {
    return compactPath(packageTree.rootPath);
  }

  if (movePackage.path.startsWith("/")) {
    return compactPath(movePackage.path);
  }

  return compactPath(`${packageTree.rootPath}/${movePackage.path}`);
}
