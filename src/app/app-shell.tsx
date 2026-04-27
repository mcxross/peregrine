import { useCallback, useEffect, useMemo, useState } from "react";
import { Titlebar } from "@/app/titlebar";
import type { WorkspaceTab } from "@/app/titlebar";
import { Sidebar } from "@/app/sidebar";
import { Workspace } from "@/app/workspace";
import {
  buildMovePackage,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import type {
  BuildLogRun,
  BuildLogSheetController,
} from "@/features/project-workspace/build-log-sheet";
import { defaultLayoutSettings } from "@/layout/layout-store";
import { titlebarHeight } from "@/layout/window-chrome";
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
  const [activePackageManifestPath, setActivePackageManifestPath] = useState<string | null>(null);
  const [buildRun, setBuildRun] = useState<BuildLogRun | null>(null);
  const [isBuildSheetOpen, setIsBuildSheetOpen] = useState(false);
  const [isLeftPanelOpen, setIsLeftPanelOpen] = useState(true);
  const layout = defaultLayoutSettings;
  const isSettings = screen === "settings";
  const showSidebar = isSettings;
  const activeMovePackage = useMemo(
    () => resolveActiveMovePackage(packageTree, activePackageManifestPath),
    [activePackageManifestPath, packageTree],
  );
  const isBuildRunning = buildRun?.state === "running";

  useEffect(() => {
    setActivePackageManifestPath(packageTree?.activePackageManifestPath ?? null);
    setBuildRun(null);
    setIsBuildSheetOpen(false);
  }, [packageTree?.rootPath, packageTree?.activePackageManifestPath]);

  const runBuild = useCallback(async () => {
    if (!packageTree || !activeMovePackage || isBuildRunning) {
      return;
    }

    const startedAt = new Date();
    const workingDirectory = packagePathLabel(activeMovePackage, packageTree);
    const nextRun: BuildLogRun = {
      command: "sui move build",
      error: null,
      finishedAt: null,
      id: startedAt.getTime(),
      output: null,
      packageName: activeMovePackage.name,
      packagePath: activeMovePackage.path || ".",
      startedAt,
      state: "running",
      workingDirectory,
    };

    setBuildRun(nextRun);
    setIsBuildSheetOpen(true);

    try {
      const output = await buildMovePackage(packageTree, activeMovePackage.path);
      const state = output.status === 0 ? "success" : "error";

      setBuildRun({
        ...nextRun,
        finishedAt: new Date(),
        output,
        state,
      });
    } catch (error) {
      setBuildRun({
        ...nextRun,
        error: getBuildErrorMessage(error),
        finishedAt: new Date(),
        state: "error",
      });
    }
  }, [activeMovePackage, isBuildRunning, packageTree]);
  const buildLogSheet = useMemo<BuildLogSheetController>(
    () => ({
      isOpen: isBuildSheetOpen,
      onClose: () => setIsBuildSheetOpen(false),
      onRerun: runBuild,
      run: buildRun,
    }),
    [buildRun, isBuildSheetOpen, runBuild],
  );

  return (
    <main
      className="grid h-svh overflow-hidden bg-[var(--app-window)] text-foreground"
      style={{ gridTemplateRows: `${titlebarHeight}px minmax(0, 1fr)` }}
    >
      <Titlebar
        activeWorkspaceTab={activeWorkspaceTab}
        buildActionState={{
          disabled: !activeMovePackage,
          running: isBuildRunning,
        }}
        isLeftPanelOpen={isLeftPanelOpen}
        layout={layout}
        hasWorkspace={!isSettings && Boolean(packageTree)}
        onBuildPackage={runBuild}
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
              activePackageManifestPath={activePackageManifestPath}
              buildLogSheet={buildLogSheet}
              isLeftPanelOpen={isLeftPanelOpen}
              onActivePackageManifestPathChange={setActivePackageManifestPath}
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

function resolveActiveMovePackage(
  packageTree: PackageTree | null,
  activePackageManifestPath: string | null,
) {
  if (!packageTree) {
    return null;
  }

  if (packageTree.movePackages.length === 1) {
    return packageTree.movePackages[0] ?? null;
  }

  return packageTree.movePackages.find(
    (movePackage) => movePackage.manifestPath === activePackageManifestPath,
  ) ?? null;
}

function packagePathLabel(movePackage: MovePackage, packageTree: PackageTree) {
  if (!movePackage.path || movePackage.path === ".") {
    return packageTree.rootPath;
  }

  if (movePackage.path.startsWith("/")) {
    return movePackage.path;
  }

  return `${packageTree.rootPath}/${movePackage.path}`;
}

function getBuildErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Could not run `sui move build`.";
}
