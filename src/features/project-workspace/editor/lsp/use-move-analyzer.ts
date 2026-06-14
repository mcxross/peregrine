import React from "react";

import type { OpenFileTab } from "@/features/project-workspace/editor/types";
import {
  getSuiMoveAnalyzerCompletion,
  getSuiMoveAnalyzerDefinition,
  getSuiMoveAnalyzerDiagnostics,
  getSuiMoveAnalyzerHover,
  getSuiMoveAnalyzerReferences,
  getSuiMoveAnalyzerRename,
  getSuiMoveAnalyzerStatus,
  listenSuiMoveAnalyzerConfigChanged,
} from "@peregrine/desktop-runtime";
import type {
  MoveAnalyzerCompletionContext,
  MoveAnalyzerCompletionList,
  MoveAnalyzerDiagnostic,
  MoveAnalyzerHover,
  MoveAnalyzerPosition,
  MoveAnalyzerResolvedLocation,
  MoveAnalyzerWorkspaceEdit,
} from "@peregrine/desktop-runtime";

type UseMoveAnalyzerOptions = {
  rootPath: string;
  tabs: OpenFileTab[];
};

export type MoveAnalyzerClientStatus = {
  command: string | null;
  error: string | null;
  isRunning: boolean;
  stderr: string | null;
};

export type MoveAnalyzerLspFeatures = {
  completion: (
    path: string,
    position: MoveAnalyzerPosition,
    context?: MoveAnalyzerCompletionContext,
  ) => Promise<MoveAnalyzerCompletionList | null>;
  definition: (path: string, position: MoveAnalyzerPosition) => Promise<MoveAnalyzerResolvedLocation[]>;
  hover: (path: string, position: MoveAnalyzerPosition) => Promise<MoveAnalyzerHover | null>;
  references: (path: string, position: MoveAnalyzerPosition) => Promise<MoveAnalyzerResolvedLocation[]>;
  rename: (path: string, position: MoveAnalyzerPosition, newName: string) => Promise<MoveAnalyzerWorkspaceEdit | null>;
};

type MoveTab = {
  path: string;
  source: string;
};

const DIAGNOSTICS_DELAY_MS = 900;

export function useMoveAnalyzer({ rootPath, tabs }: UseMoveAnalyzerOptions) {
  const [diagnosticsByPath, setDiagnosticsByPath] = React.useState<Record<string, MoveAnalyzerDiagnostic[]>>({});
  const [status, setStatus] = React.useState<MoveAnalyzerClientStatus>({
    command: null,
    error: null,
    isRunning: false,
    stderr: null,
  });
  const [configRevision, setConfigRevision] = React.useState(0);
  const moveTabs = React.useMemo(
    () =>
      tabs.flatMap((tab) => {
        if (tab.preview?.kind !== "text" || tab.preview.language.toLowerCase() !== "move") {
          return [];
        }
        return [{
          path: tab.path,
          source: tab.editedSource ?? tab.preview.source,
        }];
      }),
    [tabs],
  );
  const moveTabsRef = React.useRef(moveTabs);

  React.useEffect(() => {
    moveTabsRef.current = moveTabs;
  }, [moveTabs]);

  React.useEffect(() => {
    let disposed = false;
    void getSuiMoveAnalyzerStatus(rootPath)
      .then((adapter) => {
        if (!disposed) {
          setStatus({
            command: adapter.resolvedPath ?? (adapter.activeSource === "bundled" ? "bundled move-analyzer" : null),
            error: adapter.installed ? null : adapter.installHint ?? "Move Analyzer is unavailable.",
            isRunning: adapter.installed,
            stderr: null,
          });
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setStatus({
            command: null,
            error: getErrorMessage(error, "Could not start the Move Analyzer MCP server."),
            isRunning: false,
            stderr: null,
          });
        }
      });
    return () => {
      disposed = true;
    };
  }, [configRevision, rootPath]);

  React.useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof globalThis.setTimeout> | null = globalThis.setTimeout(() => {
      timer = null;
      void Promise.all(moveTabs.map(async (tab) => {
        const response = await getSuiMoveAnalyzerDiagnostics(rootPath, tab);
        return [response.path, response.diagnostics, response.warnings] as const;
      }))
        .then((reports) => {
          if (disposed) {
            return;
          }
          setDiagnosticsByPath(Object.fromEntries(
            reports.map(([path, diagnostics]) => [path, diagnostics]),
          ));
          const warnings = reports.flatMap(([, , reportWarnings]) => reportWarnings);
          setStatus((current) => ({
            ...current,
            error: warnings[0] ?? null,
            isRunning: true,
          }));
        })
        .catch((error: unknown) => {
          if (!disposed) {
            setStatus((current) => ({
              ...current,
              error: getErrorMessage(error, "Move Analyzer diagnostics failed."),
              isRunning: false,
            }));
          }
        });
    }, DIAGNOSTICS_DELAY_MS);

    if (!moveTabs.length) {
      globalThis.clearTimeout(timer);
      timer = null;
      setDiagnosticsByPath({});
    }
    return () => {
      disposed = true;
      if (timer) {
        globalThis.clearTimeout(timer);
      }
    };
  }, [configRevision, moveTabs, rootPath]);

  React.useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenSuiMoveAnalyzerConfigChanged(() => {
      setDiagnosticsByPath({});
      setConfigRevision((revision) => revision + 1);
    }).then((listener) => {
      if (disposed) {
        listener();
      } else {
        unlisten = listener;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const lspFeatures = React.useMemo<MoveAnalyzerLspFeatures>(() => ({
    completion(path, position, context) {
      return getSuiMoveAnalyzerCompletion(rootPath, documentForPath(moveTabsRef.current, path), position, context);
    },
    definition(path, position) {
      return getSuiMoveAnalyzerDefinition(rootPath, documentForPath(moveTabsRef.current, path), position);
    },
    hover(path, position) {
      return getSuiMoveAnalyzerHover(rootPath, documentForPath(moveTabsRef.current, path), position);
    },
    references(path, position) {
      return getSuiMoveAnalyzerReferences(rootPath, documentForPath(moveTabsRef.current, path), position);
    },
    rename(path, position, newName) {
      return getSuiMoveAnalyzerRename(rootPath, documentForPath(moveTabsRef.current, path), position, newName);
    },
  }), [rootPath]);

  return {
    diagnosticsByPath,
    lspFeatures,
    status,
  };
}

function documentForPath(tabs: MoveTab[], path: string) {
  const tab = tabs.find((candidate) => candidate.path === path);
  return {
    path,
    source: tab?.source ?? null,
  };
}

function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }
  return typeof error === "string" ? error : fallback;
}
