import {
  loadFilePreview,
  type MoveModule,
  type MovePackage,
  type MovePackageSurface,
  type PackageDependencyGraph,
  type PackageTree,
} from "@/features/empty-project/filesystem-tree";

const MAX_CONTEXT_CHARS = 48_000;
const MAX_SOURCE_CHARS_PER_FILE = 8_000;
const MAX_FINDINGS_PER_GROUP = 12;

export async function buildMovePackageAiContext({
  movePackage,
  packageTree,
}: {
  movePackage: MovePackage;
  packageTree: PackageTree;
}) {
  const sections = [
    packageOverviewContext(packageTree, movePackage),
    dependencyContext(packageTree.dependencyGraph),
    callGraphContext(packageTree.callGraph, movePackage),
    typeGraphContext(packageTree.typeGraph, movePackage),
    surfaceContext(movePackage.surface),
    moduleSignatureContext(movePackage.modules),
    await sourceContext(packageTree, movePackage.modules),
  ];

  return truncateText(sections.filter(Boolean).join("\n\n"), MAX_CONTEXT_CHARS);
}

function packageOverviewContext(packageTree: PackageTree, movePackage: MovePackage) {
  return [
    "# Active Move Package",
    `Name: ${movePackage.name}`,
    `Path: ${movePackage.path || "."}`,
    `Manifest: ${movePackage.manifestPath}`,
    `Workspace root: ${packageTree.rootPath}`,
    `Package summaries: ${packageTree.dependencyGraph.summaryPath ?? "not found"}`,
    `Modules: ${movePackage.modules.length}`,
  ].join("\n");
}

function dependencyContext(graph: PackageDependencyGraph) {
  const nodes = graph.nodes
    .slice(0, 24)
    .map((node) =>
      `- ${node.id}${node.isRoot ? " (root)" : ""}: ${node.moduleCount} modules, ${node.entryFunctionCount} entries, ${node.publicFunctionCount} public functions`,
    );
  const edges = graph.edges
    .slice(0, 32)
    .map((edge) =>
      `- ${edge.source} -> ${edge.target}: ${edge.dependencyCount} ${edge.dependencyKind}`,
    );

  return [
    "# Dependency Graph",
    "Packages:",
    ...emptyAware(nodes, "No dependency nodes found."),
    "Edges:",
    ...emptyAware(edges, "No dependency edges found."),
  ].join("\n");
}

function callGraphContext(
  graph: PackageTree["callGraph"],
  movePackage: MovePackage,
) {
  const packagePath = movePackage.path || ".";
  const localNodes = graph.nodes.filter((node) => node.packagePath === movePackage.path);
  const localNodeIds = new Set(localNodes.map((node) => node.id));
  const edges = graph.edges
    .filter((edge) => localNodeIds.has(edge.source))
    .slice(0, 48)
    .map((edge) => {
      const target = graph.nodes.find((node) => node.id === edge.target);
      const targetLabel = target?.qualifiedName ?? edge.rawTarget;

      return `- ${edge.rawTarget} -> ${targetLabel}: ${edge.callCount} ${edge.callKind}, ${edge.isResolved ? "resolved" : "unresolved"}${edge.isExternal ? ", external" : ""}`;
    });

  return [
    "# Call Graph",
    `Package path: ${packagePath}`,
    `Functions: ${localNodes.length}`,
    `Edges: ${graph.edges.filter((edge) => localNodeIds.has(edge.source)).length}`,
    `Unresolved calls: ${graph.unresolvedCalls.filter((call) => localNodeIds.has(call.source)).length}`,
    ...emptyAware(edges, "No call edges found for this package."),
  ].join("\n");
}

function typeGraphContext(
  graph: PackageTree["typeGraph"],
  movePackage: MovePackage,
) {
  const packagePath = movePackage.path || ".";
  const localNodes = graph.nodes.filter((node) => node.packagePath === movePackage.path);
  const localTypeIds = new Set(localNodes.map((node) => node.id));
  const edges = graph.edges
    .filter((edge) => localTypeIds.has(edge.source) || localTypeIds.has(edge.target))
    .slice(0, 48)
    .map((edge) => {
      const source = graph.nodes.find((node) => node.id === edge.source);
      const target = graph.nodes.find((node) => node.id === edge.target);

      return `- ${source?.qualifiedName ?? edge.source} -> ${target?.qualifiedName ?? edge.target}: ${edge.relationship}${edge.fieldName ? ` field=${edge.fieldName}` : ""}${edge.parameterName ? ` param=${edge.parameterName}` : ""}`;
    });

  return [
    "# Type Graph",
    `Package path: ${packagePath}`,
    `Types: ${localNodes.length}`,
    `Edges: ${graph.edges.filter((edge) => localTypeIds.has(edge.source) || localTypeIds.has(edge.target)).length}`,
    `Unresolved types: ${graph.unresolvedTypes.length}`,
    ...emptyAware(edges, "No type edges found for this package."),
  ].join("\n");
}

function surfaceContext(surface: MovePackageSurface) {
  return [
    "# Security Surface",
    `Entry functions: ${surface.entryFunctionCount}`,
    `Capabilities: ${surface.capabilityCount}`,
    `Shared objects: ${surface.sharedObjectCount}`,
    `Address-owned objects: ${surface.addressOwnedObjectCount}`,
    `Immutable objects: ${surface.immutableObjectCount}`,
    `Wrapped objects: ${surface.wrappedObjectCount}`,
    `Party objects: ${surface.partyObjectCount}`,
    `Admin controls: ${surface.adminControlCount}`,
    `External calls: ${surface.externalCallCount}`,
    `Package-internal relationships: ${surface.publicPackageRelationshipCount}`,
    findingGroup("Capability findings", surface.capabilityFindings),
    findingGroup("Object ownership findings", surface.objectOwnershipFindings),
    findingGroup("Admin control findings", surface.adminControlFindings),
    findingGroup("External calls", surface.externalCallFindings),
  ].join("\n");
}

function findingGroup(label: string, findings: unknown[]) {
  if (!findings.length) {
    return `${label}: none`;
  }

  return [
    `${label}:`,
    ...findings.slice(0, MAX_FINDINGS_PER_GROUP).map((finding) => {
      const compact = JSON.stringify(finding);

      return `- ${truncateText(compact, 700)}`;
    }),
  ].join("\n");
}

function moduleSignatureContext(modules: MoveModule[]) {
  const moduleSections = modules.map((moveModule) => {
    const structs = moveModule.structs.map((moveStruct) => `  - ${moveStruct.signature}`);
    const functions = moveModule.functions.map((moveFunction) => {
      const flags = [
        moveFunction.visibility,
        moveFunction.isEntry ? "entry" : null,
        moveFunction.isTransactionCallable ? "transaction-callable" : null,
      ].filter(Boolean);

      return `  - ${moveFunction.signature}${flags.length ? ` [${flags.join(", ")}]` : ""}`;
    });

    return [
      `## Module ${moveModule.name}`,
      `File: ${moveModule.filePath}`,
      "Structs:",
      ...emptyAware(structs, "  - none"),
      "Functions:",
      ...emptyAware(functions, "  - none"),
    ].join("\n");
  });

  return ["# Module Signatures", ...moduleSections].join("\n\n");
}

async function sourceContext(packageTree: PackageTree, modules: MoveModule[]) {
  const previews = await Promise.all(
    modules.map((moveModule) =>
      loadFilePreview(packageTree, moveModule.filePath)
        .then((preview) => ({
          moduleName: moveModule.name,
          preview,
        }))
        .catch(() => null),
    ),
  );
  const sourceSections = previews
    .filter((item): item is NonNullable<typeof item> => Boolean(item))
    .filter((item) => item.preview.kind === "text")
    .map((item) => {
      if (item.preview.kind !== "text") {
        return "";
      }

      return [
        `## Source ${item.moduleName} (${item.preview.path})`,
        "```move",
        truncateText(item.preview.source, MAX_SOURCE_CHARS_PER_FILE),
        "```",
      ].join("\n");
    })
    .filter(Boolean);

  return ["# Source Excerpts", ...emptyAware(sourceSections, "No readable Move source files found.")].join("\n\n");
}

function emptyAware(items: string[], fallback: string) {
  return items.length ? items : [fallback];
}

function truncateText(value: string, maxLength: number) {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength)}\n...[truncated]`;
}
