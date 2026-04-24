import { Box, FileCode2, Package } from "lucide-react";

import type {
  MoveModule,
  MovePackage,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

type MovePackagePanelProps = {
  activePath: string | null;
  packages: MovePackage[];
  onOpenFile: (path: string) => void;
};

export function MovePackagePanel({
  activePath,
  packages,
  onOpenFile,
}: MovePackagePanelProps) {
  return (
    <aside className="grid min-h-0 grid-rows-[auto_1fr] border-l bg-sidebar text-sidebar-foreground">
      <header className="border-b px-4 py-3">
        <h2 className="text-sm font-semibold">Move Packages</h2>
        <p className="mt-1 text-xs text-muted-foreground">
          {packageCountLabel(packages)}
        </p>
      </header>

      {packages.length ? (
        <div className="min-h-0 overflow-auto px-3 py-3">
          <div className="space-y-4">
            {packages.map((movePackage) => (
              <PackageSection
                key={movePackage.manifestPath}
                activePath={activePath}
                movePackage={movePackage}
                onOpenFile={onOpenFile}
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
  movePackage: MovePackage;
  onOpenFile: (path: string) => void;
};

function PackageSection({
  activePath,
  movePackage,
  onOpenFile,
}: PackageSectionProps) {
  return (
    <section>
      <button
        className="flex w-full min-w-0 items-start gap-2 rounded-md px-2 py-2 text-left hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
        onClick={() => onOpenFile(movePackage.manifestPath)}
        type="button"
      >
        <Package className="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span className="min-w-0">
          <span className="block truncate text-sm font-medium">{movePackage.name}</span>
          <span className="mt-0.5 block truncate text-xs text-muted-foreground">
            {movePackage.path || "."}
          </span>
        </span>
      </button>

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
              onOpenFile={onOpenFile}
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
  onOpenFile: (path: string) => void;
};

function ModuleButton({
  activePath,
  moveModule,
  onOpenFile,
}: ModuleButtonProps) {
  return (
    <button
      className={cn(
        "flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
        activePath === moveModule.filePath && "bg-sidebar-accent text-sidebar-accent-foreground",
      )}
      onClick={() => onOpenFile(moveModule.filePath)}
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
