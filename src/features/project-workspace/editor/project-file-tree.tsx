import { FileTree, useFileTree } from "@pierre/trees/react";
import React, { type CSSProperties } from "react";

import {
  displayMovePackageName,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

type FileTreeModel = ReturnType<typeof useFileTree>["model"];

type ProjectFileTreeProps = {
  packageTree: PackageTree;
  selectedPath: string | null;
  side?: "left" | "right";
  onSelectPath: (path: string | null) => void;
};

export function ProjectFileTree({
  packageTree,
  selectedPath,
  side = "left",
  onSelectPath,
}: ProjectFileTreeProps) {
  return (
    <aside
      className={cn(
        "grid min-h-0 grid-rows-[auto_1fr] bg-[var(--app-panel)] text-foreground",
        side === "left" ? "border-r border-[color:var(--app-border)]" : "border-l border-[color:var(--app-border)]",
      )}
    >
      <header className="min-w-0 border-b px-4 py-3">
        <h2 className="truncate text-sm font-semibold">
          {displayMovePackageName(packageTree.rootName)}
        </h2>
        <p className="mt-1 truncate text-xs text-muted-foreground">
          {packageTree.rootPath}
        </p>
      </header>
      <ProjectFileTreeBody
        key={packageTree.rootPath}
        packageTree={packageTree}
        selectedPath={selectedPath}
        onSelectPath={onSelectPath}
      />
    </aside>
  );
}

function ProjectFileTreeBody({
  packageTree,
  selectedPath,
  onSelectPath,
}: ProjectFileTreeProps) {
  const onSelectPathRef = React.useRef(onSelectPath);
  const activatedPathRef = React.useRef<string | null>(null);
  const isSyncingSelectionRef = React.useRef(false);

  React.useEffect(() => {
    onSelectPathRef.current = onSelectPath;
  }, [onSelectPath]);

  const { model } = useFileTree({
    flattenEmptyDirectories: true,
    initialExpansion: "closed",
    initialSelectedPaths: selectedPath ? [selectedPath] : undefined,
    onSelectionChange: (paths) => {
      if (isSyncingSelectionRef.current) {
        return;
      }

      const selectedPath = paths[0] ?? null;

      const activatedPath = activatedPathRef.current;
      activatedPathRef.current = null;

      if (activatedPath === selectedPath) {
        return;
      }

      onSelectPathRef.current(selectedPath);
    },
    paths: packageTree.paths,
    stickyFolders: true,
  });

  React.useEffect(() => {
    isSyncingSelectionRef.current = true;

    try {
      syncSelectedPath(model, selectedPath);
    } finally {
      queueMicrotask(() => {
        isSyncingSelectionRef.current = false;
      });
    }
  }, [model, selectedPath]);

  return (
    <FileTree
      model={model}
      className="min-h-0"
      onClickCapture={(event) => {
        const path = treePathFromEvent(event.nativeEvent);

        if (path) {
          activatedPathRef.current = path;
          onSelectPathRef.current(path);
        }
      }}
      style={treeStyles}
    />
  );
}

function treePathFromEvent(event: Event) {
  for (const target of event.composedPath()) {
    if (!(target instanceof HTMLElement)) {
      continue;
    }

    const path =
      target.dataset.itemPath ??
      target.closest<HTMLElement>("[data-item-path]")?.dataset.itemPath;

    if (path) {
      return path;
    }
  }

  return null;
}

function syncSelectedPath(model: FileTreeModel, selectedPath: string | null) {
  const currentSelectedPaths = model.getSelectedPaths();
  const currentSelectedPath = currentSelectedPaths[0] ?? null;

  if (!selectedPath && currentSelectedPaths.length === 0) {
    return;
  }

  if (currentSelectedPaths.length === 1 && currentSelectedPath === selectedPath) {
    return;
  }

  for (const path of currentSelectedPaths) {
    model.getItem(path)?.deselect();
  }

  if (!selectedPath) {
    return;
  }

  expandAncestorDirectories(model, selectedPath);

  const item = model.getItem(selectedPath);
  item?.select();
  item?.focus();
}

function expandAncestorDirectories(model: FileTreeModel, path: string) {
  const ancestors = ancestorDirectoryPaths(path);

  for (const ancestor of ancestors) {
    const item = model.getItem(ancestor);

    if (item && "expand" in item) {
      item.expand();
    }
  }
}

function ancestorDirectoryPaths(path: string) {
  const normalizedPath = path.replace(/\/$/, "");
  const parts = normalizedPath.split("/").filter(Boolean);
  const ancestors: string[] = [];
  let current = "";

  for (const part of parts.slice(0, -1)) {
    current = `${current}${part}/`;
    ancestors.push(current);
  }

  return ancestors;
}

const treeStyles = {
  height: "100%",
  "--trees-bg-override": "var(--app-panel)",
  "--trees-fg-override": "var(--foreground)",
  "--trees-fg-muted-override": "var(--muted-foreground)",
  "--trees-bg-muted-override": "var(--app-subtle)",
  "--trees-selected-bg-override": "var(--app-subtle)",
  "--trees-selected-fg-override": "var(--foreground)",
  "--trees-selected-focused-border-color-override": "var(--ring)",
  "--trees-border-color-override": "var(--app-border)",
  "--trees-focus-ring-color-override": "var(--ring)",
  "--trees-search-bg-override": "var(--muted)",
  "--trees-search-fg-override": "var(--foreground)",
} as CSSProperties;
