import { FileText, X } from "lucide-react";
import React from "react";

import type { OpenFileTab } from "@/features/project-workspace/editor/types";
import { cn } from "@/lib/utils";

const PreviewRenderer = React.lazy(() =>
  import("@/features/project-workspace/editor/preview-renderer"),
);

type EditorTabsProps = {
  tabs: OpenFileTab[];
  activePath: string | null;
  onActivateTab: (path: string) => void;
  onCloseTab: (path: string) => void;
  onUpdateTabSource: (path: string, source: string) => void;
};

export function EditorTabs({
  tabs,
  activePath,
  onActivateTab,
  onCloseTab,
  onUpdateTabSource,
}: EditorTabsProps) {
  const activeTab = tabs.find((tab) => tab.path === activePath) ?? null;

  return (
    <section className={cn("grid min-h-0 bg-background", tabs.length ? "grid-rows-[auto_1fr]" : "grid-rows-[1fr]")}>
      {tabs.length ? (
        <div className="flex min-w-0 items-center border-b bg-muted/20">
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
        </div>
      ) : null}

      {activeTab ? (
        <React.Suspense
          fallback={
            <div className="flex min-h-0 items-center justify-center px-6 text-sm text-muted-foreground">
              Loading renderer...
            </div>
          }
        >
          <PreviewRenderer
            tab={activeTab}
            onUpdateSource={(source) => onUpdateTabSource(activeTab.path, source)}
          />
        </React.Suspense>
      ) : (
        <div className="grid min-h-0 place-items-center bg-[var(--app-window)] px-6 text-center">
          <div className="max-w-sm">
            <div className="mx-auto grid size-10 place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-subtle)] text-muted-foreground">
              <FileText className="size-5" aria-hidden="true" />
            </div>
            <div className="mt-4 text-sm font-semibold text-foreground">Select a file to edit</div>
            <p className="mt-1 text-xs leading-5 text-muted-foreground">
              Choose a file from the project tree to open it in the editor.
            </p>
          </div>
        </div>
      )}
    </section>
  );
}

function basename(path: string) {
  return path.split("/").filter(Boolean).at(-1) ?? path;
}
