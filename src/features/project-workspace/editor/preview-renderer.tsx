import { X } from "lucide-react";
import React from "react";

import {
  CodeEditor,
  type CodeEditorJumpRequest,
} from "@/features/project-workspace/editor/code-editor";
import type { CodeEditorMoveAnalyzerFeatures } from "@/features/project-workspace/editor/lsp/code-editor-move-analyzer";
import type {
  MoveAnalyzerClientStatus,
  MoveAnalyzerLspFeatures,
} from "@/features/project-workspace/editor/lsp/use-move-analyzer";
import type {
  MoveAnalyzerDiagnostic,
  MoveAnalyzerResolvedLocation,
  MoveAnalyzerWorkspaceEdit,
} from "@/features/project-workspace/editor/lsp/types";
import type { OpenFileTab } from "@/features/project-workspace/editor/types";

type PreviewRendererProps = {
  diagnostics?: MoveAnalyzerDiagnostic[];
  jumpRequest?: CodeEditorJumpRequest | null;
  moveAnalyzerLspFeatures?: MoveAnalyzerLspFeatures | null;
  moveAnalyzerStatus?: MoveAnalyzerClientStatus;
  tab: OpenFileTab;
  onMoveAnalyzerLocation?: (location: MoveAnalyzerResolvedLocation) => void;
  onMoveAnalyzerWorkspaceEdit?: (edit: MoveAnalyzerWorkspaceEdit) => Promise<void> | void;
  onUpdateSource: (source: string) => void;
};

export function PreviewRenderer({
  diagnostics = [],
  jumpRequest = null,
  moveAnalyzerLspFeatures = null,
  moveAnalyzerStatus,
  tab,
  onMoveAnalyzerLocation,
  onMoveAnalyzerWorkspaceEdit,
  onUpdateSource,
}: PreviewRendererProps) {
  if (tab.status === "loading" || tab.status === "idle") {
    return <EmptyState message="Loading file..." />;
  }

  if (tab.status === "error") {
    return (
      <div className="m-5 rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">
        {tab.error}
      </div>
    );
  }

  if (!tab.preview) {
    return <EmptyState message="No preview available." />;
  }

  switch (tab.preview.kind) {
    case "text":
      const isMoveFile = tab.preview.language.toLowerCase() === "move";
      const moveAnalyzer: CodeEditorMoveAnalyzerFeatures | null = isMoveFile && moveAnalyzerLspFeatures
        ? {
            completion: (position, context) => moveAnalyzerLspFeatures.completion(tab.path, position, context),
            definition: (position) => moveAnalyzerLspFeatures.definition(tab.path, position),
            hover: (position) => moveAnalyzerLspFeatures.hover(tab.path, position),
            references: (position) => moveAnalyzerLspFeatures.references(tab.path, position),
            rename: (position, newName) => moveAnalyzerLspFeatures.rename(tab.path, position, newName),
          }
        : null;

      return (
        <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)]">
          {tab.error ? (
            <div className="border-b border-destructive/40 bg-destructive/10 px-4 py-2 text-xs text-destructive">
              {tab.error}
            </div>
          ) : null}
          {isMoveFile ? (
            <MoveAnalyzerUnavailableBanner status={moveAnalyzerStatus} />
          ) : null}
          <CodeEditor
            diagnostics={isMoveFile ? diagnostics : []}
            jumpRequest={jumpRequest}
            key={tab.path}
            language={tab.preview.language}
            moveAnalyzer={moveAnalyzer}
            value={tab.editedSource ?? tab.preview.source}
            onChange={onUpdateSource}
            onMoveAnalyzerLocation={onMoveAnalyzerLocation}
            onMoveAnalyzerWorkspaceEdit={onMoveAnalyzerWorkspaceEdit}
          />
        </div>
      );
    case "markdown":
      return (
        <article
          className="prose-invert min-h-0 max-w-none overflow-auto px-8 py-6 text-sm leading-7 text-foreground [&_a]:text-primary [&_blockquote]:border-l [&_blockquote]:pl-4 [&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_h1]:text-2xl [&_h1]:font-semibold [&_h2]:text-xl [&_h2]:font-semibold [&_h3]:text-lg [&_h3]:font-semibold [&_pre]:overflow-auto [&_pre]:rounded-lg [&_pre]:border [&_pre]:bg-muted/40 [&_pre]:p-4"
          dangerouslySetInnerHTML={{ __html: tab.preview.html }}
        />
      );
    case "image":
      if (!tab.preview.dataUrl) {
        return <EmptyState message="Image data was not available." />;
      }

      return (
        <div className="flex min-h-0 items-center justify-center overflow-auto bg-muted/10 p-6">
          <img
            alt={tab.preview.path}
            className="max-h-full max-w-full rounded border object-contain"
            src={tab.preview.dataUrl}
          />
        </div>
      );
    case "video":
      if (!tab.preview.dataUrl) {
        return <EmptyState message="Video data was not available." />;
      }

      return (
        <div className="flex min-h-0 items-center justify-center overflow-auto bg-muted/10 p-6">
          <video
            className="max-h-full max-w-full rounded border bg-background"
            controls
            src={tab.preview.dataUrl}
          />
        </div>
      );
    case "unsupported":
      return (
        <EmptyState
          message={`${tab.preview.reason} (${formatBytes(tab.preview.size)})`}
        />
      );
  }
}

export default PreviewRenderer;

function MoveAnalyzerUnavailableBanner({
  status,
}: {
  status?: MoveAnalyzerClientStatus;
}) {
  const [dismissedError, setDismissedError] = React.useState<string | null>(null);
  const error = status?.error;

  if (!error || dismissedError === error) {
    return null;
  }

  return (
    <div className="flex min-w-0 items-center gap-2 border-b border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs text-amber-300">
      <span className="min-w-0 flex-1 truncate">
        Move Analyzer unavailable: {compactStatusMessage(error)}
      </span>
      <button
        aria-label="Dismiss Move Analyzer warning"
        className="grid size-5 shrink-0 place-items-center rounded text-amber-200/80 hover:bg-amber-500/20 hover:text-amber-100"
        onClick={() => setDismissedError(error)}
        type="button"
      >
        <X className="size-3.5" aria-hidden="true" />
      </button>
    </div>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex min-h-0 items-center justify-center px-6 text-sm text-muted-foreground">
      {message}
    </div>
  );
}

function formatBytes(size: number) {
  if (size < 1024) {
    return `${size} B`;
  }

  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }

  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function compactStatusMessage(message: string) {
  const compact = message.replace(/\s+/g, " ").trim();

  return compact.length > 180 ? `${compact.slice(0, 177)}...` : compact;
}
