import { Box, FileCode2, FileText, Folder, Package, Save, SquarePen, X } from "lucide-react";
import React from "react";

import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  FilePreview,
  MoveModule,
  MovePackage,
  PackageTree,
} from "@/features/empty-project/filesystem-tree";
import {
  loadFilePreview,
  saveTextFile,
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
const CodeEditor = React.lazy(() =>
  import("@/features/project-workspace/code-editor").then((module) => ({
    default: module.CodeEditor,
  })),
);

type MovePackagesOverviewScreenProps = {
  activeMovePackage: MovePackage | null;
  onClearSelectedModule: () => void;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  packageTree: PackageTree;
  selectedModule: SelectedMoveModule | null;
};

type ModuleEditorTab = {
  error: string | null;
  isSaving: boolean;
  preview: FilePreview | null;
  savedSource: string;
  selectedModule: SelectedMoveModule;
  source: string;
  status: "error" | "idle" | "loaded" | "loading";
};

type ModuleTreeNode =
  | {
      children: ModuleTreeNode[];
      name: string;
      path: string;
      type: "directory";
    }
  | {
      module: MoveModule;
      name: string;
      path: string;
      type: "module";
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
  const [isEditorMode, setIsEditorMode] = React.useState(false);
  const [editorTabs, setEditorTabs] = React.useState<ModuleEditorTab[]>([]);
  const [activeEditorPath, setActiveEditorPath] = React.useState<string | null>(null);

  React.useEffect(() => {
    setEditorTabs([]);
    setActiveEditorPath(null);
    setIsEditorMode(false);
  }, [movePackage?.manifestPath, packageTree.rootPath]);

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

  React.useEffect(() => {
    if (!isEditorMode || !selectedModule) {
      return;
    }

    const filePath = selectedModule.moveModule.filePath;
    setActiveEditorPath(filePath);
    setEditorTabs((current) => {
      const existingTab = current.find((tab) => tab.selectedModule.moveModule.filePath === filePath);

      if (existingTab) {
        return current.map((tab) =>
          tab.selectedModule.moveModule.filePath === filePath
            ? { ...tab, selectedModule }
            : tab,
        );
      }

      return [...current, createModuleEditorTab(selectedModule)];
    });
  }, [isEditorMode, selectedModule]);

  const hasDetailPane = Boolean(selectedModule) || (isEditorMode && editorTabs.length > 0);

  return (
    <section className="grid h-full min-h-0 bg-[var(--app-window)]">
      {movePackage ? (
        <div
          ref={containerRef}
          className={cn(
            "grid min-h-0",
            !hasDetailPane && "grid-cols-1",
            isResizing && "select-none",
          )}
          style={
            hasDetailPane
              ? { gridTemplateColumns: `${treePaneWidth}px 6px minmax(0, 1fr)` }
              : undefined
          }
        >
          <ScrollArea className="min-h-0 select-none">
            <div className="grid gap-3 p-5">
              <PackageCard
                isRoot={movePackage.name === rootPackage}
                isEditorMode={isEditorMode}
                movePackage={movePackage}
                onSelectModule={onSelectModule}
                onToggleEditorMode={() => setIsEditorMode((current) => !current)}
                selectedModulePath={selectedModule?.moveModule.filePath ?? null}
              />
            </div>
          </ScrollArea>

          {hasDetailPane ? (
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

          {hasDetailPane ? (
            <div className="min-h-0 overflow-hidden">
              {isEditorMode ? (
                <ModuleSourceEditorWorkspace
                  activeEditorPath={activeEditorPath}
                  editorTabs={editorTabs}
                  onActiveEditorPathChange={setActiveEditorPath}
                  onClearSelectedModule={onClearSelectedModule}
                  onEditorTabsChange={setEditorTabs}
                  onSelectModule={onSelectModule}
                  packageTree={packageTree}
                />
              ) : (
                selectedModule ? (
                <ModuleSignatureScreen
                  selectedModule={selectedModule}
                  onClose={onClearSelectedModule}
                />
                ) : null
              )}
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
  isEditorMode,
  isRoot,
  movePackage,
  onSelectModule,
  onToggleEditorMode,
  selectedModulePath,
}: {
  isEditorMode: boolean;
  isRoot: boolean;
  movePackage: MovePackage;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  onToggleEditorMode: () => void;
  selectedModulePath: string | null;
}) {
  const moduleTree = React.useMemo(() => buildModuleTree(movePackage.modules), [movePackage.modules]);

  return (
    <section className="min-w-0 select-none">
      <div className="grid min-w-0 grid-cols-[24px_minmax(0,1fr)_32px] items-center gap-3">
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
        <button
          aria-label={isEditorMode ? "Show module surface" : "Open source editor"}
          aria-pressed={isEditorMode}
          className={cn(
            "inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground",
            isEditorMode && "bg-primary/10 text-primary",
          )}
          onClick={onToggleEditorMode}
          title={isEditorMode ? "Show module surface" : "Open source editor"}
          type="button"
        >
          <SquarePen className="size-4" aria-hidden="true" />
        </button>
      </div>

      <div className="mt-5">
        <div className="grid grid-cols-[24px_minmax(0,1fr)] items-center gap-3 text-sm text-muted-foreground">
          <Box className="size-5 justify-self-center" aria-hidden="true" />
          <span>{moduleCountLabel(movePackage.modules)}</span>
        </div>

        {movePackage.modules.length ? (
          <div className="mt-3 max-w-[640px]">
            <ModuleTreeRows
              nodes={moduleTree}
              onSelectModule={(moveModule) => onSelectModule(movePackage, moveModule)}
              selectedModulePath={selectedModulePath}
            />
          </div>
        ) : (
          <p className="mt-2 text-sm text-muted-foreground">No modules in sources/.</p>
        )}
      </div>
    </section>
  );
}

function ModuleTreeRows({
  depth = 0,
  nodes,
  onSelectModule,
  selectedModulePath,
}: {
  depth?: number;
  nodes: ModuleTreeNode[];
  onSelectModule: (moveModule: MoveModule) => void;
  selectedModulePath: string | null;
}) {
  return (
    <>
      {nodes.map((node, index) => {
        const isLast = index === nodes.length - 1;

        if (node.type === "directory") {
          return (
            <React.Fragment key={node.path}>
              <DirectoryRow depth={depth} isLast={isLast} node={node} />
              <ModuleTreeRows
                depth={depth + 1}
                nodes={node.children}
                onSelectModule={onSelectModule}
                selectedModulePath={selectedModulePath}
              />
            </React.Fragment>
          );
        }

        return (
          <ModuleRow
            depth={depth}
            isLast={isLast}
            key={node.module.filePath}
            moveModule={node.module}
            onSelect={() => onSelectModule(node.module)}
            selected={selectedModulePath === node.module.filePath}
          />
        );
      })}
    </>
  );
}

function DirectoryRow({
  depth,
  isLast,
  node,
}: {
  depth: number;
  isLast: boolean;
  node: Extract<ModuleTreeNode, { type: "directory" }>;
}) {
  const gutterWidth = 40 + depth * 28;
  const branchLeft = 12 + depth * 28;

  return (
    <div
      className="grid min-h-[48px] select-none"
      style={{ gridTemplateColumns: `${gutterWidth}px minmax(0, 1fr)` }}
    >
      <div className="relative" aria-hidden="true">
        <span
          className={cn(
            "absolute top-0 w-px bg-[var(--app-border)]",
            isLast ? "h-[24px]" : "bottom-0",
          )}
          style={{ left: branchLeft }}
        />
        <span
          className="absolute top-[24px] h-px w-7 bg-[var(--app-border)]"
          style={{ left: branchLeft }}
        />
      </div>
      <div className="mb-1.5 grid min-w-0 grid-cols-[24px_minmax(0,1fr)] items-center gap-3 rounded-md px-3 py-2 text-left text-muted-foreground">
        <Folder className="size-5 justify-self-center text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0 truncate text-sm font-medium text-foreground">{node.name}</div>
      </div>
    </div>
  );
}

function ModuleSourceEditorWorkspace({
  activeEditorPath,
  editorTabs,
  onActiveEditorPathChange,
  onClearSelectedModule,
  onEditorTabsChange,
  onSelectModule,
  packageTree,
}: {
  activeEditorPath: string | null;
  editorTabs: ModuleEditorTab[];
  onActiveEditorPathChange: (path: string | null) => void;
  onClearSelectedModule: () => void;
  onEditorTabsChange: React.Dispatch<React.SetStateAction<ModuleEditorTab[]>>;
  onSelectModule: (movePackage: MovePackage, moveModule: MoveModule) => void;
  packageTree: PackageTree;
}) {
  const activeTab = editorTabs.find((tab) => tab.selectedModule.moveModule.filePath === activeEditorPath)
    ?? editorTabs[0]
    ?? null;
  const activePath = activeTab?.selectedModule.moveModule.filePath ?? null;
  const isDirty = activeTab ? activeTab.source !== activeTab.savedSource : false;

  const updateTab = React.useCallback((
    path: string,
    update: (tab: ModuleEditorTab) => ModuleEditorTab,
  ) => {
    onEditorTabsChange((current) =>
      current.map((tab) =>
        tab.selectedModule.moveModule.filePath === path ? update(tab) : tab,
      ),
    );
  }, [onEditorTabsChange]);

  React.useEffect(() => {
    if (!activeTab || activeTab.status !== "idle") {
      return;
    }

    const filePath = activeTab.selectedModule.moveModule.filePath;

    updateTab(filePath, (tab) => ({
      ...tab,
      error: null,
      preview: null,
      source: "",
      savedSource: "",
      status: "loading",
    }));

    loadFilePreview(packageTree, filePath)
      .then((nextPreview) => {
        if (nextPreview.kind !== "text") {
          updateTab(filePath, (tab) => ({
            ...tab,
            error: "This module cannot be opened in the editor.",
            status: "error",
          }));
          return;
        }

        updateTab(filePath, (tab) => ({
          ...tab,
          error: null,
          preview: nextPreview,
          savedSource: nextPreview.source,
          source: nextPreview.source,
          status: "loaded",
        }));
      })
      .catch((reason: unknown) => {
        updateTab(filePath, (tab) => ({
          ...tab,
          error: reason instanceof Error ? reason.message : "Could not load this module.",
          status: "error",
        }));
      });
  }, [activeTab, packageTree, updateTab]);

  const saveModule = React.useCallback(async () => {
    if (!activeTab || !isDirty || activeTab.isSaving) {
      return;
    }

    const filePath = activeTab.selectedModule.moveModule.filePath;
    updateTab(filePath, (tab) => ({ ...tab, error: null, isSaving: true }));

    try {
      const nextPreview = await saveTextFile(packageTree, filePath, activeTab.source);

      if (nextPreview.kind !== "text") {
        throw new Error("The saved module could not be reopened as text.");
      }

      updateTab(filePath, (tab) => ({
        ...tab,
        error: null,
        isSaving: false,
        preview: nextPreview,
        savedSource: nextPreview.source,
        source: nextPreview.source,
        status: "loaded",
      }));
    } catch (reason) {
      updateTab(filePath, (tab) => ({
        ...tab,
        error: reason instanceof Error ? reason.message : "Could not save this module.",
        isSaving: false,
      }));
    }
  }, [activeTab, isDirty, packageTree, updateTab]);

  const activateTab = React.useCallback((tab: ModuleEditorTab) => {
    const filePath = tab.selectedModule.moveModule.filePath;
    onActiveEditorPathChange(filePath);
    onSelectModule(tab.selectedModule.movePackage, tab.selectedModule.moveModule);
  }, [onActiveEditorPathChange, onSelectModule]);

  const closeTab = React.useCallback((path: string) => {
    const nextTabs = editorTabs.filter((tab) => tab.selectedModule.moveModule.filePath !== path);
    onEditorTabsChange(nextTabs);

    if (activePath !== path) {
      return;
    }

    const closedIndex = editorTabs.findIndex((tab) => tab.selectedModule.moveModule.filePath === path);
    const nextActiveTab = nextTabs[Math.max(0, closedIndex - 1)] ?? nextTabs[0] ?? null;
    const nextActivePath = nextActiveTab?.selectedModule.moveModule.filePath ?? null;
    onActiveEditorPathChange(nextActivePath);

    if (nextActiveTab) {
      onSelectModule(nextActiveTab.selectedModule.movePackage, nextActiveTab.selectedModule.moveModule);
    } else {
      onClearSelectedModule();
    }
  }, [
    activePath,
    editorTabs,
    onActiveEditorPathChange,
    onClearSelectedModule,
    onEditorTabsChange,
    onSelectModule,
  ]);

  const updateSource = React.useCallback((source: string) => {
    if (!activePath) {
      return;
    }

    updateTab(activePath, (tab) => ({ ...tab, source }));
  }, [activePath, updateTab]);

  if (!activeTab) {
    return (
      <section className="flex h-full min-h-0 items-center justify-center bg-[var(--app-window)] px-6 text-sm text-muted-foreground">
        Select a module to open it in the editor.
      </section>
    );
  }

  return (
    <section className="grid h-full min-h-0 animate-in fade-in slide-in-from-right-3 duration-200 grid-rows-[auto_auto_minmax(0,1fr)] bg-[var(--app-window)]">
      <header className="grid min-h-[58px] min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-[color:var(--app-border)] px-3">
        <div className="flex min-w-0 items-end gap-1 overflow-x-auto pt-2">
          {editorTabs.map((tab) => {
            const path = tab.selectedModule.moveModule.filePath;
            const tabIsDirty = tab.source !== tab.savedSource;
            const isActive = path === activePath;

            return (
              <button
                className={cn(
                  "group flex h-10 max-w-64 shrink-0 items-center gap-2 rounded-t-md border border-b-0 border-transparent px-3 text-left text-xs text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground",
                  isActive && "border-[color:var(--app-border)] bg-[var(--app-window)] text-foreground",
                )}
                key={path}
                onClick={() => activateTab(tab)}
                title={path}
                type="button"
              >
                <FileText className="size-3.5 shrink-0" aria-hidden="true" />
                <span className="min-w-0 truncate font-medium">
                  {tab.selectedModule.moveModule.name}{tabIsDirty ? " *" : ""}
                </span>
                <span
                  aria-label={`Close ${tab.selectedModule.moveModule.name}`}
                  className="ml-1 inline-flex size-5 shrink-0 items-center justify-center rounded text-muted-foreground opacity-70 transition hover:bg-background/50 hover:text-foreground group-hover:opacity-100"
                  onClick={(event) => {
                    event.stopPropagation();
                    closeTab(path);
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <X className="size-3" aria-hidden="true" />
                </span>
              </button>
            );
          })}
        </div>
        <Button
          disabled={!isDirty || activeTab.isSaving || activeTab.status !== "loaded"}
          onClick={saveModule}
          size="xs"
          type="button"
          variant="ghost"
        >
          <Save className="size-3.5" aria-hidden="true" />
          {activeTab.isSaving ? "Saving" : "Save"}
        </Button>
      </header>

      {activeTab.error ? (
        <div className="border-b border-destructive/40 bg-destructive/10 px-6 py-2 text-xs text-destructive">
          {activeTab.error}
        </div>
      ) : null}

      {activeTab.status === "loading" || activeTab.status === "idle" ? (
        <div className="flex min-h-0 items-center justify-center px-6 text-sm text-muted-foreground">
          Loading module...
        </div>
      ) : activeTab.status === "loaded" && activeTab.preview?.kind === "text" ? (
        <React.Suspense
          fallback={
            <div className="flex min-h-0 items-center justify-center px-6 text-sm text-muted-foreground">
              Loading editor...
            </div>
          }
        >
          <CodeEditor
            key={activePath}
            language={activeTab.preview.language || "move"}
            value={activeTab.source}
            onChange={updateSource}
          />
        </React.Suspense>
      ) : (
        <div className="flex min-h-0 items-center justify-center px-6 text-sm text-muted-foreground">
          No editor preview available.
        </div>
      )}
    </section>
  );
}

function createModuleEditorTab(selectedModule: SelectedMoveModule): ModuleEditorTab {
  return {
    error: null,
    isSaving: false,
    preview: null,
    savedSource: "",
    selectedModule,
    source: "",
    status: "idle",
  };
}

function ModuleRow({
  depth = 0,
  isLast,
  moveModule,
  onSelect,
  selected,
}: {
  depth?: number;
  isLast: boolean;
  moveModule: MoveModule;
  onSelect: () => void;
  selected: boolean;
}) {
  const gutterWidth = 40 + depth * 28;
  const branchLeft = 12 + depth * 28;

  return (
    <div
      className="grid min-h-[66px] select-none"
      style={{ gridTemplateColumns: `${gutterWidth}px minmax(0, 1fr)` }}
    >
      <div className="relative" aria-hidden="true">
        <span
          className={cn(
            "absolute top-0 w-px bg-[var(--app-border)]",
            isLast ? "h-[29px]" : "bottom-0",
          )}
          style={{ left: branchLeft }}
        />
        <span
          className="absolute top-[29px] h-px w-7 bg-[var(--app-border)]"
          style={{ left: branchLeft }}
        />
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

function buildModuleTree(modules: MoveModule[]): ModuleTreeNode[] {
  const rootNodes: ModuleTreeNode[] = [];

  for (const moveModule of modules) {
    const parts = moduleTreePathParts(moveModule);
    const moduleName = parts.at(-1)?.replace(/\.move$/i, "") || moveModule.name;
    let currentLevel = rootNodes;
    let currentPath = "";

    for (const directoryName of parts.slice(0, -1)) {
      currentPath = currentPath ? `${currentPath}/${directoryName}` : directoryName;
      let directoryNode = currentLevel.find(
        (node): node is Extract<ModuleTreeNode, { type: "directory" }> =>
          node.type === "directory" && node.name === directoryName,
      );

      if (!directoryNode) {
        directoryNode = {
          children: [],
          name: directoryName,
          path: currentPath,
          type: "directory",
        };
        currentLevel.push(directoryNode);
        sortModuleTreeNodes(currentLevel);
      }

      currentLevel = directoryNode.children;
    }

    currentLevel.push({
      module: moveModule,
      name: moduleName,
      path: moveModule.filePath,
      type: "module",
    });
    sortModuleTreeNodes(currentLevel);
  }

  return rootNodes;
}

function sortModuleTreeNodes(nodes: ModuleTreeNode[]) {
  nodes.sort((left, right) => {
    if (left.type !== right.type) {
      return left.type === "directory" ? -1 : 1;
    }

    return left.name.localeCompare(right.name) || left.path.localeCompare(right.path);
  });
}

function moduleTreePathParts(moveModule: MoveModule) {
  const normalized = moveModule.filePath.replace(/\\/g, "/");
  const sourcesIndex = normalized.lastIndexOf("/sources/");
  const pathUnderSources = sourcesIndex >= 0
    ? normalized.slice(sourcesIndex + "/sources/".length)
    : normalized.replace(/^sources\//, "");
  const parts = pathUnderSources.split("/").filter(Boolean);

  if (parts.length) {
    return parts;
  }

  return [`${moveModule.name}.move`];
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
