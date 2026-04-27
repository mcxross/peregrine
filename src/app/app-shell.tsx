import { useState } from "react";
import { Titlebar } from "@/app/titlebar";
import type { WorkspaceTab } from "@/app/titlebar";
import { Sidebar } from "@/app/sidebar";
import { Workspace } from "@/app/workspace";
import type { PackageTree } from "@/features/empty-project/filesystem-tree";
import { defaultLayoutSettings } from "@/layout/layout-store";
import { SettingsScreen } from "@/screens/settings-screen";

type AppShellProps = {
  packageTree: PackageTree | null;
  screen: "workspace" | "settings";
  onCloseSettings: () => void;
  onProjectSelected: (packageTree: PackageTree) => void;
};

export function AppShell({
  packageTree,
  screen,
  onCloseSettings,
  onProjectSelected,
}: AppShellProps) {
  const [activeWorkspaceTab, setActiveWorkspaceTab] = useState<WorkspaceTab>("Overview");
  const [isLeftPanelOpen, setIsLeftPanelOpen] = useState(true);
  const layout = defaultLayoutSettings;
  const isSettings = screen === "settings";
  const showSidebar = isSettings;

  return (
    <main className="grid h-svh grid-rows-[58px_1fr] overflow-hidden bg-[var(--app-window)] text-foreground">
      <Titlebar
        activeWorkspaceTab={activeWorkspaceTab}
        isLeftPanelOpen={isLeftPanelOpen}
        layout={layout}
        hasWorkspace={!isSettings && Boolean(packageTree)}
        onToggleLeftPanel={() => setIsLeftPanelOpen((isOpen) => !isOpen)}
        onWorkspaceTabChange={setActiveWorkspaceTab}
      />

      <section className="flex min-h-0">
        {showSidebar ? <Sidebar layout={layout} /> : null}
        <div className="min-w-0 flex-1">
          {isSettings ? (
            <SettingsScreen onBack={onCloseSettings} />
          ) : (
            <Workspace
              activeWorkspaceTab={activeWorkspaceTab}
              isLeftPanelOpen={isLeftPanelOpen}
              onWorkspaceTabChange={setActiveWorkspaceTab}
              packageTree={packageTree}
              onProjectSelected={onProjectSelected}
            />
          )}
        </div>
      </section>
    </main>
  );
}
