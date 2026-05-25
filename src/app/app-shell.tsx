import React, { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { CheckCircle2, Loader2, XCircle } from "lucide-react";
import { Titlebar } from "@/app/titlebar";
import { Workspace } from "@/app/workspace";
import { Button } from "@/components/ui/button";
import {
  createPackageLoadAssessment,
  buildMovePackage,
  defaultProjectMetadata,
  defaultSuiNetworkSelection,
  displayMovePackageName,
  loadPackageTree,
  loadPackageTreeDetails,
  loadProjectMetadata,
  projectMoveCoverageScriptPath,
  projectMoveTestScriptPath,
  projectPackageConfigKey,
  runFormalVerification,
  runMovyFuzz,
  runSecurityScript,
  runSecurityCommand,
  saveTextExport,
  saveProjectMetadata,
  type AuditReportExport,
  type BuildLogRun,
  type BuildLogUpdateOptions,
  type FormalVerificationTarget,
  type MovePackage,
  type PackageTree,
  type PackageLoadAssessment,
  type PackageLoadAssessmentState,
  type SuiNetworkSelection,
  type WorkspaceMode,
  type WorkspaceTab,
} from "@peregrine/desktop-runtime";
import type {
  BuildLogSheetController,
} from "@/features/project-workspace/build-log-sheet";
import {
  type LaunchIndexState,
  useLaunchIndexer,
} from "@/features/project-workspace/indexer/use-launch-indexer";
import { defaultLayoutSettings } from "@/layout/layout-store";
import { titlebarHeight } from "@/layout/window-chrome";

const ProjectConfigDialog = React.lazy(() =>
  import("@/features/project-workspace/project-config-dialog").then((module) => ({
    default: module.ProjectConfigDialog,
  })),
);
const SettingsScreen = React.lazy(() =>
  import("@/screens/settings-screen").then((module) => ({
    default: module.SettingsScreen,
  })),
);

const BUILD_FRESHNESS_WINDOW_MS = 3 * 60 * 1000;
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
  const [activeWorkspaceTab, setActiveWorkspaceTab] = useState<WorkspaceTab>("Agents");
  const [workspaceMode, setWorkspaceMode] = useState<WorkspaceMode>("security");
  const [activePackageManifestPath, setActivePackageManifestPath] = useState<string | null>(null);
  const [buildRuns, setBuildRuns] = useState<BuildLogRun[]>([]);
  const [isBuildSheetOpen, setIsBuildSheetOpen] = useState(false);
  const [isLeftPanelOpen, setIsLeftPanelOpen] = useState(true);
  const [isProjectConfigOpen, setIsProjectConfigOpen] = useState(false);
  const [isExportingAuditReport, setIsExportingAuditReport] = useState(false);
  const [latestAuditReportExport, setLatestAuditReportExport] =
    useState<AuditReportExport | null>(null);
  const [formalVerificationTarget, setFormalVerificationTarget] =
    useState<FormalVerificationTarget | null>(null);
  const [lastScannedAt, setLastScannedAt] = useState<number | null>(null);
  const [launchBuild, setLaunchBuild] = useState<LaunchBuildState | null>(null);
  const [network, setNetwork] = useState<SuiNetworkSelection>(defaultSuiNetworkSelection);
  const {
    launchIndex,
    resetLaunchIndex,
    setLaunchIndex,
    startLaunchIndex,
  } = useLaunchIndexer();
  const currentCommandLogIdRef = useRef<number | null>(null);
  const detailHydratedRootRef = useRef<string | null>(null);
  const launchBuildKeysRef = useRef<Set<string>>(new Set());
  const launchBuildMessageIndexRef = useRef(0);
  const latestPackageTreeRef = useRef<PackageTree | null>(packageTree);
  const layout = defaultLayoutSettings;
  const isSettings = screen === "settings";
  const activeMovePackage = useMemo(
    () => resolveActiveMovePackage(packageTree, activePackageManifestPath),
    [activePackageManifestPath, packageTree],
  );
  const loadAssessment = useMemo(
    () => createVisibleLoadAssessment(packageTree, activeMovePackage, buildRuns),
    [activeMovePackage, buildRuns, packageTree],
  );
  const isBuildRunning = buildRuns.some((run) => run.state === "running");
  const isCommandRunning = isBuildRunning;
  const isDependencyGraphLoading = Boolean(
    packageTree && (!packageTree.isDetailed || (launchBuild && launchBuild.state !== "error")),
  );
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
    resetLaunchIndex();
    setActivePackageManifestPath(packageTree?.activePackageManifestPath ?? null);
    setBuildRuns([]);
    setLatestAuditReportExport(null);
    setFormalVerificationTarget(null);
    setIsBuildSheetOpen(false);
    setLaunchBuild(null);
    setLastScannedAt(packageTree ? Date.now() : null);
  }, [packageTree?.rootPath, packageTree?.activePackageManifestPath, resetLaunchIndex]);

  const exportLatestAuditReport = useCallback(async () => {
    if (!latestAuditReportExport || isExportingAuditReport) {
      return;
    }

    setIsExportingAuditReport(true);

    try {
      await exportAuditReport(latestAuditReportExport);
    } catch (error) {
      console.error("Could not export audit report.", error);
      window.alert(error instanceof Error ? error.message : "Could not export audit report.");
    } finally {
      setIsExportingAuditReport(false);
    }
  }, [isExportingAuditReport, latestAuditReportExport]);

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
            stateAccessGraph: hasStateAccessGraphPayload(latestPackageTree)
              ? latestPackageTree.stateAccessGraph
              : detailedPackageTree.stateAccessGraph,
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

    let isCancelled = false;
    const launchBuildKey = projectBuildRuntimeKey(packageTree, activeMovePackage);
    const packageMetadataKey = projectPackageConfigKey(activeMovePackage);

    if (launchBuildKeysRef.current.has(launchBuildKey)) {
      return;
    }

    launchBuildKeysRef.current.add(launchBuildKey);

    void loadProjectMetadata(packageTree.rootPath)
      .catch((error) => {
        console.warn("Could not load project metadata; running launch build.", error);
        return defaultProjectMetadata();
      })
      .then((metadata) => {
        if (isCancelled) {
          launchBuildKeysRef.current.delete(launchBuildKey);
          return;
        }

        const lastSuccessfulBuildAt =
          metadata.builds[packageMetadataKey]?.lastSuccessfulBuildAt ?? null;

        if (lastSuccessfulBuildAt && Date.now() - lastSuccessfulBuildAt < BUILD_FRESHNESS_WINDOW_MS) {
          launchBuildKeysRef.current.delete(launchBuildKey);
          startLaunchIndex(packageTree, activeMovePackage);
          return;
        }

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
              await rememberSuccessfulLaunchBuild(packageTree.rootPath, packageMetadataKey, Date.now());
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

            if (latestPackageTreeRef.current?.rootPath === packageTree.rootPath) {
              startLaunchIndex(packageTree, activeMovePackage);
            }

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

            if (latestPackageTreeRef.current?.rootPath === packageTree.rootPath) {
              startLaunchIndex(packageTree, activeMovePackage);
            }
          })
          .finally(() => {
            window.clearInterval(messageTimer);
            launchBuildKeysRef.current.delete(launchBuildKey);
          });
      });

    return () => {
      isCancelled = true;
    };
  }, [activeMovePackage, handleProjectSelected, packageTree, startLaunchIndex]);

  useEffect(() => {
    if (!launchBuild || launchBuild.state === "running") {
      return;
    }

    const timer = window.setTimeout(() => {
      setLaunchBuild((current) => current?.key === launchBuild.key ? null : current);
    }, launchBuild.state === "success" ? 3_500 : 7_000);

    return () => window.clearTimeout(timer);
  }, [launchBuild]);

  useEffect(() => {
    if (!launchIndex || launchIndex.state === "running") {
      return;
    }

    const timer = window.setTimeout(() => {
      setLaunchIndex((current) => current?.key === launchIndex.key ? null : current);
    }, launchIndex.state === "success" ? 3_500 : 7_000);

    return () => window.clearTimeout(timer);
  }, [launchIndex]);

  const runBuild = useCallback(async () => {
    if (!packageTree || !activeMovePackage || isCommandRunning) {
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
        await rememberSuccessfulLaunchBuild(
          packageTree.rootPath,
          projectPackageConfigKey(activeMovePackage),
          Date.now(),
        );
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
  }, [activeMovePackage, handleProjectSelected, isCommandRunning, packageTree]);

  const runTests = useCallback(async () => {
    if (!packageTree || !activeMovePackage || isCommandRunning) {
      return;
    }

    const startedAt = new Date();
    const workingDirectory = packagePathLabel(activeMovePackage, packageTree);
    const nextRun: BuildLogRun = {
      canRerun: false,
      command: "sui move test",
      error: null,
      finishedAt: null,
      id: startedAt.getTime(),
      output: null,
      packageName: activeMovePackage.name,
      packagePath: activeMovePackage.path || ".",
      runningText: "Running Move tests...",
      startedAt,
      state: "running",
      title: "Move tests",
      workingDirectory,
    };

    const metadata = await loadProjectMetadata(packageTree.rootPath).catch((error) => {
      console.warn("Could not load project metadata; running default Move tests.", error);
      return defaultProjectMetadata();
    });
    const moveTestScriptPath = projectMoveTestScriptPath(metadata, activeMovePackage);

    if (moveTestScriptPath) {
      nextRun.command = `bash ${moveTestScriptPath}`;
      nextRun.metadata = [
        { label: "Mode", value: "Project script" },
        { label: "Default", value: "sui move test" },
      ];
    }

    currentCommandLogIdRef.current = nextRun.id;
    setBuildRuns([nextRun]);
    setIsBuildSheetOpen(true);

    try {
      const output = moveTestScriptPath
        ? await runSecurityScript(packageTree, activeMovePackage.path, moveTestScriptPath, {
            streamId: nextRun.id,
            onOutput: (output) => {
              setBuildRuns((current) =>
                updateLogRun(current, nextRun.id, (run) =>
                  run.state === "running" ? { ...run, output } : run,
                ),
              );
            },
          })
        : await runSecurityCommand(packageTree, activeMovePackage.path, "move-test", {
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
    } catch (error) {
      setBuildRuns((current) =>
        updateLogRun(current, nextRun.id, (run) => ({
          ...run,
          error: getCommandErrorMessage(
            error,
            moveTestScriptPath
              ? `Could not run project test script ${moveTestScriptPath}.`
              : "Could not run `sui move test`.",
          ),
          finishedAt: new Date(),
          state: "error",
        })),
      );
    }
  }, [activeMovePackage, isCommandRunning, packageTree]);

  const checkCoverage = useCallback(async () => {
    if (!packageTree || !activeMovePackage || isCommandRunning) {
      return;
    }

    const startedAt = new Date();
    const workingDirectory = packagePathLabel(activeMovePackage, packageTree);
    const metadata = await loadProjectMetadata(packageTree.rootPath).catch((error) => {
      console.warn("Could not load project metadata; running default Move coverage.", error);
      return defaultProjectMetadata();
    });
    const moveCoverageScriptPath = projectMoveCoverageScriptPath(metadata, activeMovePackage);
    const moveTestScriptPath = projectMoveTestScriptPath(metadata, activeMovePackage);
    const coverageScriptPath = moveCoverageScriptPath ?? moveTestScriptPath;
    const coverageScriptArgs = moveCoverageScriptPath ? [] : moveTestScriptPath ? ["--coverage"] : [];
    const coverageRun: BuildLogRun = {
      canRerun: false,
      command: coverageScriptPath
        ? `bash ${coverageScriptPath}${coverageScriptArgs.length ? ` ${coverageScriptArgs.join(" ")}` : ""}`
        : "sui move test --coverage",
      error: null,
      finishedAt: null,
      id: startedAt.getTime(),
      metadata: coverageScriptPath
        ? [
            { label: "Mode", value: moveCoverageScriptPath ? "Project coverage script" : "Project test script" },
            { label: "Default", value: "sui move test --coverage" },
          ]
        : undefined,
      output: null,
      packageName: activeMovePackage.name,
      packagePath: activeMovePackage.path || ".",
      runningText: "Running tests with coverage...",
      startedAt,
      state: "running",
      title: "Coverage test run",
      workingDirectory,
    };

    currentCommandLogIdRef.current = coverageRun.id;
    setBuildRuns([coverageRun]);
    setIsBuildSheetOpen(true);

    try {
      const coverageOutput = coverageScriptPath
        ? await runSecurityScript(packageTree, activeMovePackage.path, coverageScriptPath, {
            args: coverageScriptArgs,
            streamId: coverageRun.id,
            onOutput: (output) => {
              setBuildRuns((current) =>
                updateLogRun(current, coverageRun.id, (run) =>
                  run.state === "running" ? { ...run, output } : run,
                ),
              );
            },
          })
        : await runSecurityCommand(packageTree, activeMovePackage.path, "move-coverage", {
            streamId: coverageRun.id,
            onOutput: (output) => {
              setBuildRuns((current) =>
                updateLogRun(current, coverageRun.id, (run) =>
                  run.state === "running" ? { ...run, output } : run,
                ),
              );
            },
          });
      const coverageState = coverageOutput.status === 0 ? "success" : "error";

      setBuildRuns((current) =>
        updateLogRun(current, coverageRun.id, (run) => ({
          ...run,
          finishedAt: new Date(),
          output: coverageOutput,
          state: coverageState,
        })),
      );

      if (coverageState !== "success") {
        return;
      }

      if (moveCoverageScriptPath) {
        return;
      }

      const summaryStartedAt = new Date();
      const summaryRun: BuildLogRun = {
        canRerun: false,
        command: "sui move coverage summary",
        error: null,
        finishedAt: null,
        id: summaryStartedAt.getTime(),
        output: null,
        packageName: activeMovePackage.name,
        packagePath: activeMovePackage.path || ".",
        runningText: "Reading coverage summary...",
        startedAt: summaryStartedAt,
        state: "running",
        title: "Coverage summary",
        workingDirectory,
      };

      currentCommandLogIdRef.current = summaryRun.id;
      setBuildRuns((current) => [...current, summaryRun]);

      const summaryOutput = await runSecurityCommand(
        packageTree,
        activeMovePackage.path,
        "move-coverage-summary",
        {
          streamId: summaryRun.id,
          onOutput: (output) => {
            setBuildRuns((current) =>
              updateLogRun(current, summaryRun.id, (run) =>
                run.state === "running" ? { ...run, output } : run,
              ),
            );
          },
        },
      );
      const summaryState = summaryOutput.status === 0 ? "success" : "error";

      setBuildRuns((current) =>
        updateLogRun(current, summaryRun.id, (run) => ({
          ...run,
          finishedAt: new Date(),
          output: summaryOutput,
          state: summaryState,
        })),
      );
    } catch (error) {
      const activeRunId = currentCommandLogIdRef.current ?? coverageRun.id;

      setBuildRuns((current) =>
        updateLogRun(current, activeRunId, (run) => ({
          ...run,
          error: getCommandErrorMessage(
            error,
            coverageScriptPath
              ? `Could not run project coverage script ${coverageScriptPath}.`
              : "Could not run coverage. Peregrine runs `sui move test --coverage` before reading the summary.",
          ),
          finishedAt: new Date(),
          state: "error",
        })),
      );
    }
  }, [activeMovePackage, isCommandRunning, packageTree]);

  const runFuzz = useCallback(async () => {
    if (!packageTree || !activeMovePackage || isCommandRunning) {
      return;
    }

    const startedAt = new Date();
    const workingDirectory = packagePathLabel(activeMovePackage, packageTree);
    const nextRun: BuildLogRun = {
      canRerun: false,
      command: "movy fuzz public-functions",
      error: null,
      finishedAt: null,
      id: startedAt.getTime(),
      metadata: [{ label: "Scope", value: "Public functions only" }],
      output: null,
      packageName: activeMovePackage.name,
      packagePath: activeMovePackage.path || ".",
      runningText: "Deploying package into Movy's executor...",
      startedAt,
      state: "running",
      title: "Movy fuzzing",
      workingDirectory,
    };

    currentCommandLogIdRef.current = nextRun.id;
    setBuildRuns([nextRun]);
    setIsBuildSheetOpen(true);

    try {
      const output = await runMovyFuzz(packageTree, activeMovePackage.path, {
        streamId: nextRun.id,
        onOutput: (output) => {
          setBuildRuns((current) =>
            updateLogRun(current, nextRun.id, (run) =>
              run.state === "running"
                ? { ...run, output, runningText: "Running Movy fuzzing..." }
                : run,
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
    } catch (error) {
      setBuildRuns((current) =>
        updateLogRun(current, nextRun.id, (run) => ({
          ...run,
          error: getCommandErrorMessage(error, "Could not run Movy fuzzing."),
          finishedAt: new Date(),
          state: "error",
        })),
      );
    }
  }, [activeMovePackage, isCommandRunning, packageTree]);

  const runFormalChecks = useCallback(async () => {
    if (!packageTree || !formalVerificationTarget || isCommandRunning) {
      return;
    }

    const startedAt = new Date();
    const nextRun: BuildLogRun = {
      canRerun: false,
      command: `bundled sui-prover --path ${formalVerificationTarget.packagePath || "."} --modules ${formalVerificationTarget.moduleName}`,
      error: null,
      finishedAt: null,
      id: startedAt.getTime(),
      metadata: [
        { label: "Mode", value: "Bundled Sui Prover" },
        { label: "File", value: formalVerificationTarget.filePath },
        { label: "Module filter", value: formalVerificationTarget.moduleName },
        { label: "Timeout", value: "45 seconds" },
      ],
      output: null,
      packageName: formalVerificationTarget.packageName,
      packagePath: formalVerificationTarget.packagePath || ".",
      runningText: "Running bundled Sui Prover...",
      startedAt,
      state: "running",
      title: "Formal verification",
      workingDirectory:
        formalVerificationTarget.packagePath && formalVerificationTarget.packagePath !== "."
          ? `${packageTree.rootPath}/${formalVerificationTarget.packagePath}`
          : packageTree.rootPath,
    };

    currentCommandLogIdRef.current = nextRun.id;
    setBuildRuns([nextRun]);
    setIsBuildSheetOpen(true);

    try {
      const output = await runFormalVerification(
        packageTree,
        formalVerificationTarget.packagePath,
        formalVerificationTarget.filePath,
        formalVerificationTarget.moduleName,
        {
          streamId: nextRun.id,
          timeoutSeconds: 45,
          onOutput: (output) => {
            setBuildRuns((current) =>
              updateLogRun(current, nextRun.id, (run) =>
                run.state === "running"
                  ? { ...run, output, runningText: "Verifying formal specifications..." }
                  : run,
              ),
            );
          },
        },
      );
      const state = output.status === 0 ? "success" : "error";

      setBuildRuns((current) =>
        updateLogRun(current, nextRun.id, (run) => ({
          ...run,
          finishedAt: new Date(),
          output,
          state,
        })),
      );
    } catch (error) {
      setBuildRuns((current) =>
        updateLogRun(current, nextRun.id, (run) => ({
          ...run,
          error: getCommandErrorMessage(error, "Could not run bundled Sui Prover."),
          finishedAt: new Date(),
          state: "error",
        })),
      );
    }
  }, [formalVerificationTarget, isCommandRunning, packageTree]);

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
          running: isCommandRunning,
        }}
        isLeftPanelOpen={isLeftPanelOpen}
        layout={layout}
        hasWorkspace={!isSettings && Boolean(packageTree)}
        network={network}
        auditReportExportAvailable={Boolean(latestAuditReportExport)}
        isExportingAuditReport={isExportingAuditReport}
        onBuildPackage={runBuild}
        onCheckCoverage={checkCoverage}
        onExportAuditReport={exportLatestAuditReport}
        onFuzzPackage={runFuzz}
        onNetworkChange={setNetwork}
        onOpenProjectConfig={() => setIsProjectConfigOpen(true)}
        onTestPackage={runTests}
        onToggleLeftPanel={() => setIsLeftPanelOpen((isOpen) => !isOpen)}
        testActionState={{
          disabled: !activeMovePackage,
          running: isCommandRunning,
        }}
        coverageActionState={{
          disabled: !activeMovePackage,
          running: isCommandRunning,
        }}
        fuzzActionState={{
          disabled: !activeMovePackage,
          running: isCommandRunning,
        }}
        formalActionState={{
          disabled: !formalVerificationTarget,
          running: isCommandRunning,
        }}
        onFormalVerification={runFormalChecks}
        onWorkspaceTabChange={setActiveWorkspaceTab}
        showNetworkSelector={!isSettings && Boolean(packageTree)}
      />

      <section className="flex min-h-0">
        <div className="min-w-0 flex-1">
          {isSettings ? (
            <React.Suspense fallback={<PanelLoadingState label="Loading settings..." />}>
              <SettingsScreen
                activeMovePackage={activeMovePackage}
                onBack={onCloseSettings}
                packageTree={packageTree}
              />
            </React.Suspense>
          ) : (
            <Workspace
              activeWorkspaceTab={activeWorkspaceTab}
              activePackageManifestPath={activePackageManifestPath}
              buildLogSheet={buildLogSheet}
              isDependencyGraphLoading={isDependencyGraphLoading}
              isLeftPanelOpen={isLeftPanelOpen}
              lastScannedAt={lastScannedAt}
              loadAssessment={loadAssessment}
              mode={workspaceMode}
              network={network}
              onNetworkChange={setNetwork}
              onToggleMode={() => setWorkspaceMode((mode) => mode === "security" ? "editor" : "security")}
              onAuditReportExportReady={setLatestAuditReportExport}
              onFormalVerificationTargetChange={setFormalVerificationTarget}
              onActivePackageManifestPathChange={setActivePackageManifestPath}
              onWorkspaceTabChange={setActiveWorkspaceTab}
              packageTree={packageTree}
              onCommandLog={showCommandLog}
              onProjectSelected={handleProjectSelected}
            />
          )}
        </div>
      </section>

      {!isSettings && packageTree ? (
        <React.Suspense fallback={null}>
          <ProjectConfigDialog
            activeMovePackage={activeMovePackage}
            open={isProjectConfigOpen}
            onOpenChange={setIsProjectConfigOpen}
            packageTree={packageTree}
          />
        </React.Suspense>
      ) : null}

      <LaunchStatusToasts
        buildState={launchBuild}
        indexState={launchIndex}
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

function PanelLoadingState({ label }: { label: string }) {
  return (
    <div className="grid h-full min-h-0 place-items-center bg-[var(--app-window)] px-6 text-sm text-muted-foreground">
      {label}
    </div>
  );
}

function LaunchStatusToasts({
  buildState,
  indexState,
  onOpenLogs,
}: {
  buildState: LaunchBuildState | null;
  indexState: LaunchIndexState | null;
  onOpenLogs: () => void;
}) {
  if (!buildState && !indexState) {
    return null;
  }

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[120] flex max-w-[22rem] flex-col gap-2">
      {buildState ? (
        <LaunchStatusToastCard
          action={
            buildState.state === "error" ? (
              <Button className="h-7 shrink-0 px-2 text-xs" onClick={onOpenLogs} type="button" variant="outline">
                Logs
              </Button>
            ) : null
          }
          message={buildState.message}
          packageName={buildState.packageName}
          state={buildState.state}
        />
      ) : null}
      {indexState ? (
        <LaunchStatusToastCard
          message={indexState.message}
          packageName={indexState.packageName}
          state={indexState.state}
        />
      ) : null}
    </div>
  );
}

function LaunchStatusToastCard({
  action,
  message,
  packageName,
  state,
}: {
  action?: ReactNode;
  message: string;
  packageName: string;
  state: "running" | "success" | "error";
}) {
  const isRunning = state === "running";
  const isSuccess = state === "success";

  return (
    <div className="pointer-events-auto flex items-center gap-3 rounded-md border border-[color:var(--app-border)] bg-[color-mix(in_oklch,var(--app-panel)_88%,transparent)] px-3 py-2 text-sm shadow-2xl shadow-black/35 backdrop-blur-md">
      <div className="grid size-8 shrink-0 place-items-center rounded bg-[var(--app-subtle)]">
        {isRunning ? <Loader2 className="size-4 animate-spin text-sky-300" aria-hidden="true" /> : null}
        {isSuccess ? <CheckCircle2 className="size-4 text-emerald-400" aria-hidden="true" /> : null}
        {!isRunning && !isSuccess ? <XCircle className="size-4 text-red-400" aria-hidden="true" /> : null}
      </div>
      <div className="min-w-0">
        <div className="truncate text-xs font-semibold text-foreground">
          {message}
        </div>
        <div className="truncate text-[11px] text-muted-foreground">
          {displayMovePackageName(packageName)}
        </div>
      </div>
      {action}
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

function projectBuildRuntimeKey(packageTree: PackageTree, movePackage: MovePackage) {
  return `${packageTree.rootPath}::${projectPackageConfigKey(movePackage)}`;
}

async function rememberSuccessfulLaunchBuild(
  rootPath: string,
  packageKey: string,
  timestamp: number,
) {
  try {
    const metadata = await loadProjectMetadata(rootPath);
    await saveProjectMetadata(rootPath, {
      ...metadata,
      builds: {
        ...metadata.builds,
        [packageKey]: {
          ...metadata.builds[packageKey],
          lastSuccessfulBuildAt: timestamp,
        },
      },
    });
  } catch (error) {
    console.warn("Could not store project build metadata.", error);
  }
}

function createVisibleLoadAssessment(
  packageTree: PackageTree | null,
  activeMovePackage: MovePackage | null,
  buildRuns: BuildLogRun[],
): PackageLoadAssessment | null {
  if (!packageTree || !activeMovePackage) {
    return null;
  }

  const packagePath = activeMovePackage.path || ".";
  const latestBuildRun = [...buildRuns]
    .reverse()
    .find(
      (run) =>
        run.command === "sui move build"
        && run.packageName === activeMovePackage.name
        && run.packagePath === packagePath,
    ) ?? null;
  const assessment = applyProjectLoadReportAssessment(
    createPackageLoadAssessment({
      movePackage: activeMovePackage,
      packageTree,
      startedAt: latestBuildRun?.startedAt ?? new Date(),
    }),
    packageTree,
    activeMovePackage,
  );

  if (!latestBuildRun) {
    return assessment;
  }

  const { detail, state, value } = buildAssessmentDisplay(latestBuildRun);

  return {
    ...assessment,
    finishedAt: latestBuildRun.finishedAt,
    startedAt: latestBuildRun.startedAt,
    steps: assessment.steps.map((step) =>
      step.id === "build"
        ? {
            ...step,
            detail,
            finishedAt: latestBuildRun.finishedAt,
            output: latestBuildRun.output,
            startedAt: latestBuildRun.startedAt,
            state,
            value,
          }
        : step,
    ),
  };
}

function applyProjectLoadReportAssessment(
  assessment: PackageLoadAssessment,
  packageTree: PackageTree,
  activeMovePackage: MovePackage,
): PackageLoadAssessment {
  const manifestPath = activeMovePackage.manifestPath;
  const capability = packageTree.loadReport.capabilities[manifestPath] ?? null;
  const report = packageTree.loadReport.analysisReports[manifestPath] ?? null;
  const analyzerStage = packageTree.loadReport.stages.find((stage) => stage.id === "analyzer") ?? null;
  const analyzerDiagnostic = analyzerStage?.diagnostics.find(
    (diagnostic) =>
      diagnostic.packageManifestPath === manifestPath && diagnostic.level === "error",
  ) ?? analyzerStage?.diagnostics.find(
    (diagnostic) =>
      diagnostic.packageManifestPath === manifestPath || diagnostic.packageManifestPath === null,
  ) ?? null;
  let riskStep: Partial<PackageLoadAssessment["steps"][number]>;

  if (report) {
    riskStep = analyzerAssessmentDisplay(report);
  } else if (capability && !capability.canRunStaticAnalysis) {
    riskStep = {
      caption: "Requires modules",
      detail: "Static analysis requires parseable Move modules.",
      enabled: false,
      state: "muted",
      value: "Skipped",
    };
  } else if (analyzerDiagnostic?.level === "error" || analyzerStage?.status === "failed") {
    riskStep = {
      caption: "Analyzer failed",
      detail: analyzerDiagnostic?.message ?? "Static analysis could not complete.",
      enabled: false,
      state: "error",
      value: "Fail",
    };
  } else {
    riskStep = {
      caption: packageTree.isDetailed ? "No analyzer report" : "Runs after launch",
      detail: packageTree.isDetailed
        ? "Static analysis did not return a package report."
        : "Static analysis runs during detailed project hydration.",
      enabled: false,
      state: "muted",
      value: "Pending",
    };
  }

  return {
    ...assessment,
    steps: assessment.steps.map((step) =>
      step.id === "risk"
        ? {
            ...step,
            ...riskStep,
            command: null,
            output: null,
          }
        : step,
    ),
  };
}

function analyzerAssessmentDisplay(
  report: PackageTree["loadReport"]["analysisReports"][string],
): Partial<PackageLoadAssessment["steps"][number]> {
  const findingCount = report.findings.length;
  const errorCount = report.diagnostics.filter((diagnostic) => diagnostic.level === "error").length;
  const warningCount = report.diagnostics.filter((diagnostic) => diagnostic.level === "warning").length;
  const state: PackageLoadAssessmentState =
    errorCount > 0
      ? "error"
      : findingCount > 0 || warningCount > 0
        ? "attention"
        : "success";
  const detailParts = [
    formatLoadReportCount(report.loadedRulesets.length, "ruleset"),
    report.loadedPlugins.length
      ? formatLoadReportCount(report.loadedPlugins.length, "plugin")
      : null,
    warningCount ? formatLoadReportCount(warningCount, "warning") : null,
    errorCount ? formatLoadReportCount(errorCount, "error") : null,
  ].filter(Boolean);

  return {
    caption: errorCount > 0
      ? "Analyzer reported errors"
      : findingCount > 0
        ? "Findings detected"
        : "Analyzer completed",
    detail: detailParts.join(" / ") || "Analyzer completed",
    enabled: true,
    state,
    value: findingCount === 0 ? "Clean" : formatLoadReportCount(findingCount, "finding"),
  };
}

function formatLoadReportCount(count: number, label: string) {
  return `${count} ${label}${count === 1 ? "" : "s"}`;
}

async function exportAuditReport(report: AuditReportExport) {
  const defaultPath = report.defaultFileName || "peregrine-audit-report.md";
  let path: string | null;

  try {
    path = await save({
      defaultPath,
      filters: [
        { name: "Markdown report", extensions: ["md"] },
        { name: "JSON report packet", extensions: ["json"] },
      ],
      title: "Export audit report",
    });
  } catch (error) {
    if (isTauriExportError(error)) {
      downloadTextFallback(defaultPath, report.markdown, "text/markdown;charset=utf-8");
      return;
    }

    throw error;
  }

  if (!path) {
    return;
  }

  const selectedPath = path;
  const contents = selectedPath.toLowerCase().endsWith(".json")
    ? report.reportJson
    : report.markdown;

  try {
    await saveTextExport(selectedPath, contents);
  } catch (error) {
    if (isTauriExportError(error)) {
      downloadTextFallback(
        selectedPath.split(/[\\/]/).pop() || defaultPath,
        contents,
        selectedPath.toLowerCase().endsWith(".json")
          ? "application/json;charset=utf-8"
          : "text/markdown;charset=utf-8",
      );
      return;
    }

    throw error;
  }
}

function isTauriExportError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error ?? "");
  return /invoke|tauri|not implemented|permission|command save_text_export not found/i.test(message);
}

function downloadTextFallback(fileName: string, contents: string, type: string) {
  const blob = new Blob([contents], { type });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.rel = "noopener";
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

function buildAssessmentDisplay(run: BuildLogRun): {
  detail: string;
  state: PackageLoadAssessmentState;
  value: string;
} {
  if (run.state === "running") {
    return {
      detail: run.runningText ?? "Build running",
      state: "running",
      value: "Run",
    };
  }

  if (run.state === "success") {
    return {
      detail: "Build passed",
      state: "success",
      value: "Pass",
    };
  }

  return {
    detail: run.error ?? "Build failed",
    state: "error",
    value: "Fail",
  };
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

function hasStateAccessGraphPayload(packageTree: PackageTree) {
  return packageTree.stateAccessGraph.nodes.length > 0
    || packageTree.stateAccessGraph.edges.length > 0
    || packageTree.stateAccessGraph.unresolvedAccesses.length > 0;
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
  return getCommandErrorMessage(error, "Could not run `sui move build`.");
}

function getCommandErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : fallback;
}
