import React from "react";
import { createPortal } from "react-dom";
import {
  Binary,
  Boxes,
  Braces,
  ChevronDown,
  ChevronRight,
  Circle,
  Cpu,
  FileCode2,
  Filter,
  FunctionSquare,
  GitBranch,
  GitFork,
  GripVertical,
  Loader2,
  Package,
  Pause,
  Play,
  Repeat,
  X,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { CodeEditorJumpRequest } from "@/features/project-workspace/code-editor";
import {
  loadFilePreview,
  loadMoveBytecodeView,
  type FilePreview,
  type MoveBytecodeBasicBlockView,
  type MoveBytecodeCallView,
  type MoveBytecodeControlFlowEdgeView,
  type MoveBytecodeFunctionView,
  type MoveBytecodeInstructionView,
  type MoveBytecodeModuleView,
  type MoveBytecodePackageView,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

const CodeEditor = React.lazy(() =>
  import("@/features/project-workspace/code-editor").then((module) => ({
    default: module.CodeEditor,
  })),
);

type BytecodeViewScreenProps = {
  activeMovePackage: MovePackage | null;
  packageTree: PackageTree;
};

type BytecodeEditorTarget = {
  functionName: string | null;
  moduleName: string;
  sourcePath: string;
};

const BYTECODE_COLUMN_WIDTHS = [236, 470, 260, 292];
const BYTECODE_COLUMN_MIN_WIDTHS = [184, 340, 210, 236];
const BYTECODE_RESIZE_HANDLE_WIDTH = 8;
const BYTECODE_TREE_ANIMATION_MS = 160;

type BytecodeModuleGroup = {
  id: string;
  isDependency: boolean;
  label: string;
  moduleCount: number;
  modules: MoveBytecodeModuleView[];
};

type BytecodeFunctionCategoryId =
  | "entry"
  | "private"
  | "public"
  | "public-entry"
  | "public-friend"
  | "public-package"
  | "view";

type BytecodeFunctionCategory = {
  count: number;
  functions: MoveBytecodeFunctionView[];
  id: BytecodeFunctionCategoryId;
  label: string;
  tone: "entry" | "friend" | "package" | "private" | "public" | "publicEntry" | "view";
};

export function BytecodeViewScreen({
  activeMovePackage,
  packageTree,
}: BytecodeViewScreenProps) {
  const [view, setView] = React.useState<MoveBytecodePackageView | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [isLoading, setIsLoading] = React.useState(false);
  const [selectedModulePath, setSelectedModulePath] = React.useState<string | null>(null);
  const [selectedFunctionName, setSelectedFunctionName] = React.useState<string | null>(null);
  const [selectedOffset, setSelectedOffset] = React.useState<number | null>(null);
  const [columnWidths, setColumnWidths] = React.useState(BYTECODE_COLUMN_WIDTHS);
  const [resizingHandleIndex, setResizingHandleIndex] = React.useState<number | null>(null);
  const [editorTarget, setEditorTarget] = React.useState<BytecodeEditorTarget | null>(null);

  React.useEffect(() => {
    if (!activeMovePackage) {
      setView(null);
      setError(null);
      return;
    }

    let isCancelled = false;
    setIsLoading(true);
    setError(null);

    void loadMoveBytecodeView(packageTree, activeMovePackage)
      .then((bytecodeView) => {
        if (isCancelled) {
          return;
        }

        const firstModule = bytecodeView.modules[0] ?? null;
        const firstFunction = firstModule?.functions[0] ?? null;
        setView(bytecodeView);
        setSelectedModulePath((current) =>
          current && bytecodeView.modules.some((module) => module.bytecodePath === current)
            ? current
            : firstModule?.bytecodePath ?? null,
        );
        setSelectedFunctionName((current) =>
          current && bytecodeView.modules
            .find((module) => module.bytecodePath === (selectedModulePath ?? firstModule?.bytecodePath))
            ?.functions.some((fn) => fn.name === current)
            ? current
            : firstFunction?.name ?? null,
        );
        setSelectedOffset(firstFunction?.instructions[0]?.offset ?? null);
      })
      .catch((error) => {
        if (isCancelled) {
          return;
        }

        setView(null);
        setError(error instanceof Error ? error.message : String(error));
      })
      .finally(() => {
        if (!isCancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      isCancelled = true;
    };
  }, [activeMovePackage, packageTree]);

  React.useEffect(() => {
    setView(null);
    setError(null);
    setSelectedModulePath(null);
    setSelectedFunctionName(null);
    setSelectedOffset(null);
    setEditorTarget(null);
  }, [activeMovePackage?.manifestPath, packageTree.rootPath]);

  const selectedModule = view?.modules.find((module) => module.bytecodePath === selectedModulePath)
    ?? view?.modules[0]
    ?? null;
  const selectedFunction = selectedModule?.functions.find((fn) => fn.name === selectedFunctionName)
    ?? selectedModule?.functions[0]
    ?? null;
  const selectedInstruction = selectedFunction?.instructions.find(
    (instruction) => instruction.offset === selectedOffset,
  ) ?? selectedFunction?.instructions[0] ?? null;
  const blocks = React.useMemo(
    () => selectedFunction?.controlFlow.blocks ?? [],
    [selectedFunction],
  );
  const edges = React.useMemo(
    () => selectedFunction?.controlFlow.edges ?? [],
    [selectedFunction],
  );
  const selectedBlock = selectedInstruction
    ? blocks.find((block) =>
        selectedInstruction.offset >= block.startOffset && selectedInstruction.offset <= block.endOffset,
      ) ?? null
    : null;
  const openCallTarget = React.useCallback((call: MoveBytecodeCallView) => {
    const target = findBytecodeCallTarget(view, call);

    if (!target) {
      return;
    }

    setEditorTarget(null);
    setSelectedModulePath(target.module.bytecodePath);
    setSelectedFunctionName(target.fn.name);
    setSelectedOffset(target.fn.instructions[0]?.offset ?? null);
  }, [view]);
  const isEditorOpen = Boolean(editorTarget);
  const gridTemplateColumns = React.useMemo(
    () =>
      isEditorOpen
        ? `${columnWidths[0]}px ${BYTECODE_RESIZE_HANDLE_WIDTH}px minmax(0,1fr)`
        : [
            `${columnWidths[0]}px`,
            `${BYTECODE_RESIZE_HANDLE_WIDTH}px`,
            `${columnWidths[1]}px`,
            `${BYTECODE_RESIZE_HANDLE_WIDTH}px`,
            `${columnWidths[2]}px`,
            `${BYTECODE_RESIZE_HANDLE_WIDTH}px`,
            `${columnWidths[3]}px`,
          ].join(" "),
    [columnWidths, isEditorOpen],
  );
  const gridMinWidth = React.useMemo(
    () =>
      isEditorOpen
        ? columnWidths[0] + BYTECODE_RESIZE_HANDLE_WIDTH + 420
        : columnWidths.reduce((sum, width) => sum + width, 0) + BYTECODE_RESIZE_HANDLE_WIDTH * 3,
    [columnWidths, isEditorOpen],
  );
  const handleColumnResizeStart = React.useCallback((
    handleIndex: number,
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    event.preventDefault();
    event.stopPropagation();

    const startX = event.clientX;
    const startWidths = [...columnWidths];
    const pairTotal = startWidths[handleIndex] + startWidths[handleIndex + 1];
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    setResizingHandleIndex(handleIndex);

    const handleMove = (moveEvent: PointerEvent) => {
      const delta = moveEvent.clientX - startX;
      let left = startWidths[handleIndex] + delta;
      let right = startWidths[handleIndex + 1] - delta;
      const leftMin = BYTECODE_COLUMN_MIN_WIDTHS[handleIndex];
      const rightMin = BYTECODE_COLUMN_MIN_WIDTHS[handleIndex + 1];

      if (left < leftMin) {
        left = leftMin;
        right = pairTotal - left;
      }

      if (right < rightMin) {
        right = rightMin;
        left = pairTotal - right;
      }

      setColumnWidths((current) =>
        current.map((width, index) => {
          if (index === handleIndex) {
            return left;
          }

          if (index === handleIndex + 1) {
            return right;
          }

          return width;
        }),
      );
    };

    const handleEnd = () => {
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleEnd);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      setResizingHandleIndex(null);
    };

    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleEnd, { once: true });
  }, [columnWidths]);

  if (!activeMovePackage) {
    return (
      <div className="grid h-full min-h-0 place-items-center bg-[var(--app-window)] text-sm text-muted-foreground">
        Select a Move package to inspect bytecode.
      </div>
    );
  }

  return (
    <div className="grid h-full min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)] text-foreground">
      <BytecodeHeader
        activeMovePackage={activeMovePackage}
        selectedFunction={selectedFunction}
        selectedModule={selectedModule}
      />

      {error ? (
        <div className="grid min-h-0 place-items-center px-6">
          <div className="max-w-xl rounded-md border border-red-500/25 bg-red-500/10 p-4 text-sm text-red-100">
            <div className="font-semibold">Bytecode unavailable</div>
            <p className="mt-2 text-xs leading-5 text-red-100/80">{error}</p>
          </div>
        </div>
      ) : (
        <div
          className={cn(
            "min-h-0 min-w-0 overflow-x-auto overflow-y-hidden",
            resizingHandleIndex !== null && "cursor-col-resize select-none",
          )}
        >
          <div
            className="grid h-full min-h-[36rem] min-w-0"
            style={{
              gridTemplateColumns,
              minWidth: gridMinWidth,
            }}
          >
            <BytecodeExplorer
              isLoading={isLoading}
              onOpenInEditor={(module, fn) => {
                if (!module.sourcePath) {
                  return;
                }

                setEditorTarget({
                  functionName: fn?.name ?? null,
                  moduleName: module.name,
                  sourcePath: module.sourcePath,
                });
              }}
              selectedFunctionName={selectedFunction?.name ?? null}
              selectedModulePath={selectedModule?.bytecodePath ?? null}
              view={view}
              onSelectFunction={(modulePath, functionName) => {
                const module = view?.modules.find((item) => item.bytecodePath === modulePath);
                const fn = module?.functions.find((item) => item.name === functionName);

                setSelectedModulePath(modulePath);
                setSelectedFunctionName(functionName);
                setSelectedOffset(fn?.instructions[0]?.offset ?? null);
              }}
              onSelectModule={(modulePath) => {
                const module = view?.modules.find((item) => item.bytecodePath === modulePath);

                setSelectedModulePath(modulePath);
                setSelectedFunctionName(module?.functions[0]?.name ?? null);
                setSelectedOffset(module?.functions[0]?.instructions[0]?.offset ?? null);
              }}
            />
            <ColumnResizeHandle
              active={resizingHandleIndex === 0}
              index={0}
              onPointerDown={handleColumnResizeStart}
            />

            {editorTarget ? (
              <BytecodeSourceEditorPanel
                packageTree={packageTree}
                target={editorTarget}
                onClose={() => setEditorTarget(null)}
              />
            ) : (
              <>
                <InstructionPanel
                  onOpenCallTarget={openCallTarget}
                  selectedInstruction={selectedInstruction}
                  selectedModule={selectedModule}
                  selectedFunction={selectedFunction}
                  view={view}
                  onSelectInstruction={(instruction) => setSelectedOffset(instruction.offset)}
                />
                <ColumnResizeHandle
                  active={resizingHandleIndex === 1}
                  index={1}
                  onPointerDown={handleColumnResizeStart}
                />

                <ControlFlowPanel
                  blocks={blocks}
                  edges={edges}
                  selectedBlock={selectedBlock}
                  selectedInstruction={selectedInstruction}
                  onSelectBlock={(block) => setSelectedOffset(block.startOffset)}
                />
                <ColumnResizeHandle
                  active={resizingHandleIndex === 2}
                  index={2}
                  onPointerDown={handleColumnResizeStart}
                />

                <ExplanationPanel
                  block={selectedBlock}
                  instruction={selectedInstruction}
                  moveFunction={selectedFunction}
                  module={selectedModule}
                  onOpenCallTarget={openCallTarget}
                  view={view}
                />
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function BytecodeHeader({
  activeMovePackage,
  selectedFunction,
  selectedModule,
}: {
  activeMovePackage: MovePackage;
  selectedFunction: MoveBytecodeFunctionView | null;
  selectedModule: MoveBytecodeModuleView | null;
}) {
  return (
    <header className="flex min-w-0 items-center border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] px-4 py-2">
      <div className="flex min-w-0 items-center gap-2 text-sm">
        <Package className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span className="truncate font-semibold">{activeMovePackage.name}</span>
        <Chevron className="text-muted-foreground" />
        <span className="truncate text-muted-foreground">{selectedModule?.name ?? "module"}</span>
        <Chevron className="text-muted-foreground" />
        <span className="truncate text-muted-foreground">{selectedFunction?.name ?? "function"}</span>
        {selectedFunction ? (
          <>
            <Chevron className="text-muted-foreground" />
            <Badge className="rounded bg-primary/15 px-1.5 py-0.5 text-[11px] font-semibold text-primary" variant="secondary">
              {selectedFunction.isEntry ? "ENTRY" : selectedFunction.visibility.toUpperCase()}
            </Badge>
          </>
        ) : null}
      </div>
    </header>
  );
}

function BytecodeExplorer({
  isLoading,
  onOpenInEditor,
  onSelectFunction,
  onSelectModule,
  selectedFunctionName,
  selectedModulePath,
  view,
}: {
  isLoading: boolean;
  onOpenInEditor: (module: MoveBytecodeModuleView, fn: MoveBytecodeFunctionView | null) => void;
  onSelectFunction: (modulePath: string, functionName: string) => void;
  onSelectModule: (modulePath: string) => void;
  selectedFunctionName: string | null;
  selectedModulePath: string | null;
  view: MoveBytecodePackageView | null;
}) {
  const [branchOnly, setBranchOnly] = React.useState(false);
  const [callsOnly, setCallsOnly] = React.useState(false);
  const [packageOnly, setPackageOnly] = React.useState(false);
  const moduleGroups = React.useMemo(
    () => groupBytecodeModules(view, { branchOnly, callsOnly, packageOnly }),
    [branchOnly, callsOnly, packageOnly, view],
  );
  const branchFunctionCount = React.useMemo(
    () => view?.modules.reduce(
      (total, module) => total + module.functions.filter(hasBranchControlFlow).length,
      0,
    ) ?? 0,
    [view],
  );
  const callFunctionCount = React.useMemo(
    () => view?.modules.reduce(
      (total, module) => total + module.functions.filter(hasBytecodeCall).length,
      0,
    ) ?? 0,
    [view],
  );
  const localModuleCount = React.useMemo(
    () => view?.modules.filter((module) => !module.isDependency).length ?? 0,
    [view],
  );
  const activeFilterCount = Number(branchOnly) + Number(callsOnly) + Number(packageOnly);
  const [expandedGroups, setExpandedGroups] = React.useState<Set<string>>(() => new Set());
  const [expandedModules, setExpandedModules] = React.useState<Set<string>>(() => new Set());
  const [collapsedFunctionGroups, setCollapsedFunctionGroups] = React.useState<Set<string>>(() => new Set());

  React.useEffect(() => {
    if (!view) {
      setExpandedGroups(new Set());
      setExpandedModules(new Set());
      setCollapsedFunctionGroups(new Set());
      return;
    }

    setExpandedGroups((current) => {
      const next = new Set(current);
      const packageGroup = moduleGroups.find((group) => !group.isDependency) ?? moduleGroups[0];

      if (packageGroup) {
        next.add(packageGroup.id);
      }

      const selectedGroup = moduleGroups.find((group) =>
        group.modules.some((module) => module.bytecodePath === selectedModulePath),
      );

      if (selectedGroup) {
        next.add(selectedGroup.id);
      }

      return next;
    });
  }, [moduleGroups, selectedModulePath, view]);

  React.useEffect(() => {
    if (!selectedModulePath) {
      return;
    }

    setExpandedModules((current) => {
      const next = new Set(current);
      next.add(selectedModulePath);
      return next;
    });
  }, [selectedModulePath]);

  const toggleGroup = React.useCallback((groupId: string) => {
    setExpandedGroups((current) => {
      const next = new Set(current);

      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }

      return next;
    });
  }, []);

  const toggleModule = React.useCallback((modulePath: string) => {
    setExpandedModules((current) => {
      const next = new Set(current);

      if (next.has(modulePath)) {
        next.delete(modulePath);
      } else {
        next.add(modulePath);
      }

      return next;
    });
  }, []);

  const toggleFunctionGroup = React.useCallback((groupKey: string) => {
    setCollapsedFunctionGroups((current) => {
      const next = new Set(current);

      if (next.has(groupKey)) {
        next.delete(groupKey);
      } else {
        next.add(groupKey);
      }

      return next;
    });
  }, []);

  return (
    <aside className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-panel)]">
      <div className="min-w-0 border-b border-[color:var(--app-border)] px-3 py-3">
        <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground">
          <Binary className="size-3.5" aria-hidden="true" />
          Bytecode Workspace
        </div>
        {view ? (
          <div className="mt-3 grid grid-cols-2 gap-2">
            <MiniMetric icon={FileCode2} label="Modules" value={view.moduleCount} />
            <MiniMetric icon={Cpu} label="Ops" value={view.instructionCount} />
            <MiniMetric icon={Braces} label="Functions" value={view.functionCount} />
            <MiniMetric icon={Boxes} label="Structs" value={view.structCount} />
          </div>
        ) : null}
        {view ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                className={cn(
                  "mt-3 flex h-8 w-full min-w-0 items-center gap-2 rounded border border-[color:var(--app-border)] bg-black/10 px-2 text-left text-xs font-medium text-muted-foreground transition-colors hover:bg-[var(--app-subtle)] hover:text-foreground",
                  activeFilterCount > 0 && "border-sky-400/40 bg-sky-500/10 text-sky-200",
                )}
                type="button"
              >
                <Filter className="size-3.5 shrink-0" aria-hidden="true" />
                <span className="min-w-0 flex-1 truncate">Filters</span>
                {activeFilterCount > 0 ? (
                  <span className="shrink-0 rounded bg-sky-500/20 px-1.5 py-0.5 font-mono text-[10px] text-sky-200">
                    {activeFilterCount}
                  </span>
                ) : null}
                <ChevronDown className="size-3 shrink-0" aria-hidden="true" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-64 border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground">
              <DropdownMenuLabel className="px-2 py-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                Bytecode filters
              </DropdownMenuLabel>
              <DropdownMenuSeparator className="bg-[var(--app-border)]" />
              <DropdownMenuCheckboxItem
                checked={branchOnly}
                className="py-2 text-xs"
                onCheckedChange={(checked) => setBranchOnly(checked === true)}
              >
                <GitFork className="size-3.5 text-sky-300" aria-hidden="true" />
                <span className="min-w-0 flex-1 truncate">Branch functions</span>
                <span className="font-mono text-[10px] text-muted-foreground">{branchFunctionCount}</span>
              </DropdownMenuCheckboxItem>
              <DropdownMenuCheckboxItem
                checked={callsOnly}
                className="py-2 text-xs"
                onCheckedChange={(checked) => setCallsOnly(checked === true)}
              >
                <FunctionSquare className="size-3.5 text-violet-300" aria-hidden="true" />
                <span className="min-w-0 flex-1 truncate">Call functions</span>
                <span className="font-mono text-[10px] text-muted-foreground">{callFunctionCount}</span>
              </DropdownMenuCheckboxItem>
              <DropdownMenuCheckboxItem
                checked={packageOnly}
                className="py-2 text-xs"
                onCheckedChange={(checked) => setPackageOnly(checked === true)}
              >
                <Package className="size-3.5 text-primary" aria-hidden="true" />
                <span className="min-w-0 flex-1 truncate">Active package only</span>
                <span className="font-mono text-[10px] text-muted-foreground">{localModuleCount}</span>
              </DropdownMenuCheckboxItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
      </div>

      <ScrollArea className="min-h-0 min-w-0">
        <div className="min-w-0 overflow-hidden p-2">
          {isLoading ? (
            <div className="flex items-center gap-2 px-2 py-3 text-xs text-muted-foreground">
              <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
              Loading compiled modules...
            </div>
          ) : null}

          {moduleGroups.map((group) => {
            const isExpandedGroup = expandedGroups.has(group.id);

            return (
              <div className="mb-2 min-w-0 overflow-hidden" key={group.id}>
                <button
                  className={cn(
                    "flex h-7 w-full min-w-0 max-w-full items-center gap-2 overflow-hidden rounded px-2 text-left text-[11px] font-semibold uppercase tracking-wide text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
                    isExpandedGroup && "text-foreground",
                  )}
                  aria-expanded={isExpandedGroup}
                  onClick={() => toggleGroup(group.id)}
                  type="button"
                >
                  <ChevronRight
                    className={cn("size-3 shrink-0 transition-transform duration-150 ease-out", isExpandedGroup && "rotate-90")}
                    aria-hidden="true"
                  />
                  <Package className="size-3.5 shrink-0" aria-hidden="true" />
                  <span className="min-w-0 flex-1 truncate">{group.label}</span>
                  <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                    {group.moduleCount}
                  </span>
                </button>

                <CollapsibleTreeBody isOpen={isExpandedGroup}>
                  <div className="ml-3 min-w-0 overflow-hidden border-l border-[color:var(--app-border)] py-1 pl-2">
                    {group.modules.map((module) => {
                      const isSelectedModule = module.bytecodePath === selectedModulePath;
                      const isExpandedModule = expandedModules.has(module.bytecodePath);

                      return (
                        <div className="mb-1 min-w-0 overflow-hidden" key={module.bytecodePath}>
                          <BytecodeExplorerContextMenu
                            module={module}
                            onOpenInEditor={onOpenInEditor}
                          >
                            <button
                              className={cn(
                                "flex h-8 w-full min-w-0 max-w-full items-center gap-2 overflow-hidden rounded px-2 text-left text-xs hover:bg-[var(--app-subtle)]",
                                isSelectedModule && "bg-[var(--app-subtle)] text-foreground",
                              )}
                              aria-expanded={isExpandedModule}
                              onClick={() => {
                                if (isSelectedModule) {
                                  toggleModule(module.bytecodePath);
                                  return;
                                }

                                setExpandedModules((current) => {
                                  const next = new Set(current);
                                  next.add(module.bytecodePath);
                                  return next;
                                });
                                onSelectModule(module.bytecodePath);
                              }}
                              type="button"
                            >
                              <ChevronRight
                                className={cn("size-3 shrink-0 text-muted-foreground transition-transform duration-150 ease-out", isExpandedModule && "rotate-90")}
                                aria-hidden="true"
                              />
                              <FileCode2 className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                              <span className="min-w-0 flex-1 truncate font-semibold">{module.name}</span>
                              <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                                {module.functionCount}
                              </span>
                            </button>
                          </BytecodeExplorerContextMenu>

                          <CollapsibleTreeBody isOpen={isExpandedModule}>
                            <div className="ml-6 min-w-0 overflow-hidden border-l border-[color:var(--app-border)] py-1 pl-2">
                              {groupBytecodeFunctions(module.functions).map((functionGroup) => {
                                const groupKey = `${module.bytecodePath}:${functionGroup.id}`;
                                const isFunctionGroupOpen = !collapsedFunctionGroups.has(groupKey);

                                return (
                                  <div className="mb-1 min-w-0 overflow-hidden" key={groupKey}>
                                    <button
                                      className={cn(
                                        "flex h-7 w-full min-w-0 max-w-full items-center gap-2 overflow-hidden rounded px-2 text-left text-[11px] font-semibold uppercase tracking-wide hover:bg-[var(--app-subtle)]",
                                        functionGroupToneTextClass(functionGroup.tone),
                                      )}
                                      aria-expanded={isFunctionGroupOpen}
                                      onClick={() => toggleFunctionGroup(groupKey)}
                                      type="button"
                                    >
                                      <ChevronRight
                                        className={cn(
                                          "size-3 shrink-0 transition-transform duration-150 ease-out",
                                          isFunctionGroupOpen && "rotate-90",
                                        )}
                                        aria-hidden="true"
                                      />
                                      <span
                                        className={cn(
                                          "size-1.5 shrink-0 rounded-full",
                                          functionGroupToneDotClass(functionGroup.tone),
                                        )}
                                      />
                                      <span className="min-w-0 flex-1 truncate">
                                        {functionGroup.label}
                                      </span>
                                      <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                                        {functionGroup.count}
                                      </span>
                                    </button>
                                    <CollapsibleTreeBody isOpen={isFunctionGroupOpen}>
                                      <div className="ml-5 min-w-0 overflow-hidden border-l border-[color:var(--app-border)] py-1 pl-2">
                                        {functionGroup.functions.map((fn) => {
                                          const isSelectedFunction = isSelectedModule && fn.name === selectedFunctionName;

                                          return (
                                            <BytecodeExplorerContextMenu
                                              fn={fn}
                                              key={`${module.bytecodePath}:${fn.name}`}
                                              module={module}
                                              onOpenInEditor={onOpenInEditor}
                                            >
                                              <div className="group/callgraph relative min-w-0">
                                                <button
                                                  className={cn(
                                                    "flex h-7 w-full min-w-0 max-w-full items-center gap-2 overflow-hidden rounded px-2 text-left text-xs text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
                                                    isSelectedFunction && "bg-primary/15 text-foreground",
                                                  )}
                                                  onClick={() => onSelectFunction(module.bytecodePath, fn.name)}
                                                  type="button"
                                                >
                                                  <FunctionSquare
                                                    className={cn(
                                                      "size-3 shrink-0",
                                                      functionGroupToneTextClass(functionGroup.tone),
                                                    )}
                                                    aria-hidden="true"
                                                  />
                                                  <span className="min-w-0 flex-1 truncate">{fn.name}</span>
                                                  <span className="flex shrink-0 items-center gap-1">
                                                    <span
                                                      className={cn(
                                                        "rounded px-1 text-[10px] font-semibold",
                                                        functionGroupToneBadgeClass(functionGroup.tone),
                                                      )}
                                                    >
                                                      {functionBadgeLabel(fn, functionGroup.id)}
                                                    </span>
                                                  </span>
                                                </button>
                                                <FunctionCallGraphHover
                                                  fn={fn}
                                                  module={module}
                                                  view={view}
                                                  onSelectFunction={onSelectFunction}
                                                />
                                              </div>
                                            </BytecodeExplorerContextMenu>
                                          );
                                        })}
                                      </div>
                                    </CollapsibleTreeBody>
                                  </div>
                                );
                              })}
                            </div>
                          </CollapsibleTreeBody>
                        </div>
                      );
                    })}
                  </div>
                </CollapsibleTreeBody>
              </div>
            );
          })}
          {!isLoading && view && activeFilterCount > 0 && !moduleGroups.length ? (
            <div className="px-3 py-6 text-xs leading-5 text-muted-foreground">
              No bytecode functions match the active filters.
            </div>
          ) : null}
        </div>
      </ScrollArea>
    </aside>
  );
}

function BytecodeExplorerContextMenu({
  children,
  fn = null,
  module,
  onOpenInEditor,
}: {
  children: React.ReactElement;
  fn?: MoveBytecodeFunctionView | null;
  module: MoveBytecodeModuleView;
  onOpenInEditor: (module: MoveBytecodeModuleView, fn: MoveBytecodeFunctionView | null) => void;
}) {
  const canOpenInEditor = Boolean(module.sourcePath);

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-44 border-[color:var(--app-border)] bg-[var(--app-panel)] text-foreground">
        <ContextMenuItem
          className="text-xs"
          disabled={!canOpenInEditor}
          onSelect={() => {
            if (!module.sourcePath) {
              return;
            }

            onOpenInEditor(module, fn);
          }}
        >
          <FileCode2 className="size-3.5" aria-hidden="true" />
          Open in editor
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function BytecodeSourceEditorPanel({
  onClose,
  packageTree,
  target,
}: {
  onClose: () => void;
  packageTree: PackageTree;
  target: BytecodeEditorTarget;
}) {
  const [preview, setPreview] = React.useState<FilePreview | null>(null);
  const [source, setSource] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [isLoading, setIsLoading] = React.useState(false);
  const [jumpRequest, setJumpRequest] = React.useState<CodeEditorJumpRequest | null>(null);

  React.useEffect(() => {
    let isCancelled = false;
    setPreview(null);
    setSource("");
    setError(null);
    setIsLoading(true);
    setJumpRequest(null);

    loadFilePreview(packageTree, target.sourcePath)
      .then((nextPreview) => {
        if (isCancelled) {
          return;
        }

        setPreview(nextPreview);

        if (nextPreview.kind !== "text") {
          setError("This source file cannot be opened in the editor.");
          return;
        }

        setSource(nextPreview.source);
        setJumpRequest({
          line: target.functionName ? findMoveFunctionLine(nextPreview.source, target.functionName) : 1,
          token: Date.now(),
        });
      })
      .catch((reason: unknown) => {
        if (!isCancelled) {
          setError(reason instanceof Error ? reason.message : "Could not open this source file.");
        }
      })
      .finally(() => {
        if (!isCancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      isCancelled = true;
    };
  }, [packageTree, target.functionName, target.sourcePath]);

  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <header className="flex min-h-11 min-w-0 items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3">
        <div className="flex min-w-0 items-center gap-2">
          <FileCode2 className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <div className="min-w-0">
            <div className="truncate text-sm font-semibold">
              {target.functionName ? `${target.moduleName}::${target.functionName}` : target.moduleName}
            </div>
            <div className="truncate text-[11px] text-muted-foreground">{target.sourcePath}</div>
          </div>
        </div>
        <button
          aria-label="Close source editor"
          className="inline-flex size-8 shrink-0 items-center justify-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
          onClick={onClose}
          type="button"
        >
          <X className="size-4" aria-hidden="true" />
        </button>
      </header>

      {error ? (
        <div className="grid min-h-0 place-items-center px-6">
          <div className="max-w-lg rounded-md border border-red-500/25 bg-red-500/10 p-4 text-sm text-red-100">
            {error}
          </div>
        </div>
      ) : isLoading ? (
        <div className="flex min-h-0 items-center justify-center gap-2 px-6 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          Loading source...
        </div>
      ) : preview?.kind === "text" ? (
        <React.Suspense
          fallback={
            <div className="flex min-h-0 items-center justify-center px-6 text-sm text-muted-foreground">
              Loading editor...
            </div>
          }
        >
          <CodeEditor
            jumpRequest={jumpRequest}
            key={target.sourcePath}
            language={preview.language || "move"}
            value={source}
            onChange={setSource}
          />
        </React.Suspense>
      ) : (
        <div className="grid min-h-0 place-items-center px-6 text-sm text-muted-foreground">
          No source preview available.
        </div>
      )}
    </section>
  );
}

function FunctionCallGraphHover({
  fn,
  module,
  onSelectFunction,
  view,
}: {
  fn: MoveBytecodeFunctionView;
  module: MoveBytecodeModuleView;
  onSelectFunction: (modulePath: string, functionName: string) => void;
  view: MoveBytecodePackageView | null;
}) {
  const calls = React.useMemo(() => functionCallSummaries(view, fn), [fn, view]);
  const anchorRef = React.useRef<HTMLSpanElement | null>(null);
  const closeTimerRef = React.useRef<number | null>(null);
  const openTimerRef = React.useRef<number | null>(null);
  const [position, setPosition] = React.useState<{ left: number; top: number } | null>(null);

  const clearOpenTimer = React.useCallback(() => {
    if (openTimerRef.current !== null) {
      window.clearTimeout(openTimerRef.current);
      openTimerRef.current = null;
    }
  }, []);

  const clearCloseTimer = React.useCallback(() => {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
  }, []);

  const scheduleClose = React.useCallback(() => {
    clearOpenTimer();
    clearCloseTimer();
    closeTimerRef.current = window.setTimeout(() => setPosition(null), 120);
  }, [clearCloseTimer, clearOpenTimer]);

  React.useEffect(() => {
    const anchor = anchorRef.current;
    const row = anchor?.parentElement;

    if (!row) {
      return;
    }

    const scheduleOpen = () => {
      clearOpenTimer();
      clearCloseTimer();
      openTimerRef.current = window.setTimeout(() => {
        const rect = row.getBoundingClientRect();
        const cardWidth = 448;
        const cardHeight = 360;
        setPosition({
          left: Math.max(8, Math.min(rect.right + 8, window.innerWidth - cardWidth - 8)),
          top: Math.max(8, Math.min(rect.top, window.innerHeight - cardHeight - 8)),
        });
      }, 180);
    };

    row.addEventListener("mouseenter", scheduleOpen);
    row.addEventListener("mouseleave", scheduleClose);

    return () => {
      row.removeEventListener("mouseenter", scheduleOpen);
      row.removeEventListener("mouseleave", scheduleClose);
      clearOpenTimer();
      clearCloseTimer();
    };
  }, [clearCloseTimer, clearOpenTimer, scheduleClose]);

  return (
    <>
      <span ref={anchorRef} className="hidden" />
      {position
        ? createPortal(
          <div
            className="fixed z-[9999] w-[28rem] rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-3 text-left shadow-2xl"
            onMouseEnter={clearCloseTimer}
            onMouseLeave={scheduleClose}
            style={{ left: position.left, top: position.top }}
          >
            <div className="mb-2 min-w-0">
              <div className="truncate text-xs font-semibold text-foreground">{fn.name}</div>
              <div className="truncate text-[11px] text-muted-foreground">
                {shortAddress(module.address)}::{module.name} bytecode interactions
              </div>
            </div>
            {calls.length ? (
              <div className="max-h-80 space-y-2 overflow-y-auto pr-1">
                {calls.map((call) => (
                  <button
                    className="grid w-full gap-2 rounded border border-[color:var(--app-border)] bg-black/10 p-2 text-left transition hover:border-primary/40 hover:bg-primary/10"
                    key={call.key}
                    onClick={(event) => {
                      event.stopPropagation();

                      if (call.target) {
                        onSelectFunction(call.target.module.bytecodePath, call.target.fn.name);
                      }
                    }}
                    type="button"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <FunctionSquare className="size-3.5 shrink-0 text-primary" aria-hidden="true" />
                      <span className="min-w-0 flex-1 truncate text-xs font-semibold text-foreground">
                        {call.functionName}
                      </span>
                      <span className="shrink-0 rounded bg-primary/10 px-1.5 py-0.5 text-[10px] font-semibold text-primary">
                        {call.kind}
                      </span>
                      <span className="shrink-0 rounded bg-white/5 px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground">
                        {call.visibility}
                      </span>
                    </div>
                    <div className="grid grid-cols-[5rem_minmax(0,1fr)] gap-x-2 gap-y-1 text-[11px] text-muted-foreground">
                      <span>Module</span>
                      <span className="truncate text-foreground">
                        {call.moduleLabel}
                      </span>
                      <span>Types</span>
                      <span className="truncate">{call.genericTypeArguments}</span>
                    </div>
                    <div className="flex flex-wrap gap-1">
                      <CallFact label="state" value={call.mutatesState} />
                      <CallFact label="abort" value={call.canAbort} />
                      <CallFact label="transfer" value={call.transfersAssets} />
                      <CallFact label="event" value={call.emitsEvents} />
                      <CallFact label="cap/signer" value={call.usesCapabilitiesOrSigner} />
                    </div>
                  </button>
                ))}
              </div>
            ) : (
              <div className="rounded border border-[color:var(--app-border)] bg-black/10 px-3 py-2 text-xs text-muted-foreground">
                No resolved calls or notable bytecode effects in this function.
              </div>
            )}
          </div>,
          document.body,
        )
        : null}
    </>
  );
}

function CallFact({ label, value }: { label: string; value: boolean }) {
  return (
    <span
      className={cn(
        "rounded px-1.5 py-0.5 text-[10px] font-semibold",
        value ? "bg-amber-500/15 text-amber-200" : "bg-white/5 text-muted-foreground",
      )}
    >
      {label}: {value ? "yes" : "no"}
    </span>
  );
}

function InstructionPanel({
  onOpenCallTarget,
  onSelectInstruction,
  selectedFunction,
  selectedInstruction,
  selectedModule,
  view,
}: {
  onOpenCallTarget: (call: MoveBytecodeCallView) => void;
  onSelectInstruction: (instruction: MoveBytecodeInstructionView) => void;
  selectedFunction: MoveBytecodeFunctionView | null;
  selectedInstruction: MoveBytecodeInstructionView | null;
  selectedModule: MoveBytecodeModuleView | null;
  view: MoveBytecodePackageView | null;
}) {
  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <PanelHeader
        icon={Cpu}
        title="Instructions"
        subtitle={
          selectedModule && selectedFunction
            ? `${shortAddress(selectedModule.address)}::${selectedModule.name}::${selectedFunction.name}`
            : "Compiled bytecode"
        }
      />

      <ScrollArea className="min-h-0 min-w-0">
        <div className="max-w-full min-w-0 overflow-x-auto overflow-y-hidden">
          <table className="w-full min-w-[34rem] table-fixed border-collapse font-mono text-xs">
          <thead className="sticky top-0 z-10 bg-[var(--app-window)] text-[11px] text-muted-foreground shadow-[0_1px_0_var(--app-border)]">
            <tr className="text-left">
              <th className="w-12 px-3 py-2 font-semibold">#</th>
              <th className="w-14 px-2 py-2 font-semibold">Offset</th>
              <th className="w-48 px-2 py-2 font-semibold">Opcode</th>
              <th className="px-2 py-2 font-semibold">Operands</th>
              <th className="w-28 px-3 py-2 text-right font-semibold">Source</th>
            </tr>
          </thead>
          <tbody>
            {selectedFunction?.instructions.map((instruction, index) => {
              const isSelected = selectedInstruction?.offset === instruction.offset;
              const callTarget = instruction.call ? findBytecodeCallTarget(view, instruction.call) : null;

              return (
                <tr
                  className={cn(
                    "cursor-default border-b border-transparent text-muted-foreground hover:bg-[var(--app-subtle)]",
                    isSelected && "bg-primary/20 text-foreground hover:bg-primary/20",
                  )}
                  key={`${instruction.offset}:${instruction.detail}`}
                  onClick={() => onSelectInstruction(instruction)}
                >
                  <td className="px-3 py-1.5 text-[11px] text-muted-foreground">{index.toString().padStart(4, "0")}</td>
                  <td className="px-2 py-1.5">{formatOffset(instruction.offset)}</td>
                  <td className="truncate px-2 py-1.5 font-semibold text-foreground" title={formatOpcode(instruction.opcode)}>
                    {formatOpcode(instruction.opcode)}
                  </td>
                  <td className="truncate px-2 py-1.5" title={instruction.call?.qualifiedName ?? operandText(instruction)}>
                    {instruction.call ? (
                      <button
                        className="max-w-full truncate text-left text-sky-200 underline-offset-2 hover:underline disabled:text-muted-foreground disabled:no-underline"
                        disabled={!callTarget}
                        onClick={(event) => {
                          event.stopPropagation();
                          onOpenCallTarget(instruction.call!);
                        }}
                        type="button"
                      >
                        {callLabel(instruction.call)}
                      </button>
                    ) : (
                      operandText(instruction)
                    )}
                  </td>
                  <td className="px-3 py-1.5 text-right text-muted-foreground">
                    {formatSourceBytes(instruction)}
                  </td>
                </tr>
              );
            })}
          </tbody>
          </table>
        </div>
      </ScrollArea>
    </section>
  );
}

function CollapsibleTreeBody({
  children,
  isOpen,
}: {
  children: React.ReactNode;
  isOpen: boolean;
}) {
  const contentRef = React.useRef<HTMLDivElement>(null);
  const isInitialRenderRef = React.useRef(true);
  const [height, setHeight] = React.useState(isOpen ? "auto" : "0px");
  const [isVisible, setIsVisible] = React.useState(isOpen);
  const [shouldRender, setShouldRender] = React.useState(isOpen);

  React.useEffect(() => {
    isInitialRenderRef.current = false;
  }, []);

  React.useLayoutEffect(() => {
    if (isOpen) {
      setShouldRender(true);
    }
  }, [isOpen]);

  React.useLayoutEffect(() => {
    const node = contentRef.current;

    if (!node || !shouldRender) {
      return;
    }

    const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    if (isInitialRenderRef.current || prefersReducedMotion) {
      setHeight(isOpen ? "auto" : "0px");
      setIsVisible(isOpen);

      if (!isOpen) {
        setShouldRender(false);
      }

      return;
    }

    let animationFrame = 0;
    let timeout = 0;

    if (isOpen) {
      setHeight("0px");
      setIsVisible(false);

      animationFrame = window.requestAnimationFrame(() => {
        setHeight(`${node.scrollHeight}px`);
        setIsVisible(true);
        timeout = window.setTimeout(() => setHeight("auto"), BYTECODE_TREE_ANIMATION_MS);
      });
    } else {
      setHeight(`${node.scrollHeight}px`);
      setIsVisible(true);

      animationFrame = window.requestAnimationFrame(() => {
        setHeight("0px");
        setIsVisible(false);
        timeout = window.setTimeout(() => setShouldRender(false), BYTECODE_TREE_ANIMATION_MS);
      });
    }

    return () => {
      window.cancelAnimationFrame(animationFrame);
      window.clearTimeout(timeout);
    };
  }, [isOpen, shouldRender]);

  return (
    <div
      className={cn(
        "min-w-0 overflow-hidden transition-[height] ease-[cubic-bezier(0.2,0,0,1)] will-change-[height]",
        !shouldRender && "pointer-events-none",
      )}
      aria-hidden={!isOpen}
      style={{
        height,
        transitionDuration: `${BYTECODE_TREE_ANIMATION_MS}ms`,
      }}
    >
      <div
        ref={contentRef}
        className={cn(
          "min-w-0 transition-[opacity,transform] duration-150 ease-out will-change-[opacity,transform]",
          isVisible ? "translate-y-0 opacity-100" : "-translate-y-0.5 opacity-0",
        )}
      >
        {shouldRender ? children : null}
      </div>
    </div>
  );
}

function ControlFlowPanel({
  blocks,
  edges,
  onSelectBlock,
  selectedBlock,
  selectedInstruction,
}: {
  blocks: MoveBytecodeBasicBlockView[];
  edges: MoveBytecodeControlFlowEdgeView[];
  onSelectBlock: (block: MoveBytecodeBasicBlockView) => void;
  selectedBlock: MoveBytecodeBasicBlockView | null;
  selectedInstruction: MoveBytecodeInstructionView | null;
}) {
  const paths = React.useMemo(() => controlFlowPaths(blocks, edges), [blocks, edges]);
  const animationFrames = React.useMemo(() => controlFlowAnimationFrames(paths), [paths]);
  const [isPlaying, setIsPlaying] = React.useState(false);
  const [loopPlayback, setLoopPlayback] = React.useState(false);
  const [animationFrameIndex, setAnimationFrameIndex] = React.useState(0);
  const canAnimate = animationFrames.length > 0;

  React.useEffect(() => {
    setIsPlaying(false);
    setAnimationFrameIndex(0);
  }, [animationFrames.length]);

  React.useEffect(() => {
    if (!isPlaying || !canAnimate) {
      return;
    }

    const interval = window.setInterval(() => {
      setAnimationFrameIndex((current) => {
        const next = current + 1;

        if (next < animationFrames.length) {
          return next;
        }

        if (loopPlayback) {
          return 0;
        }

        setIsPlaying(false);
        return current;
      });
    }, 520);

    return () => window.clearInterval(interval);
  }, [animationFrames.length, canAnimate, isPlaying, loopPlayback]);

  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <PanelHeader
        actions={
          <div className="flex shrink-0 items-center gap-1">
            <button
              aria-label={isPlaying ? "Pause control-flow playback" : "Play control-flow paths"}
              className={cn(
                "grid size-6 place-items-center rounded border border-[color:var(--app-border)] text-muted-foreground transition-colors hover:bg-[var(--app-subtle)] hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40",
                isPlaying && "border-sky-400/50 bg-sky-500/10 text-sky-200",
              )}
              disabled={!canAnimate}
              onClick={() => {
                if (!isPlaying && animationFrameIndex >= animationFrames.length - 1) {
                  setAnimationFrameIndex(0);
                }
                setIsPlaying((current) => !current);
              }}
              title="Play all possible paths"
              type="button"
            >
              {isPlaying ? <Pause className="size-3" aria-hidden="true" /> : <Play className="size-3" aria-hidden="true" />}
            </button>
            <button
              aria-label="Loop control-flow playback"
              aria-pressed={loopPlayback}
              className={cn(
                "grid size-6 place-items-center rounded border border-[color:var(--app-border)] text-muted-foreground transition-colors hover:bg-[var(--app-subtle)] hover:text-foreground",
                loopPlayback && "border-sky-400/50 bg-sky-500/10 text-sky-200",
              )}
              onClick={() => setLoopPlayback((current) => !current)}
              title="Loop playback"
              type="button"
            >
              <Repeat className="size-3" aria-hidden="true" />
            </button>
          </div>
        }
        icon={GitBranch}
        title="Control Flow"
        subtitle={`${blocks.length} blocks, ${edges.length} edges`}
      />
      <ScrollArea className="min-h-0 min-w-0">
        <ControlFlowGraph
          activeAnimationFrame={(isPlaying || animationFrameIndex > 0) ? (animationFrames[animationFrameIndex] ?? null) : null}
          blocks={blocks}
          edges={edges}
          selectedBlock={selectedBlock}
          selectedInstruction={selectedInstruction}
          onSelectBlock={onSelectBlock}
        />
      </ScrollArea>
    </section>
  );
}

function ControlFlowGraph({
  activeAnimationFrame,
  blocks,
  edges,
  onSelectBlock,
  selectedBlock,
  selectedInstruction,
}: {
  activeAnimationFrame: ControlFlowAnimationFrame | null;
  blocks: MoveBytecodeBasicBlockView[];
  edges: MoveBytecodeControlFlowEdgeView[];
  onSelectBlock: (block: MoveBytecodeBasicBlockView) => void;
  selectedBlock: MoveBytecodeBasicBlockView | null;
  selectedInstruction: MoveBytecodeInstructionView | null;
}) {
  const layout = React.useMemo(() => layoutControlFlow(blocks, edges), [blocks, edges]);
  const [pan, setPan] = React.useState({ x: 0, y: 0 });
  const activeBlockIds = React.useMemo(
    () => new Set(activeAnimationFrame?.blockIds ?? []),
    [activeAnimationFrame],
  );

  React.useEffect(() => {
    setPan({ x: 0, y: 0 });
  }, [layout.width, layout.height]);

  const handlePanStart = React.useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || event.target !== event.currentTarget) {
      return;
    }

    event.preventDefault();

    const startX = event.clientX;
    const startY = event.clientY;
    const startPan = pan;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "grabbing";
    document.body.style.userSelect = "none";

    const handleMove = (moveEvent: PointerEvent) => {
      setPan({
        x: startPan.x + moveEvent.clientX - startX,
        y: startPan.y + moveEvent.clientY - startY,
      });
    };

    const handleEnd = () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("pointermove", handleMove);
    };

    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleEnd, { once: true });
  }, [pan]);

  if (!blocks.length) {
    return (
      <div className="grid h-full min-h-48 place-items-center text-sm text-muted-foreground">
        Native function; no bytecode body.
      </div>
    );
  }

  return (
    <div
      className="min-w-0 cursor-grab overflow-hidden p-4 active:cursor-grabbing"
      onPointerDown={handlePanStart}
    >
      <div
        className="pointer-events-none relative"
        style={{
          height: layout.height,
          minWidth: layout.width,
          transform: `translate(${pan.x}px, ${pan.y}px)`,
        }}
      >
        <svg
          aria-hidden="true"
          className="pointer-events-none absolute inset-0"
          height={layout.height}
          width={layout.width}
        >
          <defs>
            <marker
              id="cfg-arrow"
              markerHeight="7"
              markerWidth="7"
              orient="auto"
              refX="6"
              refY="3.5"
            >
              <path d="M0,0 L7,3.5 L0,7 Z" fill="rgb(148 163 184 / 0.8)" />
            </marker>
            <marker
              id="cfg-arrow-selected"
              markerHeight="7"
              markerWidth="7"
              orient="auto"
              refX="6"
              refY="3.5"
            >
              <path d="M0,0 L7,3.5 L0,7 Z" fill="rgb(96 165 250)" />
            </marker>
          </defs>
          {layout.edges.map((edge) => {
            const isActive = edge.source === selectedBlock?.id || edge.target === selectedBlock?.id;
            const isAnimated = activeAnimationFrame?.edgeKey === controlFlowEdgeKey(edge);

            return (
              <g key={`${edge.source}:${edge.target}:${edge.kind}:${edge.sourceOffset}`}>
                <path
                  d={edge.path}
                  fill="none"
                  markerEnd={`url(#${isActive || isAnimated ? "cfg-arrow-selected" : "cfg-arrow"})`}
                  stroke={isAnimated ? "rgb(56 189 248)" : isActive ? "rgb(96 165 250)" : edgeColor(edge.kind)}
                  strokeDasharray={edge.kind === "fallthrough" ? undefined : "5 4"}
                  strokeWidth={isActive || isAnimated ? 2.4 : 1.4}
                />
              </g>
            );
          })}
          {layout.blocks.length === 1 && edges.length === 0 ? (
            <path
              d={`M ${layout.centerX} 22 L ${layout.centerX} ${layout.blocks[0].top} M ${layout.centerX} ${layout.blocks[0].bottom} L ${layout.centerX} ${layout.height - 24}`}
              fill="none"
              markerEnd="url(#cfg-arrow)"
              stroke="rgb(148 163 184 / 0.55)"
              strokeWidth={1.4}
            />
          ) : null}
        </svg>

        <Circle
          className="absolute size-3 text-muted-foreground"
          style={{ left: layout.centerX - 6, top: 10 }}
          aria-hidden="true"
        />

        {layout.edges.map((edge) => (
          <div
            className="pointer-events-none absolute flex h-[18px] min-w-10 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded bg-black/70 px-2 font-mono text-[10px] leading-[18px] text-muted-foreground"
            key={`${edge.source}:${edge.target}:${edge.kind}:${edge.sourceOffset}:label`}
            style={{
              left: edge.labelX,
              top: edge.labelY,
            }}
          >
            {edge.kind}
          </div>
        ))}

        {layout.blocks.map((item) => {
          const containsInstruction =
            selectedInstruction
            && selectedInstruction.offset >= item.block.startOffset
            && selectedInstruction.offset <= item.block.endOffset;
          const isSelected = item.block.id === selectedBlock?.id;
          const isAnimated = activeBlockIds.has(item.block.id);

          return (
            <button
              className={cn(
                "pointer-events-auto absolute rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2 text-center text-xs shadow-sm transition-colors hover:border-primary/50 hover:bg-[var(--app-subtle)]",
                (isSelected || containsInstruction) && "border-primary/70 bg-primary/15 text-foreground shadow-[0_0_0_1px_rgba(71,139,255,0.18)]",
                isAnimated && "border-sky-400/80 bg-sky-500/10 text-foreground shadow-[0_0_0_1px_rgba(56,189,248,0.18)]",
              )}
              key={item.block.id}
              onClick={() => onSelectBlock(item.block)}
              style={{
                height: item.height,
                left: item.x,
                top: item.y,
                width: item.width,
              }}
              type="button"
            >
              <div className="truncate font-semibold">{item.block.label}</div>
              <div className="mt-1 font-mono text-[11px] text-muted-foreground">
                {formatOffset(item.block.startOffset)}...{formatOffset(item.block.endOffset)}
              </div>
              <div className="mt-1 truncate text-[10px] text-muted-foreground">
                {item.block.instructionOffsets.length} ops
              </div>
            </button>
          );
        })}

        <Circle
          className="absolute size-3 text-muted-foreground"
          style={{ left: layout.centerX - 6, top: layout.height - 20 }}
          aria-hidden="true"
        />
      </div>
    </div>
  );
}

function ExplanationPanel({
  block,
  instruction,
  moveFunction,
  module,
  onOpenCallTarget,
  view,
}: {
  block: MoveBytecodeBasicBlockView | null;
  instruction: MoveBytecodeInstructionView | null;
  moveFunction: MoveBytecodeFunctionView | null;
  module: MoveBytecodeModuleView | null;
  onOpenCallTarget: (call: MoveBytecodeCallView) => void;
  view: MoveBytecodePackageView | null;
}) {
  const callSummary = instruction?.call
    ? callSummaryForInstruction(view, instruction.call)
    : null;

  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <PanelHeader
        icon={Braces}
        title="Instruction Data"
        subtitle={instruction ? `${formatOpcode(instruction.opcode)} at ${formatOffset(instruction.offset)}` : "No instruction selected"}
      />
      <ScrollArea className="min-h-0">
        <div className="divide-y divide-[color:var(--app-border)]">
          <section className="p-4">
            <h3 className="font-mono text-sm font-semibold text-foreground">
              {instruction ? `${formatOpcode(instruction.opcode)} ${operandText(instruction)}` : "Select an instruction"}
            </h3>
            <p className="mt-4 text-sm leading-6 text-muted-foreground">
              {instruction ? opcodeExplanation(instruction) : "Choose an instruction row to inspect its bytecode payload."}
            </p>
          </section>

          <ExplanationSection title="Bytecode">
            <div className="grid gap-2 text-xs text-muted-foreground">
              <ContextRow label="Offset" value={instruction ? formatOffset(instruction.offset) : "-"} />
              <ContextRow label="Opcode" value={instruction ? formatOpcode(instruction.opcode) : "-"} />
              <ContextRow label="Operands" value={instruction?.call ? callLabel(instruction.call) : instruction ? operandText(instruction) || "-" : "-"} />
              <ContextRow label="Raw" value={instruction?.detail ?? "-"} />
            </div>
          </ExplanationSection>

          {callSummary ? (
            <ExplanationSection title="Call Target">
              <div className="grid gap-3 text-xs text-muted-foreground">
                <button
                  className="flex min-w-0 items-center gap-2 rounded border border-primary/30 bg-primary/10 px-2 py-1.5 text-left text-sky-100 transition hover:bg-primary/15"
                  disabled={!callSummary.target}
                  onClick={() => instruction?.call && onOpenCallTarget(instruction.call)}
                  type="button"
                >
                  <FunctionSquare className="size-3.5 shrink-0" aria-hidden="true" />
                  <span className="min-w-0 flex-1 truncate">
                    {callSummary.moduleLabel}::{callSummary.functionName}
                  </span>
                </button>
                <div className="grid gap-2">
                  <ContextRow label="Module" value={callSummary.moduleLabel} />
                  <ContextRow label="Visibility" value={callSummary.visibility} />
                  <ContextRow label="Type args" value={callSummary.genericTypeArguments} />
                  <ContextRow label="Mutates state" value={callSummary.mutatesState ? "yes" : "no"} />
                  <ContextRow label="Can abort" value={callSummary.canAbort ? "yes" : "no"} />
                  <ContextRow label="Transfers assets" value={callSummary.transfersAssets ? "yes" : "no"} />
                  <ContextRow label="Emits events" value={callSummary.emitsEvents ? "yes" : "no"} />
                  <ContextRow label="Cap/signer" value={callSummary.usesCapabilitiesOrSigner ? "yes" : "no"} />
                </div>
              </div>
            </ExplanationSection>
          ) : null}

          <ExplanationSection title="Source Map">
            <div className="grid gap-2 text-xs text-muted-foreground">
              <ContextRow label="Bytes" value={instruction?.source ? `${instruction.source.startByte}...${instruction.source.endByte}` : "No source map entry"} />
              <ContextRow label="Map file" value={module?.sourceMapPath ?? "No source map file"} />
              <ContextRow label="Source" value={module?.sourcePath ?? "No source copy"} />
            </div>
          </ExplanationSection>

          <ExplanationSection title="Function">
            <div className="grid gap-2 text-xs text-muted-foreground">
              <ContextRow label="Name" value={moveFunction?.name ?? "-"} />
              <ContextRow label="Visibility" value={moveFunction?.visibility ?? "-"} />
              <ContextRow label="Entry" value={moveFunction?.isEntry ? "true" : "false"} />
              <ContextRow label="Locals" value={String(moveFunction?.localCount ?? 0)} />
              <ContextRow label="Acquires" value={moveFunction?.acquires.length ? moveFunction.acquires.join(", ") : "-"} />
            </div>
          </ExplanationSection>

          <ExplanationSection title="Control Flow">
            <div className="grid gap-2 text-xs text-muted-foreground">
              <ContextRow label="Block" value={block?.label ?? "-"} />
              <ContextRow label="Span" value={block ? `${formatOffset(block.startOffset)}...${formatOffset(block.endOffset)}` : "-"} />
              <ContextRow label="Ops" value={block ? String(block.instructionOffsets.length) : "-"} />
            </div>
          </ExplanationSection>

          <ExplanationSection title="Module">
            <div className="grid gap-2 text-xs text-muted-foreground">
              <ContextRow label="Module" value={module ? `${shortAddress(module.address)}::${module.name}` : "-"} />
              <ContextRow label="Version" value={module ? String(module.version) : "-"} />
              <ContextRow label="Size" value={module ? formatBytes(module.byteSize) : "-"} />
              <ContextRow label="Bytecode" value={module?.bytecodePath ?? "-"} />
            </div>
          </ExplanationSection>
        </div>
      </ScrollArea>
    </section>
  );
}

function PanelHeader({
  actions,
  icon: Icon,
  subtitle,
  title,
}: {
  actions?: React.ReactNode;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  subtitle?: string;
  title: string;
}) {
  return (
    <header className="flex h-11 min-w-0 overflow-hidden items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold">{title}</h2>
          {subtitle ? <p className="truncate text-[11px] text-muted-foreground">{subtitle}</p> : null}
        </div>
      </div>
      {actions}
    </header>
  );
}

function ColumnResizeHandle({
  active,
  index,
  onPointerDown,
}: {
  active: boolean;
  index: number;
  onPointerDown: (index: number, event: React.PointerEvent<HTMLButtonElement>) => void;
}) {
  return (
    <button
      aria-label={`Resize bytecode column ${index + 1}`}
      className={cn(
        "group flex h-full min-h-0 cursor-col-resize items-center justify-center border-x border-[color:var(--app-border)] bg-[var(--app-window)] text-muted-foreground outline-none transition-colors hover:bg-primary/10 hover:text-foreground focus-visible:bg-primary/10 focus-visible:text-foreground",
        active && "bg-primary/15 text-primary",
      )}
      onPointerDown={(event) => onPointerDown(index, event)}
      type="button"
    >
      <GripVertical className="size-3 opacity-60 transition-opacity group-hover:opacity-100" aria-hidden="true" />
    </button>
  );
}

function ExplanationSection({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="p-4">
      <h4 className="mb-3 text-sm font-semibold text-foreground">{title}</h4>
      {children}
    </section>
  );
}

function ContextRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid min-w-0 grid-cols-[5rem_minmax(0,1fr)] gap-2">
      <span>{label}</span>
      <span className="truncate font-mono text-foreground" title={value}>{value}</span>
    </div>
  );
}

function MiniMetric({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="rounded border border-[color:var(--app-border)] bg-black/10 px-2 py-1.5">
      <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
        <Icon className="size-3 shrink-0" aria-hidden="true" />
        <span className="truncate">{label}</span>
      </div>
      <div className="mt-1 truncate font-mono text-xs font-semibold text-foreground">{value}</div>
    </div>
  );
}

function Chevron({ className }: { className?: string }) {
  return <ChevronRight className={cn("size-3 shrink-0", className)} aria-hidden="true" />;
}

function groupBytecodeModules(
  view: MoveBytecodePackageView | null,
  filters: {
    branchOnly?: boolean;
    callsOnly?: boolean;
    packageOnly?: boolean;
  } = {},
): BytecodeModuleGroup[] {
  if (!view) {
    return [];
  }

  const { branchOnly = false, callsOnly = false, packageOnly = false } = filters;
  const groups = new Map<string, BytecodeModuleGroup>();

  for (const module of view.modules) {
    if (packageOnly && module.isDependency) {
      continue;
    }

    const functions = module.functions.filter((fn) => {
      if (branchOnly && !hasBranchControlFlow(fn)) {
        return false;
      }

      if (callsOnly && !hasBytecodeCall(fn)) {
        return false;
      }

      return true;
    });

    if ((branchOnly || callsOnly) && functions.length === 0) {
      continue;
    }

    const hasFunctionFilter = branchOnly || callsOnly;
    const visibleModule: MoveBytecodeModuleView = hasFunctionFilter
      ? {
          ...module,
          functionCount: functions.length,
          functions,
          instructionCount: functions.reduce((total, fn) => total + fn.instructionCount, 0),
        }
      : module;
    const id = module.isDependency
      ? `dependency:${module.packageName}`
      : `package:${view.packageName}`;
    const existing = groups.get(id);

    if (existing) {
      existing.modules.push(visibleModule);
      existing.moduleCount += 1;
      continue;
    }

    groups.set(id, {
      id,
      isDependency: module.isDependency,
      label: module.isDependency
        ? `Dependency: ${module.packageName}`
        : `Package: ${view.packageName}`,
      moduleCount: 1,
      modules: [visibleModule],
    });
  }

  return [...groups.values()]
    .map((group) => ({
      ...group,
      modules: [...group.modules].sort((left, right) => left.name.localeCompare(right.name)),
    }))
    .sort((left, right) => {
      if (left.isDependency !== right.isDependency) {
        return left.isDependency ? 1 : -1;
      }

      return left.label.localeCompare(right.label);
    });
}

function groupBytecodeFunctions(functions: MoveBytecodeFunctionView[]) {
  const groups = new Map<BytecodeFunctionCategoryId, BytecodeFunctionCategory>();

  for (const fn of functions) {
    const category = bytecodeFunctionCategory(fn);
    const group = groups.get(category.id);

    if (group) {
      group.functions.push(fn);
      group.count += 1;
      continue;
    }

    groups.set(category.id, {
      ...category,
      count: 1,
      functions: [fn],
    });
  }

  return FUNCTION_CATEGORY_ORDER
    .map((id) => groups.get(id))
    .filter((group): group is BytecodeFunctionCategory => Boolean(group))
    .map((group) => ({
      ...group,
      functions: [...group.functions].sort((left, right) =>
        left.name.localeCompare(right.name),
      ),
    }));
}

function hasBranchControlFlow(fn: MoveBytecodeFunctionView) {
  return fn.controlFlow.edges.some((edge) => edge.kind !== "fallthrough");
}

function hasBytecodeCall(fn: MoveBytecodeFunctionView) {
  return fn.instructions.some((instruction) =>
    instruction.opcode === "Call" ||
    instruction.opcode === "CallGeneric" ||
    instruction.call !== null,
  );
}

const FUNCTION_CATEGORY_ORDER: BytecodeFunctionCategoryId[] = [
  "public-entry",
  "entry",
  "public-package",
  "public-friend",
  "public",
  "view",
  "private",
];

function bytecodeFunctionCategory(fn: MoveBytecodeFunctionView): Omit<BytecodeFunctionCategory, "count" | "functions"> {
  const visibility = normalizedVisibility(fn.visibility);

  if (fn.isEntry && visibility === "public") {
    return {
      id: "public-entry",
      label: "Public entry",
      tone: "publicEntry",
    };
  }

  if (fn.isEntry) {
    return {
      id: "entry",
      label: "Entry only",
      tone: "entry",
    };
  }

  if (isBytecodeGetter(fn)) {
    return {
      id: "view",
      label: "View",
      tone: "view",
    };
  }

  if (visibility === "public(package)") {
    return {
      id: "public-package",
      label: "Public(package)",
      tone: "package",
    };
  }

  if (visibility === "public(friend)") {
    return {
      id: "public-friend",
      label: "Public(friend)",
      tone: "friend",
    };
  }

  if (visibility === "public") {
    return {
      id: "public",
      label: "Public",
      tone: "public",
    };
  }

  return {
    id: "private",
    label: "Private",
    tone: "private",
  };
}

function normalizedVisibility(visibility: string) {
  return visibility.trim().toLowerCase() || "private";
}

function isBytecodeGetter(fn: MoveBytecodeFunctionView) {
  if (fn.isEntry || !fn.instructions.length || fn.returnCount === 0) {
    return false;
  }

  const visibility = normalizedVisibility(fn.visibility);
  const isExternallyUseful =
    visibility === "public" ||
    visibility === "public(package)" ||
    visibility === "public(friend)";

  if (!isExternallyUseful) {
    return false;
  }

  return !fn.instructions.some((instruction) => {
    const opcode = instruction.opcode.toUpperCase();

    return (
      opcode.includes("MUT_BORROW") ||
      opcode.includes("CALL") ||
      opcode.includes("WRITE_REF") ||
      opcode.includes("MOVE_TO") ||
      opcode.includes("MOVE_FROM") ||
      opcode.includes("PACK") ||
      opcode.includes("UNPACK") ||
      opcode.includes("VEC_PUSH") ||
      opcode.includes("VEC_POP") ||
      opcode.includes("VEC_SWAP")
    );
  });
}

function functionBadgeLabel(
  fn: MoveBytecodeFunctionView,
  category: BytecodeFunctionCategoryId,
) {
  if (category === "public-entry") {
    return "public entry";
  }
  if (category === "entry") {
    return "entry only";
  }
  if (category === "view") {
    return normalizedVisibility(fn.visibility);
  }

  return normalizedVisibility(fn.visibility);
}

function functionGroupToneTextClass(tone: BytecodeFunctionCategory["tone"]) {
  switch (tone) {
    case "publicEntry":
      return "text-emerald-300";
    case "entry":
      return "text-lime-300";
    case "view":
      return "text-fuchsia-300";
    case "package":
      return "text-orange-300";
    case "friend":
      return "text-yellow-300";
    case "public":
      return "text-cyan-300";
    case "private":
      return "text-muted-foreground";
  }
}

function functionGroupToneDotClass(tone: BytecodeFunctionCategory["tone"]) {
  switch (tone) {
    case "publicEntry":
      return "bg-emerald-300";
    case "entry":
      return "bg-lime-300";
    case "view":
      return "bg-fuchsia-300";
    case "package":
      return "bg-orange-300";
    case "friend":
      return "bg-yellow-300";
    case "public":
      return "bg-cyan-300";
    case "private":
      return "bg-slate-400";
  }
}

function functionGroupToneBadgeClass(tone: BytecodeFunctionCategory["tone"]) {
  switch (tone) {
    case "publicEntry":
      return "bg-emerald-500/15 text-emerald-300";
    case "entry":
      return "bg-lime-500/15 text-lime-300";
    case "view":
      return "bg-fuchsia-500/15 text-fuchsia-300";
    case "package":
      return "bg-orange-500/15 text-orange-300";
    case "friend":
      return "bg-yellow-500/15 text-yellow-300";
    case "public":
      return "bg-cyan-500/15 text-cyan-300";
    case "private":
      return "bg-muted text-muted-foreground";
  }
}

type ControlFlowBlockLayout = {
  block: MoveBytecodeBasicBlockView;
  x: number;
  y: number;
  width: number;
  height: number;
  centerX: number;
  centerY: number;
  top: number;
  bottom: number;
};

type ControlFlowEdgeLayout = MoveBytecodeControlFlowEdgeView & {
  path: string;
  labelX: number;
  labelY: number;
};

type ControlFlowPath = {
  blockIds: string[];
  edgeKeys: string[];
};

type ControlFlowAnimationFrame = {
  blockIds: string[];
  edgeKey: string | null;
};

function layoutControlFlow(
  blocks: MoveBytecodeBasicBlockView[],
  edges: MoveBytecodeControlFlowEdgeView[],
) {
  const nodeWidth = 148;
  const nodeHeight = 68;
  const verticalGap = 68;
  const topPadding = 48;
  const bottomPadding = 52;
  const centerX = 180;
  const minWidth = 360;
  const blockIndex = new Map(blocks.map((block, index) => [block.id, index]));
  const blockLayouts: ControlFlowBlockLayout[] = blocks.map((block, index) => {
    const y = topPadding + index * (nodeHeight + verticalGap);

    return {
      block,
      x: centerX - nodeWidth / 2,
      y,
      width: nodeWidth,
      height: nodeHeight,
      centerX,
      centerY: y + nodeHeight / 2,
      top: y,
      bottom: y + nodeHeight,
    };
  });
  const layoutById = new Map(blockLayouts.map((layout) => [layout.block.id, layout]));
  const edgeLayouts = edges
    .map((edge, index) => {
      const source = layoutById.get(edge.source);
      const target = layoutById.get(edge.target);

      if (!source || !target) {
        return null;
      }

      const sourceIndex = blockIndex.get(edge.source) ?? 0;
      const targetIndex = blockIndex.get(edge.target) ?? 0;
      const isFallthrough = edge.kind === "fallthrough" && targetIndex === sourceIndex + 1;

      if (isFallthrough) {
        return {
          ...edge,
          path: `M ${source.centerX} ${source.bottom} L ${target.centerX} ${target.top}`,
          labelX: source.centerX + 36,
          labelY: (source.bottom + target.top) / 2 - 4,
        };
      }

      const lane = 36 + (index % 3) * 24;
      const goesForward = targetIndex > sourceIndex;
      const usesRightLane = goesForward || index % 2 === 0;
      const laneX = usesRightLane
        ? Math.max(source.x + source.width, target.x + target.width) + lane
        : Math.min(source.x, target.x) - lane;
      const sourceY = goesForward ? source.bottom : source.top;
      const targetY = goesForward ? target.top : target.bottom;
      const curveOutY = sourceY + (goesForward ? 24 : -24);
      const curveInY = targetY + (goesForward ? -24 : 24);

      return {
        ...edge,
        path: `M ${source.centerX} ${sourceY} C ${laneX} ${curveOutY}, ${laneX} ${curveInY}, ${target.centerX} ${targetY}`,
        labelX: laneX,
        labelY: (sourceY + targetY) / 2,
      };
    })
    .filter((edge): edge is ControlFlowEdgeLayout => edge !== null);
  const lastBlock = blockLayouts[blockLayouts.length - 1];
  const height = Math.max(260, (lastBlock?.bottom ?? topPadding) + bottomPadding);

  return {
    blocks: blockLayouts,
    centerX,
    edges: edgeLayouts,
    height,
    width: minWidth,
  };
}

function controlFlowPaths(
  blocks: MoveBytecodeBasicBlockView[],
  edges: MoveBytecodeControlFlowEdgeView[],
) {
  const entryBlock = blocks[0];

  if (!entryBlock) {
    return [];
  }

  if (!edges.length) {
    return [{ blockIds: [entryBlock.id], edgeKeys: [] }];
  }

  const outgoing = new Map<string, MoveBytecodeControlFlowEdgeView[]>();

  for (const edge of edges) {
    const current = outgoing.get(edge.source) ?? [];
    current.push(edge);
    outgoing.set(edge.source, current);
  }

  for (const group of outgoing.values()) {
    group.sort((left, right) =>
      left.sourceOffset - right.sourceOffset
      || left.targetOffset - right.targetOffset
      || left.kind.localeCompare(right.kind),
    );
  }

  const paths: ControlFlowPath[] = [];
  const maxDepth = Math.max(4, blocks.length * 2);

  const walk = (blockId: string, blockIds: string[], edgeKeys: string[]) => {
    const nextEdges = outgoing.get(blockId) ?? [];

    if (!nextEdges.length || blockIds.length >= maxDepth) {
      paths.push({ blockIds, edgeKeys });
      return;
    }

    for (const edge of nextEdges) {
      const nextBlockIds = [...blockIds, edge.target];
      const nextEdgeKeys = [...edgeKeys, controlFlowEdgeKey(edge)];

      if (blockIds.includes(edge.target)) {
        paths.push({ blockIds: nextBlockIds, edgeKeys: nextEdgeKeys });
        continue;
      }

      walk(edge.target, nextBlockIds, nextEdgeKeys);
    }
  };

  walk(entryBlock.id, [entryBlock.id], []);

  return paths;
}

function controlFlowAnimationFrames(paths: ControlFlowPath[]) {
  const frames: ControlFlowAnimationFrame[] = [];

  for (const path of paths) {
    if (!path.blockIds.length) {
      continue;
    }

    frames.push({
      blockIds: [path.blockIds[0]],
      edgeKey: null,
    });

    for (const [index, edgeKey] of path.edgeKeys.entries()) {
      frames.push({
        blockIds: path.blockIds.slice(0, index + 2),
        edgeKey,
      });
    }
  }

  return frames;
}

function controlFlowEdgeKey(edge: Pick<MoveBytecodeControlFlowEdgeView, "kind" | "source" | "sourceOffset" | "target" | "targetOffset">) {
  return `${edge.source}:${edge.target}:${edge.kind}:${edge.sourceOffset}:${edge.targetOffset}`;
}

function operandText(instruction: MoveBytecodeInstructionView) {
  const match = instruction.detail.match(/^[^(]+\((.*)\)$/);

  if (!match) {
    return "";
  }

  return match[1]
    .replace(/FunctionHandleIndex\((\d+)\)/g, "fn#$1")
    .replace(/ConstantPoolIndex\((\d+)\)/g, "const#$1")
    .replace(/StructDefinitionIndex\((\d+)\)/g, "struct#$1")
    .replace(/FieldHandleIndex\((\d+)\)/g, "field#$1")
    .replace(/VariantJumpTableIndex\((\d+)\)/g, "table#$1");
}

function formatOpcode(opcode: string) {
  return opcode
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
    .toUpperCase();
}

function formatOffset(offset: number) {
  return offset.toString(16).toUpperCase().padStart(2, "0");
}

function formatSourceBytes(instruction: MoveBytecodeInstructionView) {
  return instruction.source
    ? `${instruction.source.startByte}...${instruction.source.endByte}`
    : "-";
}

type BytecodeCallTarget = {
  fn: MoveBytecodeFunctionView;
  module: MoveBytecodeModuleView;
};

type BytecodeCallSummary = {
  canAbort: boolean;
  emitsEvents: boolean;
  functionName: string;
  genericTypeArguments: string;
  key: string;
  kind: "call" | "op";
  moduleLabel: string;
  mutatesState: boolean;
  target: BytecodeCallTarget | null;
  transfersAssets: boolean;
  usesCapabilitiesOrSigner: boolean;
  visibility: string;
};

function functionCallSummaries(
  view: MoveBytecodePackageView | null,
  fn: MoveBytecodeFunctionView,
) {
  const seen = new Set<string>();
  const summaries: BytecodeCallSummary[] = [];

  for (const instruction of fn.instructions) {
    const summary = instruction.call
      ? callSummaryForInstruction(view, instruction.call)
      : bytecodeOperationSummary(instruction);

    if (!summary) {
      continue;
    }

    if (seen.has(summary.key)) {
      continue;
    }

    seen.add(summary.key);
    summaries.push(summary);
  }

  return summaries;
}

function callSummaryForInstruction(
  view: MoveBytecodePackageView | null,
  call: MoveBytecodeCallView,
): BytecodeCallSummary {
  const target = findBytecodeCallTarget(view, call);
  const targetFunction = target?.fn ?? null;

  return {
    canAbort: targetFunction ? functionCanAbort(targetFunction) : callMayAbort(call),
    emitsEvents: targetFunction ? functionEmitsEvents(targetFunction) : callMayEmitEvents(call),
    functionName: call.functionName,
    genericTypeArguments: call.genericTypeArguments.length
      ? call.genericTypeArguments.join(", ")
      : "-",
    key: `call:${call.qualifiedName}<${call.genericTypeArguments.join(",")}>`,
    kind: "call",
    moduleLabel: `${shortAddress(call.moduleAddress)}::${call.moduleName}`,
    mutatesState: targetFunction ? functionMutatesState(targetFunction) : callMayMutateState(call),
    target,
    transfersAssets: targetFunction ? functionTransfersAssets(targetFunction) : callMayTransferAssets(call),
    usesCapabilitiesOrSigner: targetFunction
      ? functionUsesCapabilitiesOrSigner(targetFunction)
      : callMayUseCapabilitiesOrSigner(call),
    visibility: targetFunction ? functionVisibilityLabel(targetFunction) : "external",
  };
}

function bytecodeOperationSummary(instruction: MoveBytecodeInstructionView): BytecodeCallSummary | null {
  const operation = trackedBytecodeOperation(instruction.opcode);

  if (!operation) {
    return null;
  }

  return {
    canAbort: bytecodeOperationCanAbort(instruction.opcode),
    emitsEvents: false,
    functionName: operation,
    genericTypeArguments: signatureOperandLabel(instruction.detail),
    key: `op:${instruction.opcode}:${signatureOperandLabel(instruction.detail)}`,
    kind: "op",
    moduleLabel: "Move VM",
    mutatesState: bytecodeOperationMutates(instruction.opcode),
    target: null,
    transfersAssets: false,
    usesCapabilitiesOrSigner: false,
    visibility: "bytecode",
  };
}

function trackedBytecodeOperation(opcode: string) {
  if (isNotableBytecodeOperation(opcode)) {
    return bytecodeOperationName(opcode);
  }

  return null;
}

function isNotableBytecodeOperation(opcode: string) {
  return opcode.startsWith("Vec") ||
    opcode.includes("Borrow") ||
    opcode.includes("Ref") ||
    opcode.includes("Pack") ||
    opcode.includes("Unpack") ||
    opcode.includes("Branch") ||
    opcode.startsWith("Br") ||
    opcode === "Abort" ||
    opcode === "Ret";
}

function bytecodeOperationName(opcode: string) {
  return formatOpcode(opcode).toLowerCase();
}

function bytecodeOperationCanAbort(opcode: string) {
  return opcode.includes("Borrow") ||
    opcode.includes("Unpack") ||
    opcode === "Abort" ||
    opcode === "VecPopBack" ||
    opcode === "VecSwap";
}

function bytecodeOperationMutates(opcode: string) {
  return opcode === "VecPushBack" ||
    opcode === "VecPopBack" ||
    opcode === "VecSwap" ||
    opcode === "WriteRef" ||
    opcode.includes("MutBorrow") ||
    opcode === "StLoc";
}

function signatureOperandLabel(detail: string) {
  const match = detail.match(/SignatureIndex\((\d+)\)/);
  return match ? `signature #${match[1]}` : "-";
}

function findBytecodeCallTarget(
  view: MoveBytecodePackageView | null,
  call: MoveBytecodeCallView,
): BytecodeCallTarget | null {
  if (!view) {
    return null;
  }

  const module = view.modules.find((candidate) =>
    sameMoveAddress(candidate.address, call.moduleAddress)
    && candidate.name === call.moduleName,
  ) ?? null;
  const fn = module?.functions.find((candidate) => candidate.name === call.functionName) ?? null;

  return module && fn ? { fn, module } : null;
}

function sameMoveAddress(left: string, right: string) {
  return normalizeMoveAddress(left) === normalizeMoveAddress(right);
}

function normalizeMoveAddress(address: string) {
  return address.replace(/^0x/i, "").replace(/^0+/, "").toLowerCase() || "0";
}

function callLabel(call: MoveBytecodeCallView) {
  const typeArguments = call.genericTypeArguments.length
    ? `<${call.genericTypeArguments.join(", ")}>`
    : "";

  return `${shortAddress(call.moduleAddress)}::${call.moduleName}::${call.functionName}${typeArguments}`;
}

function functionVisibilityLabel(fn: MoveBytecodeFunctionView) {
  if (fn.isEntry && fn.visibility === "Public") {
    return "public entry";
  }

  if (fn.isEntry) {
    return "entry";
  }

  return fn.visibility;
}

function functionMutatesState(fn: MoveBytecodeFunctionView) {
  return fn.instructions.some((instruction) =>
    ["WriteRef", "MoveTo", "MoveFrom", "MutBorrowGlobal", "MutBorrowField", "MutBorrowFieldGeneric", "MutBorrowVariantField", "MutBorrowVariantFieldGeneric"].includes(instruction.opcode)
    || callMayMutateState(instruction.call),
  );
}

function functionCanAbort(fn: MoveBytecodeFunctionView) {
  return fn.instructions.some((instruction) =>
    instruction.opcode === "Abort"
    || instruction.opcode.startsWith("Branch")
    || instruction.opcode.startsWith("Br")
    || callMayAbort(instruction.call),
  );
}

function functionTransfersAssets(fn: MoveBytecodeFunctionView) {
  return fn.instructions.some((instruction) => callMayTransferAssets(instruction.call));
}

function functionEmitsEvents(fn: MoveBytecodeFunctionView) {
  return fn.instructions.some((instruction) => callMayEmitEvents(instruction.call));
}

function functionUsesCapabilitiesOrSigner(fn: MoveBytecodeFunctionView) {
  return [...fn.parameters, ...fn.returns, ...fn.acquires].some(typeLooksPrivileged)
    || fn.instructions.some((instruction) => callMayUseCapabilitiesOrSigner(instruction.call));
}

function callMayMutateState(call: MoveBytecodeCallView | null) {
  if (!call) {
    return false;
  }

  return /(transfer|withdraw|deposit|mint|burn|create|destroy|set|push|remove|delete|claim|distribute|latch|borrow_mut|share|freeze|receive)/i
    .test(`${call.moduleName}::${call.functionName}`);
}

function callMayAbort(call: MoveBytecodeCallView | null) {
  if (!call) {
    return false;
  }

  return !/^(is_|has_|get_|borrow_|contains|length|len|value|balance|supply|name|symbol|decimals)/i
    .test(call.functionName);
}

function callMayTransferAssets(call: MoveBytecodeCallView | null) {
  if (!call) {
    return false;
  }

  return /(coin|balance|funds|asset|token|bucket|vault|transfer|withdraw|deposit)/i
    .test(`${call.moduleName}::${call.functionName}`);
}

function callMayEmitEvents(call: MoveBytecodeCallView | null) {
  if (!call) {
    return false;
  }

  return /(event|emit)/i.test(`${call.moduleName}::${call.functionName}`);
}

function callMayUseCapabilitiesOrSigner(call: MoveBytecodeCallView | null) {
  if (!call) {
    return false;
  }

  return /(cap|capability|treasury|admin|owner|signer|auth|witness|publisher)/i
    .test(`${call.moduleName}::${call.functionName}::${call.genericTypeArguments.join("::")}`);
}

function typeLooksPrivileged(value: string) {
  return /(signer|cap|capability|treasury|admin|owner|witness|publisher)/i.test(value);
}

function shortAddress(address: string | null | undefined) {
  if (!address) {
    return "_";
  }

  if (address.length <= 18) {
    return address;
  }

  return `${address.slice(0, 8)}...${address.slice(-6)}`;
}

function findMoveFunctionLine(source: string, functionName: string) {
  const functionPattern = new RegExp(`\\bfun\\s+${escapeRegExp(functionName)}\\b`);
  const lines = source.split(/\r?\n/);

  for (let index = 0; index < lines.length; index += 1) {
    if (functionPattern.test(lines[index])) {
      return index + 1;
    }
  }

  return 1;
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function edgeColor(kind: string) {
  if (kind === "true") {
    return "rgb(52 211 153 / 0.95)";
  }

  if (kind === "false") {
    return "rgb(248 113 113 / 0.95)";
  }

  if (kind === "branch") {
    return "rgb(96 165 250 / 0.95)";
  }

  if (kind === "variant") {
    return "rgb(168 85 247 / 0.95)";
  }

  if (kind === "fallthrough") {
    return "rgb(148 163 184 / 0.7)";
  }

  return "rgb(148 163 184 / 0.8)";
}

function opcodeExplanation(instruction: MoveBytecodeInstructionView) {
  const opcode = instruction.opcode;
  const operands = operandText(instruction);

  if (opcode === "BrTrue") {
    return `Conditional branch encoded in the compiled bytecode. Target operand: ${operands || "-"}.`;
  }

  if (opcode === "BrFalse") {
    return `Conditional branch encoded in the compiled bytecode. Target operand: ${operands || "-"}.`;
  }

  if (opcode === "Branch") {
    return `Unconditional branch encoded in the compiled bytecode. Target operand: ${operands || "-"}.`;
  }

  if (opcode === "Call" || opcode === "CallGeneric") {
    return `Function call instruction from the compiled bytecode. Operand: ${operands || "-"}.`;
  }

  if (opcode === "WriteRef") {
    return "Reference write instruction from the compiled bytecode.";
  }

  if (opcode === "LdConst") {
    return `Constant-load instruction from the compiled bytecode. Operand: ${operands || "-"}.`;
  }

  if (opcode === "Ret") {
    return "Return instruction from the compiled bytecode.";
  }

  return `Move VM instruction represented by the raw bytecode enum value ${instruction.detail}.`;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  return `${(bytes / 1024).toFixed(bytes < 1024 * 100 ? 1 : 0)} KB`;
}
