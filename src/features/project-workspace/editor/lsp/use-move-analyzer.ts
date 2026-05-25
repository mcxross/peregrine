import React from "react";

import type { OpenFileTab } from "@/features/project-workspace/editor/types";
import {
  listenMoveAnalyzerAdapterSettingsChanged,
  listenMoveAnalyzerExit,
  listenMoveAnalyzerMessages,
  listenMoveAnalyzerStderr,
  normalizeMoveAnalyzerCompletionList,
  normalizeMoveAnalyzerHover,
  normalizeMoveAnalyzerLocations,
  normalizeMoveAnalyzerPublishDiagnostics,
  normalizeMoveAnalyzerWorkspaceEdit,
  sendMoveAnalyzerMessage,
  showMoveAnalyzerMessageText,
  startMoveAnalyzerServer,
  stopMoveAnalyzerServer,
  textDocumentPositionParams,
  type JsonRpcMessage,
} from "@peregrine/desktop-runtime";
import { fileUri } from "@peregrine/desktop-runtime";
import type {
  MoveAnalyzerCompletionList,
  MoveAnalyzerCompletionContext,
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
  uri: string;
};

type MoveTextTab = Omit<MoveTab, "uri">;

type PendingRequest = {
  reject: (error: Error) => void;
  resolve: (result: unknown) => void;
  timeout: ReturnType<typeof globalThis.setTimeout>;
};

const DID_CHANGE_DELAY_MS = 900;
const LSP_REQUEST_TIMEOUT_MS = 12_000;

export function useMoveAnalyzer({ rootPath, tabs }: UseMoveAnalyzerOptions) {
  const [diagnosticsByPath, setDiagnosticsByPath] = React.useState<Record<string, MoveAnalyzerDiagnostic[]>>({});
  const [status, setStatus] = React.useState<MoveAnalyzerClientStatus>({
    command: null,
    error: null,
    isRunning: false,
    stderr: null,
  });
  const [listenersReady, setListenersReady] = React.useState(false);
  const [restartToken, setRestartToken] = React.useState(0);
  const sessionIdRef = React.useRef<string | null>(null);
  const sessionRootPathRef = React.useRef(rootPath);
  const initializedRef = React.useRef(false);
  const initializeRequestIdRef = React.useRef<number | null>(null);
  const nextRequestIdRef = React.useRef(1);
  const pendingRequestsRef = React.useRef<Map<number | string, PendingRequest>>(new Map());
  const openedTabsRef = React.useRef<Map<string, MoveTab>>(new Map());
  const versionByUriRef = React.useRef<Map<string, number>>(new Map());
  const changeTimersRef = React.useRef<Map<string, ReturnType<typeof globalThis.setTimeout>>>(new Map());
  const latestMoveTabsRef = React.useRef<MoveTextTab[]>([]);
  const latestMoveTabsSignatureRef = React.useRef("");
  const crashedMoveTabsSignatureRef = React.useRef<string | null>(null);

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
  const moveTabsSignature = React.useMemo(() => signatureForMoveTabs(moveTabs), [moveTabs]);

  React.useEffect(() => {
    latestMoveTabsRef.current = moveTabs;
    latestMoveTabsSignatureRef.current = moveTabsSignature;
  }, [moveTabs, moveTabsSignature]);

  const send = React.useCallback((message: JsonRpcMessage) => {
    const sessionId = sessionIdRef.current;

    if (!sessionId) {
      return Promise.resolve();
    }

    return sendMoveAnalyzerMessage(sessionId, message).catch((error: unknown) => {
      setStatus((current) => ({
        ...current,
        error: getErrorMessage(error, "Could not send Move Analyzer message."),
      }));
    });
  }, []);

  const request = React.useCallback((method: string, params: unknown) => {
    const sessionId = sessionIdRef.current;

    if (!sessionId || !initializedRef.current) {
      return Promise.resolve(null);
    }

    const id = nextRequestIdRef.current++;

    return new Promise<unknown>((resolve, reject) => {
      const timeout = globalThis.setTimeout(() => {
        pendingRequestsRef.current.delete(id);
        reject(new Error(`Move Analyzer request timed out: ${method}`));
      }, LSP_REQUEST_TIMEOUT_MS);

      pendingRequestsRef.current.set(id, {
        reject,
        resolve,
        timeout,
      });

      sendMoveAnalyzerMessage(sessionId, {
        id,
        jsonrpc: "2.0",
        method,
        params,
      }).catch((error: unknown) => {
        const pending = pendingRequestsRef.current.get(id);

        if (pending) {
          globalThis.clearTimeout(pending.timeout);
          pendingRequestsRef.current.delete(id);
        }

        reject(new Error(getErrorMessage(error, `Could not send Move Analyzer request: ${method}`)));
      });
    }).catch((error: unknown) => {
      if (import.meta.env.DEV) {
        console.debug("[MoveAnalyzer] LSP request failed", {
          error,
          method,
        });
      }

      return null;
    });
  }, []);

  const lspFeatures = React.useMemo<MoveAnalyzerLspFeatures>(() => ({
    async completion(path, position, context) {
      const result = await request("textDocument/completion", {
        ...textDocumentPositionParams(
          sessionRootPathRef.current || rootPath,
          path,
          position,
        ),
        context,
      });

      return normalizeMoveAnalyzerCompletionList(result);
    },
    async definition(path, position) {
      const result = await request("textDocument/definition", textDocumentPositionParams(
        sessionRootPathRef.current || rootPath,
        path,
        position,
      ));

      return normalizeMoveAnalyzerLocations(sessionRootPathRef.current || rootPath, result);
    },
    async hover(path, position) {
      const result = await request("textDocument/hover", textDocumentPositionParams(
        sessionRootPathRef.current || rootPath,
        path,
        position,
      ));

      return normalizeMoveAnalyzerHover(result);
    },
    async references(path, position) {
      const result = await request("textDocument/references", {
        ...textDocumentPositionParams(
          sessionRootPathRef.current || rootPath,
          path,
          position,
        ),
        context: {
          includeDeclaration: true,
        },
      });

      return normalizeMoveAnalyzerLocations(sessionRootPathRef.current || rootPath, result);
    },
    async rename(path, position, newName) {
      const result = await request("textDocument/rename", {
        ...textDocumentPositionParams(
          sessionRootPathRef.current || rootPath,
          path,
          position,
        ),
        newName,
      });

      return normalizeMoveAnalyzerWorkspaceEdit(sessionRootPathRef.current || rootPath, result);
    },
  }), [request, rootPath]);

  const syncOpenTabs = React.useCallback(async () => {
    if (!initializedRef.current || !sessionIdRef.current) {
      return;
    }

    const rootForUris = sessionRootPathRef.current || rootPath;
    const nextTabs = new Map(
      latestMoveTabsRef.current.map((tab) => {
        const moveTab = {
          ...tab,
          uri: fileUri(rootForUris, tab.path),
        };

        return [moveTab.uri, moveTab] as const;
      }),
    );
    const openedTabs = openedTabsRef.current;

    for (const [uri, previousTab] of [...openedTabs.entries()]) {
      if (nextTabs.has(uri)) {
        continue;
      }

      openedTabs.delete(uri);
      versionByUriRef.current.delete(uri);
      clearChangeTimer(uri, changeTimersRef.current);
      await send({
        jsonrpc: "2.0",
        method: "textDocument/didClose",
        params: {
          textDocument: { uri },
        },
      });
      setDiagnosticsByPath((current) => {
        const next = { ...current };
        delete next[previousTab.path];
        return next;
      });
    }

    for (const tab of nextTabs.values()) {
      const previous = openedTabs.get(tab.uri);

      if (!previous) {
        openedTabs.set(tab.uri, tab);
        versionByUriRef.current.set(tab.uri, 1);
        await send({
          jsonrpc: "2.0",
          method: "textDocument/didOpen",
          params: {
            textDocument: {
              languageId: "move",
              text: tab.source,
              uri: tab.uri,
              version: 1,
            },
          },
        });
        continue;
      }

      if (previous.source === tab.source) {
        continue;
      }

      openedTabs.set(tab.uri, tab);
      clearChangeTimer(tab.uri, changeTimersRef.current);
      changeTimersRef.current.set(
        tab.uri,
        globalThis.setTimeout(() => {
          void sendDidChange(tab, send, versionByUriRef.current);
          changeTimersRef.current.delete(tab.uri);
        }, DID_CHANGE_DELAY_MS),
      );
    }
  }, [rootPath, send]);

  const stopSession = React.useCallback(async () => {
    const sessionId = sessionIdRef.current;

    for (const timer of changeTimersRef.current.values()) {
      globalThis.clearTimeout(timer);
    }
    changeTimersRef.current.clear();
    openedTabsRef.current.clear();
    versionByUriRef.current.clear();
    rejectPendingRequests(pendingRequestsRef.current, "Move Analyzer stopped.");
    initializedRef.current = false;
    initializeRequestIdRef.current = null;
    sessionIdRef.current = null;
    sessionRootPathRef.current = rootPath;
    crashedMoveTabsSignatureRef.current = null;

    if (sessionId) {
      await stopMoveAnalyzerServer(sessionId).catch(() => undefined);
    }

    setStatus({
      command: null,
      error: null,
      isRunning: false,
      stderr: null,
    });
  }, [rootPath]);

  const startSession = React.useCallback(async () => {
    if (sessionIdRef.current || !latestMoveTabsRef.current.length) {
      return;
    }

    setStatus({
      command: null,
      error: null,
      isRunning: false,
      stderr: null,
    });

    try {
      const session = await startMoveAnalyzerServer(rootPath);
      const requestId = nextRequestIdRef.current++;
      const sessionRootPath = session.rootPath || rootPath;

      sessionIdRef.current = session.sessionId;
      sessionRootPathRef.current = sessionRootPath;
      initializeRequestIdRef.current = requestId;
      crashedMoveTabsSignatureRef.current = null;
      setStatus({
        command: session.command,
        error: null,
        isRunning: true,
        stderr: null,
      });
      await sendMoveAnalyzerMessage(session.sessionId, {
        id: requestId,
        jsonrpc: "2.0",
        method: "initialize",
        params: {
          capabilities: {
            textDocument: {
              completion: {
                completionItem: {
                  documentationFormat: ["markdown", "plaintext"],
                  snippetSupport: false,
                },
                dynamicRegistration: false,
              },
              definition: {
                dynamicRegistration: false,
              },
              hover: {
                contentFormat: ["markdown", "plaintext"],
                dynamicRegistration: false,
              },
              publishDiagnostics: {
                relatedInformation: false,
              },
              references: {
                dynamicRegistration: false,
              },
              rename: {
                dynamicRegistration: false,
                prepareSupport: false,
              },
            },
            workspace: {
              configuration: false,
            },
          },
          processId: null,
          rootUri: fileUri(sessionRootPath),
          workspaceFolders: [
            {
              name: basename(sessionRootPath),
              uri: fileUri(sessionRootPath),
            },
          ],
        },
      });
    } catch (error) {
      setStatus({
        command: null,
        error: getErrorMessage(error, "Could not start Move Analyzer."),
        isRunning: false,
        stderr: null,
      });
    }
  }, [rootPath]);

  React.useEffect(() => {
    let disposed = false;
    let cleanupListeners: Array<() => void> = [];

    setListenersReady(false);

    Promise.all([
      listenMoveAnalyzerMessages((event) => {
        if (disposed || event.sessionId !== sessionIdRef.current) {
          return;
        }

        const message = event.message;

        if (message.method === "window/workDoneProgress/create" && message.id != null) {
          void send({
            id: message.id,
            jsonrpc: "2.0",
            result: null,
          });
          return;
        }

        if (message.method === "window/showMessage") {
          const analyzerMessage = showMoveAnalyzerMessageText(message.params);

          if (analyzerMessage) {
            setStatus((current) => ({
              ...current,
              error: analyzerMessage,
            }));
          }
          return;
        }

        if (message.id === initializeRequestIdRef.current && !message.method) {
          if (message.error) {
            setStatus((current) => ({
              ...current,
              error: getErrorMessage(message.error, "Move Analyzer initialization failed."),
              isRunning: false,
            }));
            return;
          }

          initializedRef.current = true;
          initializeRequestIdRef.current = null;
          void send({
            jsonrpc: "2.0",
            method: "initialized",
            params: {},
          }).then(syncOpenTabs);
          return;
        }

        if (message.id != null && !message.method) {
          settlePendingRequest(pendingRequestsRef.current, message);
          return;
        }

        if (message.method === "textDocument/publishDiagnostics") {
          applyPublishDiagnostics(sessionRootPathRef.current || rootPath, message.params, setDiagnosticsByPath);
        }
      }),
      listenMoveAnalyzerExit((event) => {
        if (disposed || event.sessionId !== sessionIdRef.current) {
          return;
        }

        sessionIdRef.current = null;
        initializedRef.current = false;
        openedTabsRef.current.clear();
        versionByUriRef.current.clear();
        rejectPendingRequests(pendingRequestsRef.current, "Move Analyzer exited.");
        crashedMoveTabsSignatureRef.current = latestMoveTabsSignatureRef.current;
        setStatus((current) => ({
          ...current,
          error: `Move Analyzer crashed while analyzing this edit. It will retry after the next change. Status: ${event.status ?? "unknown"}.`,
          isRunning: false,
          stderr: event.error
            ? `${current.stderr ?? ""}\n${event.error}`.slice(-4000)
            : current.stderr,
        }));
      }),
      listenMoveAnalyzerStderr((event) => {
        if (disposed || event.sessionId !== sessionIdRef.current) {
          return;
        }

        setStatus((current) => ({
          ...current,
          stderr: `${current.stderr ?? ""}${event.chunk}`.slice(-4000),
        }));
      }),
      listenMoveAnalyzerAdapterSettingsChanged(() => {
        if (disposed) {
          return;
        }

        void stopSession().then(() => setRestartToken((token) => token + 1));
      }),
    ]).then((unlisteners) => {
      if (disposed) {
        for (const unlisten of unlisteners) {
          unlisten();
        }
        return;
      }

      cleanupListeners = unlisteners;
      setListenersReady(true);
    }).catch((error: unknown) => {
      if (disposed) {
        return;
      }

      setStatus((current) => ({
        ...current,
        error: getErrorMessage(error, "Could not connect to Move Analyzer events."),
        isRunning: false,
      }));
    });

    return () => {
      disposed = true;
      for (const unlisten of cleanupListeners) {
        unlisten();
      }
    };
  }, [rootPath, send, stopSession, syncOpenTabs]);

  React.useEffect(() => {
    if (!listenersReady || !moveTabs.length || sessionIdRef.current) {
      return;
    }
    if (crashedMoveTabsSignatureRef.current === moveTabsSignature) {
      return;
    }

    void startSession();
  }, [listenersReady, moveTabs.length, moveTabsSignature, restartToken, startSession]);

  React.useEffect(() => {
    void syncOpenTabs();
  }, [moveTabs, syncOpenTabs]);

  React.useEffect(() => {
    return () => {
      void stopSession();
    };
  }, [rootPath, stopSession]);

  React.useEffect(() => {
    setDiagnosticsByPath({});
    sessionRootPathRef.current = rootPath;
  }, [rootPath]);

  return {
    diagnosticsByPath,
    lspFeatures,
    status,
  };
}

async function sendDidChange(
  tab: MoveTab,
  send: (message: JsonRpcMessage) => Promise<void>,
  versionByUri: Map<string, number>,
) {
  const version = (versionByUri.get(tab.uri) ?? 1) + 1;
  versionByUri.set(tab.uri, version);

  await send({
    jsonrpc: "2.0",
    method: "textDocument/didChange",
    params: {
      contentChanges: [{ text: tab.source }],
      textDocument: {
        uri: tab.uri,
        version,
      },
    },
  });
}

function settlePendingRequest(
  pendingRequests: Map<number | string, PendingRequest>,
  message: JsonRpcMessage,
) {
  if (message.id == null) {
    return;
  }

  const pending = pendingRequests.get(message.id);

  if (!pending) {
    return;
  }

  globalThis.clearTimeout(pending.timeout);
  pendingRequests.delete(message.id);

  if (message.error) {
    pending.reject(new Error(getErrorMessage(message.error, "Move Analyzer request failed.")));
    return;
  }

  pending.resolve(message.result);
}

function rejectPendingRequests(
  pendingRequests: Map<number | string, PendingRequest>,
  reason: string,
) {
  for (const pending of pendingRequests.values()) {
    globalThis.clearTimeout(pending.timeout);
    pending.reject(new Error(reason));
  }

  pendingRequests.clear();
}

function clearChangeTimer(uri: string, timers: Map<string, ReturnType<typeof globalThis.setTimeout>>) {
  const timer = timers.get(uri);

  if (timer) {
    globalThis.clearTimeout(timer);
    timers.delete(uri);
  }
}

function signatureForMoveTabs(tabs: MoveTextTab[]) {
  return tabs
    .map((tab) => `${tab.path}\u0000${tab.source}`)
    .join("\u0001");
}

function applyPublishDiagnostics(
  rootPath: string,
  params: unknown,
  setDiagnosticsByPath: React.Dispatch<React.SetStateAction<Record<string, MoveAnalyzerDiagnostic[]>>>,
) {
  const normalized = normalizeMoveAnalyzerPublishDiagnostics(rootPath, params);

  if (!normalized) {
    return;
  }

  setDiagnosticsByPath((current) => ({
    ...current,
    [normalized.path]: normalized.diagnostics,
  }));
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : fallback;
}
