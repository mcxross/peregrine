import React from "react";

import type { PackageDependencyGraph } from "@/features/empty-project/filesystem-tree";
import { PackageLoadAssessmentCards } from "@/features/project-workspace/package-load-assessment-cards";
import type { PackageLoadAssessment } from "@/features/project-workspace/package-load-assessment";

const DependencyGraphView = React.lazy(() =>
  import("@/features/project-workspace/dependency-graph-view"),
);

type DependencyGraphScreenProps = {
  graph: PackageDependencyGraph;
  loadAssessment?: PackageLoadAssessment | null;
  packageName: string;
};

export function DependencyGraphScreen({
  graph,
  loadAssessment,
  packageName,
}: DependencyGraphScreenProps) {
  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      {loadAssessment ? (
        <div className="row-start-1 px-5 pb-2 pt-3">
          <PackageLoadAssessmentCards assessment={loadAssessment} />
        </div>
      ) : null}

      <div className="row-start-2 grid min-h-0 px-5 pb-3">
        <React.Suspense
          fallback={
            <div className="flex h-full min-h-0 items-center justify-center rounded-md border bg-card text-sm text-muted-foreground">
              Loading graph...
            </div>
          }
        >
          <DependencyGraphView className="h-full rounded-md border" graph={graph} packageName={packageName} />
        </React.Suspense>
      </div>
    </section>
  );
}
