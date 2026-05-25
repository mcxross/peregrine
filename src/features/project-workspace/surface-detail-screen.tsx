import * as React from "react";
import {
  ArrowRight,
  Boxes,
  Box,
  ChevronDown,
  ChevronRight,
  CircleDot,
  FileCode2,
  Filter,
  GitBranch,
  KeyRound,
  PackagePlus,
  Link2,
  Lock,
  Network,
  RefreshCw,
  Search,
  ShieldAlert,
  Share2,
  TestTube2,
  Trash2,
  UsersRound,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  AdminControlFinding,
  ExternalCallFinding,
  MoveFunctionSignature,
  MoveModule,
  MovePackage,
  ObjectLifecycleFunctionRef,
  ObjectLifecycleMap,
  ObjectLifecycleStage,
  ObjectLifecycleStageKind,
  ObjectOwnershipFinding,
  PublicPackageRelationship,
} from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";

const OBJECT_TREE_PANE_DEFAULT_WIDTH = 460;
const OBJECT_TREE_PANE_MIN_WIDTH = 320;
const OBJECT_TREE_PANE_MAX_WIDTH = 760;
const OBJECT_TREE_DETAIL_PANE_MIN_WIDTH = 420;
const OBJECT_LIFECYCLE_STAGE_GROUPS = [
  {
    id: "created",
    title: "Create",
    stageKinds: ["created"] as ObjectLifecycleStageKind[],
    icon: PackagePlus,
    accent: "text-sky-400 border-sky-400/35 bg-sky-400/10",
  },
  {
    id: "owned",
    title: "Owned",
    stageKinds: ["owned", "immutable", "party"] as ObjectLifecycleStageKind[],
    icon: CircleDot,
    accent: "text-cyan-300 border-cyan-300/35 bg-cyan-300/10",
  },
  {
    id: "mutated",
    title: "Mutated",
    stageKinds: ["mutated"] as ObjectLifecycleStageKind[],
    icon: RefreshCw,
    accent: "text-amber-300 border-amber-300/35 bg-amber-300/10",
  },
  {
    id: "transferred",
    title: "Transferred",
    stageKinds: ["transferred"] as ObjectLifecycleStageKind[],
    icon: ArrowRight,
    accent: "text-emerald-300 border-emerald-300/35 bg-emerald-300/10",
  },
  {
    id: "wrapped",
    title: "Wrapped / Shared",
    stageKinds: ["wrapped", "shared"] as ObjectLifecycleStageKind[],
    icon: Share2,
    accent: "text-violet-300 border-violet-300/35 bg-violet-300/10",
  },
  {
    id: "deleted",
    title: "Deleted",
    stageKinds: ["deleted"] as ObjectLifecycleStageKind[],
    icon: Trash2,
    accent: "text-red-300 border-red-300/35 bg-red-300/10",
  },
] as const;

export type SurfaceDetailKind =
  | "entry-functions"
  | "capabilities"
  | "objects"
  | "shared-objects"
  | "address-owned"
  | "immutable-objects"
  | "wrapped-objects"
  | "party-objects"
  | "admin-controls"
  | "external-calls"
  | "package-internals";

type SurfaceDetailScreenProps = {
  detail: SurfaceDetailKind;
  movePackage: MovePackage | null;
};

type ObjectDetailTabKind = "capabilities" | ObjectOwnershipFinding["ownershipKind"];

const detailMeta = {
  "entry-functions": {
    icon: GitBranch,
    title: "Entry Functions",
    description: "Transaction-callable functions exposed by the active Move package.",
  },
  capabilities: {
    icon: KeyRound,
    title: "Capabilities",
    description: "Authority-bearing structs and the privileged functions they protect.",
  },
  objects: {
    icon: Boxes,
    title: "Objects",
    description: "Shared, owned, immutable, wrapped, and party objects detected in the active package.",
  },
  "shared-objects": {
    icon: Boxes,
    title: "Shared Objects",
    description: "Key objects made globally accessible through Sui shared-object APIs.",
  },
  "address-owned": {
    icon: Box,
    title: "Address-Owned Objects",
    description: "Key objects transferred to, or returned for, transaction callers.",
  },
  "immutable-objects": {
    icon: Lock,
    title: "Immutable Objects",
    description: "Objects frozen into immutable state after creation.",
  },
  "wrapped-objects": {
    icon: Boxes,
    title: "Wrapped Objects",
    description: "Key objects stored inside other key objects.",
  },
  "party-objects": {
    icon: UsersRound,
    title: "Party Objects",
    description: "Objects moved through party ownership and transfer APIs.",
  },
  "admin-controls": {
    icon: ShieldAlert,
    title: "Admin Controls",
    description: "Privileged transaction-callable functions and their apparent guards.",
  },
  "external-calls": {
    icon: Network,
    title: "External Calls",
    description: "Calls from active package modules into external modules.",
  },
  "package-internals": {
    icon: Link2,
    title: "Package Internals",
    description: "Observed uses of public(package) functions inside the active package.",
  },
} satisfies Record<SurfaceDetailKind, {
  icon: typeof GitBranch;
  title: string;
  description: string;
}>;

export function SurfaceDetailScreen({
  detail,
  movePackage,
}: SurfaceDetailScreenProps) {
  const meta = detailMeta[detail];
  const Icon = meta.icon;
  const ownershipKind = ownershipKindForDetail(detail);

  if (detail === "objects" || detail === "capabilities" || ownershipKind) {
    return (
      <ObjectOwnershipTreeScreen
        initialKind={detail === "capabilities" ? "capabilities" : ownershipKind ?? "capabilities"}
        movePackage={movePackage}
      />
    );
  }

  return (
    <section className="grid h-full min-h-0 grid-rows-[auto_1fr] bg-[var(--app-window)]">
      <header className="border-b border-[color:var(--app-border)] px-6 py-5">
        <div className="flex items-start gap-3">
          <span className="inline-flex size-9 items-center justify-center rounded-md bg-[var(--app-elevated)] text-muted-foreground">
            <Icon className="size-5" aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <h1 className="text-xl font-semibold tracking-tight">{meta.title}</h1>
            <p className="mt-1 max-w-2xl text-sm text-muted-foreground">{meta.description}</p>
          </div>
        </div>
      </header>

      <ScrollArea className="min-h-0">
        <div className="grid gap-3 p-6">
          {movePackage ? renderDetail(detail, movePackage) : <EmptyState label="No active Move package selected." />}
        </div>
      </ScrollArea>
    </section>
  );
}

function ObjectOwnershipTreeScreen({
  initialKind,
  movePackage,
}: {
  initialKind: ObjectDetailTabKind;
  movePackage: MovePackage | null;
}) {
  const containerRef = React.useRef<HTMLDivElement | null>(null);
  const [activeKind, setActiveKind] = React.useState<ObjectDetailTabKind>(initialKind);
  const [query, setQuery] = React.useState("");
  const [highRiskOnly, setHighRiskOnly] = React.useState(false);
  const [showTestFunctions, setShowTestFunctions] = React.useState(false);
  const [treePaneWidth, setTreePaneWidth] = React.useState(OBJECT_TREE_PANE_DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = React.useState(false);
  const allLifecycleMaps = React.useMemo(
    () => (movePackage ? objectLifecycleViewMaps(movePackage) : []),
    [movePackage],
  );
  const lifecycleMaps = React.useMemo(
    () =>
      movePackage
        ? lifecycleMapsWithTestFunctionVisibility(
            allLifecycleMaps,
            movePackage.modules,
            showTestFunctions,
          )
        : [],
    [allLifecycleMaps, movePackage, showTestFunctions],
  );
  const groups = React.useMemo(
    () => activeKind === "capabilities" ? [] : objectLifecycleGroups(lifecycleMaps, activeKind, query, highRiskOnly),
    [activeKind, highRiskOnly, lifecycleMaps, query],
  );
  const tabs = React.useMemo(
    () => objectOwnershipTabs(movePackage),
    [movePackage],
  );
  const lifecycleByKey = React.useMemo(
    () =>
      new Map(
        lifecycleMaps.map((lifecycleMap) => [
          lifecycleMap.qualifiedName,
          lifecycleMap,
        ]),
      ),
    [lifecycleMaps],
  );
  const [collapsedGroups, setCollapsedGroups] = React.useState<Set<string>>(() => new Set());
  const firstItemKey = groups[0]?.items[0]?.objectKey ?? null;
  const [selectedKey, setSelectedKey] = React.useState<string | null>(firstItemKey);
  const selectedLifecycleMap = selectedKey ? lifecycleByKey.get(selectedKey) ?? null : null;

  React.useEffect(() => {
    setActiveKind(initialKind);
  }, [initialKind, movePackage?.manifestPath]);

  React.useEffect(() => {
    setCollapsedGroups(new Set());
  }, [activeKind, movePackage?.manifestPath, showTestFunctions]);

  React.useEffect(() => {
    if (!groups.length) {
      setSelectedKey(null);
      return;
    }

    if (!selectedKey || !groups.some((group) => group.items.some((item) => item.objectKey === selectedKey))) {
      setSelectedKey(groups[0]?.items[0]?.objectKey ?? null);
    }
  }, [groups, selectedKey]);

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
      OBJECT_TREE_PANE_MIN_WIDTH,
      Math.min(OBJECT_TREE_PANE_MAX_WIDTH, bounds.width - OBJECT_TREE_DETAIL_PANE_MIN_WIDTH),
    );
    const nextWidth = Math.min(
      maxWidth,
      Math.max(OBJECT_TREE_PANE_MIN_WIDTH, clientX - bounds.left),
    );
    setTreePaneWidth(nextWidth);
  }, []);

  return (
    <section
      ref={containerRef}
      className={cn("grid h-full min-h-0 bg-[var(--app-window)]", isResizing && "select-none")}
      style={{ gridTemplateColumns: `${treePaneWidth}px 6px minmax(0, 1fr)` }}
    >
      <div className="grid min-h-0 grid-rows-[auto_1fr]">
        <div className="grid gap-3 border-b border-[color:var(--app-border)] px-5 py-4">
          <div className="grid grid-cols-5 gap-1 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-1">
            {tabs.map((tab) => {
              const TabIcon = tab.icon;
              const active = activeKind === tab.kind;

              return (
                <button
                  aria-pressed={active}
                  className={cn(
                    "grid min-w-0 gap-1 rounded px-2 py-2 text-left transition hover:bg-[var(--app-subtle)]",
                    active && "border-primary/35 bg-[var(--app-subtle)] text-foreground shadow-none",
                    !active && "text-muted-foreground",
                  )}
                  key={tab.kind}
                  onClick={() => setActiveKind(tab.kind)}
                  title={tab.title}
                  type="button"
                >
                  <span className="flex min-w-0 items-center justify-between gap-2">
                    <TabIcon className="size-3.5 shrink-0" aria-hidden="true" />
                    <span className={cn(
                      "rounded px-1.5 py-0.5 text-[10px] font-semibold",
                      active ? tab.activeCountClassName : "bg-[var(--app-subtle)] text-muted-foreground",
                    )}>
                      {tab.count}
                    </span>
                  </span>
                  <span className="truncate text-[11px] font-medium leading-4">{tab.shortTitle}</span>
                </button>
              );
            })}
          </div>
          {activeKind === "capabilities" ? null : (
            <div className="flex h-9 items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3">
              <Search className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
              <input
                className="min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search modules / objects"
                value={query}
              />
              <button
                aria-pressed={highRiskOnly}
                className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground data-[active=true]:bg-primary/10 data-[active=true]:text-primary"
                data-active={highRiskOnly}
                onClick={() => setHighRiskOnly((active) => !active)}
                title="Show high-risk objects"
                type="button"
              >
                <Filter className="size-3.5" aria-hidden="true" />
              </button>
              <button
                aria-pressed={showTestFunctions}
                className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground data-[active=true]:bg-primary/10 data-[active=true]:text-primary"
                data-active={showTestFunctions}
                onClick={() => setShowTestFunctions((visible) => !visible)}
                title={showTestFunctions ? "Hide test helpers" : "Show test helpers"}
                type="button"
              >
                <TestTube2 className="size-3.5" aria-hidden="true" />
              </button>
            </div>
          )}
        </div>

        <ScrollArea className="min-h-0">
          <div className="px-5 py-4">
            {activeKind === "capabilities" ? (
              movePackage ? (
                <CapabilitiesDetail movePackage={movePackage} />
              ) : (
                <ObjectTreeEmpty label="No active Move package selected." />
              )
            ) : movePackage ? (
              groups.length ? (
                <div className="grid gap-3">
                  {groups.map((group) => {
                    const collapsed = collapsedGroups.has(group.key);

                    return (
                      <ObjectTreeGroup
                        collapsed={collapsed}
                        group={group}
                        key={group.key}
                        onSelectItem={setSelectedKey}
                        onToggle={() => {
                          setCollapsedGroups((current) => {
                            const next = new Set(current);

                            if (next.has(group.key)) {
                              next.delete(group.key);
                            } else {
                              next.add(group.key);
                            }

                            return next;
                          });
                        }}
                        selectedKey={selectedKey}
                      />
                    );
                  })}
                </div>
              ) : (
                <ObjectTreeEmpty label="No matching object lifecycle findings." />
              )
            ) : (
              <ObjectTreeEmpty label="No active Move package selected." />
            )}
          </div>
        </ScrollArea>
      </div>

      <div
        aria-label="Resize object analysis tree"
        aria-orientation="vertical"
        className={cn(
          "group relative cursor-col-resize border-r border-[color:var(--app-border)]",
          isResizing && "border-primary/50",
        )}
        onDragStart={(event) => event.preventDefault()}
        onPointerCancel={() => setIsResizing(false)}
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

      {activeKind === "capabilities" ? (
        <CapabilityOverview movePackage={movePackage} />
      ) : (
        <ObjectLifecycleDetail lifecycleMap={selectedLifecycleMap} />
      )}
    </section>
  );
}

type ObjectTreeGroupModel = {
  count: number;
  fileName: string;
  items: ObjectTreeItemModel[];
  key: string;
};

type ObjectTreeItemModel = {
  badge: string;
  children?: ObjectTreeItemModel[];
  key: string;
  label: string;
  meta?: string;
  objectKey: string;
  type: "struct" | "function";
};

function ObjectTreeGroup({
  collapsed,
  group,
  onSelectItem,
  onToggle,
  selectedKey,
}: {
  collapsed: boolean;
  group: ObjectTreeGroupModel;
  onSelectItem: (key: string) => void;
  onToggle: () => void;
  selectedKey: string | null;
}) {
  return (
    <div>
      <button
        className="flex h-8 w-full items-center gap-2 rounded-md px-1.5 text-left text-sm font-semibold text-foreground transition hover:bg-[var(--app-subtle)]"
        onClick={onToggle}
        type="button"
      >
        {collapsed ? (
          <ChevronRight className="size-3.5 text-muted-foreground" aria-hidden="true" />
        ) : (
          <ChevronDown className="size-3.5 text-muted-foreground" aria-hidden="true" />
        )}
        <FileCode2 className="size-4 text-muted-foreground" aria-hidden="true" />
        <span className="min-w-0 flex-1 truncate">{group.fileName}</span>
        <span className="rounded bg-[var(--app-subtle)] px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">
          {group.count}
        </span>
      </button>

      {!collapsed ? (
        <div className="relative ml-[25px] mt-1 grid gap-1 border-l border-[color:var(--app-border)] pl-4">
          {group.items.map((item) => (
            <React.Fragment key={item.key}>
              <ObjectTreeItem
                item={item}
                onSelect={() => onSelectItem(item.objectKey)}
                selected={selectedKey === item.objectKey && item.type === "struct"}
              />
              {selectedKey === item.objectKey && item.children?.length ? (
                <div className="relative ml-5 grid gap-1 border-l border-[color:var(--app-border)]/75 pl-4">
                  {item.children.map((child) => (
                    <ObjectTreeItem
                      item={child}
                      key={child.key}
                      onSelect={() => onSelectItem(child.objectKey)}
                      selected={false}
                    />
                  ))}
                </div>
              ) : null}
            </React.Fragment>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function ObjectTreeItem({
  item,
  onSelect,
  selected,
}: {
  item: ObjectTreeItemModel;
  onSelect: () => void;
  selected: boolean;
}) {
  return (
    <button
      className={[
        "group relative grid min-h-9 w-full grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 rounded-md px-2 py-1.5 text-left transition",
        selected
          ? "border-l-2 border-primary bg-primary/15 text-primary shadow-none"
          : "text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
      ].join(" ")}
      onClick={onSelect}
      title={item.type === "function" ? `${item.label} touches this object` : item.label}
      type="button"
    >
      <span className="absolute -left-[17px] top-1/2 h-px w-4 bg-[var(--app-border)]" />
      {item.type === "struct" ? (
        <Box className="size-3.5 shrink-0" aria-hidden="true" />
      ) : (
        <span className="inline-flex size-3.5 shrink-0 items-center justify-center font-serif text-sm italic leading-none">
          f
        </span>
      )}
      <span className="min-w-0 truncate text-sm">
        <span className="mr-1 text-xs text-muted-foreground/80">
          {item.type === "struct" ? "struct" : "fn"}
        </span>
        {item.label}
        {item.meta ? <span className="ml-1 text-xs text-muted-foreground/70">{item.meta}</span> : null}
      </span>
      <span className="rounded bg-[var(--app-subtle)] px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
        {item.badge}
      </span>
    </button>
  );
}

function renderDetail(detail: SurfaceDetailKind, movePackage: MovePackage) {
  switch (detail) {
    case "entry-functions":
      return <EntryFunctionsDetail movePackage={movePackage} />;
    case "capabilities":
      return <CapabilitiesDetail movePackage={movePackage} />;
    case "objects":
    case "shared-objects":
    case "address-owned":
    case "immutable-objects":
    case "wrapped-objects":
    case "party-objects":
      return null;
    case "admin-controls":
      return <AdminControlsDetail findings={movePackage.surface.adminControlFindings} />;
    case "external-calls":
      return <ExternalCallsDetail findings={movePackage.surface.externalCallFindings} />;
    case "package-internals":
      return <PackageInternalsDetail relationships={movePackage.surface.publicPackageRelationships} />;
  }
}

function EntryFunctionsDetail({ movePackage }: { movePackage: MovePackage }) {
  const functions = movePackage.modules.flatMap((module) =>
    module.functions
      .filter((functionSignature) => functionSignature.isTransactionCallable)
      .map((functionSignature) => ({ functionSignature, moduleName: module.name })),
  );

  return (
    <FindingList
      emptyLabel="No transaction-callable functions found."
      items={functions}
      renderItem={({ functionSignature, moduleName }) => (
        <FunctionCard
          key={`${moduleName}::${functionSignature.name}`}
          functionSignature={functionSignature}
          moduleName={moduleName}
        />
      )}
    />
  );
}

function ownershipKindForDetail(detail: SurfaceDetailKind): ObjectOwnershipFinding["ownershipKind"] | null {
  switch (detail) {
    case "shared-objects":
      return "shared";
    case "address-owned":
      return "addressOwned";
    case "immutable-objects":
      return "immutable";
    case "wrapped-objects":
      return "wrapped";
    case "party-objects":
      return "party";
    default:
      return null;
  }
}

function objectOwnershipTabs(movePackage: MovePackage | null) {
  const surface = movePackage?.surface;

  return [
    {
      activeCountClassName: "bg-amber-500/20 text-amber-200",
      count: surface?.capabilityCount ?? 0,
      icon: KeyRound,
      kind: "capabilities" as const,
      shortTitle: "Caps",
      title: "Capabilities",
    },
    {
      activeCountClassName: "bg-yellow-500/20 text-yellow-200",
      count: surface?.sharedObjectCount ?? 0,
      icon: Boxes,
      kind: "shared" as const,
      shortTitle: "Shared",
      title: "Shared Objects",
    },
    {
      activeCountClassName: "bg-muted text-foreground",
      count: surface?.addressOwnedObjectCount ?? 0,
      icon: Box,
      kind: "addressOwned" as const,
      shortTitle: "Owned",
      title: "Address-Owned Objects",
    },
    {
      activeCountClassName: "bg-emerald-500/20 text-emerald-200",
      count: surface?.immutableObjectCount ?? 0,
      icon: Lock,
      kind: "immutable" as const,
      shortTitle: "Immutable",
      title: "Immutable Objects",
    },
    {
      activeCountClassName: "bg-violet-500/20 text-violet-200",
      count: surface?.wrappedObjectCount ?? 0,
      icon: Boxes,
      kind: "wrapped" as const,
      shortTitle: "Wrapped",
      title: "Wrapped Objects",
    },
    {
      activeCountClassName: "bg-purple-500/20 text-purple-200",
      count: surface?.partyObjectCount ?? 0,
      icon: UsersRound,
      kind: "party" as const,
      shortTitle: "Party",
      title: "Party Objects",
    },
  ];
}

function CapabilitiesDetail({ movePackage }: { movePackage: MovePackage }) {
  return (
    <FindingList
      emptyLabel="No capability-like structs found."
      items={movePackage.surface.capabilityFindings.filter((finding) => finding.confidence !== "low")}
      renderItem={(finding) => (
        <FindingCard
          key={finding.qualifiedName}
          badge={finding.confidence}
          evidence={finding.evidence}
          title={finding.qualifiedName}
          subtitle={finding.protectedFunctions.length ? "Protects privileged functions" : "Capability candidate"}
        >
          <InlineList label="Protected functions" values={finding.protectedFunctions} />
        </FindingCard>
      )}
    />
  );
}

function CapabilityOverview({ movePackage }: { movePackage: MovePackage | null }) {
  const findings = movePackage?.surface.capabilityFindings.filter((finding) => finding.confidence !== "low") ?? [];
  const protectedFunctionCount = new Set(findings.flatMap((finding) => finding.protectedFunctions)).size;

  return (
    <div className="grid h-full min-h-0 place-items-center bg-[var(--app-window)] p-6">
      <div className="w-full max-w-xl rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-5">
        <div className="flex items-center gap-3">
          <span className="grid size-10 place-items-center rounded-md bg-amber-500/10 text-amber-200">
            <KeyRound className="size-5" aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <h2 className="text-base font-semibold">Capabilities</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              Authority-bearing structs and the privileged functions they protect.
            </p>
          </div>
        </div>
        <div className="mt-5 grid grid-cols-2 gap-3 text-sm">
          <div className="rounded-md bg-[var(--app-subtle)] p-3">
            <div className="text-2xl font-semibold">{findings.length}</div>
            <div className="mt-1 text-xs text-muted-foreground">capability structs</div>
          </div>
          <div className="rounded-md bg-[var(--app-subtle)] p-3">
            <div className="text-2xl font-semibold">{protectedFunctionCount}</div>
            <div className="mt-1 text-xs text-muted-foreground">protected functions</div>
          </div>
        </div>
      </div>
    </div>
  );
}

function objectLifecycleViewMaps(movePackage: MovePackage): ObjectLifecycleMap[] {
  const byObject = new Map<string, ObjectLifecycleMap>();

  for (const lifecycleMap of movePackage.surface.objectLifecycleMaps ?? []) {
    byObject.set(lifecycleMap.qualifiedName, {
      ...lifecycleMap,
      stages: lifecycleMap.stages.map((stage) => ({
        ...stage,
        functions: [...stage.functions],
        evidence: [...stage.evidence],
      })),
      touchedBy: [...lifecycleMap.touchedBy],
      risks: [...lifecycleMap.risks],
    });
  }

  for (const finding of movePackage.surface.objectOwnershipFindings ?? []) {
    if (finding.confidence === "low") {
      continue;
    }

    const stageKind = lifecycleStageForOwnershipKind(finding.ownershipKind);
    const existing = byObject.get(finding.qualifiedName);
    const functionRefs = ownershipFindingFunctionRefs(movePackage.modules, finding, stageKind);
    const evidence = finding.evidence.length
      ? finding.evidence
      : [`${finding.ownershipKind} ownership detected by surface scanner`];

    if (existing) {
      addFallbackLifecycleStage(existing, stageKind, functionRefs, evidence);
      for (const functionRef of functionRefs) {
        if (!existing.touchedBy.some((current) => current.qualifiedName === functionRef.qualifiedName && current.direct === functionRef.direct)) {
          existing.touchedBy.push(functionRef);
        }
      }
      continue;
    }

    const module = movePackage.modules.find((candidate) => candidate.name === finding.moduleName);
    const moveStruct = module?.structs.find((candidate) => candidate.name === finding.typeName);

    byObject.set(finding.qualifiedName, {
      typeName: finding.typeName,
      moduleName: finding.moduleName,
      qualifiedName: finding.qualifiedName,
      filePath: module?.filePath ?? "",
      abilities: moveStruct?.abilities?.length ? moveStruct.abilities : ["key"],
      isCapabilityLike:
        movePackage.surface.capabilityStructs.includes(finding.qualifiedName) ||
        /cap|admin|treasury|authority/i.test(finding.typeName),
      stages: [
        {
          kind: stageKind,
          functions: functionRefs,
          evidence,
        },
      ],
      touchedBy: functionRefs,
      risks: [],
    });
  }

  for (const lifecycleMap of byObject.values()) {
    addSourceDetectedStage(movePackage.modules, lifecycleMap, "created");
    addSourceDetectedStage(movePackage.modules, lifecycleMap, "mutated");
    lifecycleMap.touchedBy = uniqueLifecycleFunctions(lifecycleMap.touchedBy);
  }

  return [...byObject.values()].sort((left, right) =>
    left.filePath.localeCompare(right.filePath) || left.qualifiedName.localeCompare(right.qualifiedName),
  );
}

function lifecycleMapsWithTestFunctionVisibility(
  lifecycleMaps: ObjectLifecycleMap[],
  modules: MoveModule[],
  showTestFunctions: boolean,
) {
  if (showTestFunctions) {
    return lifecycleMaps;
  }

  const testFunctionKeys = testFunctionKeySet(modules);

  return lifecycleMaps
    .map((lifecycleMap) => filterLifecycleMapTestFunctions(lifecycleMap, testFunctionKeys))
    .filter((lifecycleMap) => lifecycleMap.stages.length || lifecycleMap.risks.length);
}

function filterLifecycleMapTestFunctions(
  lifecycleMap: ObjectLifecycleMap,
  testFunctionKeys: Set<string>,
): ObjectLifecycleMap {
  const hiddenFunctions = lifecycleMap.touchedBy.filter((functionRef) =>
    isTestLifecycleFunction(functionRef, testFunctionKeys),
  );
  const stages = lifecycleMap.stages
    .map((stage) => {
      const functions = stage.functions.filter((functionRef) =>
        !isTestLifecycleFunction(functionRef, testFunctionKeys),
      );
      const evidence = stage.evidence.filter((item) =>
        !hiddenFunctions.some((functionRef) => evidenceReferencesFunction(item, functionRef)),
      );

      return {
        ...stage,
        functions,
        evidence,
      };
    })
    .filter((stage) => stage.functions.length || stage.evidence.length);
  const touchedBy = lifecycleMap.touchedBy.filter((functionRef) =>
    !isTestLifecycleFunction(functionRef, testFunctionKeys),
  );
  const risks = lifecycleMap.risks
    .map((risk) => {
      const originalFunctionCount = risk.functions.length;
      const functions = risk.functions.filter((functionRef) =>
        !isTestLifecycleFunction(functionRef, testFunctionKeys),
      );
      const hiddenRiskFunctions = risk.functions.filter((functionRef) =>
        isTestLifecycleFunction(functionRef, testFunctionKeys),
      );
      const evidence = risk.evidence.filter((item) =>
        !hiddenRiskFunctions.some((functionRef) => evidenceReferencesFunction(item, functionRef)),
      );

      return {
        ...risk,
        originalFunctionCount,
        functions,
        evidence,
      };
    })
    .filter((risk) => risk.functions.length || risk.evidence.length || risk.originalFunctionCount === 0)
    .map((risk) => ({
      kind: risk.kind,
      severity: risk.severity,
      message: risk.message,
      evidence: risk.evidence,
      functions: risk.functions,
    }));

  return {
    ...lifecycleMap,
    stages,
    touchedBy,
    risks,
  };
}

function testFunctionKeySet(modules: MoveModule[]) {
  const keys = new Set<string>();

  for (const module of modules) {
    const moduleIsTest = isTestModule(module);

    for (const functionSignature of module.functions) {
      if (
        moduleIsTest ||
        hasTestAttribute(functionSignature.attributes) ||
        isTestLikeFunctionName(functionSignature.name)
      ) {
        keys.add(`${module.name}::${functionSignature.name}`);
      }
    }
  }

  return keys;
}

function isTestLifecycleFunction(
  functionRef: ObjectLifecycleFunctionRef,
  testFunctionKeys: Set<string>,
) {
  return (
    testFunctionKeys.has(functionRef.qualifiedName) ||
    isTestLikeFunctionName(functionRef.functionName) ||
    isTestPath(functionRef.filePath) ||
    functionRef.callPath.some((qualifiedName) => {
      const [, functionName] = splitQualifiedFunctionName(qualifiedName, functionRef.moduleName);

      return testFunctionKeys.has(qualifiedName) || isTestLikeFunctionName(functionName);
    })
  );
}

function evidenceReferencesFunction(
  evidence: string,
  functionRef: ObjectLifecycleFunctionRef,
) {
  return (
    evidence.includes(functionRef.qualifiedName) ||
    evidence.includes(`${functionRef.functionName}(`) ||
    evidence.includes(functionRef.functionName)
  );
}

function isTestPath(filePath: string) {
  return filePath
    .split(/[\\/]/)
    .some((segment) => {
      const normalized = segment.toLocaleLowerCase();

      return (
        ["test", "tests", "testing", "fixtures", "fixture", "mocks"].includes(normalized) ||
        normalized.startsWith("test_") ||
        normalized.endsWith("_test.move") ||
        normalized.includes("fixture") ||
        normalized.includes("mock")
      );
    });
}

function isTestLikeFunctionName(functionName: string) {
  const normalized = functionName.toLocaleLowerCase();

  return (
    normalized.startsWith("test_") ||
    normalized.endsWith("_test") ||
    normalized.includes("_test_") ||
    normalized.includes("for_testing") ||
    normalized.endsWith("_for_testing") ||
    normalized.startsWith("mock_") ||
    normalized.endsWith("_mock") ||
    normalized.startsWith("fixture_") ||
    normalized.endsWith("_fixture")
  );
}

function lifecycleStageForOwnershipKind(kind: ObjectOwnershipFinding["ownershipKind"]): ObjectLifecycleStageKind {
  switch (kind) {
    case "shared":
      return "shared";
    case "addressOwned":
      return "owned";
    case "immutable":
      return "immutable";
    case "wrapped":
      return "wrapped";
    case "party":
      return "party";
  }
}

function addFallbackLifecycleStage(
  lifecycleMap: ObjectLifecycleMap,
  stageKind: ObjectLifecycleStageKind,
  functionRefs: ObjectLifecycleFunctionRef[],
  evidence: string[],
) {
  const existing = lifecycleMap.stages.find((stage) => stage.kind === stageKind);

  if (!existing) {
    lifecycleMap.stages.push({
      kind: stageKind,
      functions: functionRefs,
      evidence,
    });
    lifecycleMap.stages.sort((left, right) => lifecycleStageRank(left.kind) - lifecycleStageRank(right.kind));
    return;
  }

  for (const item of evidence) {
    if (!existing.evidence.includes(item)) {
      existing.evidence.push(item);
    }
  }

  for (const functionRef of functionRefs) {
    if (!existing.functions.some((current) => current.qualifiedName === functionRef.qualifiedName && current.direct === functionRef.direct)) {
      existing.functions.push(functionRef);
    }
  }
}

function ownershipFindingFunctionRefs(
  modules: MoveModule[],
  finding: ObjectOwnershipFinding,
  stageKind: ObjectLifecycleStageKind,
): ObjectLifecycleFunctionRef[] {
  return uniqueStrings(finding.relatedFunctions)
    .map((qualifiedName) => ownershipFunctionRef(modules, finding.moduleName, qualifiedName, finding, stageKind))
    .filter((functionRef): functionRef is ObjectLifecycleFunctionRef => Boolean(functionRef));
}

function ownershipFunctionRef(
  modules: MoveModule[],
  fallbackModuleName: string,
  relatedFunction: string,
  finding: ObjectOwnershipFinding,
  stageKind: ObjectLifecycleStageKind,
): ObjectLifecycleFunctionRef | null {
  const normalized = relatedFunction.trim();

  if (!normalized) {
    return null;
  }

  const [moduleName, functionName] = splitQualifiedFunctionName(normalized, fallbackModuleName);
  const module = modules.find((candidate) => candidate.name === moduleName);
  const functionSignature = module?.functions.find((candidate) => candidate.name === functionName);

  if (
    module &&
    functionSignature &&
    !sourceFunctionTouchesStage(functionSignature, stageKind, finding.typeName, finding.qualifiedName)
  ) {
    return null;
  }

  return {
    moduleName,
    functionName,
    qualifiedName: `${moduleName}::${functionName}`,
    filePath: module?.filePath ?? "",
    visibility: functionSignature?.visibility ?? "unknown",
    isEntry: functionSignature?.isEntry ?? false,
    isTransactionCallable: functionSignature?.isTransactionCallable ?? false,
    direct: true,
    callPath: [],
    evidence: [`related function ${moduleName}::${functionName}`],
  };
}

function addSourceDetectedStage(
  modules: MoveModule[],
  lifecycleMap: ObjectLifecycleMap,
  stageKind: ObjectLifecycleStageKind,
) {
  if (hasLifecycleStage(lifecycleMap, stageKind)) {
    return;
  }

  const functions = sourceLifecycleFunctionRefs(modules, lifecycleMap, stageKind);

  if (!functions.length) {
    return;
  }

  addFallbackLifecycleStage(
    lifecycleMap,
    stageKind,
    functions,
    functions.map((functionRef) => sourceDetectedEvidence(lifecycleMap.typeName, stageKind, functionRef.qualifiedName)),
  );

  for (const functionRef of functions) {
    if (!lifecycleMap.touchedBy.some((current) => current.qualifiedName === functionRef.qualifiedName && current.direct === functionRef.direct)) {
      lifecycleMap.touchedBy.push(functionRef);
    }
  }
}

function sourceLifecycleFunctionRefs(
  modules: MoveModule[],
  lifecycleMap: ObjectLifecycleMap,
  stageKind: ObjectLifecycleStageKind,
): ObjectLifecycleFunctionRef[] {
  return modules
    .filter((module) => !isTestModule(module))
    .flatMap((module) =>
      module.functions
        .filter((functionSignature) => !hasTestAttribute(functionSignature.attributes))
        .filter((functionSignature) =>
          sourceFunctionTouchesStage(functionSignature, stageKind, lifecycleMap.typeName, lifecycleMap.qualifiedName),
        )
        .map((functionSignature) =>
          lifecycleFunctionRefFromSource(
            module,
            functionSignature,
            `${stageKind} path detected from source`,
          ),
        ),
    );
}

function sourceFunctionTouchesStage(
  functionSignature: MoveFunctionSignature,
  stageKind: ObjectLifecycleStageKind,
  typeName: string,
  qualifiedName: string,
) {
  const body = functionSignature.body ?? "";

  switch (stageKind) {
    case "created":
      return sourceFunctionCreatesType(functionSignature, typeName, qualifiedName);
    case "owned":
      return (
        (functionSignature.isTransactionCallable && sourceFunctionReturnsType(functionSignature.signature, typeName, qualifiedName)) ||
        sourceOperationTouchesType(body, functionSignature.signature, typeName, qualifiedName, [
          "transfer::transfer",
          "transfer::public_transfer",
          "public_transfer",
        ])
      );
    case "transferred":
      return (
        (functionSignature.isTransactionCallable && sourceFunctionReturnsType(functionSignature.signature, typeName, qualifiedName)) ||
        sourceOperationTouchesType(body, functionSignature.signature, typeName, qualifiedName, [
          "transfer::transfer",
          "transfer::public_transfer",
          "public_transfer",
        ])
      );
    case "shared":
      return sourceOperationTouchesType(body, functionSignature.signature, typeName, qualifiedName, [
        "transfer::share_object",
        "transfer::public_share_object",
        "share_object",
      ]);
    case "immutable":
      return sourceOperationTouchesType(body, functionSignature.signature, typeName, qualifiedName, [
        "transfer::freeze_object",
        "transfer::public_freeze_object",
        "freeze_object",
      ]);
    case "party":
      return sourceOperationTouchesType(body, functionSignature.signature, typeName, qualifiedName, [
        "transfer::party_transfer",
        "transfer::public_party_transfer",
        "party_transfer",
        "party::",
      ]);
    case "mutated":
      return (
        sourceFunctionMutablyTouchesType(functionSignature.signature, typeName, qualifiedName) ||
        sourceBorrowedIdentityMutatesRelatedState(body, functionSignature.signature, typeName, qualifiedName)
      );
    case "deleted":
      return sourceDeleteTouchesType(body, functionSignature.signature, typeName, qualifiedName);
    case "wrapped":
      return sourceOperationTouchesType(body, functionSignature.signature, typeName, qualifiedName, [
        "dynamic_field::add",
        "dynamic_object_field::add",
        "table::add",
        "bag::add",
      ]);
  }
}

function sourceDetectedEvidence(
  typeName: string,
  stageKind: ObjectLifecycleStageKind,
  qualifiedName: string,
) {
  switch (stageKind) {
    case "created":
      return `${typeName} constructed in ${qualifiedName}`;
    case "mutated":
      return `${qualifiedName} mutates state keyed by ${typeName} identity`;
    default:
      return `${stageKind} path detected in ${qualifiedName}`;
  }
}

function lifecycleFunctionRefFromSource(
  module: MoveModule,
  functionSignature: MoveFunctionSignature,
  evidence: string,
): ObjectLifecycleFunctionRef {
  return {
    moduleName: module.name,
    functionName: functionSignature.name,
    qualifiedName: `${module.name}::${functionSignature.name}`,
    filePath: module.filePath,
    visibility: functionSignature.visibility,
    isEntry: functionSignature.isEntry,
    isTransactionCallable: functionSignature.isTransactionCallable,
    direct: true,
    callPath: [],
    evidence: [evidence],
  };
}

function sourceFunctionCreatesType(
  functionSignature: MoveFunctionSignature,
  typeName: string,
  qualifiedName: string,
) {
  const body = functionSignature.body ?? "";

  return (
    (sourceConstructsType(body, typeName) || sourceConstructsType(body, qualifiedName)) &&
    (body.toLocaleLowerCase().includes("object::new") ||
      sourceFunctionReturnsType(functionSignature.signature, typeName, qualifiedName))
  );
}

function sourceFunctionReturnsType(signature: string, typeName: string, qualifiedName: string) {
  const closeParameters = signature.lastIndexOf(")");

  if (closeParameters === -1) {
    return false;
  }

  const returnType = signature.slice(closeParameters + 1).trimStart();
  const normalizedReturnType = returnType.startsWith(":") ? returnType.slice(1) : "";

  return sourceTypeReferenceMatches(normalizedReturnType, typeName) || sourceTypeReferenceMatches(normalizedReturnType, qualifiedName);
}

function sourceFunctionMutablyTouchesType(signature: string, typeName: string, qualifiedName: string) {
  const parameters = sourceFunctionParameters(signature);

  return Boolean(
    parameters &&
      sourceSplitTopLevel(parameters, ",").some((parameter) => {
        const [, parameterType] = parameter.split(":");
        const normalizedParameterType = parameterType?.trimStart();

        return (
          normalizedParameterType?.startsWith("&mut") &&
          (sourceTypeReferenceMatches(normalizedParameterType, typeName) ||
            sourceTypeReferenceMatches(normalizedParameterType, qualifiedName))
        );
      }),
  );
}

function sourceBorrowedIdentityMutatesRelatedState(
  body: string,
  signature: string,
  typeName: string,
  qualifiedName: string,
) {
  const bodyBlock = sourceFunctionBodyBlock(body);
  const borrowedNames = uniqueStrings([
    ...sourceBorrowedParameterNames(signature, typeName),
    ...sourceBorrowedParameterNames(signature, qualifiedName),
  ]);

  if (!borrowedNames.length) {
    return false;
  }

  const identityNames = sourceObjectIdentityNames(bodyBlock, borrowedNames);

  if (!identityNames.length) {
    return false;
  }

  return bodyBlock.split(";").some(
    (statement) =>
      identityNames.some((identityName) => sourceContainsIdentifier(statement, identityName)) &&
      sourceStatementHasMutationSignal(statement),
  );
}

function sourceFunctionBodyBlock(functionSource: string) {
  const start = functionSource.indexOf("{");
  const end = functionSource.lastIndexOf("}");

  if (start === -1) {
    return functionSource;
  }

  if (end === -1 || end <= start) {
    return functionSource.slice(start + 1);
  }

  return functionSource.slice(start + 1, end);
}

function sourceOperationTouchesType(
  body: string,
  signature: string,
  typeName: string,
  qualifiedName: string,
  operations: string[],
) {
  const valueNames = sourceOwnedOrConstructedValueNames(body, signature, typeName, qualifiedName);

  return sourceOperationSnippets(body, operations).some(
    (snippet) =>
      sourceConstructsType(snippet, typeName) ||
      sourceConstructsType(snippet, qualifiedName) ||
      valueNames.some((valueName) => sourceContainsIdentifier(snippet, valueName)),
  );
}

function sourceDeleteTouchesType(body: string, signature: string, typeName: string, qualifiedName: string) {
  const lowerBody = body.toLocaleLowerCase();

  if (
    !lowerBody.includes(".delete(") &&
    !lowerBody.includes("object::delete") &&
    !lowerBody.includes("id.delete") &&
    !lowerBody.includes("uid.delete")
  ) {
    return false;
  }

  return (
    sourceDestructuresType(body, typeName) ||
    sourceDestructuresType(body, qualifiedName) ||
    sourceOwnedOrConstructedValueNames(body, signature, typeName, qualifiedName).some((valueName) =>
      sourceContainsIdentifier(body, valueName),
    )
  );
}

function sourceOwnedOrConstructedValueNames(
  body: string,
  signature: string,
  typeName: string,
  qualifiedName: string,
) {
  return uniqueStrings([
    ...sourceOwnedParameterNames(signature, typeName),
    ...sourceOwnedParameterNames(signature, qualifiedName),
    ...sourceConstructedValueNames(body, typeName),
    ...sourceConstructedValueNames(body, qualifiedName),
  ]);
}

function sourceOwnedParameterNames(signature: string, typeName: string) {
  const parameters = sourceFunctionParameters(signature);

  if (!parameters) {
    return [];
  }

  return sourceSplitTopLevel(parameters, ",")
    .map((parameter) => {
      const [name, parameterType] = parameter.split(":");

      if (!name || !parameterType || parameterType.trimStart().startsWith("&") || !sourceTypeReferenceMatches(parameterType, typeName)) {
        return null;
      }

      return name
        .trim()
        .split(/\s+/)
        .filter((part) => part !== "mut")
        .at(-1)
        ?.replace(/[^\w]/g, "") ?? null;
    })
    .filter((name): name is string => Boolean(name));
}

function sourceBorrowedParameterNames(signature: string, typeName: string) {
  const parameters = sourceFunctionParameters(signature);

  if (!parameters) {
    return [];
  }

  return sourceSplitTopLevel(parameters, ",")
    .map((parameter) => {
      const [name, parameterType] = parameter.split(":");
      const normalizedParameterType = parameterType?.trimStart();

      if (
        !name ||
        !normalizedParameterType ||
        !normalizedParameterType.startsWith("&") ||
        normalizedParameterType.startsWith("&mut") ||
        !sourceTypeReferenceMatches(normalizedParameterType, typeName)
      ) {
        return null;
      }

      return name
        .trim()
        .split(/\s+/)
        .at(-1)
        ?.replace(/[^\w]/g, "") ?? null;
    })
    .filter((name): name is string => Boolean(name));
}

function sourceObjectIdentityNames(body: string, objectNames: string[]) {
  return uniqueStrings(
    body
      .split(";")
      .map((statement) => {
        const [left, right] = statement.split("=");

        if (!left?.includes("let") || !right) {
          return null;
        }

        const derivesIdentity = objectNames.some(
          (objectName) =>
            right.includes(`object::id(${objectName})`) ||
            right.includes(`object::id(&${objectName})`) ||
            right.includes(`${objectName}.id`),
        );

        if (!derivesIdentity) {
          return null;
        }

        return left
          .split("let")
          .at(-1)
          ?.split(":")
          .at(0)
          ?.trim()
          .split(/\s+/)
          .filter((part) => part !== "mut")
          .at(-1)
          ?.replace(/[^\w]/g, "") ?? null;
      })
      .filter((name): name is string => Boolean(name)),
  );
}

function sourceStatementHasMutationSignal(statement: string) {
  const normalized = statement.toLocaleLowerCase();
  const signals = [
    "&mut",
    "_mut",
    "borrow_mut",
    "set_",
    "add_",
    "remove",
    "delete",
    "destroy",
    "insert",
    "push_back",
    ".add(",
    "::add(",
    ".remove(",
    "::remove(",
    "deposit",
    "withdraw",
    "supply",
    "split(",
    "decrease",
    "increase",
    "increment",
    "decrement",
    "latch",
    "refresh",
  ];

  return signals.some((signal) => normalized.includes(signal));
}

function sourceConstructedValueNames(body: string, typeName: string) {
  return body
    .split(";")
    .map((statement) => {
      const [left, right] = statement.split("=");

      if (!left?.includes("let") || !right || !sourceConstructsType(right, typeName)) {
        return null;
      }

      return left
        .split("let")
        .at(-1)
        ?.split(":")
        .at(0)
        ?.trim()
        .split(/\s+/)
        .filter((part) => part !== "mut")
        .at(-1)
        ?.replace(/[^\w]/g, "") ?? null;
    })
    .filter((name): name is string => Boolean(name));
}

function sourceFunctionParameters(signature: string) {
  const start = signature.indexOf("(");

  if (start === -1) {
    return null;
  }

  let depth = 0;

  for (let index = start; index < signature.length; index += 1) {
    const character = signature[index];

    if (character === "(") {
      depth += 1;
    } else if (character === ")") {
      depth -= 1;

      if (depth === 0) {
        return signature.slice(start + 1, index);
      }
    }
  }

  return null;
}

function sourceConstructsType(source: string, typeName: string) {
  const shortName = typeName.split("::").at(-1) ?? typeName;

  return [typeName, shortName].some((name) =>
    new RegExp(`\\b${escapeRegExp(name)}\\s*(?:<[^>{;]*>)?\\s*\\{`).test(source),
  );
}

function sourceDestructuresType(source: string, typeName: string) {
  const shortName = typeName.split("::").at(-1) ?? typeName;

  return [typeName, shortName].some((name) =>
    new RegExp(`\\blet\\s+${escapeRegExp(name)}\\s*(?:<[^>{;]*>)?\\s*\\{`).test(source),
  );
}

function sourceTypeReferenceMatches(source: string, typeName: string) {
  const shortName = typeName.split("::").at(-1) ?? typeName;

  return source
    .split(/[^\w:]+/)
    .some((token) => token === shortName || token === typeName);
}

function sourceOperationSnippets(body: string, operations: string[]) {
  const lowerBody = body.toLocaleLowerCase();
  const snippets: string[] = [];

  for (const operation of operations) {
    const lowerOperation = operation.toLocaleLowerCase();
    let start = 0;

    while (start < lowerBody.length) {
      const index = lowerBody.indexOf(lowerOperation, start);

      if (index === -1) {
        break;
      }

      const end = lowerBody.indexOf(";", index);
      snippets.push(body.slice(index, end === -1 ? body.length : end));
      start = end === -1 ? lowerBody.length : end + 1;
    }
  }

  return snippets;
}

function sourceSplitTopLevel(source: string, delimiter: string) {
  const parts: string[] = [];
  let start = 0;
  let angleDepth = 0;
  let parenDepth = 0;

  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];

    if (character === "<") {
      angleDepth += 1;
    } else if (character === ">") {
      angleDepth -= 1;
    } else if (character === "(") {
      parenDepth += 1;
    } else if (character === ")") {
      parenDepth -= 1;
    } else if (character === delimiter && angleDepth === 0 && parenDepth === 0) {
      parts.push(source.slice(start, index).trim());
      start = index + 1;
    }
  }

  parts.push(source.slice(start).trim());
  return parts;
}

function sourceContainsIdentifier(source: string, identifier: string) {
  return source.split(/[^\w]+/).some((token) => token === identifier);
}

function isTestModule(module: MoveModule) {
  return module.filePath.split("/").includes("tests") || hasTestAttribute(module.attributes);
}

function hasTestAttribute(attributes: string[] = []) {
  return attributes.some((attribute) => {
    const normalized = attribute.toLocaleLowerCase();
    return normalized.includes("test") || normalized.includes("test_only") || normalized.includes("expected_failure");
  });
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function splitQualifiedFunctionName(value: string, fallbackModuleName: string) {
  const parts = value.split("::").filter(Boolean);

  if (parts.length >= 2) {
    return [parts[parts.length - 2], parts[parts.length - 1]];
  }

  return [fallbackModuleName, value.replace(/\(\)$/, "")];
}

function lifecycleStageRank(kind: ObjectLifecycleStageKind) {
  const index = OBJECT_LIFECYCLE_STAGE_GROUPS.findIndex((stageGroup) =>
    stageGroup.stageKinds.includes(kind),
  );

  return index === -1 ? OBJECT_LIFECYCLE_STAGE_GROUPS.length : index;
}

function objectLifecycleGroups(
  lifecycleMaps: ObjectLifecycleMap[],
  kind: ObjectOwnershipFinding["ownershipKind"],
  query: string,
  highRiskOnly: boolean,
) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const groups = new Map<string, ObjectTreeGroupModel>();
  const visibleLifecycleMaps = lifecycleMaps.filter(
    (lifecycleMap) =>
      lifecycleMatchesKind(lifecycleMap, kind) &&
      (!highRiskOnly || highestRiskSeverity(lifecycleMap.risks) === "high") &&
      matchesLifecycleQuery(lifecycleMap, normalizedQuery),
  );

  for (const lifecycleMap of visibleLifecycleMaps) {
    const fileName = moduleFileName(lifecycleMap.filePath, lifecycleMap.moduleName);
    const group = ensureObjectTreeGroup(groups, lifecycleMap.filePath || fileName, fileName);
    const riskSeverity = highestRiskSeverity(lifecycleMap.risks);
    const functionRefs = uniqueLifecycleFunctions(lifecycleMap.touchedBy);

    group.items.push({
      badge: riskSeverity ?? `${functionRefs.length}`,
      children: functionRefs.map((functionRef) => {
        const stageLabel = functionStageBadge(lifecycleMap, functionRef);

        return {
          badge: stageLabel,
          key: `${kind}:${lifecycleMap.qualifiedName}:${functionRef.qualifiedName}:${functionRef.direct ? "direct" : "via"}`,
          label: `${functionRef.qualifiedName}()`,
          meta: functionRef.direct ? undefined : "indirect",
          objectKey: lifecycleMap.qualifiedName,
          type: "function" as const,
        };
      }),
      key: `${kind}:${lifecycleMap.qualifiedName}`,
      label: lifecycleMap.typeName,
      meta: [
        lifecycleMap.isCapabilityLike ? "capability" : null,
        `${lifecycleMap.stages.length} stages`,
      ].filter(Boolean).join(" · "),
      objectKey: lifecycleMap.qualifiedName,
      type: "struct",
    });
  }

  return [...groups.values()]
    .map((group) => ({
      ...group,
      count: group.items.length,
      items: group.items.sort((left, right) => left.label.localeCompare(right.label)),
    }))
    .sort((left, right) => left.fileName.localeCompare(right.fileName));
}

function functionStageBadge(
  lifecycleMap: ObjectLifecycleMap,
  functionRef: ObjectLifecycleFunctionRef,
) {
  const stages = lifecycleMap.stages
    .filter((stage) =>
      stage.functions.some((candidate) =>
        candidate.qualifiedName === functionRef.qualifiedName &&
        candidate.direct === functionRef.direct,
      ),
    )
    .map((stage) => stage.kind)
    .sort((left, right) => lifecycleStageRank(left) - lifecycleStageRank(right));

  if (!stages.length) {
    return functionRef.direct ? "direct" : "via";
  }

  return stages.length === 1 ? lifecycleStageBadge(stages[0]) : `${lifecycleStageBadge(stages[0])} +${stages.length - 1}`;
}

function lifecycleStageBadge(kind: ObjectLifecycleStageKind) {
  switch (kind) {
    case "created":
      return "create";
    case "owned":
      return "owned";
    case "mutated":
      return "mutate";
    case "transferred":
      return "transfer";
    case "shared":
      return "share";
    case "wrapped":
      return "wrap";
    case "immutable":
      return "freeze";
    case "party":
      return "party";
    case "deleted":
      return "delete";
  }
}

function ensureObjectTreeGroup(groups: Map<string, ObjectTreeGroupModel>, key: string, fileName: string) {
  const existing = groups.get(key);

  if (existing) {
    return existing;
  }

  const group: ObjectTreeGroupModel = {
    count: 0,
    fileName,
    items: [],
    key,
  };

  groups.set(key, group);
  return group;
}

function lifecycleMatchesKind(
  lifecycleMap: ObjectLifecycleMap,
  kind: ObjectOwnershipFinding["ownershipKind"],
) {
  switch (kind) {
    case "shared":
      return hasLifecycleStage(lifecycleMap, "shared");
    case "addressOwned":
      return hasLifecycleStage(lifecycleMap, "owned") || hasLifecycleStage(lifecycleMap, "transferred");
    case "immutable":
      return hasLifecycleStage(lifecycleMap, "immutable");
    case "wrapped":
      return hasLifecycleStage(lifecycleMap, "wrapped");
    case "party":
      return hasLifecycleStage(lifecycleMap, "party");
  }
}

function hasLifecycleStage(lifecycleMap: ObjectLifecycleMap, kind: ObjectLifecycleStageKind) {
  return lifecycleMap.stages.some((stage) => stage.kind === kind);
}

function matchesLifecycleQuery(lifecycleMap: ObjectLifecycleMap, query: string) {
  if (!query) {
    return true;
  }

  const haystack = [
    lifecycleMap.filePath,
    lifecycleMap.moduleName,
    lifecycleMap.qualifiedName,
    lifecycleMap.typeName,
    lifecycleMap.isCapabilityLike ? "capability" : "",
    ...lifecycleMap.abilities,
    ...lifecycleMap.risks.map((risk) => `${risk.severity} ${risk.message}`),
    ...lifecycleMap.stages.flatMap((stage) => [
      stage.kind,
      ...stage.evidence,
      ...stage.functions.map((functionRef) => functionRef.qualifiedName),
    ]),
  ]
    .filter(Boolean)
    .join(" ")
    .toLocaleLowerCase();

  return haystack.includes(query);
}

function uniqueLifecycleFunctions(functions: ObjectLifecycleFunctionRef[]) {
  const byFunction = new Map<string, ObjectLifecycleFunctionRef>();

  for (const functionRef of functions) {
    const existing = byFunction.get(functionRef.qualifiedName);

    if (!existing || (!existing.direct && functionRef.direct)) {
      byFunction.set(functionRef.qualifiedName, functionRef);
    }
  }

  return [...byFunction.values()].sort((left, right) =>
    left.filePath.localeCompare(right.filePath) || left.qualifiedName.localeCompare(right.qualifiedName),
  );
}

function highestRiskSeverity(risks: ObjectLifecycleMap["risks"]) {
  if (risks.some((risk) => risk.severity === "high")) {
    return "high";
  }

  if (risks.some((risk) => risk.severity === "medium")) {
    return "medium";
  }

  if (risks.some((risk) => risk.severity === "low")) {
    return "low";
  }

  return null;
}

function moduleFileName(filePath: string | undefined, moduleName: string) {
  if (!filePath) {
    return `${moduleName}.move`;
  }

  return filePath.split("/").at(-1) || `${moduleName}.move`;
}

function ObjectTreeEmpty({ label }: { label: string }) {
  return (
    <div className="flex min-h-44 items-center justify-center rounded-md border border-dashed border-[color:var(--app-border)] px-5 text-center text-sm text-muted-foreground">
      {label}
    </div>
  );
}

function ObjectLifecycleDetail({ lifecycleMap }: { lifecycleMap: ObjectLifecycleMap | null }) {
  if (!lifecycleMap) {
    return (
      <div className="flex min-h-0 items-center justify-center px-8 text-center text-sm text-muted-foreground">
        Select an object to inspect its lifecycle.
      </div>
    );
  }

  return (
    <ScrollArea className="min-h-0">
      <div className="grid gap-5 p-6">
        <div className="flex min-w-0 items-start justify-between gap-4 border-b border-[color:var(--app-border)] pb-5">
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              <Box className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
              <h2 className="truncate text-lg font-semibold tracking-tight">{lifecycleMap.qualifiedName}</h2>
              {lifecycleMap.isCapabilityLike ? (
                <Badge className="rounded bg-amber-400/10 px-2 py-0.5 text-[11px] text-amber-300" variant="secondary">
                  privileged
                </Badge>
              ) : null}
            </div>
            <p className="mt-1 truncate text-sm text-muted-foreground">{lifecycleMap.filePath}</p>
          </div>
          <div className="shrink-0 rounded-md bg-[var(--app-subtle)] px-2.5 py-1 text-xs text-muted-foreground">
            {lifecycleMap.stages.length} stages
          </div>
        </div>

        <LifecycleRail lifecycleMap={lifecycleMap} />

        <div className="grid gap-3">
          {OBJECT_LIFECYCLE_STAGE_GROUPS.map((stageGroup) => (
            <LifecycleStageCard
              key={stageGroup.id}
              lifecycleMap={lifecycleMap}
              stageGroup={stageGroup}
            />
          ))}
        </div>

        <LifecycleRiskList risks={lifecycleMap.risks} />
      </div>
    </ScrollArea>
  );
}

function LifecycleRail({ lifecycleMap }: { lifecycleMap: ObjectLifecycleMap }) {
  return (
    <div className="flex min-w-0 items-center overflow-x-auto rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3 py-2">
      {OBJECT_LIFECYCLE_STAGE_GROUPS.map((stageGroup, index) => {
        const active = stagesForGroup(lifecycleMap, stageGroup.stageKinds).length > 0;
        const Icon = stageGroup.icon;

        return (
          <React.Fragment key={stageGroup.id}>
            <div
              className={cn(
                "flex min-w-max items-center gap-2 rounded-md border px-2.5 py-1.5 text-xs font-medium",
                active ? stageGroup.accent : "border-transparent text-muted-foreground",
              )}
            >
              <Icon className="size-3.5" aria-hidden="true" />
              <span>{index + 1}</span>
              <span>{stageGroup.title}</span>
            </div>
            {index < OBJECT_LIFECYCLE_STAGE_GROUPS.length - 1 ? (
              <ArrowRight className="mx-2 size-4 shrink-0 text-muted-foreground/55" aria-hidden="true" />
            ) : null}
          </React.Fragment>
        );
      })}
    </div>
  );
}

function LifecycleStageCard({
  lifecycleMap,
  stageGroup,
}: {
  lifecycleMap: ObjectLifecycleMap;
  stageGroup: (typeof OBJECT_LIFECYCLE_STAGE_GROUPS)[number];
}) {
  const stages = stagesForGroup(lifecycleMap, stageGroup.stageKinds);
  const active = stages.length > 0;
  const Icon = stageGroup.icon;
  const functions = uniqueLifecycleFunctions(stages.flatMap((stage) => stage.functions));
  const evidence = uniqueStrings(stages.flatMap((stage) => stage.evidence));

  return (
    <div
      className={cn(
        "rounded-md border bg-[var(--app-surface)] p-4",
        active ? "border-[color:var(--app-border)]" : "border-[color:var(--app-border)]/70 opacity-75",
      )}
    >
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <span
            className={cn(
              "inline-flex size-8 shrink-0 items-center justify-center rounded-md border",
              active ? stageGroup.accent : "border-[color:var(--app-border)] text-muted-foreground",
            )}
          >
            <Icon className="size-4" aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold">{stageGroup.title}</h3>
            <p className="mt-0.5 text-xs text-muted-foreground">
              {active ? `${functions.length} function${functions.length === 1 ? "" : "s"} observed` : "No path detected"}
            </p>
          </div>
        </div>
        <span className="shrink-0 rounded bg-[var(--app-subtle)] px-2 py-0.5 text-[11px] text-muted-foreground">
          {active ? "detected" : "none"}
        </span>
      </div>

      {functions.length ? (
        <div className="mt-3 flex flex-wrap gap-1.5">
          {functions.map((functionRef) => (
            <LifecycleFunctionChip functionRef={functionRef} key={`${functionRef.qualifiedName}:${functionRef.direct}`} />
          ))}
        </div>
      ) : null}

      {evidence.length ? (
        <div className="mt-3 grid gap-1.5">
          {evidence.slice(0, 4).map((item) => (
            <div className="rounded bg-[var(--app-subtle)] px-2 py-1 font-mono text-[11px] leading-5 text-muted-foreground" key={item}>
              {item}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function LifecycleFunctionChip({ functionRef }: { functionRef: ObjectLifecycleFunctionRef }) {
  return (
    <span className="inline-flex max-w-full items-center gap-1.5 rounded bg-[var(--app-subtle)] px-2 py-1 text-xs text-muted-foreground">
      <span className="truncate font-mono text-foreground">{functionRef.qualifiedName}</span>
      <span className="shrink-0 text-[10px] uppercase tracking-wide text-muted-foreground/80">
        {functionRef.direct ? "direct" : "via"}
      </span>
      {!functionRef.direct && functionRef.callPath.length > 1 ? (
        <span className="min-w-0 truncate text-[11px] text-muted-foreground/70">
          {functionRef.callPath.join(" -> ")}
        </span>
      ) : null}
    </span>
  );
}

function LifecycleRiskList({ risks }: { risks: ObjectLifecycleMap["risks"] }) {
  if (!risks.length) {
    return (
      <div className="rounded-md border border-emerald-400/20 bg-emerald-400/5 px-4 py-3 text-sm text-emerald-200">
        No lifecycle risks were flagged by the static scanner.
      </div>
    );
  }

  return (
    <div className="grid gap-2">
      <div className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">Lifecycle risks</div>
      {risks.map((risk) => (
        <div
          className={cn(
            "rounded-md border px-4 py-3",
            risk.severity === "high"
              ? "border-red-400/25 bg-red-400/10"
              : risk.severity === "medium"
                ? "border-amber-400/25 bg-amber-400/10"
                : "border-[color:var(--app-border)] bg-[var(--app-surface)]",
          )}
          key={`${risk.kind}:${risk.message}`}
        >
          <div className="flex min-w-0 items-start justify-between gap-3">
            <p className="min-w-0 text-sm font-medium text-foreground">{risk.message}</p>
            <span className="shrink-0 rounded bg-[var(--app-subtle)] px-2 py-0.5 text-[11px] text-muted-foreground">
              {risk.severity}
            </span>
          </div>
          {risk.functions.length ? (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {uniqueLifecycleFunctions(risk.functions).map((functionRef) => (
                <LifecycleFunctionChip functionRef={functionRef} key={functionRef.qualifiedName} />
              ))}
            </div>
          ) : null}
        </div>
      ))}
    </div>
  );
}

function stagesForGroup(
  lifecycleMap: ObjectLifecycleMap,
  kinds: readonly ObjectLifecycleStageKind[],
): ObjectLifecycleStage[] {
  return lifecycleMap.stages.filter((stage) => kinds.includes(stage.kind));
}

function uniqueStrings(values: string[]) {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}

function AdminControlsDetail({ findings }: { findings: AdminControlFinding[] }) {
  return (
    <FindingList
      emptyLabel="No admin-control findings found."
      items={findings}
      renderItem={(finding) => (
        <FindingCard
          key={finding.qualifiedName}
          badge={finding.confidence}
          evidence={finding.evidence}
          title={finding.qualifiedName}
          subtitle="Privileged transaction-callable function"
        >
          <InlineList label="Guarding types" values={finding.guardingTypes} />
        </FindingCard>
      )}
    />
  );
}

function ExternalCallsDetail({ findings }: { findings: ExternalCallFinding[] }) {
  return (
    <FindingList
      emptyLabel="No external calls found."
      items={findings}
      renderItem={(finding) => (
        <Card className="gap-0 rounded-md p-4" key={`${finding.callerModule}:${finding.callerFunction}:${finding.target}`}>
          <div className="flex min-w-0 items-start gap-3">
            <Network className="mt-1 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-sm font-semibold">
                {finding.callerModule}::{finding.callerFunction}
              </h2>
              <p className="mt-1 text-sm text-muted-foreground">calls external target</p>
              <CodeLine>{finding.target}</CodeLine>
            </div>
          </div>
        </Card>
      )}
    />
  );
}

function PackageInternalsDetail({
  relationships,
}: {
  relationships: PublicPackageRelationship[];
}) {
  return (
    <FindingList
      emptyLabel="No public(package) relationships found."
      items={relationships}
      renderItem={(relationship) => (
        <Card
          className="gap-0 rounded-md p-4"
          key={`${relationship.sourceModule}:${relationship.sourceFunction}:${relationship.targetModule}:${relationship.targetFunction}`}
        >
          <div className="flex min-w-0 items-start gap-3">
            <Link2 className="mt-1 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-sm font-semibold">
                {relationship.sourceModule}::{relationship.sourceFunction}
              </h2>
              <p className="mt-1 text-sm text-muted-foreground">uses package-scoped function</p>
              <CodeLine>
                {relationship.targetModule}::{relationship.targetFunction}
              </CodeLine>
            </div>
          </div>
        </Card>
      )}
    />
  );
}

function FunctionCard({
  functionSignature,
  moduleName,
}: {
  functionSignature: MoveFunctionSignature;
  moduleName: string;
}) {
  return (
    <Card className="gap-0 rounded-md p-4">
      <div className="flex min-w-0 items-start gap-3">
        <FileCode2 className="mt-1 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <h2 className="truncate text-sm font-semibold">
              {moduleName}::{functionSignature.name}
            </h2>
            <Badge className="rounded bg-primary/10 px-2 py-0.5 text-[11px] text-primary" variant="secondary">
              {functionSignature.isEntry ? "entry" : functionSignature.visibility}
            </Badge>
          </div>
          <CodeLine>{functionSignature.signature}</CodeLine>
        </div>
      </div>
    </Card>
  );
}

function FindingCard({
  badge,
  children,
  evidence,
  subtitle,
  title,
}: {
  badge: string;
  children?: React.ReactNode;
  evidence: string[];
  subtitle: string;
  title: string;
}) {
  return (
    <Card className="gap-0 rounded-md p-4">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold">{title}</h2>
          <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
        </div>
        <Badge className="rounded bg-[var(--app-subtle)] px-2 py-0.5 text-[11px]" variant="secondary">
          {badge}
        </Badge>
      </div>
      <InlineList label="Evidence" values={evidence} />
      {children}
    </Card>
  );
}

function InlineList({ label, values }: { label: string; values: string[] }) {
  if (!values.length) {
    return null;
  }

  return (
    <div className="mt-3">
      <div className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">{label}</div>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {values.map((value) => (
          <span
            className="rounded bg-[var(--app-subtle)] px-2 py-1 font-mono text-xs text-muted-foreground"
            key={value}
          >
            {value}
          </span>
        ))}
      </div>
    </div>
  );
}

function FindingList<TItem>({
  emptyLabel,
  items,
  renderItem,
}: {
  emptyLabel: string;
  items: TItem[];
  renderItem: (item: TItem) => React.ReactNode;
}) {
  if (!items.length) {
    return <EmptyState label={emptyLabel} />;
  }

  return <div className="grid gap-3">{items.map(renderItem)}</div>;
}

function CodeLine({ children }: { children: React.ReactNode }) {
  return (
    <pre className="mt-3 overflow-x-auto rounded-md bg-[var(--app-subtle)] px-3 py-2 font-mono text-xs leading-5 text-foreground">
      {children}
    </pre>
  );
}

function EmptyState({ label }: { label: string }) {
  return (
    <div className="flex min-h-64 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)] px-6 text-center text-sm text-muted-foreground">
      {label}
    </div>
  );
}
