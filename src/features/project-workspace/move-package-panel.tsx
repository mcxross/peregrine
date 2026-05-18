import {
  Box,
  FileCode2,
  Loader2,
  Package,
  PanelLeftClose,
  Play,
} from "lucide-react";

import type {
  MoveModule,
  MovePackage,
} from "@/features/empty-project/filesystem-tree";
import { displayMovePackageName } from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

export type PackageBuildStatus = {
  message: string | null;
  state: "idle" | "running" | "success" | "error";
};

type MovePackagePanelProps = {
  activePath: string | null;
  buildStatuses: Record<string, PackageBuildStatus>;
  packages: MovePackage[];
  rootPackage: string | null;
  onBuildPackage: (movePackage: MovePackage) => void;
  onCollapse: () => void;
  onOpenFile: (path: string) => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  selectedModulePath: string | null;
};

export function MovePackagePanel({
  activePath,
  buildStatuses,
  packages,
  rootPackage,
  onBuildPackage,
  onCollapse,
  onOpenFile,
  onSelectModule,
  selectedModulePath,
}: MovePackagePanelProps) {
  return (
    <aside className="grid min-h-0 grid-rows-[auto_1fr] border-r border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground">
      <header className="border-b px-4 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h2 className="text-sm font-semibold">Move Packages</h2>
            <p className="mt-1 text-xs text-muted-foreground">
              {packageCountLabel(packages)}
            </p>
          </div>
          <button
            className="inline-flex size-7 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground"
            onClick={onCollapse}
            title="Collapse package panel"
            type="button"
          >
            <PanelLeftClose className="size-4" aria-hidden="true" />
          </button>
        </div>
      </header>

      {packages.length ? (
        <div className="min-h-0 overflow-auto px-3 py-3">
          <div className="space-y-4">
            {packages.map((movePackage) => (
              <PackageSection
                key={movePackage.manifestPath}
                activePath={activePath}
                buildStatus={buildStatuses[movePackage.path]}
                isRoot={movePackage.name === rootPackage}
                movePackage={movePackage}
                onBuildPackage={onBuildPackage}
                onOpenFile={onOpenFile}
                onSelectModule={onSelectModule}
                selectedModulePath={selectedModulePath}
              />
            ))}
          </div>
        </div>
      ) : (
        <div className="flex min-h-0 items-center justify-center px-5 text-center text-sm text-muted-foreground">
          No Move.toml files found in this workspace.
        </div>
      )}
    </aside>
  );
}

type PackageSectionProps = {
  activePath: string | null;
  buildStatus?: PackageBuildStatus;
  isRoot: boolean;
  movePackage: MovePackage;
  onBuildPackage: (movePackage: MovePackage) => void;
  onOpenFile: (path: string) => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  selectedModulePath: string | null;
};

function PackageSection({
  activePath,
  buildStatus,
  isRoot,
  movePackage,
  onBuildPackage,
  onOpenFile,
  onSelectModule,
  selectedModulePath,
}: PackageSectionProps) {
  return (
    <section>
      <div className="flex items-start gap-1">
        <button
          className="flex min-w-0 flex-1 items-start gap-2 rounded-md px-2 py-2 text-left hover:bg-[var(--app-subtle)] hover:text-foreground"
          onClick={() => onOpenFile(movePackage.manifestPath)}
          type="button"
        >
          <Package className="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span className="min-w-0">
            <span className="flex min-w-0 items-center gap-2">
              <span className="truncate text-sm font-medium">
                {displayMovePackageName(movePackage.name)}
              </span>
              {isRoot ? (
                <span className="rounded bg-primary/15 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                  root
                </span>
              ) : null}
            </span>
            <span className="mt-0.5 block truncate text-xs text-muted-foreground">
              {movePackage.path || "."}
            </span>
          </span>
        </button>
        {isRoot ? (
          <button
            className="mt-1 inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground disabled:pointer-events-none disabled:opacity-50"
            disabled={buildStatus?.state === "running"}
            onClick={() => onBuildPackage(movePackage)}
            title="Run sui move build"
            type="button"
          >
            {buildStatus?.state === "running" ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <Play className="size-4" aria-hidden="true" />
            )}
          </button>
        ) : null}
      </div>

      {isRoot && buildStatus?.message ? (
        <div
          className={cn(
            "mx-2 mb-2 rounded border px-2 py-1 text-xs",
            buildStatus.state === "success" &&
              "border-emerald-500/30 bg-emerald-500/10 text-emerald-300",
            buildStatus.state === "error" &&
              "border-destructive/40 bg-destructive/10 text-destructive",
            buildStatus.state === "running" && "bg-muted text-muted-foreground",
          )}
        >
          {buildStatus.message}
        </div>
      ) : null}

      <div className="mt-2 space-y-1 pl-3">
        <div className="flex items-center gap-2 px-2 text-xs font-medium text-muted-foreground">
          <Box className="size-3.5" aria-hidden="true" />
          {moduleCountLabel(movePackage.modules)}
        </div>
        {movePackage.modules.length ? (
          movePackage.modules.map((moveModule) => (
            <ModuleButton
              key={moveModule.filePath}
              activePath={activePath}
              moveModule={moveModule}
              onSelectModule={(module) => onSelectModule(movePackage, module)}
              selectedModulePath={selectedModulePath}
            />
          ))
        ) : (
          <div className="px-2 py-1 text-xs text-muted-foreground">
            No modules in sources/.
          </div>
        )}
      </div>
    </section>
  );
}

type ModuleButtonProps = {
  activePath: string | null;
  moveModule: MoveModule;
  onSelectModule: (moveModule: MoveModule) => void;
  selectedModulePath: string | null;
};

function ModuleButton({
  activePath,
  moveModule,
  onSelectModule,
  selectedModulePath,
}: ModuleButtonProps) {
  const isSelected = selectedModulePath === moveModule.filePath;

  return (
    <button
      className={cn(
        "flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-[var(--app-subtle)] hover:text-foreground",
        (activePath === moveModule.filePath || isSelected) &&
          "bg-[var(--app-subtle)] text-foreground",
      )}
      onClick={() => onSelectModule(moveModule)}
      type="button"
    >
      <FileCode2 className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span className="min-w-0">
        <span className="block truncate">{moveModule.name}</span>
        {moveModule.address ? (
          <span className="block truncate text-xs text-muted-foreground">
            {moveModule.address}
          </span>
        ) : null}
      </span>
    </button>
  );
}

function packageCountLabel(packages: MovePackage[]) {
  if (packages.length === 1) {
    return "1 package found";
  }

  return `${packages.length} packages found`;
}

function moduleCountLabel(modules: MoveModule[]) {
  if (modules.length === 1) {
    return "1 module";
  }

  return `${modules.length} modules`;
}
