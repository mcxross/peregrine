import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { CheckCircle2, Loader2, XCircle } from "lucide-react";
import { Titlebar } from "@/app/titlebar";
import type { WorkspaceTab } from "@/app/titlebar";
import { Sidebar } from "@/app/sidebar";
import { Workspace } from "@/app/workspace";
import { Button } from "@/components/ui/button";
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

const BUILD_FRESHNESS_WINDOW_MS = 3 * 60 * 1000;
const LAUNCH_BUILD_TIMESTAMPS_STORAGE_KEY = "peregrine.launchBuild.successTimestamps.v1";
const LAUNCH_BUILD_STATUS_MESSAGES = [
  "Preparing Sui environment...",
  "Compiling Move packages...",
  "Building project...",
];

type AppShellProps = {
  packageTree: PackageTree | null;
  screen: "workspace" | "settings";
  onCloseSettings: () => void;
  onProjectSelected: (packageTree: PackageTree) => void;
};

type LaunchBuildState = {
  key: string;
  message: string;
  packageName: string;
  runId: number;
  state: "running" | "success" | "error";
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
  const [launchBuild, setLaunchBuild] = useState<LaunchBuildState | null>(null);
  const loadAssessment: PackageLoadAssessment | null = null;
  const currentCommandLogIdRef = useRef<number | null>(null);
  const detailHydratedRootRef = useRef<string | null>(null);
  const launchBuildKeysRef = useRef<Set<string>>(new Set());
  const launchBuildMessageIndexRef = useRef(0);
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
    setLaunchBuild(null);
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

  useEffect(() => {
    if (!packageTree || !activeMovePackage) {
      return;
    }

    const launchBuildKey = projectBuildKey(packageTree, activeMovePackage);

    if (launchBuildKeysRef.current.has(launchBuildKey)) {
      return;
    }

    const lastSuccessfulBuildAt = lastSuccessfulLaunchBuildAt(launchBuildKey);

    if (lastSuccessfulBuildAt && Date.now() - lastSuccessfulBuildAt < BUILD_FRESHNESS_WINDOW_MS) {
      return;
    }

    launchBuildKeysRef.current.add(launchBuildKey);
    launchBuildMessageIndexRef.current = 0;

    const startedAt = new Date();
    const runId = startedAt.getTime();
    const workingDirectory = packagePathLabel(activeMovePackage, packageTree);
    const nextRun: BuildLogRun = {
      canRerun: false,
      command: "sui move build",
      error: null,
      finishedAt: null,
      id: runId,
      metadata: [{ label: "Trigger", value: "Project launch" }],
      output: null,
      packageName: activeMovePackage.name,
      packagePath: activeMovePackage.path || ".",
      runningText: LAUNCH_BUILD_STATUS_MESSAGES[0],
      startedAt,
      state: "running",
      title: "Launch build",
      workingDirectory,
    };

    currentCommandLogIdRef.current = nextRun.id;
    setBuildRuns((current) => upsertLogRun(current, nextRun));
    setLaunchBuild({
      key: launchBuildKey,
      message: LAUNCH_BUILD_STATUS_MESSAGES[0],
      packageName: activeMovePackage.name,
      runId,
      state: "running",
    });

    const messageTimer = window.setInterval(() => {
      launchBuildMessageIndexRef.current =
        (launchBuildMessageIndexRef.current + 1) % LAUNCH_BUILD_STATUS_MESSAGES.length;
      const message = LAUNCH_BUILD_STATUS_MESSAGES[launchBuildMessageIndexRef.current];

      setLaunchBuild((current) =>
        current?.key === launchBuildKey && current.state === "running"
          ? { ...current, message }
          : current,
      );
      setBuildRuns((current) =>
        updateLogRun(current, runId, (run) =>
          run.state === "running" ? { ...run, runningText: message } : run,
        ),
      );
    }, 2_800);

    void buildMovePackage(packageTree, activeMovePackage.path, {
      streamId: runId,
      onOutput: (output) => {
        setBuildRuns((current) =>
          updateLogRun(current, runId, (run) =>
            run.state === "running" ? { ...run, output } : run,
          ),
        );
      },
    })
      .then(async (output) => {
        const state = output.status === 0 ? "success" : "error";

        if (state === "success") {
          rememberSuccessfulLaunchBuild(launchBuildKey, Date.now());
        }

        setBuildRuns((current) =>
          updateLogRun(current, runId, (run) => ({
            ...run,
            finishedAt: new Date(),
            output,
            state,
          })),
        );

        setLaunchBuild((current) =>
          current?.key === launchBuildKey
            ? {
                ...current,
                message: state === "success" ? "Project build completed." : "Project build failed.",
                state,
              }
            : current,
        );

        if (state === "success" && latestPackageTreeRef.current?.rootPath === packageTree.rootPath) {
          try {
            const rescannedPackageTree = await loadPackageTree(packageTree.rootPath);
            const latestPackageTree = latestPackageTreeRef.current;

            if (!latestPackageTree || latestPackageTree.rootPath !== packageTree.rootPath) {
              return;
            }

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
            console.error("Could not rescan package after launch build.", error);
          }
        }
      })
      .catch((error) => {
        setBuildRuns((current) =>
          updateLogRun(current, runId, (run) => ({
            ...run,
            error: getBuildErrorMessage(error),
            finishedAt: new Date(),
            state: "error",
          })),
        );
        setLaunchBuild((current) =>
          current?.key === launchBuildKey
            ? { ...current, message: "Project build failed.", state: "error" }
            : current,
        );
      })
      .finally(() => {
        window.clearInterval(messageTimer);
      });

    return () => {
      window.clearInterval(messageTimer);
    };
  }, [activeMovePackage, handleProjectSelected, packageTree]);

  useEffect(() => {
    if (!launchBuild || launchBuild.state === "running") {
      return;
    }

    const timer = window.setTimeout(() => {
      setLaunchBuild((current) => current?.key === launchBuild.key ? null : current);
    }, launchBuild.state === "success" ? 3_500 : 7_000);

    return () => window.clearTimeout(timer);
  }, [launchBuild]);

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

      if (state === "success") {
        rememberSuccessfulLaunchBuild(projectBuildKey(packageTree, activeMovePackage), Date.now());
      }

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

      <LaunchBuildStatusToast
        state={launchBuild}
        onOpenLogs={() => {
          if (launchBuild) {
            currentCommandLogIdRef.current = launchBuild.runId;
            setIsBuildSheetOpen(true);
          }
        }}
      />
    </main>
  );
}

function LaunchBuildStatusToast({
  onOpenLogs,
  state,
}: {
  onOpenLogs: () => void;
  state: LaunchBuildState | null;
}) {
  if (!state) {
    return null;
  }

  const isRunning = state.state === "running";
  const isSuccess = state.state === "success";

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[120] max-w-[22rem]">
      <div className="pointer-events-auto flex items-center gap-3 rounded-md border border-[color:var(--app-border)] bg-[color-mix(in_oklch,var(--app-panel)_88%,transparent)] px-3 py-2 text-sm shadow-2xl shadow-black/35 backdrop-blur-md">
        <div className="grid size-8 shrink-0 place-items-center rounded bg-[var(--app-subtle)]">
          {isRunning ? <Loader2 className="size-4 animate-spin text-sky-300" aria-hidden="true" /> : null}
          {isSuccess ? <CheckCircle2 className="size-4 text-emerald-400" aria-hidden="true" /> : null}
          {!isRunning && !isSuccess ? <XCircle className="size-4 text-red-400" aria-hidden="true" /> : null}
        </div>
        <div className="min-w-0">
          <div className="truncate text-xs font-semibold text-foreground">
            {state.message}
          </div>
          <div className="truncate text-[11px] text-muted-foreground">
            {state.packageName}
          </div>
        </div>
        {!isRunning && !isSuccess ? (
          <Button className="h-7 shrink-0 px-2 text-xs" onClick={onOpenLogs} type="button" variant="outline">
            Logs
          </Button>
        ) : null}
      </div>
    </div>
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

function projectBuildKey(packageTree: PackageTree, movePackage: MovePackage) {
  return `${packageTree.rootPath}::${movePackage.manifestPath || movePackage.path || "."}`;
}

function lastSuccessfulLaunchBuildAt(buildKey: string) {
  return readLaunchBuildTimestamps()[buildKey] ?? null;
}

function rememberSuccessfulLaunchBuild(buildKey: string, timestamp: number) {
  const timestamps = readLaunchBuildTimestamps();
  timestamps[buildKey] = timestamp;

  try {
    window.localStorage.setItem(
      LAUNCH_BUILD_TIMESTAMPS_STORAGE_KEY,
      JSON.stringify(timestamps),
    );
  } catch (error) {
    console.warn("Could not store launch build timestamp.", error);
  }
}

function readLaunchBuildTimestamps(): Record<string, number> {
  try {
    const rawValue = window.localStorage.getItem(LAUNCH_BUILD_TIMESTAMPS_STORAGE_KEY);

    if (!rawValue) {
      return {};
    }

    const parsedValue = JSON.parse(rawValue) as unknown;

    if (!parsedValue || typeof parsedValue !== "object") {
      return {};
    }

    return Object.fromEntries(
      Object.entries(parsedValue)
        .filter(([, value]) => typeof value === "number" && Number.isFinite(value)),
    ) as Record<string, number>;
  } catch (error) {
    console.warn("Could not read launch build timestamps.", error);
    return {};
  }
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
