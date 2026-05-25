import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const TERMINAL_OUTPUT_EVENT = "terminal-output";
export const TERMINAL_EXIT_EVENT = "terminal-exit";

export type TerminalStartRequest = {
  command?: string;
  cwd: string;
  cols: number;
  rows: number;
};

export type TerminalStartResponse = {
  sessionId: string;
};

export type TerminalOutputEvent = {
  sessionId: string;
  data: string;
};

export type TerminalExitEvent = {
  sessionId: string;
  code: number | null;
};

export function startTerminal(request: TerminalStartRequest) {
  return invoke<TerminalStartResponse>("terminal_start", { request });
}

export function writeTerminal(sessionId: string, data: string) {
  return invoke<void>("terminal_write", {
    request: { sessionId, data },
  });
}

export function resizeTerminal(sessionId: string, cols: number, rows: number) {
  return invoke<void>("terminal_resize", {
    request: { sessionId, cols, rows },
  });
}

export function stopTerminal(sessionId: string) {
  return invoke<void>("terminal_stop", {
    request: { sessionId },
  });
}

export function listenTerminalOutput(
  handler: (event: TerminalOutputEvent) => void,
) {
  return listen<TerminalOutputEvent>(TERMINAL_OUTPUT_EVENT, (event) => {
    handler(event.payload);
  });
}

export function listenTerminalExit(handler: (event: TerminalExitEvent) => void) {
  return listen<TerminalExitEvent>(TERMINAL_EXIT_EVENT, (event) => {
    handler(event.payload);
  });
}
