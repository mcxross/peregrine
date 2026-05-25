import { Clock3, FolderOpen, Package, RotateCw, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import type { RecentProject } from "@peregrine/desktop-runtime";

type RecentProjectsProps = {
  projects: RecentProject[];
  onClear?: () => void;
  onOpenProject?: (project: RecentProject) => void;
  onRemoveProject?: (project: RecentProject) => void;
};

export function RecentProjects({
  projects,
  onClear,
  onOpenProject,
  onRemoveProject,
}: RecentProjectsProps) {
  return (
    <section className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold tracking-tight">Recent Projects</h2>
        {projects.length > 0 ? (
          <Button className="h-8 gap-2 text-xs" variant="ghost" size="sm" onClick={onClear}>
            <RotateCw className="size-3.5" aria-hidden="true" />
            Clear
          </Button>
        ) : null}
      </div>

      {projects.length > 0 ? (
        <div className="grid gap-2">
          {projects.map((project) => (
            <RecentProjectRow
              key={project.id}
              project={project}
              onOpen={() => onOpenProject?.(project)}
              onRemove={() => onRemoveProject?.(project)}
            />
          ))}
        </div>
      ) : (
        <Card className="rounded-md px-4 py-4 text-sm text-muted-foreground shadow-none">
          No recent packages yet. Open a Move package and it will appear here.
        </Card>
      )}
    </section>
  );
}

function RecentProjectRow({
  project,
  onOpen,
  onRemove,
}: {
  project: RecentProject;
  onOpen: () => void;
  onRemove?: () => void;
}) {
  return (
    <div className="group grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-1 rounded-md border bg-card text-card-foreground shadow-xs">
      <Button
        type="button"
        onClick={onOpen}
        className="grid h-auto w-full min-w-0 grid-cols-[1fr_auto] items-center gap-3 rounded-md px-3 py-3 text-left text-card-foreground"
        variant="ghost"
        title={`${project.name} - ${project.packagePath}`}
      >
        <div className="flex min-w-0 items-center gap-3">
          <div className="flex size-9 shrink-0 items-center justify-center rounded-md border bg-muted text-muted-foreground">
            <Package className="size-4.5" aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <div className="truncate text-sm font-semibold">{project.name}</div>
            <div className="truncate text-xs text-muted-foreground">{compactPath(project.packagePath)}</div>
          </div>
        </div>

        <div className="grid justify-items-end gap-1 text-right">
          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
            <Clock3 className="size-3" aria-hidden="true" />
            <span>{formatRecentTime(project.lastOpenedAt)}</span>
          </div>
          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
            <FolderOpen className="size-3" aria-hidden="true" />
            <span>
              {moduleCountLabel(project.moduleCount)}
              {project.packageCount > 1 ? ` / ${project.packageCount} packages` : ""}
            </span>
          </div>
        </div>
      </Button>

      {onRemove ? (
        <Button
          type="button"
          aria-label={`Remove ${project.name} from recent projects`}
          className="mr-2 size-8 shrink-0 opacity-70 transition-opacity hover:opacity-100 focus-visible:opacity-100"
          onClick={onRemove}
          size="icon"
          title="Remove from recent projects"
          variant="ghost"
        >
          <X className="size-4" aria-hidden="true" />
        </Button>
      ) : null}
    </div>
  );
}

function compactPath(path: string) {
  return path.replace(/^\/Users\/[^/]+/, "~");
}

function moduleCountLabel(count: number) {
  if (count === 1) {
    return "1 module";
  }

  return `${count} modules`;
}

function formatRecentTime(value: number) {
  const elapsed = Date.now() - value;
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;

  if (elapsed < minute) {
    return "just now";
  }

  if (elapsed < hour) {
    return `${Math.max(1, Math.floor(elapsed / minute))}m ago`;
  }

  if (elapsed < day) {
    return `${Math.floor(elapsed / hour)}h ago`;
  }

  return `${Math.floor(elapsed / day)}d ago`;
}
