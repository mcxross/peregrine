import React from "react";

import { displayMovePackageName } from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

export function CanvasNotice({
  icon: Icon,
  message,
  title,
}: {
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  message: string;
  title: string;
}) {
  return (
    <div className="pointer-events-none absolute left-1/2 top-1/2 z-10 grid w-[min(24rem,calc(100%-3rem))] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-md border border-dashed border-[color:var(--app-border)] bg-background/70 px-4 py-5 text-center shadow-sm backdrop-blur-[2px]">
      <Icon className="size-5 text-muted-foreground" aria-hidden="true" />
      <div className="mt-2 text-sm font-semibold text-foreground">{title}</div>
      <p className="mt-1 text-xs leading-5 text-muted-foreground">{message}</p>
    </div>
  );
}

export function EmptyTypeGraphState({
  className,
  packageName,
}: {
  className: string;
  packageName: string;
}) {
  return (
    <div className={cn(className, "grid place-items-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-6 text-center")}>
      <div className="max-w-md">
        <div className="text-sm font-semibold text-foreground">No type graph found</div>
        <p className="mt-2 text-sm text-muted-foreground">
          Peregrine did not find graphable types for {displayMovePackageName(packageName)}.
        </p>
      </div>
    </div>
  );
}
