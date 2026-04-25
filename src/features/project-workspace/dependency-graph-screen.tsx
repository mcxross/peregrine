import { GitBranch } from "lucide-react";
import React from "react";

import type { PackageDependencyGraph } from "@/features/empty-project/filesystem-tree";

const DependencyGraphView = React.lazy(() =>
  import("@/features/project-workspace/dependency-graph-view"),
);

type DependencyGraphScreenProps = {
  graph: PackageDependencyGraph;
  packageName: string;
};

export function DependencyGraphScreen({
  graph,
  packageName,
}: DependencyGraphScreenProps) {
  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] bg-background">
      <header className="flex min-h-14 items-center justify-between border-b px-5">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <GitBranch className="size-4 text-muted-foreground" aria-hidden="true" />
            <h2 className="truncate text-sm font-semibold">Dependency Graph</h2>
          </div>
          <p className="mt-1 truncate text-xs text-muted-foreground">
            {graph.summaryPath ?? `${packageName} has no package_summaries directory`}
          </p>
        </div>
        {graph.edges.length ? (
          <div className="shrink-0 text-xs text-muted-foreground">
            {graph.nodes.length} packages / {graph.edges.length} links
          </div>
        ) : null}
      </header>

      <div className="min-h-0 p-4">
        <React.Suspense
          fallback={
            <div className="flex h-full min-h-0 items-center justify-center rounded-lg border bg-background/40 text-sm text-muted-foreground">
              Loading graph...
            </div>
          }
        >
          <DependencyGraphView className="h-full rounded-lg border" graph={graph} />
        </React.Suspense>
      </div>
    </section>
  );
}
