import type { FilePreview } from "@peregrine/desktop-runtime";

export type OpenFileTab = {
  path: string;
  preview: FilePreview | null;
  editedSource: string | null;
  error: string | null;
  isDirty: boolean;
  isSaving: boolean;
  status: "idle" | "loading" | "loaded" | "error";
};
