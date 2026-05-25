import { useCallback, useRef, useState } from "react";

import type { MovePackage, PackageTree } from "@peregrine/desktop-runtime";
import {
  cancelIndex,
  createIndexRunId,
  indexPackage,
  listenToIndexProgress,
} from "@peregrine/desktop-runtime";

export type LaunchIndexState = {
  key: string;
  message: string;
  packageName: string;
  runId: string;
  state: "running" | "success" | "error";
};

export function useLaunchIndexer() {
  const [launchIndex, setLaunchIndex] = useState<LaunchIndexState | null>(null);
  const activeLaunchIndexCancelRef = useRef<(() => void) | null>(null);
  const launchIndexKeysRef = useRef<Set<string>>(new Set());

  const resetLaunchIndex = useCallback(() => {
    activeLaunchIndexCancelRef.current?.();
    activeLaunchIndexCancelRef.current = null;
    launchIndexKeysRef.current.clear();
    setLaunchIndex(null);
  }, []);

  const startLaunchIndex = useCallback((packageTree: PackageTree, activeMovePackage: MovePackage) => {
    const indexKey = projectIndexRuntimeKey(packageTree, activeMovePackage);

    if (launchIndexKeysRef.current.has(indexKey)) {
      return;
    }

    launchIndexKeysRef.current.add(indexKey);

    const packageRoot = packageRootPath(activeMovePackage, packageTree);
    const runId = createIndexRunId();
    let isCurrent = true;
    let unlisten: (() => void) | null = null;

    const cancelActiveIndex = () => {
      isCurrent = false;

      void cancelIndex(runId).catch((error) => {
        console.warn("Could not cancel project index.", error);
      });

      unlisten?.();
    };

    activeLaunchIndexCancelRef.current?.();
    activeLaunchIndexCancelRef.current = cancelActiveIndex;

    setLaunchIndex({
      key: indexKey,
      message: "Preparing project index...",
      packageName: activeMovePackage.name,
      runId,
      state: "running",
    });

    void listenToIndexProgress((event) => {
      if (!isCurrent || event.runId !== runId) {
        return;
      }

      setLaunchIndex((current) =>
        current?.key === indexKey
          ? {
              ...current,
              message: event.message,
              runId,
              state: "running",
            }
          : current,
      );
    })
      .then((cleanup) => {
        if (!isCurrent) {
          cleanup();
          return;
        }

        unlisten = cleanup;

        void indexPackage(packageRoot, runId)
          .then((report) => {
            if (!isCurrent) {
              return;
            }

            setLaunchIndex((current) =>
              current?.key === indexKey
                ? {
                    ...current,
                    message: `Project index completed: ${report.moduleCount} modules, ${report.functionCount} functions.`,
                    runId: report.runId,
                    state: "success",
                  }
                : current,
            );
          })
          .catch((error) => {
            if (!isCurrent) {
              return;
            }

            console.error("Project index failed.", error);
            setLaunchIndex((current) =>
              current?.key === indexKey
                ? {
                    ...current,
                    message: getIndexErrorMessage(error),
                    runId,
                    state: "error",
                  }
                : current,
            );
          })
          .finally(() => {
            cleanup();

            if (activeLaunchIndexCancelRef.current === cancelActiveIndex) {
              activeLaunchIndexCancelRef.current = null;
            }
          });
      })
      .catch((error) => {
        if (!isCurrent) {
          return;
        }

        console.error("Could not listen for project index progress.", error);
        setLaunchIndex((current) =>
          current?.key === indexKey
            ? {
                ...current,
                message: "Could not listen for project index progress.",
                state: "error",
              }
            : current,
        );
      });
  }, []);

  return {
    launchIndex,
    resetLaunchIndex,
    setLaunchIndex,
    startLaunchIndex,
  };
}

function projectIndexRuntimeKey(packageTree: PackageTree, movePackage: MovePackage) {
  return `${packageTree.rootPath}::${movePackage.manifestPath || movePackage.path || "."}`;
}

function packageRootPath(movePackage: MovePackage, packageTree: PackageTree) {
  if (!movePackage.path || movePackage.path === ".") {
    return packageTree.rootPath;
  }

  if (movePackage.path.startsWith("/")) {
    return movePackage.path;
  }

  return `${packageTree.rootPath}/${movePackage.path}`;
}

function getIndexErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Project index failed.";
}
