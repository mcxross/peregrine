import { FileText, X } from "lucide-react";
import React from "react";

import type { OpenFileTab } from "@/features/project-workspace/project-workspace";
import { cn } from "@/lib/utils";

const PreviewRenderer = React.lazy(() =>
  import("@/features/project-workspace/preview-renderer"),
);

type EditorTabsProps = {
  packageName: string;
  tabs: OpenFileTab[];
  activePath: string | null;
  onActivateTab: (path: string) => void;
  onCloseTab: (path: string) => void;
  onSaveTab: (path: string) => void;
  onUpdateTabSource: (path: string, source: string) => void;
};

export function EditorTabs({
  packageName,
  tabs,
  activePath,
  onActivateTab,
  onCloseTab,
  onSaveTab,
  onUpdateTabSource,
}: EditorTabsProps) {
  const activeTab = tabs.find((tab) => tab.path === activePath) ?? null;

  return (
    <section className="grid min-h-0 grid-rows-[auto_1fr] bg-background">
      <div className="flex min-w-0 items-center border-b bg-muted/20">
        {tabs.length ? (
          <div className="flex min-w-0 flex-1 overflow-x-auto">
            {tabs.map((tab) => (
              <button
                key={tab.path}
                className={cn(
                  "group flex h-10 max-w-56 shrink-0 items-center gap-2 border-r px-3 text-left text-sm text-muted-foreground",
                  tab.path === activePath && "bg-background text-foreground",
                )}
                onClick={() => onActivateTab(tab.path)}
                type="button"
              >
                <FileText className="size-4 shrink-0" aria-hidden="true" />
                <span className="truncate">
                  {tab.isDirty ? `${basename(tab.path)} *` : basename(tab.path)}
                </span>
                <span
                  className="rounded p-0.5 text-muted-foreground opacity-70 hover:bg-accent hover:text-accent-foreground"
                  onClick={(event) => {
                    event.stopPropagation();
                    onCloseTab(tab.path);
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <X className="size-3.5" aria-hidden="true" />
                </span>
              </button>
            ))}
          </div>
        ) : (
          <div className="flex h-10 min-w-0 items-center px-5 text-sm text-muted-foreground">
            {packageName}
          </div>
        )}
      </div>

      {activeTab ? (
        <React.Suspense fallback={<EditorEmptyState message="Loading renderer..." />}>
          <PreviewRenderer
            tab={activeTab}
            onSave={() => onSaveTab(activeTab.path)}
            onUpdateSource={(source) => onUpdateTabSource(activeTab.path, source)}
          />
        </React.Suspense>
      ) : (
        <EditorEmptyState message="Select a file from the tree." />
      )}
    </section>
  );
}

function EditorEmptyState({ message }: { message: string }) {
  return (
    <div className="flex min-h-0 items-center justify-center px-6 text-sm text-muted-foreground">
      {message}
    </div>
  );
}

function basename(path: string) {
  return path.split("/").filter(Boolean).at(-1) ?? path;
}
