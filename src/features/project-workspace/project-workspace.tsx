import React from "react";
import {
  Boxes,
  Box,
  Check,
  ChevronDown,
  Gauge,
  GitBranch,
  KeyRound,
  Link2,
  Lock,
  Network,
  PanelLeftOpen,
  PanelRightClose,
  ScanEye,
  ShieldAlert,
  Sparkles,
  UsersRound,
  Workflow,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { WorkspaceTab } from "@/app/titlebar";
import type {
  FilePreview,
  MoveModule,
  MovePackage,
  PackageTree,
  SuiCliStatus,
} from "@/features/empty-project/filesystem-tree";
import { checkSuiCli } from "@/features/empty-project/filesystem-tree";
import { AiFloatingWindow } from "@/features/project-workspace/ai/ai-floating-window";
import {
  BuildLogSheet,
  type BuildLogRun,
  type BuildLogSheetController,
  type BuildLogUpdateOptions,
} from "@/features/project-workspace/build-log-sheet";
import { DependencyGraphScreen } from "@/features/project-workspace/dependency-graph-screen";
import { ExecutionBuilderScreen } from "@/features/project-workspace/execution-builder-screen";
import { MovePackagesOverviewScreen } from "@/features/project-workspace/move-packages-overview-screen";
import { assessmentSidebarItems } from "@/features/project-workspace/package-load-assessment-cards";
import type { PackageLoadAssessment } from "@/features/project-workspace/package-load-assessment";
import type { SelectedMoveModule } from "@/features/project-workspace/module-signature-screen";
import {
  SurfaceDetailScreen,
  type SurfaceDetailKind,
} from "@/features/project-workspace/surface-detail-screen";
import {
  workspaceSidebarWidth,
  workspaceStatusBarHeight,
} from "@/layout/window-chrome";
import { cn } from "@/lib/utils";

type ProjectWorkspaceProps = {
  activeWorkspaceTab: WorkspaceTab;
  activePackageManifestPath: string | null;
  buildLogSheet: BuildLogSheetController;
  isLeftPanelOpen: boolean;
  lastScannedAt: number | null;
  loadAssessment: PackageLoadAssessment | null;
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
  packageTree: PackageTree;
};

export type OpenFileTab = {
  path: string;
  preview: FilePreview | null;
  editedSource: string | null;
  error: string | null;
  isDirty: boolean;
  isSaving: boolean;
  status: "idle" | "loading" | "loaded" | "error";
};

export function ProjectWorkspace({
  activePackageManifestPath,
  activeWorkspaceTab,
  buildLogSheet,
  isLeftPanelOpen,
  lastScannedAt,
  loadAssessment,
  onActivePackageManifestPathChange,
  onCommandLog,
  onProjectSelected,
  onWorkspaceTabChange,
  packageTree,
}: ProjectWorkspaceProps) {
  const [isRightPanelOpen, setIsRightPanelOpen] = React.useState(true);
  const [isAiOpen, setIsAiOpen] = React.useState(true);
  const [selectedModule, setSelectedModule] = React.useState<SelectedMoveModule | null>(null);
  const [activeSurfaceDetail, setActiveSurfaceDetail] = React.useState<SurfaceDetailKind | null>(null);
  const activeMovePackage =
    packageTree.movePackages.length === 1
      ? packageTree.movePackages[0]
      : packageTree.movePackages.find(
          (movePackage) => movePackage.manifestPath === activePackageManifestPath,
        ) ?? null;
  const packageName = activeMovePackage?.name || packageTree.rootName || packageTree.movePackages[0]?.name || "savings_personal";

  React.useEffect(() => {
    setSelectedModule(null);
    setActiveSurfaceDetail(null);
  }, [activePackageManifestPath, packageTree.rootPath]);

  React.useEffect(() => {
    const firstModule = activeMovePackage?.modules[0] ?? null;

    setSelectedModule(
      firstModule && activeMovePackage
        ? { moveModule: firstModule, movePackage: activeMovePackage }
        : null,
    );
  }, [activeMovePackage]);

  React.useEffect(() => {
    if (activeWorkspaceTab === "Overview" || activeWorkspaceTab === "Execution") {
      setIsRightPanelOpen(false);
    }
  }, [activeWorkspaceTab]);

  return (
    <div
      className="relative grid h-full min-h-0 overflow-hidden bg-[var(--app-window)] text-foreground"
      style={{
        gridTemplateColumns: isLeftPanelOpen
          ? `${workspaceSidebarWidth}px minmax(0, 1fr) ${isRightPanelOpen ? "clamp(360px, 24vw, 400px)" : "44px"}`
          : `minmax(0, 1fr) ${isRightPanelOpen ? "clamp(360px, 24vw, 400px)" : "44px"}`,
      }}
    >
      {isLeftPanelOpen ? (
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
          onOpenAi={() => setIsAiOpen(true)}
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
        <WorkspaceMainPanel
          activeWorkspaceTab={activeWorkspaceTab}
          activeSurfaceDetail={activeSurfaceDetail}
          activeMovePackage={activeMovePackage}
          loadAssessment={loadAssessment}
          packageTree={packageTree}
          packageName={packageName}
          selectedModule={selectedModule}
          onCommandLog={onCommandLog}
          onProjectSelected={onProjectSelected}
          onClearSelectedModule={() => setSelectedModule(null)}
          onSelectModule={(movePackage, moveModule) => {
            setActiveSurfaceDetail(null);
            setSelectedModule({ moveModule, movePackage });
          }}
        />
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

      {isRightPanelOpen ? (
        <InspectorPanel
          onCollapse={() => setIsRightPanelOpen(false)}
        />
      ) : (
        <CollapsedPanelRail
          label="Open inspector"
          side="right"
          onOpen={() => setIsRightPanelOpen(true)}
        />
      )}

      {activeMovePackage ? (
        <AiFloatingWindow
          activeMovePackage={activeMovePackage}
          isOpen={isAiOpen}
          onOpenChange={setIsAiOpen}
          packageTree={packageTree}
        />
      ) : null}
    </div>
  );
}

function WorkspaceMainPanel({
  activeWorkspaceTab,
  activeSurfaceDetail,
  activeMovePackage,
  loadAssessment,
  onClearSelectedModule,
  onSelectModule,
  packageTree,
  packageName,
  selectedModule,
  onProjectSelected,
  onCommandLog,
}: {
  activeWorkspaceTab: WorkspaceTab;
  activeSurfaceDetail: SurfaceDetailKind | null;
  activeMovePackage: MovePackage | null;
  loadAssessment: PackageLoadAssessment | null;
  onClearSelectedModule: () => void;
  onCommandLog: (run: BuildLogRun, options?: BuildLogUpdateOptions) => void;
  onProjectSelected: (packageTree: PackageTree) => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  packageTree: PackageTree;
  packageName: string;
  selectedModule: SelectedMoveModule | null;
}) {
  if (activeSurfaceDetail) {
    return <SurfaceDetailScreen detail={activeSurfaceDetail} movePackage={activeMovePackage} />;
  }

  if (activeWorkspaceTab === "Explore") {
    return (
      <MovePackagesOverviewScreen
        activeMovePackage={activeMovePackage}
        packageTree={packageTree}
        onClearSelectedModule={onClearSelectedModule}
        onSelectModule={onSelectModule}
        selectedModule={selectedModule}
      />
    );
  }

  if (activeWorkspaceTab === "Execution") {
    return (
      <ExecutionBuilderScreen
        activeMovePackage={activeMovePackage}
        onCommandLog={onCommandLog}
        onProjectSelected={onProjectSelected}
        packageTree={packageTree}
      />
    );
  }

  return (
    <DependencyGraphScreen
      graph={packageTree.dependencyGraph}
      loadAssessment={loadAssessment}
      packageName={packageName}
    />
  );
}

function SecuritySidebar({
  activeMovePackage,
  activeSurfaceDetail,
  activeWorkspaceTab,
  loadAssessment,
  packageTree,
  onOpenAi,
  onSelectPackage,
  onSelectSurfaceDetail,
  onWorkspaceTabChange,
}: {
  activeMovePackage: MovePackage | null;
  activeSurfaceDetail: SurfaceDetailKind | null;
  activeWorkspaceTab: WorkspaceTab;
  loadAssessment: PackageLoadAssessment | null;
  packageTree: PackageTree;
  onOpenAi: () => void;
  onSelectPackage: (movePackage: MovePackage) => void;
  onSelectSurfaceDetail: (detail: SurfaceDetailKind) => void;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
}) {
  const surfaceItems = packageSurfaceItems(activeMovePackage);
  const validationItems = assessmentSidebarItems(loadAssessment);
  const [suiCliStatus, setSuiCliStatus] = React.useState<SuiCliStatus | null>(null);

  React.useEffect(() => {
    let isMounted = true;

    checkSuiCli()
      .then((status) => {
        if (isMounted) {
          setSuiCliStatus(status);
        }
      })
      .catch(() => {
        if (isMounted) {
          setSuiCliStatus({
            installed: false,
            installHint: "Install the Sui CLI and make sure `sui` is on PATH.",
            version: null,
          });
        }
      });

    return () => {
      isMounted = false;
    };
  }, []);

  return (
    <aside className="grid min-h-0 grid-rows-[1fr_auto] border-r border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground">
      <ScrollArea className="min-h-0">
        <div className="px-3 pb-3 pt-2.5">
        <ProjectSwitcher
          activeMovePackage={activeMovePackage}
          packageTree={packageTree}
          onSelectPackage={onSelectPackage}
        />
        <SidebarSection title="Security Workspace">
          <SidebarItem
            active={activeWorkspaceTab === "Overview" && !activeSurfaceDetail}
            icon={GitBranch}
            label="Overview"
            onClick={() => onWorkspaceTabChange("Overview")}
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

        <Card className="mt-4 gap-0 rounded-md p-3">
          <div className="flex items-center justify-between text-xs font-semibold uppercase tracking-wide text-primary">
            <span>AI Copilot</span>
            <Sparkles className="size-3.5" aria-hidden="true" />
          </div>
          <p className="mt-3 text-sm text-muted-foreground">
            Ask anything about your Move package security.
          </p>
          <Button
            className="mt-3 h-9 w-full text-foreground"
            onClick={onOpenAi}
            type="button"
            variant="outline"
          >
            <Sparkles className="size-4 text-chart-4" aria-hidden="true" />
            Open Copilot
          </Button>
        </Card>
        </div>
      </ScrollArea>

      <SuiCliFooter status={suiCliStatus} />
    </aside>
  );
}

function SuiCliFooter({ status }: { status: SuiCliStatus | null }) {
  const isInstalled = status?.installed ?? false;
  const versionLabel = status?.version ? `v${status.version}` : status ? "Install Sui CLI" : "Checking...";

  return (
    <div
      className="flex items-center justify-between gap-2 border-t border-[color:var(--app-border)] bg-[var(--app-panel-strong)] px-4 text-xs text-muted-foreground"
      style={{ height: workspaceStatusBarHeight }}
      title={!isInstalled && status?.installHint ? status.installHint : undefined}
    >
      <span className="shrink-0">Sui CLI</span>
      <Badge
        className={cn(
          "font-semibold",
          isInstalled
            ? "bg-emerald-500/15 text-emerald-400"
            : "bg-amber-500/15 text-amber-400",
        )}
        variant="secondary"
      >
        {status ? (isInstalled ? "Installed" : "Missing") : "Checking"}
      </Badge>
      <span className="min-w-0 truncate text-right">{versionLabel}</span>
    </div>
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
      icon: KeyRound,
      detail: "capabilities" as const,
      label: "Capabilities",
      value: String(surface?.capabilityCount ?? 0),
      tone: "warning",
    },
    {
      icon: Boxes,
      detail: "shared-objects" as const,
      label: "Shared Objects",
      value: String(surface?.sharedObjectCount ?? 0),
      tone: "yellow",
    },
    {
      icon: Box,
      detail: "address-owned" as const,
      label: "Address-Owned",
      value: String(surface?.addressOwnedObjectCount ?? 0),
      tone: "muted",
    },
    {
      icon: Lock,
      detail: "immutable-objects" as const,
      label: "Immutable Objects",
      value: String(surface?.immutableObjectCount ?? 0),
      tone: "success",
    },
    {
      icon: Boxes,
      detail: "wrapped-objects" as const,
      label: "Wrapped Objects",
      value: String(surface?.wrappedObjectCount ?? 0),
      tone: "violet",
    },
    {
      icon: UsersRound,
      detail: "party-objects" as const,
      label: "Party Objects",
      value: String(surface?.partyObjectCount ?? 0),
      tone: "violet",
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
  const name = activeMovePackage?.name ?? packageTree.rootName;
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
                  <span className="block truncate text-sm font-semibold">{movePackage.name}</span>
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
          "flex h-12 items-center justify-center",
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
