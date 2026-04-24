import { Folder } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { RecentProject } from "@/features/empty-project/types";

type RecentProjectsProps = {
  projects: RecentProject[];
  onClear?: () => void;
  onOpenProject?: (project: RecentProject) => void;
};

export function RecentProjects({
  projects,
  onClear,
  onOpenProject,
}: RecentProjectsProps) {
  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold tracking-tight">Recent Projects</h2>
        {projects.length > 0 ? (
          <Button variant="ghost" size="sm" onClick={onClear}>
            Clear
          </Button>
        ) : null}
      </div>

      {projects.length > 0 ? (
        <div className="space-y-2">
          {projects.map((project) => (
            <RecentProjectRow
              key={project.id}
              project={project}
              onOpen={() => onOpenProject?.(project)}
            />
          ))}
        </div>
      ) : (
        <div className="rounded-lg border bg-card px-5 py-6 text-sm text-muted-foreground">
          No recent projects yet.
        </div>
      )}
    </section>
  );
}

function RecentProjectRow({
  project,
  onOpen,
}: {
  project: RecentProject;
  onOpen: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onOpen}
      className="grid w-full grid-cols-[1fr_auto] items-center gap-4 rounded-lg border bg-card px-4 py-3 text-left text-card-foreground transition hover:border-ring hover:bg-accent/40 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
    >
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-md border bg-muted text-muted-foreground">
          <Folder className="size-5" aria-hidden="true" />
        </div>
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold">{project.name}</div>
          <div className="truncate text-sm text-muted-foreground">{project.path}</div>
        </div>
      </div>

      <ProjectStatus project={project} />
    </button>
  );
}

function ProjectStatus({ project }: { project: RecentProject }) {
  if (project.status.kind === "new") {
    return (
      <div className="rounded-md border px-3 py-2 text-sm font-medium text-primary">
        {project.status.label}
      </div>
    );
  }

  return (
    <div className="flex items-center gap-4">
      <span
        className={cn(
          "text-sm",
          project.status.score >= 80 ? "text-green-500" : "text-yellow-500",
        )}
      >
        {project.status.summary}
      </span>
      <div className="min-w-14 rounded-md border px-3 py-2 text-center text-sm font-semibold text-primary">
        {project.status.score}
      </div>
    </div>
  );
}
