import { Box, FileCode2, Package } from "lucide-react";

import { Card } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  MoveModule,
  MovePackage,
  PackageTree,
} from "@/features/empty-project/filesystem-tree";
import {
  ModuleSignatureScreen,
  type SelectedMoveModule,
} from "@/features/project-workspace/module-signature-screen";
import { cn } from "@/lib/utils";

type MovePackagesOverviewScreenProps = {
  activeMovePackage: MovePackage | null;
  onClearSelectedModule: () => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  packageTree: PackageTree;
  selectedModule: SelectedMoveModule | null;
};

export function MovePackagesOverviewScreen({
  activeMovePackage,
  onClearSelectedModule,
  onSelectModule,
  packageTree,
  selectedModule,
}: MovePackagesOverviewScreenProps) {
  const rootPackage = packageTree.dependencyGraph.root;
  const movePackage = activeMovePackage ?? orderedPackages(packageTree.movePackages, rootPackage)[0] ?? null;

  return (
    <section className="grid h-full min-h-0 bg-[var(--app-window)]">
      {movePackage ? (
        <div
          className={cn(
            "grid min-h-0",
            selectedModule
              ? "grid-cols-[minmax(300px,38%)_minmax(0,1fr)]"
              : "grid-cols-1",
          )}
        >
          <ScrollArea
            className={cn(
              "min-h-0",
              selectedModule && "border-r border-[color:var(--app-border)]",
            )}
          >
            <div className="grid gap-3 p-4">
              <PackageCard
                isRoot={movePackage.name === rootPackage}
                movePackage={movePackage}
                onSelectModule={onSelectModule}
                selectedModulePath={selectedModule?.moveModule.filePath ?? null}
              />
            </div>
          </ScrollArea>

          {selectedModule ? (
            <div className="min-h-0">
              <ModuleSignatureScreen
                selectedModule={selectedModule}
                onClose={onClearSelectedModule}
              />
            </div>
          ) : null}
        </div>
      ) : (
        <div className="flex min-h-0 items-center justify-center px-6 text-center text-sm text-muted-foreground">
          No Move.toml files found in this workspace.
        </div>
      )}
    </section>
  );
}

function PackageCard({
  isRoot,
  movePackage,
  onSelectModule,
  selectedModulePath,
}: {
  isRoot: boolean;
  movePackage: MovePackage;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  selectedModulePath: string | null;
}) {
  return (
    <Card className="min-w-0 gap-0 rounded-md p-4">
      <div className="flex min-w-0 items-start gap-3">
        <Package className="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <h2 className="truncate text-base font-semibold">{movePackage.name}</h2>
            {isRoot ? (
              <span className="rounded bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                root
              </span>
            ) : null}
          </div>
          <p className="mt-1 truncate text-sm text-muted-foreground">
            {movePackage.path || "."}
          </p>
        </div>
      </div>

      <div className="mt-4 border-l border-[color:var(--app-border)] pl-5">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Box className="size-4 shrink-0" aria-hidden="true" />
          <span>{moduleCountLabel(movePackage.modules)}</span>
        </div>

        {movePackage.modules.length ? (
          <div className="mt-3 grid max-w-xl gap-2">
            {movePackage.modules.map((moveModule) => (
              <ModuleRow
                key={moveModule.filePath}
                moveModule={moveModule}
                onSelect={() => onSelectModule(movePackage, moveModule)}
                selected={selectedModulePath === moveModule.filePath}
              />
            ))}
          </div>
        ) : (
          <p className="mt-2 text-sm text-muted-foreground">No modules in sources/.</p>
        )}
      </div>
    </Card>
  );
}

function ModuleRow({
  moveModule,
  onSelect,
  selected,
}: {
  moveModule: MoveModule;
  onSelect: () => void;
  selected: boolean;
}) {
  return (
    <button
      className={cn(
        "flex min-w-0 items-center gap-3 rounded-md bg-[var(--app-subtle)] px-3 py-2.5 text-left hover:bg-accent hover:text-accent-foreground",
        selected && "bg-accent text-accent-foreground ring-1 ring-ring/35",
      )}
      onClick={onSelect}
      type="button"
    >
      <FileCode2 className="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{moveModule.name}</div>
        <div className="mt-0.5 truncate text-xs text-muted-foreground">
          {moduleSurfaceLabel(moveModule)}
        </div>
      </div>
    </button>
  );
}

function orderedPackages(packages: MovePackage[], rootPackage: string | null) {
  return [...packages].sort((left: MovePackage, right: MovePackage) => {
    const leftIsRoot = left.name === rootPackage;
    const rightIsRoot = right.name === rootPackage;

    return Number(rightIsRoot) - Number(leftIsRoot)
      || left.name.localeCompare(right.name)
      || left.path.localeCompare(right.path);
  });
}

function moduleCountLabel(modules: MoveModule[]) {
  if (modules.length === 1) {
    return "1 module";
  }

  return `${modules.length} modules`;
}

function moduleSurfaceLabel(moveModule: MoveModule) {
  const structCount = moveModule.structs?.length ?? 0;
  const functionCount = moveModule.functions?.length ?? 0;
  const structs = structCount === 1 ? "1 struct" : `${structCount} structs`;
  const functions = functionCount === 1 ? "1 function" : `${functionCount} functions`;

  return `${structs} / ${functions}`;
}
