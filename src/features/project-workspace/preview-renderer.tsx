import type { OpenFileTab } from "@/features/project-workspace/project-workspace";
import { CodeEditor } from "@/features/project-workspace/code-editor";

type PreviewRendererProps = {
  tab: OpenFileTab;
  onUpdateSource: (source: string) => void;
};

export function PreviewRenderer({
  tab,
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
      return (
        <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)]">
          {tab.error ? (
            <div className="border-b border-destructive/40 bg-destructive/10 px-4 py-2 text-xs text-destructive">
              {tab.error}
            </div>
          ) : null}
          <CodeEditor
            key={tab.path}
            language={tab.preview.language}
            value={tab.editedSource ?? tab.preview.source}
            onChange={onUpdateSource}
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
