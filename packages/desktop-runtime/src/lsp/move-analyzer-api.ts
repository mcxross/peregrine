import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  MoveAnalyzerCompletionContext,
  MoveAnalyzerCompletionList,
  MoveAnalyzerDiagnostic,
  MoveAnalyzerHover,
  MoveAnalyzerPosition,
  MoveAnalyzerResolvedLocation,
  MoveAnalyzerWorkspaceEdit,
} from "./types";

const CONFIG_CHANGED_EVENT = "sui-move-analyzer-config-changed";

export type MoveAnalyzerAdapterSource = "bundled" | "system";

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

export type MoveAnalyzerDocument = {
  path: string;
  source?: string | null;
};

type DiagnosticsResponse = {
  diagnostics: MoveAnalyzerDiagnostic[];
  fresh: boolean;
  path: string;
  warnings: string[];
};

type CompletionResponse = {
  completion: MoveAnalyzerCompletionList | null;
  path: string;
};

type HoverResponse = {
  hover: MoveAnalyzerHover | null;
  path: string;
};

type LocationsResponse = {
  locations: MoveAnalyzerResolvedLocation[];
  path: string;
};

type RenameResponse = {
  edit: MoveAnalyzerWorkspaceEdit | null;
  path: string;
};

export function checkSuiMoveAnalyzerAdapter() {
  return invoke<MoveAnalyzerAdapterStatus>("check_sui_move_analyzer_adapter");
}

export function getSuiMoveAnalyzerSettings() {
  return invoke<MoveAnalyzerAdapterSettings>("get_sui_move_analyzer_settings");
}

export function saveSuiMoveAnalyzerSettings(settings: MoveAnalyzerAdapterSettings) {
  return invoke<MoveAnalyzerAdapterSettings>("save_sui_move_analyzer_settings", { settings });
}

export function getSuiMoveAnalyzerStatus(rootPath: string) {
  return invoke<MoveAnalyzerAdapterStatus>("sui_move_analyzer_status", { rootPath });
}

export function getSuiMoveAnalyzerDiagnostics(
  rootPath: string,
  document: MoveAnalyzerDocument,
) {
  return invoke<DiagnosticsResponse>("sui_move_analyzer_diagnostics", {
    document,
    rootPath,
  });
}

export async function getSuiMoveAnalyzerCompletion(
  rootPath: string,
  document: MoveAnalyzerDocument,
  position: MoveAnalyzerPosition,
  context?: MoveAnalyzerCompletionContext,
) {
  const response = await invoke<CompletionResponse>("sui_move_analyzer_completion", {
    request: {
      ...document,
      context: context ?? null,
      position,
    },
    rootPath,
  });
  return response.completion;
}

export async function getSuiMoveAnalyzerHover(
  rootPath: string,
  document: MoveAnalyzerDocument,
  position: MoveAnalyzerPosition,
) {
  const response = await invoke<HoverResponse>("sui_move_analyzer_hover", {
    request: { ...document, position },
    rootPath,
  });
  return response.hover;
}

export async function getSuiMoveAnalyzerDefinition(
  rootPath: string,
  document: MoveAnalyzerDocument,
  position: MoveAnalyzerPosition,
) {
  const response = await invoke<LocationsResponse>("sui_move_analyzer_definition", {
    request: { ...document, position },
    rootPath,
  });
  return response.locations;
}

export async function getSuiMoveAnalyzerReferences(
  rootPath: string,
  document: MoveAnalyzerDocument,
  position: MoveAnalyzerPosition,
) {
  const response = await invoke<LocationsResponse>("sui_move_analyzer_references", {
    request: { ...document, position },
    rootPath,
  });
  return response.locations;
}

export async function getSuiMoveAnalyzerRename(
  rootPath: string,
  document: MoveAnalyzerDocument,
  position: MoveAnalyzerPosition,
  newName: string,
) {
  const response = await invoke<RenameResponse>("sui_move_analyzer_rename", {
    request: { ...document, newName, position },
    rootPath,
  });
  return response.edit;
}

export function listenSuiMoveAnalyzerConfigChanged(onChanged: () => void) {
  return listen(CONFIG_CHANGED_EVENT, onChanged);
}

