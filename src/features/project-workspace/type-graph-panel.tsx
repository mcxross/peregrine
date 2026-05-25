import {
  Box,
  Boxes,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Hexagon,
  Package,
  Search,
} from "lucide-react";
import React from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  MovePackage,
  MoveTypeGraph,
  MoveTypeGraphNode,
} from "@/features/empty-project/filesystem-tree";
import { displayMovePackageName } from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

type TypeGraphPanelProps = {
  className?: string;
  movePackage: MovePackage | null;
  onCollapse?: () => void;
  onSelectedTypeIdChange?: (typeId: string) => void;
  packageName: string;
  selectedTypeId?: string | null;
  typeGraph: MoveTypeGraph;
};

type CollapsedTypeGraphPanelProps = {
  className?: string;
  onExpand: () => void;
  packageName: string;
  typeCount: number;
};

type TypePanelGroup = {
  count: number;
  id: string;
  kind: "package" | "framework" | "external";
  modules: TypePanelModule[];
  title: string;
};

type TypePanelModule = {
  count: number;
  id: string;
  nodes: MoveTypeGraphNode[];
  title: string;
};

const DISPLAY_TYPE_KINDS = new Set(["struct", "enum", "datatype", "summaryType"]);
const COPY_DROP_STORE_ABILITIES = ["copy", "drop", "store"] as const;
const BUILTIN_TYPE_ABILITIES: Record<string, readonly string[]> = {
  address: COPY_DROP_STORE_ABILITIES,
  bool: COPY_DROP_STORE_ABILITIES,
  signer: ["drop"],
  u8: COPY_DROP_STORE_ABILITIES,
  u16: COPY_DROP_STORE_ABILITIES,
  u32: COPY_DROP_STORE_ABILITIES,
  u64: COPY_DROP_STORE_ABILITIES,
  u128: COPY_DROP_STORE_ABILITIES,
  u256: COPY_DROP_STORE_ABILITIES,
  vector: COPY_DROP_STORE_ABILITIES,
};
const SUI_FRAMEWORK_TYPE_ABILITIES: Record<string, readonly string[]> = {
  "object::ID": COPY_DROP_STORE_ABILITIES,
  "object::UID": ["store"],
  "table::Table": ["key", "store"],
};
const FRAMEWORK_ADDRESSES = new Set([
  "std",
  "sui",
  "0x1",
  "0x2",
  "0x0000000000000000000000000000000000000000000000000000000000000001",
  "0x0000000000000000000000000000000000000000000000000000000000000002",
]);
const FRAMEWORK_MODULES = new Set([
  "balance",
  "clock",
  "coin",
  "dynamic_field",
  "dynamic_object_field",
  "event",
  "object",
  "table",
  "transfer",
  "tx_context",
  "vec_map",
  "vec_set",
]);

export function TypeGraphPanel({
  className,
  movePackage,
  onCollapse,
  onSelectedTypeIdChange,
  packageName,
  selectedTypeId,
  typeGraph,
}: TypeGraphPanelProps) {
  const groups = React.useMemo(
    () => buildTypePanelGroups(typeGraph, movePackage, packageName),
    [typeGraph, movePackage, packageName],
  );
  const [query, setQuery] = React.useState("");
  const filteredGroups = React.useMemo(
    () => filterTypePanelGroups(groups, query, typeGraph),
    [groups, query, typeGraph],
  );
  const [internalSelectedTypeId, setInternalSelectedTypeId] = React.useState<string | null>(null);
  const [expanded, setExpanded] = React.useState<Set<string>>(() => defaultExpandedGroups(groups));
  const activeSelectedTypeId = selectedTypeId ?? internalSelectedTypeId;
  const isControlled = selectedTypeId !== undefined;

  React.useEffect(() => {
    if (!isControlled) {
      setInternalSelectedTypeId(null);
    }
    setExpanded(defaultExpandedGroups(groups));
  }, [groups, isControlled]);

  React.useEffect(() => {
    if (!activeSelectedTypeId) {
      return;
    }

    const containing = findContainingTypeGroup(filteredGroups, activeSelectedTypeId);

    if (!containing) {
      return;
    }

    setExpanded((current) => {
      if (current.has(containing.groupId) && current.has(containing.moduleId)) {
        return current;
      }

      const next = new Set(current);
      next.add(containing.groupId);
      next.add(containing.moduleId);
      return next;
    });
  }, [activeSelectedTypeId, filteredGroups]);

  const toggleExpanded = React.useCallback((id: string) => {
    setExpanded((current) => {
      const next = new Set(current);

      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }

      return next;
    });
  }, []);
  const selectType = React.useCallback(
    (typeId: string) => {
      setInternalSelectedTypeId(typeId);
      onSelectedTypeIdChange?.(typeId);
    },
    [onSelectedTypeIdChange],
  );

  return (
    <aside
      className={cn(
        "grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-transparent",
        className,
      )}
    >
      <div className="mx-5 mb-2 mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
        <label className="grid h-8 min-w-0 grid-cols-[18px_minmax(0,1fr)] items-center gap-1.5 rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-2">
          <Search className="size-3.5 text-muted-foreground" aria-hidden="true" />
          <input
            className="min-w-0 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
            onChange={(event) => setQuery(event.target.value)}
            placeholder="type, module, ability:key, kind:capability"
            value={query}
          />
        </label>
        {onCollapse ? (
          <button
            aria-label="Collapse type navigator"
            className="grid size-8 place-items-center rounded-md text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground active:scale-95"
            onClick={onCollapse}
            title="Collapse navigator"
            type="button"
          >
            <ChevronLeft className="size-4" aria-hidden="true" />
          </button>
        ) : null}
      </div>

      <ScrollArea className="min-h-0">
        <div className="space-y-1 px-5 py-2">
          {filteredGroups.length ? (
            filteredGroups.map((group) => (
              <TypeGroup
                expanded={expanded}
                group={group}
                key={group.id}
                onSelectType={selectType}
                onToggleExpanded={toggleExpanded}
                selectedTypeId={activeSelectedTypeId}
              />
            ))
          ) : (
            <div className="flex min-h-40 items-center justify-center rounded-md border border-dashed border-[color:var(--app-border)] px-4 text-center text-xs leading-5 text-muted-foreground">
              No matching graph types found.
            </div>
          )}
        </div>
      </ScrollArea>
    </aside>
  );
}

export function CollapsedTypeGraphPanel({
  className,
  onExpand,
  packageName,
  typeCount,
}: CollapsedTypeGraphPanelProps) {
  return (
    <aside
      className={cn(
        "flex min-h-0 items-center gap-2 overflow-hidden bg-transparent p-1.5 lg:flex-col",
        className,
      )}
    >
      <button
        aria-label="Show type navigator"
        className="grid size-8 shrink-0 place-items-center rounded-md text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground"
        onClick={onExpand}
        title="Show type navigator"
        type="button"
      >
        <ChevronRight className="size-4" aria-hidden="true" />
      </button>
      <div className="flex min-w-0 flex-1 items-center gap-2 lg:min-h-0 lg:flex-col">
        <Boxes className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span className="min-w-0 truncate text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground lg:min-h-0 lg:[writing-mode:vertical-rl]">
          {displayMovePackageName(packageName)}
        </span>
      </div>
      <CountPill value={typeCount} />
    </aside>
  );
}

function TypeGroup({
  expanded,
  group,
  onSelectType,
  onToggleExpanded,
  selectedTypeId,
}: {
  expanded: Set<string>;
  group: TypePanelGroup;
  onSelectType: (id: string) => void;
  onToggleExpanded: (id: string) => void;
  selectedTypeId: string | null;
}) {
  const isOpen = expanded.has(group.id);
  const canFlattenModule = group.modules.length === 1 && group.modules[0]?.title === group.title;
  const directModule = canFlattenModule ? group.modules[0] : null;

  return (
    <section className="min-w-0">
      <TreeHeader
        count={group.count}
        iconTone={group.kind}
        isOpen={isOpen}
        label={group.title}
        level={0}
        onToggle={() => onToggleExpanded(group.id)}
      />

      {isOpen ? (
        <div className="ml-5 mt-0.5 space-y-0.5 border-l border-[color:var(--app-border)] pl-2">
          {directModule
            ? directModule.nodes.map((node) => (
                <TypeRow
                  isSelected={node.id === selectedTypeId}
                  key={node.id}
                  node={node}
                  onSelect={() => onSelectType(node.id)}
                  tone={group.kind}
                />
              ))
            : group.modules.map((module) => (
                <TypeModule
                  expanded={expanded}
                  key={module.id}
                  module={module}
                  onSelectType={onSelectType}
                  onToggleExpanded={onToggleExpanded}
                  selectedTypeId={selectedTypeId}
                  tone={group.kind}
                />
              ))}
        </div>
      ) : null}
    </section>
  );
}

function TypeModule({
  expanded,
  module,
  onSelectType,
  onToggleExpanded,
  selectedTypeId,
  tone,
}: {
  expanded: Set<string>;
  module: TypePanelModule;
  onSelectType: (id: string) => void;
  onToggleExpanded: (id: string) => void;
  selectedTypeId: string | null;
  tone: TypePanelGroup["kind"];
}) {
  const isOpen = expanded.has(module.id);

  return (
    <div className="min-w-0">
      <TreeHeader
        count={module.count}
        iconTone={tone}
        isOpen={isOpen}
        label={module.title}
        level={1}
        onToggle={() => onToggleExpanded(module.id)}
      />

      {isOpen ? (
        <div className="ml-10 space-y-0.5 border-l border-[color:var(--app-border)] pl-2">
          {module.nodes.map((node) => (
            <TypeRow
              isSelected={node.id === selectedTypeId}
              key={node.id}
              node={node}
              onSelect={() => onSelectType(node.id)}
              tone={tone}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function TreeHeader({
  count,
  iconTone,
  isOpen,
  label,
  level,
  onToggle,
}: {
  count: number;
  iconTone: TypePanelGroup["kind"];
  isOpen: boolean;
  label: string;
  level: 0 | 1;
  onToggle: () => void;
}) {
  const Icon = level === 0 ? Package : Boxes;

  return (
    <button
      className={cn(
        "grid h-8 w-full min-w-0 items-center gap-1.5 rounded-md px-1.5 text-left text-xs leading-none text-muted-foreground transition hover:bg-[var(--app-subtle)] hover:text-foreground",
        level === 0 ? "grid-cols-[16px_18px_minmax(0,1fr)_auto]" : "grid-cols-[16px_18px_minmax(0,1fr)_auto]",
      )}
      onClick={onToggle}
      type="button"
    >
      {isOpen ? (
        <ChevronDown className="size-3.5 text-muted-foreground" aria-hidden="true" />
      ) : (
        <ChevronRight className="size-3.5 text-muted-foreground" aria-hidden="true" />
      )}
      <Icon className={cn("size-4", toneText(iconTone))} aria-hidden="true" />
      <span className="min-w-0 truncate font-medium">{label}</span>
      <CountPill value={count} />
    </button>
  );
}

function TypeRow({
  isSelected,
  node,
  onSelect,
  tone,
}: {
  isSelected: boolean;
  node: MoveTypeGraphNode;
  onSelect: () => void;
  tone: TypePanelGroup["kind"];
}) {
  const displayName = typeDisplayName(node, tone);
  const Icon = typeIcon(node);
  const badges = nodeBadges(node);

  return (
    <button
      className={cn(
        "grid h-7 w-full min-w-0 grid-cols-[18px_minmax(0,1fr)_auto] items-center gap-1.5 rounded-md px-1.5 text-left text-xs leading-none transition",
        isSelected
          ? "border border-sky-500/55 bg-sky-500/15 text-sky-100 shadow-[inset_0_0_0_1px_rgba(14,165,233,0.16)]"
          : "border border-transparent text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
      )}
      onClick={onSelect}
      title={node.qualifiedName}
      type="button"
    >
      <Icon className={cn("size-4", typeTone(node, tone))} aria-hidden="true" />
      <span className="min-w-0 truncate font-medium">{displayName}</span>
      <span className="flex min-w-0 justify-end gap-1">
        {badges.slice(0, 2).map((badge) => (
          <span
            className={cn(
              "inline-flex h-4 min-w-4 items-center justify-center rounded px-1 text-[9px] font-bold leading-none",
              badge.className,
            )}
            key={badge.label}
          >
            {badge.label}
          </span>
        ))}
      </span>
    </button>
  );
}

function CountPill({ value }: { value: number }) {
  return (
    <span className="inline-flex h-5 min-w-7 shrink-0 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-elevated)] px-1.5 text-[11px] font-medium leading-none text-muted-foreground">
      {value}
    </span>
  );
}

function buildTypePanelGroups(
  graph: MoveTypeGraph,
  movePackage: MovePackage | null,
  packageName: string,
): TypePanelGroup[] {
  const packagePath = movePackage?.path ?? null;
  const localTypes = graph.nodes.filter((node) => isLocalPackageType(node, packagePath));
  const localTypeIds = new Set(localTypes.map((node) => node.id));
  const relevantTargets = relevantExternalTypeIds(graph, localTypeIds, packagePath);
  const frameworkTypes = graph.nodes.filter(
    (node) => relevantTargets.has(node.id) && !localTypeIds.has(node.id) && isFrameworkType(node),
  );
  const externalTypes = graph.nodes.filter(
    (node) =>
      relevantTargets.has(node.id)
      && !localTypeIds.has(node.id)
      && !isFrameworkType(node)
      && isDisplayTypeNode(node),
  );
  const groups = [
    buildPackageGroup(movePackage, packageName, localTypes),
    buildFrameworkGroup(frameworkTypes),
    buildExternalGroup(externalTypes),
  ].filter((group): group is TypePanelGroup => Boolean(group));

  return groups;
}

function buildPackageGroup(
  movePackage: MovePackage | null,
  packageName: string,
  nodes: MoveTypeGraphNode[],
): TypePanelGroup | null {
  if (!nodes.length) {
    return null;
  }

  const capabilities = nodes.filter(isCapabilityLike);
  const capabilityIds = new Set(capabilities.map((node) => node.id));
  const events = nodes.filter((node) => !capabilityIds.has(node.id) && isEventLike(node));
  const eventIds = new Set(events.map((node) => node.id));
  const witnesses = nodes.filter(
    (node) => !capabilityIds.has(node.id) && !eventIds.has(node.id) && isWitnessLike(node),
  );
  const witnessIds = new Set(witnesses.map((node) => node.id));
  const resources = nodes.filter(
    (node) =>
      !capabilityIds.has(node.id)
      && !eventIds.has(node.id)
      && !witnessIds.has(node.id)
      && isResourceLike(node),
  );
  const resourceIds = new Set(resources.map((node) => node.id));
  const generics = nodes.filter(
    (node) =>
      !capabilityIds.has(node.id)
      && !eventIds.has(node.id)
      && !witnessIds.has(node.id)
      && !resourceIds.has(node.id)
      && isGenericLike(node),
  );
  const categorized = new Set([
    ...capabilities.map((node) => node.id),
    ...resources.map((node) => node.id),
    ...generics.map((node) => node.id),
    ...events.map((node) => node.id),
    ...witnesses.map((node) => node.id),
  ]);
  const structs = nodes.filter((node) => !categorized.has(node.id));
  const modules = [
    buildCategoryModule("capabilities", "Capabilities", capabilities),
    buildCategoryModule("resources", "Resources", resources),
    buildCategoryModule("generics", "Generics", generics),
    buildCategoryModule("events", "Events", events),
    buildCategoryModule("witnesses", "Witnesses", witnesses),
    buildCategoryModule("structs", "Structs", structs),
  ].filter((module): module is TypePanelModule => Boolean(module));

  return {
    count: nodes.length,
    id: "group:package",
    kind: "package",
    modules,
    title: displayMovePackageName(movePackage?.name ?? packageName),
  };
}

function buildCategoryModule(id: string, title: string, nodes: MoveTypeGraphNode[]) {
  if (!nodes.length) {
    return null;
  }

  return {
    count: nodes.length,
    id: `group:package:${id}`,
    nodes: sortTypeNodes(nodes),
    title,
  };
}

function buildFrameworkGroup(nodes: MoveTypeGraphNode[]): TypePanelGroup | null {
  const displayNodes = nodes.filter((node) => node.kind === "builtin" || isDisplayTypeNode(node));

  if (!displayNodes.length) {
    return null;
  }

  return {
    count: displayNodes.length,
    id: "group:framework",
    kind: "framework",
    modules: [
      {
        count: displayNodes.length,
        id: "group:framework:types",
        nodes: sortTypeNodes(displayNodes),
        title: "Framework (sui)",
      },
    ],
    title: "Framework (sui)",
  };
}

function buildExternalGroup(nodes: MoveTypeGraphNode[]): TypePanelGroup | null {
  if (!nodes.length) {
    return null;
  }

  return {
    count: nodes.length,
    id: "group:external",
    kind: "external",
    modules: [
      {
        count: nodes.length,
        id: "group:external:types",
        nodes: sortTypeNodes(nodes),
        title: "External",
      },
    ],
    title: "External",
  };
}

function relevantExternalTypeIds(
  graph: MoveTypeGraph,
  localTypeIds: Set<string>,
  packagePath: string | null,
) {
  const ids = new Set<string>();

  for (const edge of graph.edges) {
    const sourceIsLocal = localTypeIds.has(edge.source) || sourceBelongsToPackage(edge.source, packagePath);
    const targetIsLocal = localTypeIds.has(edge.target);

    if (sourceIsLocal && !targetIsLocal) {
      ids.add(edge.target);
    }

    if (targetIsLocal && !sourceIsLocal) {
      ids.add(edge.source);
    }
  }

  return ids;
}

function filterTypePanelGroups(groups: TypePanelGroup[], query: string, graph: MoveTypeGraph) {
  const normalized = query.trim().toLowerCase();

  if (!normalized) {
    return groups;
  }

  return groups
    .map((group) => {
      const modules = group.modules
        .map((module) => {
          const nodes = module.nodes.filter((node) => nodeMatchesQuery(node, normalized, graph));

          return {
            ...module,
            count: nodes.length,
            nodes,
          };
        })
        .filter((module) => module.nodes.length > 0);

      return {
        ...group,
        count: modules.reduce((total, module) => total + module.count, 0),
        modules,
      };
    })
    .filter((group) => group.count > 0);
}

function nodeMatchesQuery(node: MoveTypeGraphNode, query: string, graph: MoveTypeGraph) {
  const [prefix, rawValue] = query.includes(":") ? query.split(/:(.*)/, 2) : ["", query];
  const value = rawValue ?? "";

  if (prefix === "ability") {
    return effectiveTypeAbilities(node).some((ability) => ability.toLowerCase() === value);
  }

  if (prefix === "origin") {
    return nodeOrigin(node).includes(value);
  }

  if (prefix === "kind") {
    return nodeBadges(node).some((badge) => badge.label.toLowerCase().includes(value));
  }

  if (prefix === "field") {
    return graph.edges.some(
      (edge) =>
        (edge.source === node.id || edge.target === node.id)
        && (edge.fieldName?.toLowerCase().includes(value) ?? false),
    );
  }

  if (prefix === "uses") {
    return graph.edges.some(
      (edge) =>
        (edge.source === node.id || edge.target === node.id)
        && (edge.functionName?.toLowerCase().includes(value) ?? edge.source.toLowerCase().includes(value)),
    );
  }

  if (prefix === "generic") {
    return node.qualifiedName.toLowerCase().includes(value) || isGenericLike(node);
  }

  return (
    node.name.toLowerCase().includes(query)
    || node.qualifiedName.toLowerCase().includes(query)
    || (node.moduleName?.toLowerCase().includes(query) ?? false)
    || effectiveTypeAbilities(node).some((ability) => ability.toLowerCase().includes(query))
  );
}

function defaultExpandedGroups(groups: TypePanelGroup[]) {
  const expanded = new Set<string>();

  for (const group of groups) {
    expanded.add(group.id);
    if (group.kind === "package") {
      group.modules.slice(0, 1).forEach((module) => expanded.add(module.id));
    } else {
      group.modules.forEach((module) => expanded.add(module.id));
    }
  }

  return expanded;
}

function findContainingTypeGroup(groups: TypePanelGroup[], typeId: string) {
  for (const group of groups) {
    for (const module of group.modules) {
      if (module.nodes.some((node) => node.id === typeId)) {
        return {
          groupId: group.id,
          moduleId: module.id,
        };
      }
    }
  }

  return null;
}

function isLocalPackageType(node: MoveTypeGraphNode, packagePath: string | null) {
  return packagePath !== null && node.packagePath === packagePath && isDisplayTypeNode(node);
}

function isDisplayTypeNode(node: MoveTypeGraphNode) {
  return DISPLAY_TYPE_KINDS.has(node.kind);
}

function isFrameworkType(node: MoveTypeGraphNode) {
  const address = node.address?.toLowerCase();
  const canonicalAddress = node.canonicalAddress?.toLowerCase();
  const moduleName = node.moduleName?.toLowerCase();

  return (
    node.kind === "builtin"
    || (address ? FRAMEWORK_ADDRESSES.has(address) : false)
    || (canonicalAddress ? FRAMEWORK_ADDRESSES.has(canonicalAddress) : false)
    || (moduleName ? FRAMEWORK_MODULES.has(moduleName) : false)
  );
}

function isCapabilityLike(node: MoveTypeGraphNode) {
  const name = node.name.toLowerCase();
  return name.includes("cap") || name.includes("admin") || name.includes("authority");
}

function isResourceLike(node: MoveTypeGraphNode) {
  return effectiveTypeAbilities(node).includes("key");
}

function effectiveTypeAbilities(node: MoveTypeGraphNode) {
  if (node.abilities.length) {
    return node.abilities;
  }

  const builtinAbilities = BUILTIN_TYPE_ABILITIES[node.name];
  if (node.kind === "builtin" && builtinAbilities) {
    return [...builtinAbilities];
  }

  const moduleName = node.moduleName ?? node.qualifiedName.split("::").slice(-2, -1)[0];
  const frameworkAbilities = moduleName
    ? SUI_FRAMEWORK_TYPE_ABILITIES[`${moduleName}::${node.name}`]
    : undefined;

  return frameworkAbilities ? [...frameworkAbilities] : [];
}

function isGenericLike(node: MoveTypeGraphNode) {
  return (
    node.qualifiedName.includes("<")
    || ["balance", "coin", "table", "vec_map", "vec_set"].includes(node.moduleName?.toLowerCase() ?? "")
  );
}

function isEventLike(node: MoveTypeGraphNode) {
  return node.name.toLowerCase().includes("event");
}

function isWitnessLike(node: MoveTypeGraphNode) {
  const name = node.name.toLowerCase();
  return name.includes("witness") || name === "otw";
}

function nodeOrigin(node: MoveTypeGraphNode) {
  if (node.packagePath !== null) {
    return "local";
  }

  if (isFrameworkType(node)) {
    return "framework";
  }

  return "external";
}

function nodeBadges(node: MoveTypeGraphNode) {
  const badges: Array<{ className: string; label: string }> = [];

  if (isCapabilityLike(node)) {
    badges.push({ className: "bg-rose-500/15 text-rose-200", label: "C" });
  }

  if (isResourceLike(node)) {
    badges.push({ className: "bg-sky-500/15 text-sky-200", label: "R" });
  }

  if (isGenericLike(node)) {
    badges.push({ className: "bg-yellow-500/15 text-yellow-200", label: "G" });
  }

  if (nodeOrigin(node) === "external") {
    badges.push({ className: "bg-violet-500/15 text-violet-200", label: "E" });
  }

  if (effectiveTypeAbilities(node).includes("key")) {
    badges.push({ className: "bg-emerald-500/15 text-emerald-200", label: "K" });
  }

  return badges.length ? badges : [{ className: "bg-muted text-muted-foreground", label: "T" }];
}

function sourceBelongsToPackage(sourceId: string, packagePath: string | null) {
  return packagePath !== null && sourceId.startsWith(`function:${packagePath}:`);
}

function sortTypeNodes(nodes: MoveTypeGraphNode[]) {
  return [...nodes].sort((left, right) =>
    typeDisplayName(left).localeCompare(typeDisplayName(right))
    || left.qualifiedName.localeCompare(right.qualifiedName),
  );
}

function typeDisplayName(node: MoveTypeGraphNode, tone?: TypePanelGroup["kind"]) {
  if (tone === "package" || node.packagePath) {
    return node.name;
  }

  if (node.kind === "builtin") {
    return node.name;
  }

  if (node.moduleName) {
    return `${node.moduleName}::${node.name}`;
  }

  return node.qualifiedName;
}

function typeIcon(node: MoveTypeGraphNode) {
  if (node.kind === "enum") {
    return Hexagon;
  }

  if (node.kind === "builtin") {
    return Box;
  }

  return Box;
}

function toneText(tone: TypePanelGroup["kind"]) {
  if (tone === "framework") {
    return "text-emerald-400";
  }

  if (tone === "external") {
    return "text-violet-400";
  }

  return "text-sky-400";
}

function typeTone(node: MoveTypeGraphNode, tone: TypePanelGroup["kind"]) {
  if (node.kind === "enum") {
    return "text-amber-400";
  }

  if (tone === "framework") {
    return "text-emerald-400";
  }

  if (tone === "external") {
    return "text-violet-400";
  }

  return "text-rose-400";
}
