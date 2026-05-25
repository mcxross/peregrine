import { listen } from "@tauri-apps/api/event";

export const OPEN_SETTINGS_EVENT = "open-settings";
export const CLOSE_PROJECT_EVENT = "close-project";

export function listenOpenSettings(handler: () => void) {
  return listen(OPEN_SETTINGS_EVENT, handler);
}

export function listenCloseProject(handler: () => void) {
  return listen(CLOSE_PROJECT_EVENT, handler);
}
