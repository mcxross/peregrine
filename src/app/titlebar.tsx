import type React from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Bug,
  FileCheck2,
  FlaskConical,
  Gauge,
  Hammer,
  PanelLeftClose,
  PanelLeftOpen,
  Play,
  RefreshCw,
  ShieldAlert,
  Share,
  SquareFunction,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { LayoutSettings } from "@/layout/layout-store";
import { trafficLightInset } from "@/layout/window-chrome";

type TitlebarProps = {
  activeWorkspaceTab?: WorkspaceTab;
  isLeftPanelOpen?: boolean;
  layout: LayoutSettings;
  hasWorkspace?: boolean;
  onToggleLeftPanel?: () => void;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
};

export function Titlebar({
  isLeftPanelOpen = true,
  layout,
  hasWorkspace = true,
  onToggleLeftPanel,
}: TitlebarProps) {
  const handlePointerDown = (event: React.PointerEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    getCurrentWindow().startDragging().catch(() => {
      // Browser previews do not expose the native Tauri window API.
    });
  };

  return (
    <header
      data-tauri-drag-region
      onPointerDown={handlePointerDown}
      className={cn(
        "grid h-[58px] select-none border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] text-foreground backdrop-blur",
        layout.chrome === "compact" && "h-12",
      )}
      style={{
        gridTemplateColumns: `${hasWorkspace && isLeftPanelOpen ? "270px" : "128px"} minmax(0, 1fr) ${hasWorkspace ? "128px" : "270px"}`,
      }}
    >
      <div
        data-tauri-drag-region
        className="flex h-full items-center border-r border-[color:var(--app-border)] pr-3"
        style={{ paddingLeft: trafficLightInset }}
      >
        {hasWorkspace ? (
          <div className="ml-auto flex h-full items-center" onPointerDown={(event) => event.stopPropagation()}>
            <Button
              aria-label={isLeftPanelOpen ? "Collapse navigation" : "Open navigation"}
              className="size-7 text-muted-foreground hover:text-foreground"
              onClick={onToggleLeftPanel}
              size="icon-xs"
              type="button"
              variant="ghost"
            >
              {isLeftPanelOpen ? (
                <PanelLeftClose className="size-4" aria-hidden="true" />
              ) : (
                <PanelLeftOpen className="size-4" aria-hidden="true" />
              )}
            </Button>
          </div>
        ) : null}
      </div>

      {hasWorkspace ? (
        <div className="flex min-w-0 items-center justify-center">
          <div className="flex items-center gap-1" onPointerDown={(event) => event.stopPropagation()}>
            {workspaceActions.map((action) => (
              <WorkspaceActionButton action={action} key={action.label} />
            ))}
          </div>
        </div>
      ) : (
        <div data-tauri-drag-region />
      )}

      {hasWorkspace ? (
        <div className="flex h-full items-center justify-end gap-4 pr-5" onPointerDown={(event) => event.stopPropagation()}>
          <TitlebarAction icon={RefreshCw} label="Rescan" />
          <TitlebarAction icon={Share} label="Export" />
        </div>
      ) : (
        <div data-tauri-drag-region />
      )}
    </header>
  );
}

const workspaceTabs = [
  "Overview",
  "Explore",
  "Attack Surface",
  "Tests",
  "Fuzzing",
  "Formal",
  "Audit",
  "CI",
] as const;

export type WorkspaceTab = (typeof workspaceTabs)[number];

const workspaceActions = [
  { icon: Hammer, label: "Build package", tone: "success" },
  { icon: FlaskConical, label: "Run tests", tone: "success" },
  { icon: Gauge, label: "Check coverage", tone: "default" },
  { icon: Bug, label: "Run fuzzing", tone: "danger" },
  { icon: SquareFunction, label: "Run formal checks", tone: "success" },
  { icon: ShieldAlert, label: "Open audit", tone: "warning" },
  { icon: FileCheck2, label: "Run CI", tone: "default" },
];

type WorkspaceAction = (typeof workspaceActions)[number];

function WorkspaceActionButton({ action }: { action: WorkspaceAction }) {
  const Icon = action.icon;

  return (
    <Button
      aria-label={action.label}
      className="h-8 gap-1.5 rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-2 text-muted-foreground hover:bg-[var(--app-elevated)] hover:text-foreground"
      title={action.label}
      type="button"
      variant="ghost"
    >
      <Icon
        className={cn(
          "size-3.5 shrink-0",
          action.tone === "success" && "text-emerald-400",
          action.tone === "danger" && "text-red-400",
          action.tone === "warning" && "text-amber-400",
        )}
        aria-hidden="true"
      />
      <Play className="size-2.5 text-muted-foreground" aria-hidden="true" />
    </Button>
  );
}

function TitlebarAction({
  icon: Icon,
  label,
  suffix,
}: {
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  label: string;
  suffix?: string;
}) {
  return (
    <Button
      className="h-auto gap-2 p-0 text-sm font-medium text-foreground hover:text-chart-1"
      type="button"
      variant="ghost"
    >
      <Icon className="size-4" aria-hidden="true" />
      <span>{label}</span>
      {suffix ? <span className="text-muted-foreground">{suffix}</span> : null}
    </Button>
  );
}
