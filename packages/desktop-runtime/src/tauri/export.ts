import { invoke } from "@tauri-apps/api/core";

export function saveTextExport(path: string, contents: string) {
  return invoke("save_text_export", {
    contents,
    path,
  });
}

export function saveGraphPng(path: string, pngDataUrl: string) {
  return invoke("save_graph_png", {
    path,
    pngDataUrl,
  });
}
