import React from "react";
import {
  Boxes,
  Box,
  Check,
  ChevronDown,
  ChevronRight,
  FileCheck2,
  FileText,
  Gauge,
  GitBranch,
  KeyRound,
  Link2,
  Lock,
  Network,
  PackageCheck,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
  ShieldAlert,
  ShieldCheck,
  Sparkles,
  Target,
  UsersRound,
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
import {
  BuildLogSheet,
  type BuildLogSheetController,
} from "@/features/project-workspace/build-log-sheet";
import { DependencyGraphScreen } from "@/features/project-workspace/dependency-graph-screen";
import { MovePackagesOverviewScreen } from "@/features/project-workspace/move-packages-overview-screen";
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
  onActivePackageManifestPathChange: (manifestPath: string | null) => void;
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

const validationItems = [
  { icon: FileCheck2, label: "Tests", value: "check", tone: "success" },
  { icon: Gauge, label: "Coverage", value: "74%", tone: "success" },
  { icon: Target, label: "Fuzzing", value: "2", tone: "danger" },
  { icon: FileText, label: "Formal Specs", value: "5 / 8", tone: "warning" },
  { icon: FileText, label: "Audit Report", value: "", tone: "muted" },
];

const overviewMetrics = [
  { label: "Entry Functions", value: "4" },
  { label: "Capabilities", value: "2" },
  { label: "Shared Object", value: "1" },
  { label: "Asset Movers", value: "3" },
  { label: "Oracle Usage", value: "1" },
  { label: "Admin Controls", value: "2" },
];

const topRisks = [
  {
    file: "vault.move:128",
    level: "High",
    text: "Withdraw allows fund movement via receipt authority.",
  },
  {
    file: "oracle.move:56",
    level: "High",
    text: "Oracle price freshness not enforced in all paths.",
  },
  {
    file: "config.move:42",
    level: "Medium",
    text: "Emergency penalty can be set without upper bound.",
  },
];

const nextActions = [
  "Prove withdrawal accounting invariant",
  "Add stale oracle rejection test",
  "Review admin update functions",
];

export function ProjectWorkspace({
  activePackageManifestPath,
  activeWorkspaceTab,
  buildLogSheet,
  isLeftPanelOpen,
  onActivePackageManifestPathChange,
  onWorkspaceTabChange,
  packageTree,
}: ProjectWorkspaceProps) {
  const [isRightPanelOpen, setIsRightPanelOpen] = React.useState(true);
  const [selectedModule, setSelectedModule] = React.useState<SelectedMoveModule | null>(null);
  const [activeSurfaceDetail, setActiveSurfaceDetail] = React.useState<SurfaceDetailKind | null>(null);
  const activeMovePackage =
    packageTree.movePackages.length === 1
      ? packageTree.movePackages[0]
      : packageTree.movePackages.find(
          (movePackage) => movePackage.manifestPath === activePackageManifestPath,
        ) ?? null;
  const packageName = activeMovePackage?.name || packageTree.rootName || packageTree.movePackages[0]?.name || "savings_personal";
  const moduleCount =
    activeMovePackage?.modules.length ??
    packageTree.movePackages.find((movePackage) => movePackage.name === packageTree.dependencyGraph.root)
      ?.modules.length ??
    packageTree.movePackages.reduce((count, movePackage) => count + movePackage.modules.length, 0);

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
    if (activeWorkspaceTab === "Overview") {
      setIsRightPanelOpen(false);
    }
  }, [activeWorkspaceTab]);

  return (
    <div
      className="grid h-full min-h-0 overflow-hidden bg-[var(--app-window)] text-foreground"
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
        <WorkspaceMainPanel
          activeWorkspaceTab={activeWorkspaceTab}
          activeSurfaceDetail={activeSurfaceDetail}
          activeMovePackage={activeMovePackage}
          packageTree={packageTree}
          packageName={packageName}
          selectedModule={selectedModule}
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
          run={buildLogSheet.run}
        />
        <footer className="flex items-center overflow-hidden border-t border-[color:var(--app-border)] bg-[var(--app-chrome)] px-4 text-[11px] leading-none text-muted-foreground">
          <span className="truncate">Last scanned: 2 minutes ago</span>
        </footer>
      </main>

      {isRightPanelOpen ? (
        <InspectorPanel
          moduleCount={Math.max(moduleCount, 5)}
          packageName={packageName}
          onCollapse={() => setIsRightPanelOpen(false)}
        />
      ) : (
        <CollapsedPanelRail
          label="Open inspector"
          side="right"
          onOpen={() => setIsRightPanelOpen(true)}
        />
      )}
    </div>
  );
}

function WorkspaceMainPanel({
  activeWorkspaceTab,
  activeSurfaceDetail,
  activeMovePackage,
  onClearSelectedModule,
  onSelectModule,
  packageTree,
  packageName,
  selectedModule,
}: {
  activeWorkspaceTab: WorkspaceTab;
  activeSurfaceDetail: SurfaceDetailKind | null;
  activeMovePackage: MovePackage | null;
  onClearSelectedModule: () => void;
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

  return <DependencyGraphScreen graph={packageTree.dependencyGraph} packageName={packageName} />;
}

function SecuritySidebar({
  activeMovePackage,
  activeSurfaceDetail,
  activeWorkspaceTab,
  packageTree,
  onSelectPackage,
  onSelectSurfaceDetail,
  onWorkspaceTabChange,
}: {
  activeMovePackage: MovePackage | null;
  activeSurfaceDetail: SurfaceDetailKind | null;
  activeWorkspaceTab: WorkspaceTab;
  packageTree: PackageTree;
  onSelectPackage: (movePackage: MovePackage) => void;
  onSelectSurfaceDetail: (detail: SurfaceDetailKind) => void;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
}) {
  const surfaceItems = packageSurfaceItems(activeMovePackage);
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
        <div className="px-3 py-4">
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
            active={activeWorkspaceTab === "Attack Surface" && !activeSurfaceDetail}
            icon={ShieldAlert}
            label="Attack Surface"
            onClick={() => onWorkspaceTabChange("Attack Surface")}
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
              badge={item.value}
              icon={item.icon}
              key={item.label}
              label={item.label}
              tone={item.tone}
            />
          ))}
        </SidebarSection>

        <Card className="mt-6 gap-0 rounded-md p-4">
          <div className="flex items-center justify-between text-xs font-semibold uppercase tracking-wide text-primary">
            <span>AI Copilot</span>
            <Sparkles className="size-3.5" aria-hidden="true" />
          </div>
          <p className="mt-5 text-sm text-muted-foreground">
            Ask anything about your Move package security.
          </p>
          <Button
            className="mt-5 h-11 w-full text-foreground"
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
      className="flex h-[58px] items-center justify-between gap-2 border-t border-[color:var(--app-border)] bg-[var(--app-panel-strong)] px-5 text-xs text-muted-foreground"
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
    <div className="relative mb-4">
      <Button
        aria-expanded={isOpen}
        aria-haspopup="menu"
        className="grid h-auto w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-3 py-2.5 text-left shadow-none hover:bg-accent"
        disabled={!hasMultiplePackages}
        onClick={() => setIsOpen((open) => !open)}
        type="button"
        variant="outline"
      >
        <span className="min-w-0">
          <span className="block max-w-full truncate text-base font-semibold leading-tight text-foreground">
            {name}
          </span>
          <span className="mt-1 block max-w-full truncate text-xs font-normal leading-tight text-muted-foreground">
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
    <section className="py-3.5 first:pt-0">
      <h2 className="mb-3 px-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        {title}
      </h2>
      <div className="space-y-1">{children}</div>
      <Separator className="mt-3.5" />
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
        "h-8 w-full justify-start gap-2 px-2 text-left text-sm font-normal text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
        active && "bg-[var(--app-subtle)] text-foreground",
      )}
      onClick={onClick}
      type="button"
      variant="ghost"
    >
      <Icon className="size-4 shrink-0" aria-hidden="true" />
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
  moduleCount,
  onCollapse,
  packageName,
}: {
  className?: string;
  moduleCount: number;
  onCollapse: () => void;
  packageName: string;
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

      <ScrollArea className="min-h-0">
      <div className="min-w-0 px-3 py-3">
        <div className="flex items-start gap-3 border-b border-[color:var(--app-border)] pb-3">
          <span className="inline-flex size-7 items-center justify-center rounded bg-chart-1 text-primary-foreground">
            <PackageCheck className="size-4" aria-hidden="true" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <h2 className="truncate text-base font-semibold">{packageName}</h2>
              <Badge
                className="max-w-full rounded bg-[var(--app-subtle)] px-2 py-0.5 text-[11px] text-muted-foreground"
                variant="secondary"
              >
                root package
              </Badge>
            </div>
            <p className="mt-1.5 text-xs text-muted-foreground">{moduleCount} modules</p>
          </div>
        </div>

        <PanelTitle>Security Overview</PanelTitle>
        <Card className="min-w-0 gap-0 overflow-hidden rounded-md p-3 shadow-none">
          <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-start gap-3">
            <div className="min-w-0">
              <div className="text-[11px] font-medium text-muted-foreground">Risk Score</div>
              <div className="mt-1.5 text-3xl font-semibold leading-none text-amber-400">
                72<span className="ml-1.5 text-base font-normal text-muted-foreground">/100</span>
              </div>
            </div>
            <div className="min-w-0 text-right">
              <div className="text-[11px] font-medium text-muted-foreground">Risk Level</div>
              <div className="mt-3 inline-flex max-w-full items-center gap-1.5 text-sm font-semibold text-amber-400">
                <span className="size-2 rounded-full bg-amber-400" />
                <span className="truncate">Medium</span>
              </div>
            </div>
          </div>
          <div className="mt-3 grid grid-cols-2 gap-x-4 gap-y-3 border-t border-[color:var(--app-border)] pt-3 min-[1680px]:grid-cols-3">
            {overviewMetrics.map((metric) => (
              <div className="min-w-0" key={metric.label}>
                <div className="text-sm font-semibold leading-none">{metric.value}</div>
                <div className="mt-1 text-[11px] leading-4 text-muted-foreground">{metric.label}</div>
              </div>
            ))}
          </div>
        </Card>

        <PanelTitle>Top Risks</PanelTitle>
        <Card className="min-w-0 gap-3 overflow-hidden rounded-md p-3 shadow-none">
          {topRisks.map((risk) => (
            <div className="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] gap-x-2 gap-y-1" key={risk.file}>
              <span
                className={cn(
                  "mt-1 size-2 rounded-full",
                  risk.level === "High" ? "bg-red-400" : "bg-amber-400",
                )}
              />
              <div className="min-w-0">
                <div
                  className={cn(
                    "text-sm font-semibold",
                    risk.level === "High" ? "text-red-400" : "text-amber-400",
                  )}
                >
                  {risk.level}
                </div>
                <p className="mt-1 line-clamp-2 text-sm leading-5 text-muted-foreground">{risk.text}</p>
              </div>
              <span className="col-start-2 min-w-0 truncate text-xs text-muted-foreground">{risk.file}</span>
            </div>
          ))}
          <Button className="h-auto justify-start gap-2 p-0 text-sm text-chart-1" type="button" variant="ghost">
            View all issues
            <ChevronRight className="size-4" aria-hidden="true" />
          </Button>
        </Card>

        <PanelTitle>Next Actions</PanelTitle>
        <Card className="gap-2 rounded-md p-3 shadow-none">
          {nextActions.map((action) => (
            <div className="flex items-center gap-3 text-sm" key={action}>
              <ShieldCheck className="size-4 text-chart-4" aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate">{action}</span>
              <Badge
                className="rounded bg-chart-4/15 px-2 py-0.5 text-[11px] font-semibold text-chart-4"
                variant="secondary"
              >
                AI
              </Badge>
            </div>
          ))}
          <Button className="h-auto justify-start gap-2 p-0 text-sm text-chart-1" type="button" variant="ghost">
            View full checklist
            <ChevronRight className="size-4" aria-hidden="true" />
          </Button>
        </Card>
      </div>
      </ScrollArea>
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
  const Icon = side === "left" ? PanelLeftOpen : PanelRightOpen;

  return (
    <aside
      className={cn(
        "grid min-h-0 grid-rows-[auto_1fr] bg-[var(--app-panel)] text-foreground",
        side === "left"
          ? "border-r border-[color:var(--app-border)]"
          : "border-l border-[color:var(--app-border)]",
      )}
    >
      <div className="flex h-12 items-center justify-center border-b border-[color:var(--app-border)]">
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

function PanelTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-2 mt-4 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
      {children}
    </h3>
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
