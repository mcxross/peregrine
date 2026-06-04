import React from "react";
import {
  Bug,
  FlaskConical,
  Gauge,
  Hammer,
  MoreVertical,
  PanelLeftClose,
  PanelLeftOpen,
  Play,
  SquareFunction,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { LayoutSettings } from "@/layout/layout-store";
import {
  trafficLightInset,
  workspaceSidebarWidth,
} from "@/layout/window-chrome";
import { SuiNetworkSelector } from "@/app/sui-network-selector";
import {
  startWindowDrag,
  type SuiNetworkSelection,
  type WorkspaceTab,
} from "@peregrine/desktop-runtime";

type TitlebarProps = {
  activeWorkspaceTab?: WorkspaceTab;
  buildActionState?: WorkspaceActionState;
  coverageActionState?: WorkspaceActionState;
  formalActionState?: WorkspaceActionState;
  fuzzActionState?: WorkspaceActionState;
  isLeftPanelOpen?: boolean;
  layout: LayoutSettings;
  hasWorkspace?: boolean;
  network: SuiNetworkSelection;
  onBuildPackage?: () => void;
  onCheckCoverage?: () => void;
  onFormalVerification?: () => void;
  onFuzzPackage?: () => void;
  onNetworkChange: (network: SuiNetworkSelection) => void;
  onOpenProjectConfig?: () => void;
  onTestPackage?: () => void;
  onToggleLeftPanel?: () => void;
  showNetworkSelector?: boolean;
  testActionState?: WorkspaceActionState;
  onWorkspaceTabChange: (tab: WorkspaceTab) => void;
};

export function Titlebar({
  buildActionState,
  coverageActionState,
  formalActionState,
  fuzzActionState,
  isLeftPanelOpen = true,
  layout,
  hasWorkspace = true,
  network,
  onBuildPackage,
  onCheckCoverage,
  onFormalVerification,
  onFuzzPackage,
  onNetworkChange,
  onOpenProjectConfig,
  onTestPackage,
  onToggleLeftPanel,
  showNetworkSelector = true,
  testActionState,
}: TitlebarProps) {
  const handlePointerDown = (event: React.PointerEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    startWindowDrag().catch(() => {
      // Browser previews do not expose the native Tauri window API.
    });
  };

  return (
    <header
      data-tauri-drag-region
      onPointerDown={handlePointerDown}
      className={cn(
        "grid h-[58px] select-none border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] text-foreground",
        layout.chrome === "compact" && "h-12",
      )}
      style={{
        gridTemplateColumns: `${hasWorkspace && isLeftPanelOpen ? `${workspaceSidebarWidth}px` : "128px"} minmax(0, 1fr) ${hasWorkspace ? "360px" : `${workspaceSidebarWidth}px`}`,
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
              <WorkspaceActionButton
                action={action}
                key={action.label}
                state={
                  action.id === "build"
                    ? buildActionState
                    : action.id === "test"
                      ? testActionState
                      : action.id === "coverage"
                        ? coverageActionState
                        : action.id === "fuzzing"
                          ? fuzzActionState
                          : action.id === "formal"
                            ? formalActionState
                          : undefined
                }
                onClick={
                  action.id === "build"
                    ? onBuildPackage
                    : action.id === "test"
                      ? onTestPackage
                      : action.id === "coverage"
                        ? onCheckCoverage
                        : action.id === "fuzzing"
                          ? onFuzzPackage
                          : action.id === "formal"
                            ? onFormalVerification
                          : undefined
                }
              />
            ))}
            <Button
              aria-label="Project configuration"
              className="group h-8 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-2 text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground"
              onClick={onOpenProjectConfig}
              title="Project configuration"
              type="button"
              variant="ghost"
            >
              <MoreVertical className="size-4" aria-hidden="true" />
            </Button>
          </div>
        </div>
      ) : (
        <div data-tauri-drag-region />
      )}

      {hasWorkspace ? (
        <div className="flex h-full items-center justify-end gap-4 pr-5" onPointerDown={(event) => event.stopPropagation()}>
          {showNetworkSelector ? (
            <SuiNetworkSelector network={network} onNetworkChange={onNetworkChange} />
          ) : null}
        </div>
      ) : showNetworkSelector ? (
        <div className="flex h-full items-center justify-end pr-5" onPointerDown={(event) => event.stopPropagation()}>
          <SuiNetworkSelector network={network} onNetworkChange={onNetworkChange} />
        </div>
      ) : (
        <div data-tauri-drag-region />
      )}
    </header>
  );
}

const workspaceActions = [
  { id: "build", icon: Hammer, label: "Build package", tone: "success" },
  { id: "test", icon: FlaskConical, label: "Run tests", tone: "success" },
  { id: "coverage", icon: Gauge, label: "Check coverage", tone: "default" },
  { id: "fuzzing", icon: Bug, label: "Run fuzzing", tone: "danger" },
  { id: "formal", icon: SquareFunction, label: "Run formal checks", tone: "success" },
] as const;

type WorkspaceAction = (typeof workspaceActions)[number];
type WorkspaceActionState = {
  disabled?: boolean;
  running?: boolean;
};

function WorkspaceActionButton({
  action,
  onClick,
  state,
}: {
  action: WorkspaceAction;
  onClick?: () => void;
  state?: WorkspaceActionState;
}) {
  const Icon = action.icon;
  const isDisabled = Boolean(state?.disabled || state?.running);

  return (
    <Button
      aria-label={action.label}
      className="group h-8 gap-1.5 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-2 text-foreground hover:bg-[var(--app-subtle)]"
      disabled={isDisabled}
      onClick={onClick}
      title={action.label}
      type="button"
      variant="ghost"
    >
      <Icon
        className={cn(
          "size-3.5 shrink-0 text-zinc-300",
          action.tone === "success" && "text-emerald-400",
          action.tone === "danger" && "text-red-400",
        )}
        aria-hidden="true"
      />
      <Play
        className={cn(
          "size-2.5 text-zinc-300 transition-colors group-hover:text-foreground",
          state?.running && "animate-pulse text-foreground",
        )}
        aria-hidden="true"
      />
    </Button>
  );
}
