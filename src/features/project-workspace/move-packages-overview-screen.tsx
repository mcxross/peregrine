import { Box, FileCode2, Package } from "lucide-react";
import React from "react";

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

const TREE_PANE_DEFAULT_WIDTH = 460;
const TREE_PANE_MIN_WIDTH = 320;
const TREE_PANE_MAX_WIDTH = 760;
const DETAIL_PANE_MIN_WIDTH = 420;

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
  const containerRef = React.useRef<HTMLDivElement | null>(null);
  const [treePaneWidth, setTreePaneWidth] = React.useState(TREE_PANE_DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = React.useState(false);

  React.useEffect(() => {
    if (!isResizing) {
      return;
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
    };
  }, [isResizing]);

  const resizeTreePane = React.useCallback((clientX: number) => {
    const bounds = containerRef.current?.getBoundingClientRect();

    if (!bounds) {
      return;
    }

    const maxWidth = Math.max(
      TREE_PANE_MIN_WIDTH,
      Math.min(TREE_PANE_MAX_WIDTH, bounds.width - DETAIL_PANE_MIN_WIDTH),
    );
    const nextWidth = Math.min(maxWidth, Math.max(TREE_PANE_MIN_WIDTH, clientX - bounds.left));
    setTreePaneWidth(nextWidth);
  }, []);

  return (
    <section className="grid h-full min-h-0 bg-[var(--app-window)]">
      {movePackage ? (
        <div
          ref={containerRef}
          className={cn(
            "grid min-h-0",
            !selectedModule && "grid-cols-1",
            isResizing && "select-none",
          )}
          style={
            selectedModule
              ? { gridTemplateColumns: `${treePaneWidth}px 6px minmax(0, 1fr)` }
              : undefined
          }
        >
          <ScrollArea className="min-h-0 select-none">
            <div className="grid gap-3 p-5">
              <PackageCard
                isRoot={movePackage.name === rootPackage}
                movePackage={movePackage}
                onSelectModule={onSelectModule}
                selectedModulePath={selectedModule?.moveModule.filePath ?? null}
              />
            </div>
          </ScrollArea>

          {selectedModule ? (
            <div
              aria-label="Resize module tree"
              aria-orientation="vertical"
              className={cn(
                "group relative cursor-col-resize border-r border-[color:var(--app-border)]",
                isResizing && "border-primary/50",
              )}
              onPointerCancel={() => setIsResizing(false)}
              onDragStart={(event) => event.preventDefault()}
              onPointerDown={(event) => {
                event.preventDefault();
                event.currentTarget.setPointerCapture(event.pointerId);
                setIsResizing(true);
                resizeTreePane(event.clientX);
              }}
              onPointerMove={(event) => {
                if (isResizing) {
                  resizeTreePane(event.clientX);
                }
              }}
              onPointerUp={(event) => {
                event.currentTarget.releasePointerCapture(event.pointerId);
                setIsResizing(false);
              }}
              role="separator"
            >
              <span
                className={cn(
                  "absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-transparent transition-colors group-hover:bg-primary/45",
                  isResizing && "bg-primary/70",
                )}
              />
            </div>
          ) : null}

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
    <section className="min-w-0 select-none">
      <div className="grid min-w-0 grid-cols-[24px_minmax(0,1fr)] items-center gap-3">
        <Package className="size-5 justify-self-center text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <h2 className="truncate text-base font-semibold">{movePackage.name}</h2>
            {isRoot ? (
              <span className="rounded bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                root
              </span>
            ) : null}
          </div>
        </div>
      </div>

      <div className="mt-5">
        <div className="grid grid-cols-[24px_minmax(0,1fr)] items-center gap-3 text-sm text-muted-foreground">
          <Box className="size-5 justify-self-center" aria-hidden="true" />
          <span>{moduleCountLabel(movePackage.modules)}</span>
        </div>

        {movePackage.modules.length ? (
          <div className="mt-3 max-w-[640px]">
            {movePackage.modules.map((moveModule, index) => (
              <ModuleRow
                isLast={index === movePackage.modules.length - 1}
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
    </section>
  );
}

function ModuleRow({
  isLast,
  moveModule,
  onSelect,
  selected,
}: {
  isLast: boolean;
  moveModule: MoveModule;
  onSelect: () => void;
  selected: boolean;
}) {
  return (
    <div className="grid min-h-[66px] select-none grid-cols-[40px_minmax(0,1fr)]">
      <div className="relative" aria-hidden="true">
        <span
          className={cn(
            "absolute left-3 top-0 w-px bg-[var(--app-border)]",
            isLast ? "h-[29px]" : "bottom-0",
          )}
        />
        <span className="absolute left-3 top-[29px] h-px w-7 bg-[var(--app-border)]" />
      </div>
      <button
        className={cn(
          "mb-1.5 grid min-w-0 select-none grid-cols-[24px_minmax(0,1fr)] items-center gap-3 rounded-md px-3 py-2.5 text-left transition hover:bg-[var(--app-subtle)] hover:text-foreground",
          selected && "bg-[var(--app-subtle)] text-foreground ring-1 ring-ring/25",
        )}
        onClick={onSelect}
        type="button"
      >
        <FileCode2 className="size-5 justify-self-center text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">{moveModule.name}</div>
          <div className="mt-0.5 truncate text-xs text-muted-foreground">
            {moduleSurfaceLabel(moveModule)}
          </div>
        </div>
      </button>
    </div>
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
