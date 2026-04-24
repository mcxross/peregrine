import { FileText } from "lucide-react";

import {
  isDirectoryPath,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";

type FileViewerProps = {
  packageTree: PackageTree;
  selectedPath: string | null;
  content: string | null;
  error: string | null;
  isLoading: boolean;
};

export function FileViewer({
  packageTree,
  selectedPath,
  content,
  error,
  isLoading,
}: FileViewerProps) {
  const isDirectory = selectedPath ? isDirectoryPath(selectedPath) : false;

  return (
    <section className="grid min-h-0 grid-rows-[auto_1fr] bg-background">
      <header className="flex min-w-0 items-center gap-3 border-b px-5 py-3">
        <FileText className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0">
          <h2 className="truncate text-sm font-medium">
            {selectedPath && !isDirectory ? selectedPath : packageTree.rootName}
          </h2>
          <p className="mt-0.5 truncate text-xs text-muted-foreground">
            {selectedPath && !isDirectory ? "Read-only preview" : "Select a file"}
          </p>
        </div>
      </header>

      <div className="min-h-0 overflow-auto">
        {!selectedPath || isDirectory ? (
          <EmptyState message="Select a file from the tree." />
        ) : isLoading ? (
          <EmptyState message="Loading file..." />
        ) : error ? (
          <div className="m-5 rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {error}
          </div>
        ) : (
          <pre className="min-h-full overflow-auto p-5 font-mono text-[13px] leading-6 text-foreground">
            <code>{content}</code>
          </pre>
        )}
      </div>
    </section>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex min-h-full items-center justify-center px-6 text-sm text-muted-foreground">
      {message}
    </div>
  );
}
