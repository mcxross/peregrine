import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Titlebar } from "@/app/titlebar";
import type { WorkspaceTab } from "@/app/titlebar";
import { Sidebar } from "@/app/sidebar";
import { Workspace } from "@/app/workspace";
import {
  buildMovePackage,
  loadPackageTree,
  loadPackageTreeDetails,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import type {
  BuildLogRun,
  BuildLogSheetController,
  BuildLogUpdateOptions,
} from "@/features/project-workspace/build-log-sheet";
import type { PackageLoadAssessment } from "@/features/project-workspace/package-load-assessment";
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
  const [buildRuns, setBuildRuns] = useState<BuildLogRun[]>([]);
  const [isBuildSheetOpen, setIsBuildSheetOpen] = useState(false);
  const [isLeftPanelOpen, setIsLeftPanelOpen] = useState(true);
  const [isRescanning, setIsRescanning] = useState(false);
  const [lastScannedAt, setLastScannedAt] = useState<number | null>(null);
  const loadAssessment: PackageLoadAssessment | null = null;
  const currentCommandLogIdRef = useRef<number | null>(null);
  const detailHydratedRootRef = useRef<string | null>(null);
  const latestPackageTreeRef = useRef<PackageTree | null>(packageTree);
  const layout = defaultLayoutSettings;
  const isSettings = screen === "settings";
  const showSidebar = isSettings;
  const activeMovePackage = useMemo(
    () => resolveActiveMovePackage(packageTree, activePackageManifestPath),
    [activePackageManifestPath, packageTree],
  );
  const isBuildRunning = buildRuns.some((run) => run.state === "running");
  const handleProjectSelected = useCallback(
    (nextPackageTree: PackageTree) => {
      onProjectSelected(nextPackageTree);
      setLastScannedAt(Date.now());
    },
    [onProjectSelected],
  );

  useEffect(() => {
    latestPackageTreeRef.current = packageTree;
  }, [packageTree]);

  useEffect(() => {
    setActivePackageManifestPath(packageTree?.activePackageManifestPath ?? null);
    setBuildRuns([]);
    setIsBuildSheetOpen(false);
    setLastScannedAt(packageTree ? Date.now() : null);
  }, [packageTree?.rootPath, packageTree?.activePackageManifestPath]);

  useEffect(() => {
    if (!packageTree) {
      detailHydratedRootRef.current = null;
      return;
    }

    if (packageTree.isDetailed) {
      if (detailHydratedRootRef.current === packageTree.rootPath) {
        detailHydratedRootRef.current = null;
      }
      return;
    }

    if (detailHydratedRootRef.current === packageTree.rootPath) {
      return;
    }

    detailHydratedRootRef.current = packageTree.rootPath;

    const timer = window.setTimeout(() => {
      void loadPackageTreeDetails(packageTree.rootPath)
        .then((detailedPackageTree) => {
          const latestPackageTree = latestPackageTreeRef.current;

          if (detailHydratedRootRef.current === detailedPackageTree.rootPath) {
            detailHydratedRootRef.current = null;
          }

          if (!latestPackageTree || latestPackageTree.rootPath !== detailedPackageTree.rootPath) {
            return;
          }

          handleProjectSelected({
            ...detailedPackageTree,
            activePackageManifestPath:
              latestPackageTree.activePackageManifestPath
              ?? activePackageManifestPath
              ?? detailedPackageTree.activePackageManifestPath
              ?? null,
            callGraph: hasCallGraphPayload(latestPackageTree)
              ? latestPackageTree.callGraph
              : detailedPackageTree.callGraph,
            typeGraph: hasTypeGraphPayload(latestPackageTree)
              ? latestPackageTree.typeGraph
              : detailedPackageTree.typeGraph,
          });
        })
        .catch((error) => {
          if (detailHydratedRootRef.current === packageTree.rootPath) {
            detailHydratedRootRef.current = null;
          }
          console.error("Could not hydrate package details.", error);
        });
    }, 350);

    return () => {
      window.clearTimeout(timer);

      if (detailHydratedRootRef.current === packageTree.rootPath) {
        detailHydratedRootRef.current = null;
      }
    };
  }, [activePackageManifestPath, handleProjectSelected, packageTree]);

  const showCommandLog = useCallback((run: BuildLogRun, options?: BuildLogUpdateOptions) => {
    const shouldReset = options?.reset === true;
    const shouldOpen = options?.open !== false;
    const shouldForceOpen = options?.open === true;
    const isSameRun = currentCommandLogIdRef.current === run.id;
    currentCommandLogIdRef.current = run.id;
    setBuildRuns((current) => shouldReset ? [run] : upsertLogRun(current, run));

    if (shouldOpen && (shouldForceOpen || shouldReset || !isSameRun)) {
      setIsBuildSheetOpen(true);
    }
  }, []);

  const rescanProject = useCallback(async () => {
    if (!packageTree || isRescanning) {
      return;
    }

    const previousActiveManifestPath =
      activePackageManifestPath ?? packageTree.activePackageManifestPath ?? null;

    setIsRescanning(true);

    try {
      const rescannedPackageTree = await loadPackageTree(packageTree.rootPath);
      const activePackageManifestPath =
        previousActiveManifestPath &&
        rescannedPackageTree.movePackages.some(
          (movePackage) => movePackage.manifestPath === previousActiveManifestPath,
        )
          ? previousActiveManifestPath
          : rescannedPackageTree.movePackages[0]?.manifestPath ?? null;

      handleProjectSelected({
        ...rescannedPackageTree,
        activePackageManifestPath,
      });
    } catch (error) {
      console.error("Could not rescan package.", error);
    } finally {
      setIsRescanning(false);
    }
  }, [activePackageManifestPath, handleProjectSelected, isRescanning, packageTree]);

  const runBuild = useCallback(async () => {
    if (!packageTree || !activeMovePackage || isBuildRunning) {
      return;
    }

    const startedAt = new Date();
    const workingDirectory = packagePathLabel(activeMovePackage, packageTree);
    const nextRun: BuildLogRun = {
      canRerun: true,
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

    currentCommandLogIdRef.current = nextRun.id;
    setBuildRuns([nextRun]);
    setIsBuildSheetOpen(true);

    try {
      const output = await buildMovePackage(packageTree, activeMovePackage.path, {
        streamId: nextRun.id,
        onOutput: (output) => {
          setBuildRuns((current) =>
            updateLogRun(current, nextRun.id, (run) =>
              run.state === "running" ? { ...run, output } : run,
            ),
          );
        },
      });
      const state = output.status === 0 ? "success" : "error";

      setBuildRuns((current) =>
        updateLogRun(current, nextRun.id, (run) => ({
          ...run,
          finishedAt: new Date(),
          output,
          state,
        })),
      );

      if (state === "success") {
        try {
          const rescannedPackageTree = await loadPackageTree(packageTree.rootPath);
          const activePackageManifestPath =
            rescannedPackageTree.movePackages.some(
              (movePackage) => movePackage.manifestPath === activeMovePackage.manifestPath,
            )
              ? activeMovePackage.manifestPath
              : rescannedPackageTree.movePackages[0]?.manifestPath ?? null;

          handleProjectSelected({
            ...rescannedPackageTree,
            activePackageManifestPath,
          });
        } catch (error) {
          console.error("Could not rescan package after build.", error);
        }
      }
    } catch (error) {
      setBuildRuns((current) =>
        updateLogRun(current, nextRun.id, (run) => ({
          ...run,
          error: getBuildErrorMessage(error),
          finishedAt: new Date(),
          state: "error",
        })),
      );
    }
  }, [activeMovePackage, handleProjectSelected, isBuildRunning, packageTree]);
  const buildLogSheet = useMemo<BuildLogSheetController>(
    () => ({
      isOpen: isBuildSheetOpen,
      onClose: () => setIsBuildSheetOpen(false),
      onRerun: runBuild,
      runs: buildRuns,
    }),
    [buildRuns, isBuildSheetOpen, runBuild],
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
        rescanActionState={{
          disabled: !packageTree,
          running: isRescanning,
        }}
        isLeftPanelOpen={isLeftPanelOpen}
        layout={layout}
        hasWorkspace={!isSettings && Boolean(packageTree)}
        onBuildPackage={runBuild}
        onRescanProject={rescanProject}
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
              lastScannedAt={lastScannedAt}
              loadAssessment={loadAssessment}
              onActivePackageManifestPathChange={setActivePackageManifestPath}
              onWorkspaceTabChange={setActiveWorkspaceTab}
              packageTree={packageTree}
              onCommandLog={showCommandLog}
              onProjectSelected={handleProjectSelected}
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

function hasCallGraphPayload(packageTree: PackageTree) {
  return packageTree.callGraph.nodes.length > 0
    || packageTree.callGraph.edges.length > 0
    || packageTree.callGraph.unresolvedCalls.length > 0;
}

function hasTypeGraphPayload(packageTree: PackageTree) {
  return packageTree.typeGraph.nodes.length > 0
    || packageTree.typeGraph.edges.length > 0
    || packageTree.typeGraph.unresolvedTypes.length > 0;
}

function upsertLogRun(runs: BuildLogRun[], nextRun: BuildLogRun) {
  const existingIndex = runs.findIndex((run) => run.id === nextRun.id);

  if (existingIndex === -1) {
    return [...runs, nextRun];
  }

  return runs.map((run, index) => index === existingIndex ? nextRun : run);
}

function updateLogRun(
  runs: BuildLogRun[],
  runId: number,
  update: (run: BuildLogRun) => BuildLogRun,
) {
  return runs.map((run) => run.id === runId ? update(run) : run);
}

function getBuildErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Could not run `sui move build`.";
}
