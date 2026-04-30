import * as React from "react";
import {
  Boxes,
  Box,
  ChevronDown,
  ChevronRight,
  FileCode2,
  Filter,
  GitBranch,
  KeyRound,
  Link2,
  Lock,
  Network,
  Search,
  ShieldAlert,
  UsersRound,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  AdminControlFinding,
  ExternalCallFinding,
  MoveFunctionSignature,
  MovePackage,
  ObjectOwnershipFinding,
  PublicPackageRelationship,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

const OBJECT_TREE_PANE_DEFAULT_WIDTH = 460;
const OBJECT_TREE_PANE_MIN_WIDTH = 320;
const OBJECT_TREE_PANE_MAX_WIDTH = 760;
const OBJECT_TREE_DETAIL_PANE_MIN_WIDTH = 420;

export type SurfaceDetailKind =
  | "entry-functions"
  | "capabilities"
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

  if (ownershipKind) {
    return (
      <ObjectOwnershipTreeScreen
        kind={ownershipKind}
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
  kind,
  movePackage,
}: {
  kind: ObjectOwnershipFinding["ownershipKind"];
  movePackage: MovePackage | null;
}) {
  const containerRef = React.useRef<HTMLDivElement | null>(null);
  const [query, setQuery] = React.useState("");
  const [highConfidenceOnly, setHighConfidenceOnly] = React.useState(false);
  const [treePaneWidth, setTreePaneWidth] = React.useState(OBJECT_TREE_PANE_DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = React.useState(false);
  const groups = React.useMemo(
    () => (movePackage ? objectOwnershipGroups(movePackage, kind, query, highConfidenceOnly) : []),
    [highConfidenceOnly, kind, movePackage, query],
  );
  const [collapsedGroups, setCollapsedGroups] = React.useState<Set<string>>(() => new Set());
  const firstItemKey = groups[0]?.items[0]?.key ?? null;
  const [selectedKey, setSelectedKey] = React.useState<string | null>(firstItemKey);

  React.useEffect(() => {
    setCollapsedGroups(new Set());
  }, [kind, movePackage?.manifestPath]);

  React.useEffect(() => {
    if (!groups.length) {
      setSelectedKey(null);
      return;
    }

    if (!selectedKey || !groups.some((group) => group.items.some((item) => item.key === selectedKey))) {
      setSelectedKey(groups[0]?.items[0]?.key ?? null);
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
        <div className="border-b border-[color:var(--app-border)] px-5 py-4">
          <div className="flex h-9 items-center gap-2 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] px-3">
            <Search className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <input
              className="min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search modules / objects"
              value={query}
            />
            <button
              aria-pressed={highConfidenceOnly}
              className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground data-[active=true]:bg-primary/10 data-[active=true]:text-primary"
              data-active={highConfidenceOnly}
              onClick={() => setHighConfidenceOnly((active) => !active)}
              title="Show high-confidence findings"
              type="button"
            >
              <Filter className="size-3.5" aria-hidden="true" />
            </button>
          </div>
        </div>

        <ScrollArea className="min-h-0">
          <div className="px-5 py-4">
            {movePackage ? (
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
                <ObjectTreeEmpty label="No matching object ownership findings." />
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

      <div className="min-h-0" aria-hidden="true" />
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
  confidence: ObjectOwnershipFinding["confidence"];
  key: string;
  label: string;
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
            <ObjectTreeItem
              item={item}
              key={item.key}
              onSelect={() => onSelectItem(item.key)}
              selected={selectedKey === item.key}
            />
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
          ? "bg-primary/15 text-primary shadow-[inset_3px_0_0_var(--primary)]"
          : "text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
      ].join(" ")}
      onClick={onSelect}
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
        <span className="mr-1 text-xs text-muted-foreground/80">{item.type}</span>
        {item.label}
      </span>
      <span className="rounded bg-[var(--app-subtle)] px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
        {item.confidence}
      </span>
    </button>
  );
}

function renderDetail(detail: SurfaceDetailKind, movePackage: MovePackage) {
  switch (detail) {
    case "entry-functions":
      return <EntryFunctionsDetail movePackage={movePackage} />;
    case "capabilities":
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

function objectOwnershipGroups(
  movePackage: MovePackage,
  kind: ObjectOwnershipFinding["ownershipKind"],
  query: string,
  highConfidenceOnly: boolean,
) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const moduleByName = new Map(movePackage.modules.map((moveModule) => [moveModule.name, moveModule]));
  const groups = new Map<string, ObjectTreeGroupModel>();
  const findings = movePackage.surface.objectOwnershipFindings.filter(
    (finding) =>
      finding.ownershipKind === kind &&
      (!highConfidenceOnly || finding.confidence === "high") &&
      matchesObjectQuery(finding, moduleByName.get(finding.moduleName)?.filePath, normalizedQuery),
  );

  for (const finding of findings) {
    const moveModule = moduleByName.get(finding.moduleName);
    const fileName = moduleFileName(moveModule?.filePath, finding.moduleName);
    const group = ensureObjectTreeGroup(groups, moveModule?.filePath ?? fileName, fileName);

    group.items.push({
      confidence: finding.confidence,
      key: `${finding.ownershipKind}:${finding.qualifiedName}`,
      label: finding.typeName,
      type: "struct",
    });

    for (const relatedFunction of finding.relatedFunctions) {
      const [relatedModuleName, functionName] = relatedFunction.split("::");
      const relatedModule = moduleByName.get(relatedModuleName);
      const relatedFileName = moduleFileName(relatedModule?.filePath, relatedModuleName || finding.moduleName);
      const relatedGroup = ensureObjectTreeGroup(
        groups,
        relatedModule?.filePath ?? relatedFileName,
        relatedFileName,
      );

      relatedGroup.items.push({
        confidence: finding.confidence,
        key: `${finding.ownershipKind}:${finding.qualifiedName}:${relatedFunction}`,
        label: `${functionName || relatedFunction}()`,
        type: "function",
      });
    }
  }

  if (kind === "shared" && !findings.length && !highConfidenceOnly) {
    for (const qualifiedName of movePackage.surface.sharedObjectStructs) {
      const [moduleName, typeName] = qualifiedName.split("::");
      const moveModule = moduleByName.get(moduleName);
      const fileName = moduleFileName(moveModule?.filePath, moduleName);
      const candidate = {
        confidence: "medium" as const,
        evidence: ["struct has key ability and appears in shared-object context"],
        moduleName,
        ownershipKind: "shared" as const,
        qualifiedName,
        relatedFunctions: [],
        typeName: typeName || qualifiedName,
        wrappedTypes: [],
      };

      if (!matchesObjectQuery(candidate, moveModule?.filePath, normalizedQuery)) {
        continue;
      }

      ensureObjectTreeGroup(groups, moveModule?.filePath ?? fileName, fileName).items.push({
        confidence: "medium",
        key: `shared-candidate:${qualifiedName}`,
        label: typeName || qualifiedName,
        type: "struct",
      });
    }
  }

  return [...groups.values()]
    .map((group) => ({
      ...group,
      count: group.items.length,
      items: group.items.sort((left, right) =>
        left.type.localeCompare(right.type) || left.label.localeCompare(right.label),
      ),
    }))
    .sort((left, right) => left.fileName.localeCompare(right.fileName));
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

function matchesObjectQuery(
  finding: Pick<
    ObjectOwnershipFinding,
    "confidence" | "evidence" | "moduleName" | "qualifiedName" | "relatedFunctions" | "typeName"
  >,
  filePath: string | undefined,
  query: string,
) {
  if (!query) {
    return true;
  }

  const haystack = [
    filePath,
    finding.confidence,
    finding.moduleName,
    finding.qualifiedName,
    finding.typeName,
    ...finding.evidence,
    ...finding.relatedFunctions,
  ]
    .filter(Boolean)
    .join(" ")
    .toLocaleLowerCase();

  return haystack.includes(query);
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
