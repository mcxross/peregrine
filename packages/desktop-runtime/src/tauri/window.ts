import { getCurrentWindow } from "@tauri-apps/api/window";

export async function startWindowDrag() {
  await getCurrentWindow().startDragging();
}
