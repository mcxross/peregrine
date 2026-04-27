import React from "react";

import { Card } from "@/components/ui/card";
import type { PackageDependencyGraph } from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

const DependencyGraphView = React.lazy(() =>
  import("@/features/project-workspace/dependency-graph-view"),
);

type DependencyGraphScreenProps = {
  graph: PackageDependencyGraph;
  packageName: string;
};

const scoreCards = [
  { label: "Build", primary: "Passing", secondary: "", tone: "success", kind: "status" },
  { label: "Tests", primary: "87/87", secondary: "Passing", tone: "success", kind: "metric" },
  { label: "Coverage", primary: "74%", secondary: "Lines", tone: "default", kind: "metric" },
  { label: "Fuzzing", primary: "2", secondary: "Crashes", tone: "danger", kind: "metric" },
  { label: "Formal", primary: "5/8", secondary: "Proven", tone: "success", kind: "metric" },
  { label: "Risk", primary: "72", secondary: "Medium", tone: "warning", kind: "risk" },
];

export function DependencyGraphScreen({
  graph,
  packageName,
}: DependencyGraphScreenProps) {
  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <div className="grid min-w-0 grid-cols-[repeat(6,minmax(0,1fr))] gap-2 px-5 pb-2 pt-3">
        {scoreCards.map((card) => (
          <ScoreCard card={card} key={card.label} />
        ))}
      </div>

      <div className="grid min-h-0 px-5 pb-3">
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

type ScoreCardData = (typeof scoreCards)[number];

function ScoreCard({ card }: { card: ScoreCardData }) {
  return (
    <Card className="grid h-[86px] min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 overflow-hidden rounded-md px-3 py-3 shadow-none min-[1500px]:px-4">
      <div className="grid min-h-0 min-w-0 grid-rows-[auto_1fr_auto] gap-0.5">
        <div className="truncate text-[11px] font-medium leading-4 text-muted-foreground">
          {card.label}
        </div>

        <div className="flex min-w-0 items-center">
          <div
            className={cn(
              "min-w-0 whitespace-nowrap font-semibold leading-none",
              card.kind === "metric" && "text-xl min-[1500px]:text-[22px]",
              card.kind === "status" && "text-[16px] text-emerald-400 min-[1500px]:text-[18px]",
              card.kind === "risk" && "text-[22px] min-[1500px]:text-[24px]",
              card.tone === "danger" && "text-red-400",
            )}
          >
            {card.primary}
          </div>
        </div>

        {card.secondary ? (
          <div
            className={cn(
              "min-h-4 truncate text-[11px] font-semibold leading-4",
              card.tone === "success" && "text-emerald-400",
              card.tone === "danger" && "text-red-400",
              card.tone === "warning" && "text-amber-400",
              card.tone === "default" && "text-muted-foreground",
            )}
          >
            {card.tone === "warning" ? (
              <span className="mr-1 inline-block size-2 rounded-full bg-amber-400" />
            ) : null}
            {card.secondary}
          </div>
        ) : (
          <span aria-hidden="true" />
        )}
      </div>

      {card.kind === "risk" ? <RiskRing /> : null}
    </Card>
  );
}

function RiskRing() {
  return (
    <span
      aria-hidden="true"
      className="size-8 shrink-0 rounded-full min-[1500px]:size-10"
      style={{
        background:
          "conic-gradient(#f59e0b 0 72%, color-mix(in oklch, var(--muted) 75%, transparent) 72% 100%)",
        mask: "radial-gradient(circle, transparent 54%, black 56%)",
      }}
    />
  );
}
