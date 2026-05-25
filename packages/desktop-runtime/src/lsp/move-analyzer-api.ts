import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const MOVE_ANALYZER_ADAPTER_SETTINGS_CHANGED_EVENT = "move-analyzer-adapter-settings-changed";
const MOVE_ANALYZER_MESSAGE_EVENT = "move-analyzer-message";
const MOVE_ANALYZER_EXIT_EVENT = "move-analyzer-exit";
const MOVE_ANALYZER_STDERR_EVENT = "move-analyzer-stderr";

export type MoveAnalyzerAdapterSource = "bundledLibrary" | "system";

export type MoveAnalyzerAdapterSettings = {
  binaryPath?: string | null;
  source: MoveAnalyzerAdapterSource;
};

export type MoveAnalyzerAdapterStatus = {
  installed: boolean;
  version: string | null;
  installHint: string | null;
  activeSource: MoveAnalyzerAdapterSource | null;
  preferredSource: MoveAnalyzerAdapterSource;
  resolvedPath: string | null;
  bundled: MoveAnalyzerAdapterSourceStatus;
  system: MoveAnalyzerAdapterSourceStatus;
};

export type MoveAnalyzerAdapterSourceStatus = {
  source: MoveAnalyzerAdapterSource;
  available: boolean;
  version: string | null;
  path: string | null;
  error: string | null;
};

export type MoveAnalyzerServerSession = {
  sessionId: string;
  rootPath: string;
  command: string;
};

export type MoveAnalyzerMessageEvent = {
  sessionId: string;
  message: JsonRpcMessage;
};

export type MoveAnalyzerExitEvent = {
  sessionId: string;
  status: number | null;
  error: string | null;
};

export type MoveAnalyzerStderrEvent = {
  sessionId: string;
  chunk: string;
};

export type JsonRpcMessage = {
  id?: number | string | null;
  jsonrpc?: "2.0";
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: unknown;
};

export async function checkMoveAnalyzerAdapter() {
  return invoke<MoveAnalyzerAdapterStatus>("check_move_analyzer_adapter");
}

export async function getMoveAnalyzerAdapterSettings() {
  return invoke<MoveAnalyzerAdapterSettings>("get_move_analyzer_adapter_settings");
}

export async function saveMoveAnalyzerAdapterSettings(settings: MoveAnalyzerAdapterSettings) {
  return invoke<MoveAnalyzerAdapterSettings>("save_move_analyzer_adapter_settings", { settings });
}

export async function startMoveAnalyzerServer(rootPath: string) {
  return invoke<MoveAnalyzerServerSession>("start_move_analyzer_server", { rootPath });
}

export async function sendMoveAnalyzerMessage(sessionId: string, message: JsonRpcMessage) {
  return invoke<void>("send_move_analyzer_message", { sessionId, message });
}

export async function stopMoveAnalyzerServer(sessionId: string) {
  return invoke<void>("stop_move_analyzer_server", { sessionId });
}

export async function listenMoveAnalyzerAdapterSettingsChanged(
  onSettingsChanged: (settings: MoveAnalyzerAdapterSettings) => void,
) {
  return listen<MoveAnalyzerAdapterSettings>(
    MOVE_ANALYZER_ADAPTER_SETTINGS_CHANGED_EVENT,
    (event) => onSettingsChanged(event.payload),
  );
}

export async function listenMoveAnalyzerMessages(
  onMessage: (event: MoveAnalyzerMessageEvent) => void,
) {
  return listen<MoveAnalyzerMessageEvent>(MOVE_ANALYZER_MESSAGE_EVENT, (event) => onMessage(event.payload));
}

export async function listenMoveAnalyzerExit(
  onExit: (event: MoveAnalyzerExitEvent) => void,
) {
  return listen<MoveAnalyzerExitEvent>(MOVE_ANALYZER_EXIT_EVENT, (event) => onExit(event.payload));
}

export async function listenMoveAnalyzerStderr(
  onStderr: (event: MoveAnalyzerStderrEvent) => void,
) {
  return listen<MoveAnalyzerStderrEvent>(MOVE_ANALYZER_STDERR_EVENT, (event) => onStderr(event.payload));
}

