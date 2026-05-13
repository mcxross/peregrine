import React from "react";
import {
  Binary,
  Boxes,
  Braces,
  ChevronRight,
  Circle,
  Cpu,
  FileCode2,
  FunctionSquare,
  GitBranch,
  GripVertical,
  Loader2,
  Package,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  loadMoveBytecodeView,
  type MoveBytecodeBasicBlockView,
  type MoveBytecodeControlFlowEdgeView,
  type MoveBytecodeFunctionView,
  type MoveBytecodeInstructionView,
  type MoveBytecodeModuleView,
  type MoveBytecodePackageView,
  type MovePackage,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

type BytecodeViewScreenProps = {
  activeMovePackage: MovePackage | null;
  packageTree: PackageTree;
};

const BYTECODE_COLUMN_WIDTHS = [272, 560, 300, 360];
const BYTECODE_COLUMN_MIN_WIDTHS = [224, 420, 240, 300];
const BYTECODE_RESIZE_HANDLE_WIDTH = 8;
const BYTECODE_TREE_ANIMATION_MS = 160;

type BytecodeModuleGroup = {
  id: string;
  isDependency: boolean;
  label: string;
  moduleCount: number;
  modules: MoveBytecodeModuleView[];
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
  const gridTemplateColumns = React.useMemo(
    () => [
      `${columnWidths[0]}px`,
      `${BYTECODE_RESIZE_HANDLE_WIDTH}px`,
      `${columnWidths[1]}px`,
      `${BYTECODE_RESIZE_HANDLE_WIDTH}px`,
      `${columnWidths[2]}px`,
      `${BYTECODE_RESIZE_HANDLE_WIDTH}px`,
      `${columnWidths[3]}px`,
    ].join(" "),
    [columnWidths],
  );
  const gridMinWidth = React.useMemo(
    () => columnWidths.reduce((sum, width) => sum + width, 0) + BYTECODE_RESIZE_HANDLE_WIDTH * 3,
    [columnWidths],
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

            <InstructionPanel
              selectedInstruction={selectedInstruction}
              selectedModule={selectedModule}
              selectedFunction={selectedFunction}
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
            />
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
  onSelectFunction,
  onSelectModule,
  selectedFunctionName,
  selectedModulePath,
  view,
}: {
  isLoading: boolean;
  onSelectFunction: (modulePath: string, functionName: string) => void;
  onSelectModule: (modulePath: string) => void;
  selectedFunctionName: string | null;
  selectedModulePath: string | null;
  view: MoveBytecodePackageView | null;
}) {
  const moduleGroups = React.useMemo(() => groupBytecodeModules(view), [view]);
  const [expandedGroups, setExpandedGroups] = React.useState<Set<string>>(() => new Set());
  const [expandedModules, setExpandedModules] = React.useState<Set<string>>(() => new Set());

  React.useEffect(() => {
    if (!view) {
      setExpandedGroups(new Set());
      setExpandedModules(new Set());
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

                          <CollapsibleTreeBody isOpen={isExpandedModule}>
                            <div className="ml-6 min-w-0 overflow-hidden border-l border-[color:var(--app-border)] py-1 pl-2">
                              {module.functions.map((fn) => {
                                const isSelectedFunction = isSelectedModule && fn.name === selectedFunctionName;

                                return (
                                  <button
                                    className={cn(
                                      "flex h-7 w-full min-w-0 max-w-full items-center gap-2 overflow-hidden rounded px-2 text-left text-xs text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
                                      isSelectedFunction && "bg-primary/15 text-foreground",
                                    )}
                                    key={`${module.bytecodePath}:${fn.name}`}
                                    onClick={() => onSelectFunction(module.bytecodePath, fn.name)}
                                    type="button"
                                  >
                                    <FunctionSquare className="size-3 shrink-0" aria-hidden="true" />
                                    <span className="min-w-0 flex-1 truncate">{fn.name}</span>
                                    {fn.isEntry ? (
                                      <span className="shrink-0 rounded bg-emerald-500/15 px-1 text-[10px] font-semibold text-emerald-400">
                                        entry
                                      </span>
                                    ) : null}
                                  </button>
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
      </ScrollArea>
    </aside>
  );
}

function InstructionPanel({
  onSelectInstruction,
  selectedFunction,
  selectedInstruction,
  selectedModule,
}: {
  onSelectInstruction: (instruction: MoveBytecodeInstructionView) => void;
  selectedFunction: MoveBytecodeFunctionView | null;
  selectedInstruction: MoveBytecodeInstructionView | null;
  selectedModule: MoveBytecodeModuleView | null;
}) {
  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <PanelHeader
        icon={Cpu}
        title="Instructions"
        subtitle={
          selectedModule && selectedFunction
            ? `${selectedModule.address}::${selectedModule.name}::${selectedFunction.name}`
            : "Compiled bytecode"
        }
      />

      <ScrollArea className="min-h-0 min-w-0">
        <div className="max-w-full min-w-0 overflow-x-auto overflow-y-hidden">
          <table className="w-full min-w-[42rem] table-fixed border-collapse font-mono text-xs">
          <thead className="sticky top-0 z-10 bg-[var(--app-window)] text-[11px] text-muted-foreground shadow-[0_1px_0_var(--app-border)]">
            <tr className="text-left">
              <th className="w-14 px-3 py-2 font-semibold">#</th>
              <th className="w-16 px-2 py-2 font-semibold">Offset</th>
              <th className="w-40 px-2 py-2 font-semibold">Opcode</th>
              <th className="px-2 py-2 font-semibold">Operands</th>
              <th className="w-36 px-3 py-2 text-right font-semibold">Source bytes</th>
            </tr>
          </thead>
          <tbody>
            {selectedFunction?.instructions.map((instruction, index) => {
              const isSelected = selectedInstruction?.offset === instruction.offset;

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
                  <td className="px-2 py-1.5 font-semibold text-foreground">{formatOpcode(instruction.opcode)}</td>
                  <td className="truncate px-2 py-1.5" title={operandText(instruction)}>
                    {operandText(instruction)}
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
  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <PanelHeader icon={GitBranch} title="Control Flow" subtitle={`${blocks.length} blocks, ${edges.length} edges`} />
      <ScrollArea className="min-h-0 min-w-0">
        <ControlFlowGraph
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
  const layout = React.useMemo(() => layoutControlFlow(blocks, edges), [blocks, edges]);

  if (!blocks.length) {
    return (
      <div className="grid h-full min-h-48 place-items-center text-sm text-muted-foreground">
        Native function; no bytecode body.
      </div>
    );
  }

  return (
    <div className="min-w-0 overflow-auto p-4">
      <div
        className="relative"
        style={{
          height: layout.height,
          minWidth: layout.width,
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

            return (
              <g key={`${edge.source}:${edge.target}:${edge.kind}:${edge.sourceOffset}`}>
                <path
                  d={edge.path}
                  fill="none"
                  markerEnd={`url(#${isActive ? "cfg-arrow-selected" : "cfg-arrow"})`}
                  stroke={isActive ? "rgb(96 165 250)" : edgeColor(edge.kind)}
                  strokeDasharray={edge.kind === "fallthrough" ? undefined : "5 4"}
                  strokeWidth={isActive ? 2 : 1.4}
                />
                <text
                  className="fill-muted-foreground font-mono text-[10px]"
                  x={edge.labelX}
                  y={edge.labelY}
                  textAnchor="middle"
                >
                  {edge.kind}
                </text>
              </g>
            );
          })}
        </svg>

        <Circle
          className="absolute size-3 text-muted-foreground"
          style={{ left: layout.centerX - 6, top: 10 }}
          aria-hidden="true"
        />

        {layout.blocks.map((item) => {
          const containsInstruction =
            selectedInstruction
            && selectedInstruction.offset >= item.block.startOffset
            && selectedInstruction.offset <= item.block.endOffset;
          const isSelected = item.block.id === selectedBlock?.id;

          return (
            <button
              className={cn(
                "absolute rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-3 py-2 text-center text-xs shadow-sm transition-colors hover:border-primary/50 hover:bg-[var(--app-subtle)]",
                (isSelected || containsInstruction) && "border-primary/70 bg-primary/15 text-foreground shadow-[0_0_0_1px_rgba(71,139,255,0.18)]",
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
}: {
  block: MoveBytecodeBasicBlockView | null;
  instruction: MoveBytecodeInstructionView | null;
  moveFunction: MoveBytecodeFunctionView | null;
  module: MoveBytecodeModuleView | null;
}) {
  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-[var(--app-window)]">
      <PanelHeader
        icon={Braces}
        title="Instruction Data"
        subtitle={instruction ? `${formatOpcode(instruction.opcode)} at ${formatOffset(instruction.offset)}` : "No instruction selected"}
      />
      <ScrollArea className="min-h-0">
        <div className="divide-y divide-[color:var(--app-border)]">
          <section className="p-5">
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
              <ContextRow label="Operands" value={instruction ? operandText(instruction) || "-" : "-"} />
              <ContextRow label="Raw" value={instruction?.detail ?? "-"} />
            </div>
          </ExplanationSection>

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
              <ContextRow label="Module" value={module ? `${module.address}::${module.name}` : "-"} />
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
  icon: Icon,
  subtitle,
  title,
}: {
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  subtitle?: string;
  title: string;
}) {
  return (
    <header className="flex h-12 min-w-0 overflow-hidden items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-4">
      <div className="flex min-w-0 items-center gap-2">
        <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold">{title}</h2>
          {subtitle ? <p className="truncate text-[11px] text-muted-foreground">{subtitle}</p> : null}
        </div>
      </div>
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
    <section className="p-5">
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

function groupBytecodeModules(view: MoveBytecodePackageView | null): BytecodeModuleGroup[] {
  if (!view) {
    return [];
  }

  const groups = new Map<string, BytecodeModuleGroup>();

  for (const module of view.modules) {
    const id = module.isDependency
      ? `dependency:${module.packageName}`
      : `package:${view.packageName}`;
    const existing = groups.get(id);

    if (existing) {
      existing.modules.push(module);
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
      modules: [module],
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

function layoutControlFlow(
  blocks: MoveBytecodeBasicBlockView[],
  edges: MoveBytecodeControlFlowEdgeView[],
) {
  const nodeWidth = 168;
  const nodeHeight = 74;
  const verticalGap = 76;
  const topPadding = 48;
  const bottomPadding = 52;
  const centerX = 220;
  const minWidth = 440;
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
          labelX: source.centerX + 46,
          labelY: (source.bottom + target.top) / 2 - 4,
        };
      }

      const lane = 52 + (index % 3) * 34;
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
