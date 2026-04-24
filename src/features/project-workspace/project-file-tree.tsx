import { FileTree, useFileTree, useFileTreeSelection } from "@pierre/trees/react";
import React, { type CSSProperties } from "react";

import type { PackageTree } from "@/features/empty-project/filesystem-tree";

type ProjectFileTreeProps = {
  packageTree: PackageTree;
  selectedPath: string | null;
  onSelectPath: (path: string | null) => void;
};

export function ProjectFileTree({
  packageTree,
  selectedPath,
  onSelectPath,
}: ProjectFileTreeProps) {
  return (
    <aside className="grid min-h-0 grid-rows-[auto_1fr] border-r bg-sidebar text-sidebar-foreground">
      <header className="min-w-0 border-b px-4 py-3">
        <h2 className="truncate text-sm font-semibold">{packageTree.rootName}</h2>
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
  const { model } = useFileTree({
    flattenEmptyDirectories: true,
    initialExpansion: "closed",
    initialSelectedPaths: selectedPath ? [selectedPath] : undefined,
    paths: packageTree.paths,
    stickyFolders: true,
  });
  const selectedPaths = useFileTreeSelection(model);
  const activePath = selectedPaths[0] ?? null;

  React.useEffect(() => {
    onSelectPath(activePath);
  }, [activePath, onSelectPath]);

  return (
    <FileTree
      model={model}
      className="min-h-0"
      style={treeStyles}
    />
  );
}

const treeStyles = {
  height: "100%",
  "--trees-bg-override": "var(--sidebar)",
  "--trees-fg-override": "var(--sidebar-foreground)",
  "--trees-fg-muted-override": "var(--muted-foreground)",
  "--trees-bg-muted-override": "var(--sidebar-accent)",
  "--trees-selected-bg-override": "var(--sidebar-accent)",
  "--trees-selected-fg-override": "var(--sidebar-accent-foreground)",
  "--trees-selected-focused-border-color-override": "var(--ring)",
  "--trees-border-color-override": "var(--sidebar-border)",
  "--trees-focus-ring-color-override": "var(--ring)",
  "--trees-search-bg-override": "var(--muted)",
  "--trees-search-fg-override": "var(--foreground)",
} as CSSProperties;
