import {
  Box,
  Boxes,
  Braces,
  FileCode2,
  Hexagon,
  KeyRound,
  ListTree,
  ShieldAlert,
  Workflow,
} from "lucide-react";

import {
  displayMovePackageName,
  type MovePackage,
  type MoveSourceSpan,
  type MoveTypeGraph,
  type MoveTypeGraphEdge,
  type MoveTypeGraphNode,
} from "@/features/empty-project/filesystem-tree";
import {
  BUILTIN_COLOR,
  BUILTIN_TYPE_ABILITIES,
  CAPABILITY_COLOR,
  DISPLAY_TYPE_KINDS,
  EXTERNAL_COLOR,
  FRAMEWORK_ADDRESSES,
  FRAMEWORK_COLOR,
  FRAMEWORK_MODULES,
  FUNCTION_COLOR,
  FUNCTION_RELATIONSHIPS,
  GENERIC_COLOR,
  GENERIC_RELATIONSHIPS,
  LOCAL_COLOR,
  MAX_FIELD_CAUSE_LABELS,
  MAX_FOCUSED_NODES,
  MAX_OVERVIEW_NODES,
  MAX_RENDER_EDGES,
  STORAGE_RELATIONSHIPS,
  SUI_FRAMEWORK_TYPE_ABILITIES,
  TYPE_GRAPH_DEPENDENCY_COLUMN_X,
  TYPE_GRAPH_FIELD_NODE_HEIGHT,
  TYPE_GRAPH_FIELD_NODE_WIDTH,
  TYPE_GRAPH_FUNCTION_COLUMN_X,
  TYPE_GRAPH_LOCAL_COLUMN_X,
  TYPE_GRAPH_NODE_HEIGHT,
  TYPE_GRAPH_NODE_WIDTH,
  TYPE_GRAPH_ROW_GAP,
  TYPE_GRAPH_STAR_FIELD_ARC_X,
  TYPE_GRAPH_STAR_FIELD_ARC_Y,
  TYPE_GRAPH_STAR_FIELD_MIN_GAP,
  TYPE_GRAPH_STAR_FIELD_OFFSET_X,
  TYPE_GRAPH_STAR_TARGET_ARC_X,
  TYPE_GRAPH_STAR_TARGET_ARC_Y,
  TYPE_GRAPH_STAR_TARGET_GROUP_GAP,
  TYPE_GRAPH_STAR_TARGET_MIN_GAP,
  TYPE_GRAPH_STAR_TARGET_OFFSET_X,
  TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X,
  TYPE_GRAPH_STORAGE_FIELD_COLUMN_X,
  TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X,
  type FunctionContext,
  type GenericInstanceInfo,
  type NormalizedEdge,
  type RelationshipCategory,
  type RenderEdge,
  type RenderNode,
  type TypeGraphLens,
  type TypeGraphLayoutRole,
  type TypeGraphScope,
  type TypeNodeKind,
  type TypeRenderGraph,
} from "@/features/project-workspace/type-graph/model";

export function selectedHasStorageEvidence(renderGraph: TypeRenderGraph, selectedTypeId: string | null) {
  if (!selectedTypeId) {
    return false;
  }

  return renderGraph.edgeEvidence.some(
    (edge) =>
      (edge.source === selectedTypeId || edge.target === selectedTypeId)
      && (edge.category === "field" || edge.category === "generic"),
  );
}

export function selectedLocalSourceMissing(renderGraph: TypeRenderGraph) {
  const node = renderGraph.selectedNode;

  return Boolean(
    node
    && node.kind === "local"
    && node.node
    && !node.isSynthetic
    && !node.sourceLocation,
  );
}

export function visibleCountLabel(visible: number, hidden: number) {
  return hidden > 0 ? `${visible}/${visible + hidden}` : String(visible);
}

export function graphScopeLabel(scope: TypeGraphScope) {
  switch (scope) {
    case "oneHop":
      return "1-hop neighborhood";
    case "twoHop":
      return "2-hop neighborhood";
    case "module":
      return "module scope";
    case "package":
      return "package scope";
    case "custom":
      return "custom query scope";
  }
}

export function selectedNodeEdges(edges: RenderEdge[], nodeId: string, category?: RelationshipCategory) {
  return edges.filter(
    (edge) =>
      (edge.source === nodeId || edge.target === nodeId)
      && (!category || edge.category === category),
  );
}

export function referencedByFieldEdges(edges: RenderEdge[], nodeId: string) {
  return edges.filter(
    (edge) =>
      edge.target === nodeId
      && (edge.category === "field" || edge.category === "generic")
      && Boolean(edge.edge.fieldName ?? edge.edge.declaringFieldName),
  );
}

export function nodeRiskClass(node: RenderNode) {
  if (node.riskTags.some((tag) => tag.includes("capability") || tag.includes("high"))) {
    return "bg-rose-500/15 text-rose-200";
  }

  if (node.riskTags.some((tag) => tag.includes("external"))) {
    return "bg-violet-500/15 text-violet-200";
  }

  if (node.riskTags.some((tag) => tag.includes("generic"))) {
    return "bg-yellow-500/15 text-yellow-200";
  }

  return "bg-sky-500/15 text-sky-200";
}

export function nodeInterpretation(node: RenderNode) {
  if (node.riskTags.includes("capability")) {
    return "Authority-bearing type. Review creation, transfer, and every public entry function that accepts this type.";
  }

  if (node.riskTags.includes("resource")) {
    return "Resource-like state type. Inspect storage fields, mutable references, and public entry points that can move value.";
  }

  if (node.riskTags.includes("trust boundary")) {
    return "External trust-boundary type. Confirm package address, freshness assumptions, and validation behavior before value movement.";
  }

  if (node.riskTags.includes("generic container") || node.riskTags.includes("generic asset")) {
    return "Generic/container type. Review concrete type arguments and phantom markers used for accounting or authorization.";
  }

  return "Type participates in the focused Move type graph. Switch modes to inspect storage, function surface, authority, generics, or external dependency links.";
}

export function nodeSecurityNotes(node: RenderNode, edges: RenderEdge[]) {
  const notes: string[] = [];

  if (node.riskTags.includes("resource")) {
    notes.push("High impact type candidate: has key ability or participates in storage edges.");
  }
  if (node.riskTags.includes("capability")) {
    notes.push("Capability-like type: confirm it is not copyable and cannot be publicly obtained.");
  }
  if (node.riskTags.includes("trust boundary")) {
    notes.push("External dependency type: review package address and whether it controls validation or value movement.");
  }
  if (edges.some((edge) => edge.category === "capability" && (edge.source === node.id || edge.target === node.id))) {
    notes.push("Capability-linked path visible in this scope.");
  }
  if (edges.some((edge) => edge.category === "input" && edge.edge.isMutable && (edge.source === node.id || edge.target === node.id))) {
    notes.push("Mutable reference path visible from callable code.");
  }

  return notes.length ? notes : ["No high-signal security notes in the current focused scope."];
}

export function edgeRelationName(edge: RenderEdge) {
  if (edge.category === "input" && edge.edge.isMutable) {
    return "mutable_borrow";
  }

  if (edge.category === "input" && edge.edge.isReference) {
    return "immutable_borrow";
  }

  if (edge.category === "capability") {
    return "authorizes";
  }

  if (edge.category === "generic") {
    return edge.edge.relationship === "phantomTypeParameter" ? "phantom_arg" : "generic_arg";
  }

  if (edge.category === "external") {
    return "external_dependency";
  }

  if (edge.category === "field") {
    return "field";
  }

  if (edge.category === "return") {
    return "function_output";
  }

  return edge.edge.relationship;
}

export function edgeConfidence(edge: RenderEdge) {
  if (["syntactic", "inferred", "heuristic"].includes(edge.edge.confidence)) {
    return edge.edge.confidence;
  }

  if (edge.category === "capability" || edge.category === "mutation") {
    return "inferred";
  }

  if (edge.category === "external" || edge.edge.confidence === "summary") {
    return "heuristic";
  }

  return "syntactic";
}

export function edgeRiskLevel(edge: RenderEdge) {
  if (edge.category === "capability") {
    return "high";
  }

  if (edge.category === "external" || edge.category === "mutation") {
    return "medium";
  }

  if (edge.edge.isMutable) {
    return "low";
  }

  return "none";
}

export function edgeSourceLocation(edge: MoveTypeGraphEdge) {
  return Array.isArray(edge.sourceSpans) ? edge.sourceSpans[0] ?? null : null;
}

export function edgeEvidenceItems(edge: RenderEdge) {
  return Array.isArray(edge.edge.evidence) && edge.edge.evidence.length
    ? edge.edge.evidence
    : [edgeLabel(edge.edge, edge.count, edge.category)];
}

export function renderNodeSourceLocation(renderGraph: TypeRenderGraph, nodeId: string) {
  const node = renderGraph.nodes.find((item) => item.id === nodeId || item.selectTypeId === nodeId);
  return node?.sourceLocation ?? null;
}

export function resolvedTypeNodeIdForEdge(renderGraph: TypeRenderGraph, edge: RenderEdge) {
  const fieldId = fieldNodeEndpoint(edge);

  if (fieldId) {
    const fieldNode = renderGraph.nodes.find((node) => node.id === fieldId);
    return fieldNode?.fieldInfo?.targetTypeId ?? null;
  }

  return edge.target;
}

export function edgeStrokeDash(edge: RenderEdge) {
  const confidence = edgeConfidence(edge);

  if (confidence === "heuristic") {
    return "1 6";
  }

  if (confidence === "inferred") {
    return "6 7";
  }

  return undefined;
}

export function edgeStrokeWidth(edge: RenderEdge) {
  const risk = edgeRiskLevel(edge);

  if (risk === "high") {
    return 2.4;
  }

  if (risk === "medium") {
    return 2;
  }

  return 1.7;
}

export function compactPath(path: string) {
  return path.split("/").slice(-2).join("/");
}

export function compactEdgeEndpoint(endpoint: string) {
  if (endpoint.startsWith("field-node:")) {
    const parts = endpoint.split(":");
    return parts.length >= 4 ? `field(${parts[2]})` : "field";
  }

  if (endpoint.startsWith("generic-instance:")) {
    return "generic instance";
  }

  const parts = endpoint.split("::");
  return parts.length >= 2 ? parts.slice(-2).join("::") : endpoint.replace(/^.*:/, "");
}

export function edgeEndpointLabel(endpoint: string, edge: RenderEdge) {
  if (endpoint.startsWith("field-node:")) {
    return edge.edge.fieldName ? `field(${edge.edge.fieldName})` : "field";
  }

  if (endpoint.startsWith("generic-instance:")) {
    return edge.edge.typeExpression ?? "generic instance";
  }

  return compactEdgeEndpoint(endpoint);
}

export function edgeNavigationTarget(edge: RenderEdge, currentNodeId: string | null) {
  const candidates = [edge.source, edge.target].filter(
    (id) => id !== currentNodeId && !id.startsWith("function:"),
  );

  if (candidates.length) {
    return candidates[0];
  }

  if (edge.source.startsWith("function:")) {
    return edge.target;
  }

  if (edge.target.startsWith("function:")) {
    return edge.source;
  }

  return edge.target === currentNodeId ? edge.source : edge.target;
}

export function edgeDisplayEndpoint(edge: RenderEdge, currentNodeId: string | null) {
  const endpoint = currentNodeId && edge.target === currentNodeId ? edge.source : edge.target;
  return compactEdgeEndpoint(endpoint);
}

export function shortAddress(address: string | null | undefined) {
  if (!address) {
    return null;
  }

  return address.length > 16 ? `${address.slice(0, 8)}...${address.slice(-6)}` : address;
}

export function sourceLocationFromMoveNode(node: MoveTypeGraphNode | null | undefined): MoveSourceSpan | null {
  if (!node?.filePath) {
    return null;
  }

  return {
    endByte: 0,
    endLine: 1,
    filePath: node.filePath,
    startByte: 0,
    startLine: 1,
  };
}

export function buildTypeRenderGraph({
  collapsedNodeIds,
  customQuery,
  expandedNodeIds,
  functionIndex,
  graph,
  lens,
  movePackage,
  scope,
  selectedTypeId,
}: {
  collapsedNodeIds: Set<string>;
  customQuery: string;
  expandedNodeIds: Set<string>;
  functionIndex: Map<string, FunctionContext>;
  graph: MoveTypeGraph;
  lens: TypeGraphLens;
  movePackage: MovePackage | null;
  scope: TypeGraphScope;
  selectedTypeId: string | null;
}): TypeRenderGraph {
  const packagePath = movePackage?.path ?? null;
  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  const localIds = new Set(
    graph.nodes
      .filter((node) => isLocalPackageType(node, packagePath))
      .map((node) => node.id),
  );
  const selectedNode = selectedTypeId ? nodeById.get(selectedTypeId) ?? null : null;
  const selectedRenderableId =
    selectedNode && isRenderableTypeNode(selectedNode) ? selectedNode.id : null;
  const allNormalizedEdges = graph.edges
    .map((edge) => normalizeEdge(edge, nodeById, packagePath))
    .filter((edge): edge is NormalizedEdge => Boolean(edge));
  const normalizedEdges = allNormalizedEdges
    .filter((edge) => edgeMatchesLens(edge, lens, localIds, packagePath));
  const compactStorageDefault = lens === "storage" && scope === "oneHop" && !customQuery.trim();
  const selectedIds = graphScopeIds({
    customQuery,
    edges: normalizedEdges,
    nodes: graph.nodes,
    packagePath,
    collapsedNodeIds,
    expandedNodeIds,
    scope,
    selectedId: selectedRenderableId,
    selectedNode,
  });
  const fieldFocusLayout = Boolean(
    lens === "storage"
    && scope === "oneHop"
    && selectedRenderableId
    && !customQuery.trim(),
  );
  const visibleNormalizedEdges = normalizedEdges.filter((normalized) => {
    const shouldInclude = selectedIds
      ? selectedIds.has(normalized.source) && selectedIds.has(normalized.target)
      : edgeTouchesPackage(normalized, localIds, packagePath);

    return shouldInclude;
  });
  const exactGenericArgumentKeys = exactGenericArgumentEdgeKeys(visibleNormalizedEdges);
  const renderNormalizedEdges = visibleNormalizedEdges.filter(
    (edge) =>
      !isOwnerGenericShortcut(edge, exactGenericArgumentKeys)
      && !(compactStorageDefault && edge.edge.relationship === "genericArgument"),
  );
  const genericInstances = genericInstanceInfo(visibleNormalizedEdges, nodeById, localIds, packagePath);
  const outgoingFieldFocusEdges = fieldFocusLayout && selectedRenderableId
    ? renderNormalizedEdges
      .filter((edge) =>
        edge.source === selectedRenderableId
        && isStorageFieldRelationship(edge.edge.relationship)
        && Boolean(edge.edge.fieldName),
      )
    : [];
  const incomingFieldFocusEdges = fieldFocusLayout && selectedRenderableId && outgoingFieldFocusEdges.length === 0
    ? renderNormalizedEdges
      .filter((edge) =>
        edge.target === selectedRenderableId
        && isStorageFieldRelationship(edge.edge.relationship)
        && Boolean(edge.edge.fieldName),
      )
    : [];
  const fieldFocusEdges = [...outgoingFieldFocusEdges, ...incomingFieldFocusEdges]
    .sort((left, right) => fieldEdgeSort(left, right));
  const fieldFocusSourceIds = new Set(fieldFocusEdges.map((edge) => edge.source));
  const fieldFocusTargetIds = new Set(fieldFocusEdges.map((edge) => edge.target));
  const includedTypeIds = new Set<string>();
  const includedFunctionIds = new Set<string>();
  const includedGenericInstanceIds = new Set<string>();
  const syntheticFieldNodes: RenderNode[] = [];
  const edgeGroups = new Map<string, RenderEdge>();

  if (selectedRenderableId) {
    includedTypeIds.add(selectedRenderableId);
  } else {
    localIds.forEach((id) => includedTypeIds.add(id));
  }

  const edgesForRender = fieldFocusLayout ? fieldFocusEdges : renderNormalizedEdges;

  for (const normalized of edgesForRender) {
    const genericInstance = genericInstanceForEdge(normalized, genericInstances);
    const targetId =
      isStorageFieldRelationship(normalized.edge.relationship) && genericInstance && !fieldFocusLayout
        ? genericInstance.id
        : normalized.target;

    if (fieldFocusLayout && selectedRenderableId) {
      const renderEdge = normalizedEdgeForRender(normalized, nodeById);
      const category = relationshipCategory(normalized, localIds, packagePath);
      const fieldId = fieldNodeId(
        normalized.source,
        normalized.edge.fieldName ?? "field",
        targetId,
      );
      const fieldEvidenceEdgeId = edgeGroupKey(fieldId, targetId, renderEdge, category);
      const fieldNode = syntheticFieldNode(
        normalized,
        syntheticFieldNodes.length,
        fieldFocusEdges.length,
        normalized.source,
        targetId,
        fieldEvidenceEdgeId,
        genericInstance,
      );
      syntheticFieldNodes.push(fieldNode);

      if (normalized.sourceKind === "type") {
        includedTypeIds.add(normalized.source);
      }

      if (normalized.targetKind === "type") {
        includedTypeIds.add(normalized.target);
      }

      addRenderEdge(edgeGroups, {
        category,
        edge: renderEdge,
        source: normalized.source,
        target: fieldNode.id,
      });
      addRenderEdge(edgeGroups, {
        category,
        edge: renderEdge,
        source: fieldNode.id,
        target: targetId,
      });
      continue;
    }

    const renderSource =
      normalized.edge.relationship === "genericArgument" && genericInstance
        ? genericInstance.id
        : normalized.source;
    const renderTarget = targetId;

    if (normalized.sourceKind === "function") {
      includedFunctionIds.add(normalized.source);
    } else if (renderSource === normalized.source) {
      includedTypeIds.add(normalized.source);
    } else {
      includedGenericInstanceIds.add(renderSource);
    }

    if (normalized.targetKind === "function") {
      includedFunctionIds.add(normalized.target);
    } else if (renderTarget === normalized.target) {
      includedTypeIds.add(normalized.target);
    } else {
      includedGenericInstanceIds.add(renderTarget);
    }

    addRenderEdge(edgeGroups, {
      category: relationshipCategory(normalized, localIds, packagePath),
      edge: normalizedEdgeForRender(normalized, nodeById),
      source: renderSource,
      target: renderTarget,
    });
  }

  const typeNodes = [...includedTypeIds]
    .map((id) => nodeById.get(id))
    .filter((node): node is MoveTypeGraphNode => Boolean(node && isRenderableTypeNode(node)));
  const localNodes = sortTypeNodes(typeNodes.filter((node) => localIds.has(node.id)), selectedRenderableId);
  const dependencyNodes = sortTypeNodes(typeNodes.filter((node) => !localIds.has(node.id)), selectedRenderableId);
  const fieldFocusSourceNodes = fieldFocusLayout && selectedRenderableId
    ? sortTypeNodes(
      typeNodes.filter((node) =>
        fieldFocusEdges.length ? fieldFocusSourceIds.has(node.id) : node.id === selectedRenderableId,
      ),
      selectedRenderableId,
    )
    : localNodes;
  const fieldFocusTargetNodes = fieldFocusLayout && selectedRenderableId
    ? sortTypeNodes(
      typeNodes.filter((node) => fieldFocusTargetIds.has(node.id)),
      selectedRenderableId,
    )
    : dependencyNodes;
  const syntheticGenericNodes = [...includedGenericInstanceIds]
    .map((id) => genericInstances.get(id))
    .filter((instance): instance is GenericInstanceInfo => Boolean(instance))
    .sort((left, right) => left.label.localeCompare(right.label) || left.id.localeCompare(right.id));
  const functionNodes = [...includedFunctionIds].sort((left, right) =>
    functionLabel(left, functionIndex).localeCompare(functionLabel(right, functionIndex)),
  );
  const renderEdges = assignEdgeRoutes([...edgeGroups.values()].sort((left, right) => left.id.localeCompare(right.id)));
  const edgeEvidence = collectEdgeEvidence(allNormalizedEdges, localIds, packagePath);
  const metricsByType = typeNodeMetrics(edgeEvidence, functionIndex);
  const maxRows = Math.max(
    fieldFocusLayout ? Math.max(1, fieldFocusSourceNodes.length) : localNodes.length,
    syntheticFieldNodes.length,
    fieldFocusTargetNodes.length + syntheticGenericNodes.length,
    functionNodes.length,
    1,
  );
  const storageLayout = lens === "storage" && functionNodes.length === 0;
  const renderNodes = fieldFocusLayout && selectedRenderableId
    ? [
      ...fieldFocusSourceNodes.map((node, index) =>
        typeRenderNode(
          node,
          classifyTypeNode(node, packagePath),
          index,
          maxRows,
          metricsByType.get(node.id),
          null,
          storageLayout,
          "parent",
        ),
      ),
      ...syntheticFieldNodes,
      ...fieldFocusTargetNodes.map((node, index) =>
        typeRenderNode(
          node,
          classifyTypeNode(node, packagePath),
          index,
          maxRows,
          metricsByType.get(node.id),
          null,
          storageLayout,
          "target",
        ),
      ),
      ...syntheticGenericNodes.map((instance, index) =>
        syntheticGenericInstanceNode(
          instance,
          nodeById.get(instance.baseTypeId) ?? null,
          fieldFocusTargetNodes.length + index,
          maxRows,
          packagePath,
          storageLayout,
        ),
      ),
    ]
    : [
    ...functionNodes.map((id, index) => syntheticFunctionNode(id, index, maxRows, functionIndex)),
    ...localNodes.map((node, index) =>
      typeRenderNode(node, "local", index, maxRows, metricsByType.get(node.id), null, storageLayout, "parent"),
    ),
    ...dependencyNodes.map((node, index) =>
      typeRenderNode(
        node,
        classifyTypeNode(node, packagePath),
        index,
        maxRows,
        metricsByType.get(node.id),
        null,
        storageLayout,
        "target",
      ),
    ),
    ...syntheticGenericNodes.map((instance, index) =>
      syntheticGenericInstanceNode(
        instance,
        nodeById.get(instance.baseTypeId) ?? null,
        dependencyNodes.length + index,
        maxRows,
        packagePath,
        storageLayout,
      ),
    ),
  ];
  const limitedGraph = limitRenderGraph(renderNodes, renderEdges, selectedRenderableId);
  const limitedTypeNodes = limitedGraph.nodes
    .map((node) => node.node)
    .filter((node): node is MoveTypeGraphNode => Boolean(node));
  const limitedDependencyNodes = limitedTypeNodes.filter((node) => !localIds.has(node.id));
  const selectedLabel = selectedRenderableId
    ? typeDisplayName(nodeById.get(selectedRenderableId)!, classifyTypeNode(nodeById.get(selectedRenderableId)!, packagePath))
    : null;
  const capabilityCount = limitedTypeNodes.filter(isCapabilityLike).length;
  const resourceCount = limitedTypeNodes.filter(isResourceLike).length;
  const rawEdgeCount = limitedGraph.edges.reduce((total, edge) => total + edge.count, 0);
  const genericEdgeCount = limitedGraph.edges
    .filter((edge) => edge.category === "generic")
    .reduce((total, edge) => total + edge.count, 0);

  return {
    capabilityCount,
    edges: limitedGraph.edges,
    edgeEvidence,
    externalCount: limitedDependencyNodes.filter((node) => classifyTypeNode(node, packagePath) === "external").length,
    frameworkCount: limitedDependencyNodes.filter((node) => {
      const kind = classifyTypeNode(node, packagePath);
      return kind === "framework" || kind === "builtin";
    }).length,
    functionCount: limitedGraph.nodes.filter((node) => node.kind === "function").length,
    genericEdgeCount,
    hiddenEdgeCount: limitedGraph.hiddenEdgeCount,
    hiddenNodeCount: limitedGraph.hiddenNodeCount,
    localCount: limitedTypeNodes.filter((node) => localIds.has(node.id)).length,
    nodes: limitedGraph.nodes,
    rawEdgeCount,
    resourceCount,
    selectedLabel,
    selectedNode: limitedGraph.nodes.find((node) => node.id === selectedRenderableId) ?? null,
  };
}

export function graphScopeIds({
  collapsedNodeIds,
  customQuery,
  edges,
  expandedNodeIds,
  nodes,
  packagePath,
  scope,
  selectedId,
  selectedNode,
}: {
  collapsedNodeIds: Set<string>;
  customQuery: string;
  edges: NormalizedEdge[];
  expandedNodeIds: Set<string>;
  nodes: MoveTypeGraphNode[];
  packagePath: string | null;
  scope: TypeGraphScope;
  selectedId: string | null;
  selectedNode: MoveTypeGraphNode | null;
}) {
  if (scope === "package") {
    return null;
  }

  if (scope === "custom") {
    const query = customQuery.trim();
    if (!query) {
      return selectedId ? oneHopNeighborhood(selectedId, edges) : null;
    }

    const matchingIds = new Set<string>();

    for (const node of nodes) {
      if (isRenderableTypeNode(node) && nodeMatchesQuery(node, query, packagePath)) {
        matchingIds.add(node.id);
      }
    }

    for (const edge of edges) {
      if (edgeMatchesQuery(edge, query)) {
        matchingIds.add(edge.source);
        matchingIds.add(edge.target);
      }
    }

    return expandIds(matchingIds, edges, 1);
  }

  if (!selectedId) {
    return null;
  }

  const expandedSeeds = new Set([selectedId, ...expandedNodeIds]);
  let ids: Set<string>;

  if (scope === "oneHop") {
    ids = expandIds(expandedSeeds, edges, 1);
    collapsedNodeIds.forEach((id) => removeNeighbors(ids, edges, id));
    return ids;
  }

  if (scope === "twoHop") {
    ids = expandIds(expandedSeeds, edges, 2);
    collapsedNodeIds.forEach((id) => removeNeighbors(ids, edges, id));
    return ids;
  }

  if (scope === "module" && selectedNode?.moduleName) {
    const ids = new Set<string>();

    for (const node of nodes) {
      if (
        isRenderableTypeNode(node)
        && node.packagePath === selectedNode.packagePath
        && node.moduleName === selectedNode.moduleName
      ) {
        ids.add(node.id);
      }
    }

    const moduleIds = expandIds(ids, edges, 1);
    collapsedNodeIds.forEach((id) => removeNeighbors(moduleIds, edges, id));
    return moduleIds;
  }

  ids = oneHopNeighborhood(selectedId, edges);
  collapsedNodeIds.forEach((id) => removeNeighbors(ids, edges, id));
  return ids;
}

export function addRenderEdge(
  edgeGroups: Map<string, RenderEdge>,
  {
    category,
    edge,
    source,
    target,
  }: {
    category: RelationshipCategory;
    edge: MoveTypeGraphEdge;
    source: string;
    target: string;
  },
) {
  const groupKey = edgeGroupKey(source, target, edge, category);
  const current = edgeGroups.get(groupKey);

  if (current) {
    current.count += 1;
    return;
  }

  edgeGroups.set(groupKey, {
    category,
    count: 1,
    edge,
    id: groupKey,
    routeCount: 1,
    routeIndex: 0,
    source,
    target,
  });
}

export function fieldEdgeSort(left: NormalizedEdge, right: NormalizedEdge) {
  return (
    fieldSortPriority(left) - fieldSortPriority(right)
    || (left.edge.fieldName ?? "").localeCompare(right.edge.fieldName ?? "")
    || left.target.localeCompare(right.target)
  );
}

export function fieldSortPriority(edge: NormalizedEdge) {
  const name = edge.edge.fieldName ?? "";

  if (name === "id" || name === "uid") {
    return 0;
  }

  if (edge.targetNode && isResourceLike(edge.targetNode)) {
    return 1;
  }

  if (edge.targetNode?.kind === "builtin") {
    return 4;
  }

  if (isGenericContainer(edge.targetNode)) {
    return 2;
  }

  return 3;
}

export function oneHopNeighborhood(selectedId: string, edges: NormalizedEdge[]) {
  return expandIds(new Set([selectedId]), edges, 1);
}

export function expandIds(seedIds: Set<string>, edges: NormalizedEdge[], depth: number) {
  const ids = new Set(seedIds);
  let frontier = new Set(seedIds);

  for (let index = 0; index < depth; index += 1) {
    const next = new Set<string>();

    for (const edge of edges) {
      if (frontier.has(edge.source)) {
        next.add(edge.target);
      }
      if (frontier.has(edge.target)) {
        next.add(edge.source);
      }
    }

    next.forEach((id) => ids.add(id));
    frontier = next;
  }

  return ids;
}

export function removeNeighbors(ids: Set<string>, edges: NormalizedEdge[], nodeId: string) {
  for (const edge of edges) {
    if (edge.source === nodeId) {
      ids.delete(edge.target);
    }
    if (edge.target === nodeId) {
      ids.delete(edge.source);
    }
  }
  ids.add(nodeId);
}

export function nodeMatchesQuery(node: MoveTypeGraphNode, query: string, packagePath: string | null) {
  const lowerQuery = query.toLowerCase();
  const [prefix, rawValue] = lowerQuery.includes(":") ? lowerQuery.split(/:(.*)/, 2) : ["", lowerQuery];
  const value = rawValue ?? "";

  if (prefix === "ability") {
    return effectiveTypeAbilities(node).some((ability) => ability.toLowerCase() === value);
  }

  if (prefix === "origin") {
    return nodeOriginLabel(node, classifyTypeNode(node, packagePath)).toLowerCase().includes(value);
  }

  if (prefix === "kind") {
    return nodeRoleLabel(node, classifyTypeNode(node, packagePath)).toLowerCase().includes(value);
  }

  if (prefix === "generic") {
    return (
      node.qualifiedName.toLowerCase().includes(value)
      || node.name.toLowerCase().includes(value)
      || (node.typeParameters ?? []).some((parameter) => parameter.name.toLowerCase().includes(value))
    );
  }

  return (
    node.name.toLowerCase().includes(lowerQuery)
    || node.qualifiedName.toLowerCase().includes(lowerQuery)
    || (node.moduleName?.toLowerCase().includes(lowerQuery) ?? false)
    || effectiveTypeAbilities(node).some((ability) => ability.toLowerCase().includes(lowerQuery))
  );
}

export function edgeMatchesQuery(edge: NormalizedEdge, query: string) {
  const lowerQuery = query.toLowerCase();
  const [prefix, rawValue] = lowerQuery.includes(":") ? lowerQuery.split(/:(.*)/, 2) : ["", lowerQuery];
  const value = rawValue ?? "";

  if (prefix === "field") {
    return edge.edge.fieldName?.toLowerCase().includes(value) ?? false;
  }

  if (prefix === "uses") {
    return edge.edge.functionName?.toLowerCase().includes(value) ?? edge.source.toLowerCase().includes(value);
  }

  return (
    edge.edge.relationship.toLowerCase().includes(lowerQuery)
    || (edge.edge.fieldName?.toLowerCase().includes(lowerQuery) ?? false)
    || (edge.edge.functionName?.toLowerCase().includes(lowerQuery) ?? false)
    || (edge.edge.typeExpression?.toLowerCase().includes(lowerQuery) ?? false)
    || (edge.edge.typeArgumentName?.toLowerCase().includes(lowerQuery) ?? false)
    || edge.source.toLowerCase().includes(lowerQuery)
    || edge.target.toLowerCase().includes(lowerQuery)
  );
}

export function typeNodeMetrics(edges: RenderEdge[], functionIndex: Map<string, FunctionContext>) {
  const metrics = new Map<string, { entryFunctionCount: number; fieldCount: number; functionCount: number }>();

  const ensure = (typeId: string) => {
    const current = metrics.get(typeId);
    if (current) {
      return current;
    }

    const next = { entryFunctionCount: 0, fieldCount: 0, functionCount: 0 };
    metrics.set(typeId, next);
    return next;
  };

  for (const edge of edges) {
    if (edge.category === "field") {
      ensure(edge.source).fieldCount += edge.count;
    }

    if (edge.source.startsWith("function:")) {
      const metric = ensure(edge.target);
      metric.functionCount += 1;
      if (functionIndex.get(edge.source)?.isEntry) {
        metric.entryFunctionCount += 1;
      }
    }

    if (edge.target.startsWith("function:")) {
      const metric = ensure(edge.source);
      metric.functionCount += 1;
      if (functionIndex.get(edge.target)?.isEntry) {
        metric.entryFunctionCount += 1;
      }
    }
  }

  return metrics;
}

export function exactGenericArgumentEdgeKeys(edges: NormalizedEdge[]) {
  const keys = new Set<string>();

  for (const edge of edges) {
    if (
      edge.edge.relationship === "genericArgument"
      && edge.edge.declaringTypeId
      && edge.edge.declaringFieldName
      && edge.source !== edge.edge.declaringTypeId
    ) {
      keys.add(genericArgumentShortcutKey(edge));
    }
  }

  return keys;
}

export function isOwnerGenericShortcut(edge: NormalizedEdge, exactKeys: Set<string>) {
  return Boolean(
    edge.edge.relationship === "genericArgument"
    && edge.edge.declaringTypeId
    && edge.source === edge.edge.declaringTypeId
    && exactKeys.has(genericArgumentShortcutKey(edge)),
  );
}

export function genericArgumentShortcutKey(edge: NormalizedEdge) {
  return [
    edge.edge.declaringTypeId ?? "",
    edge.edge.declaringFieldName ?? "",
    edge.edge.typeArgumentIndex ?? "",
    edge.target,
  ].join("|");
}

export function genericInstanceForEdge(
  edge: NormalizedEdge,
  instances: Map<string, GenericInstanceInfo>,
) {
  const declaringTypeId = edge.edge.declaringTypeId;
  const declaringFieldName = edge.edge.declaringFieldName ?? edge.edge.fieldName;

  if (!declaringTypeId || !declaringFieldName) {
    return null;
  }

  if (edge.edge.relationship === "genericArgument") {
    return instances.get(genericInstanceId(edge.source, declaringTypeId, declaringFieldName))
      ?? genericInstanceForDeclaringField(instances, declaringTypeId, declaringFieldName);
  }

  if (isStorageFieldRelationship(edge.edge.relationship)) {
    return instances.get(genericInstanceId(edge.target, declaringTypeId, declaringFieldName)) ?? null;
  }

  return null;
}

export function genericInstanceForDeclaringField(
  instances: Map<string, GenericInstanceInfo>,
  declaringTypeId: string,
  declaringFieldName: string,
) {
  for (const instance of instances.values()) {
    if (
      instance.declaringTypeId === declaringTypeId
      && instance.declaringFieldName === declaringFieldName
    ) {
      return instance;
    }
  }

  return null;
}

export function normalizedEdgeForRender(
  normalized: NormalizedEdge,
  nodeById: Map<string, MoveTypeGraphNode>,
): MoveTypeGraphEdge {
  if (normalized.edge.relationship !== "genericArgument" || normalized.edge.typeArgumentName) {
    return normalized.edge;
  }

  const sourceNode = normalized.sourceNode ?? nodeById.get(normalized.source) ?? null;
  return {
    ...normalized.edge,
    typeArgumentName: genericArgumentNameForNode(
      sourceNode,
      normalized.edge.typeArgumentIndex ?? 0,
    ),
  };
}

export function assignEdgeRoutes(edges: RenderEdge[]) {
  const groups = new Map<string, RenderEdge[]>();

  for (const edge of edges) {
    const key = [edge.source, edge.target, edge.category].join("|");
    groups.set(key, [...(groups.get(key) ?? []), edge]);
  }

  return edges.map((edge) => {
    const group = groups.get([edge.source, edge.target, edge.category].join("|")) ?? [edge];
    const routeIndex = group.findIndex((item) => item.id === edge.id);

    return {
      ...edge,
      routeCount: group.length,
      routeIndex: Math.max(0, routeIndex),
    };
  });
}

export function genericInstanceKey(baseTypeId: string, declaringTypeId: string, declaringFieldName: string) {
  return [baseTypeId, declaringTypeId, declaringFieldName].join("|");
}

export function genericInstanceId(baseTypeId: string, declaringTypeId: string, declaringFieldName: string) {
  return `generic-instance:${genericInstanceKey(baseTypeId, declaringTypeId, declaringFieldName)}`;
}

export function isConcreteGenericContainer(
  node: MoveTypeGraphNode,
  localIds: Set<string>,
  packagePath: string | null,
) {
  if (localIds.has(node.id) || isLocalPackageType(node, packagePath)) {
    return node.qualifiedName.includes("<");
  }

  return isGenericContainer(node) || knownGenericParameterNames(node).length > 0;
}

export function genericArgumentNameForNode(node: MoveTypeGraphNode | null, index: number) {
  return node?.typeParameters?.[index]?.name
    || knownGenericParameterNames(node)[index]
    || String(index);
}

export function knownGenericParameterNames(node: MoveTypeGraphNode | null) {
  const moduleName = node?.moduleName?.toLowerCase() ?? "";
  const name = node?.name?.toLowerCase() ?? "";

  if (moduleName === "table" && name === "table") {
    return ["K", "V"];
  }

  if (moduleName === "vec_map" || moduleName === "vec_set") {
    return moduleName === "vec_map" ? ["K", "V"] : ["T"];
  }

  if (moduleName === "coin" && name === "coin") {
    return ["T"];
  }

  if (moduleName === "balance" && (name === "balance" || name === "supply")) {
    return ["T"];
  }

  if (name === "vector") {
    return ["T"];
  }

  return [];
}

export function genericArgumentSort(label: string) {
  const normalized = label.toUpperCase();

  if (normalized === "K") {
    return 0;
  }

  if (normalized === "V") {
    return 1;
  }

  if (normalized === "T") {
    return 0;
  }

  const numeric = Number(label);
  return Number.isFinite(numeric) ? numeric : 20;
}

export function genericInstanceInfo(
  edges: NormalizedEdge[],
  nodeById: Map<string, MoveTypeGraphNode>,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const fieldTargets = genericFieldTargets(edges, nodeById, localIds, packagePath);
  const grouped = new Map<string, GenericInstanceInfo>();

  for (const edge of edges) {
    if (edge.edge.relationship !== "genericArgument" || edge.sourceKind !== "type") {
      continue;
    }

    const declaringTypeId = edge.edge.declaringTypeId;
    const declaringFieldName = edge.edge.declaringFieldName;
    const baseTypeId = declaringTypeId && declaringFieldName
      ? fieldTargets.get(declaringFieldKey(declaringTypeId, declaringFieldName)) ?? edge.source
      : edge.source;
    const node = nodeById.get(baseTypeId);

    if (
      !node
      || !declaringTypeId
      || !declaringFieldName
      || !isConcreteGenericContainer(node, localIds, packagePath)
    ) {
      continue;
    }

    const key = genericInstanceKey(baseTypeId, declaringTypeId, declaringFieldName);
    const current = grouped.get(key) ?? {
      arguments: [],
      baseTypeId,
      declaringFieldName,
      declaringTypeId,
      id: genericInstanceId(baseTypeId, declaringTypeId, declaringFieldName),
      label: "",
      sourceLocation: null,
    };
    const argument = {
      label: edge.edge.typeArgumentName
        ?? genericArgumentNameForNode(node, edge.edge.typeArgumentIndex ?? current.arguments.length),
      value: edge.edge.typeExpression ?? compactEdgeEndpoint(edge.target),
    };

    if (
      !current.arguments.some((item) =>
        item.label === argument.label && item.value === argument.value,
      )
    ) {
      current.arguments.push(argument);
    }
    current.sourceLocation = current.sourceLocation ?? edgeSourceLocation(edge.edge);
    grouped.set(key, current);
  }

  const instances = new Map<string, GenericInstanceInfo>();

  for (const instance of grouped.values()) {
    const node = nodeById.get(instance.baseTypeId);

    if (!node || !instance.arguments.length) {
      continue;
    }

    const sortedArgs = [...instance.arguments].sort((left, right) =>
      genericArgumentSort(left.label) - genericArgumentSort(right.label)
      || left.label.localeCompare(right.label),
    );
    const base = node.name || typeDisplayName(node);
    const label = `${base}<${sortedArgs.map((argument) => compactTypeLabel(argument.value).split("::").at(-1) ?? compactTypeLabel(argument.value)).join(", ")}>`;

    instances.set(instance.id, {
      ...instance,
      arguments: sortedArgs,
      label,
    });
  }

  return instances;
}

export function genericFieldTargets(
  edges: NormalizedEdge[],
  nodeById: Map<string, MoveTypeGraphNode>,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const targets = new Map<string, string>();

  for (const edge of edges) {
    const declaringTypeId = edge.edge.declaringTypeId;
    const declaringFieldName = edge.edge.declaringFieldName ?? edge.edge.fieldName;
    const targetNode = nodeById.get(edge.target);

    if (
      edge.sourceKind === "type"
      && edge.targetKind === "type"
      && declaringTypeId
      && declaringFieldName
      && isStorageFieldRelationship(edge.edge.relationship)
      && targetNode
      && isConcreteGenericContainer(targetNode, localIds, packagePath)
    ) {
      targets.set(declaringFieldKey(declaringTypeId, declaringFieldName), edge.target);
    }
  }

  return targets;
}

export function declaringFieldKey(declaringTypeId: string, declaringFieldName: string) {
  return `${declaringTypeId}|${declaringFieldName}`;
}

export function limitRenderGraph(
  nodes: RenderNode[],
  edges: RenderEdge[],
  selectedTypeId: string | null,
) {
  const nodeLimit = selectedTypeId ? MAX_FOCUSED_NODES : MAX_OVERVIEW_NODES;
  const totalEdgeCount = edges.reduce((total, edge) => total + edge.count, 0);

  if (nodes.length <= nodeLimit && totalEdgeCount <= MAX_RENDER_EDGES) {
    return {
      edges,
      hiddenEdgeCount: 0,
      hiddenNodeCount: 0,
      nodes: relayoutRenderNodes(nodes),
    };
  }

  const degree = nodeDegree(edges);
  const selectedDistances = selectedTypeId ? graphDistances(selectedTypeId, edges, 2) : new Map<string, number>();
  const selectedNode = selectedTypeId ? nodes.find((node) => node.id === selectedTypeId) ?? null : null;
  const prioritizedNodes = [...nodes].sort((left, right) =>
    nodeRenderPriority(left, selectedTypeId, selectedDistances, degree)
    - nodeRenderPriority(right, selectedTypeId, selectedDistances, degree)
    || left.label.localeCompare(right.label)
    || left.id.localeCompare(right.id),
  );
  const visibleIds = new Set<string>();

  if (selectedNode) {
    visibleIds.add(selectedNode.id);
  }

  for (const node of prioritizedNodes) {
    if (visibleIds.size >= nodeLimit) {
      break;
    }
    visibleIds.add(node.id);
  }

  const visibleCandidateEdges = edges.filter((edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target));
  const limitedEdges = prioritizedEdges(visibleCandidateEdges, selectedTypeId).slice(0, MAX_RENDER_EDGES);
  const edgeNodeIds = new Set<string>();

  for (const edge of limitedEdges) {
    edgeNodeIds.add(edge.source);
    edgeNodeIds.add(edge.target);
  }

  if (selectedNode) {
    edgeNodeIds.add(selectedNode.id);
  }

  const limitedNodes = relayoutRenderNodes(
    nodes.filter((node) => visibleIds.has(node.id) && (edgeNodeIds.has(node.id) || node.kind === "local")),
  );
  const visibleEdgeCount = limitedEdges.reduce((total, edge) => total + edge.count, 0);

  return {
    edges: limitedEdges,
    hiddenEdgeCount: Math.max(0, totalEdgeCount - visibleEdgeCount),
    hiddenNodeCount: Math.max(0, nodes.length - limitedNodes.length),
    nodes: limitedNodes,
  };
}

export function nodeRenderPriority(
  node: RenderNode,
  selectedTypeId: string | null,
  selectedDistances: Map<string, number>,
  degree: Map<string, number>,
) {
  let score = 0;

  if (node.id === selectedTypeId) {
    score -= 100_000;
  }

  if (selectedTypeId) {
    const distance = selectedDistances.get(node.id);
    score += distance === undefined ? 50_000 : distance * 1_000;
  }

  if (node.kind === "local") {
    score -= 5_000;
  } else if (node.kind === "framework" || node.kind === "builtin") {
    score -= 1_500;
  } else if (node.kind === "external") {
    score -= 800;
  } else if (node.kind === "function") {
    score += selectedTypeId ? 250 : 2_500;
  }

  if (isResourceLike(node.node)) {
    score -= 600;
  }

  if (isCapabilityLike(node.node)) {
    score -= 500;
  }

  score -= (degree.get(node.id) ?? 0) * 25;

  return score;
}

export function prioritizedEdges(edges: RenderEdge[], selectedTypeId: string | null) {
  return [...edges].sort((left, right) =>
    edgeRenderPriority(left, selectedTypeId) - edgeRenderPriority(right, selectedTypeId)
    || right.count - left.count
    || left.id.localeCompare(right.id),
  );
}

export function edgeRenderPriority(edge: RenderEdge, selectedTypeId: string | null) {
  let score = 0;

  if (selectedTypeId && (edge.source === selectedTypeId || edge.target === selectedTypeId)) {
    score -= 10_000;
  }

  if (edge.category === "field") {
    score -= 700;
  } else if (edge.category === "capability") {
    score -= 650;
  } else if (edge.category === "input" || edge.category === "return") {
    score -= 500;
  } else if (edge.category === "generic") {
    score -= 300;
  }

  score -= Math.min(edge.count, 20) * 10;

  return score;
}

export function nodeDegree(edges: RenderEdge[]) {
  const degree = new Map<string, number>();

  for (const edge of edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + edge.count);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + edge.count);
  }

  return degree;
}

export function graphDistances(startId: string, edges: RenderEdge[], maxDepth: number) {
  const distances = new Map([[startId, 0]]);
  let frontier = new Set([startId]);

  for (let depth = 1; depth <= maxDepth; depth += 1) {
    const next = new Set<string>();

    for (const edge of edges) {
      if (frontier.has(edge.source) && !distances.has(edge.target)) {
        distances.set(edge.target, depth);
        next.add(edge.target);
      }
      if (frontier.has(edge.target) && !distances.has(edge.source)) {
        distances.set(edge.source, depth);
        next.add(edge.source);
      }
    }

    frontier = next;
  }

  return distances;
}

export function relayoutRenderNodes(nodes: RenderNode[]) {
  const functionNodes = nodes.filter((node) => node.layoutRole === "function");
  const localNodes = nodes.filter((node) => node.layoutRole === "parent");
  const fieldNodes = nodes.filter((node) => node.layoutRole === "field");
  const dependencyNodes = nodes
    .filter((node) => node.layoutRole === "target")
    .sort((left, right) =>
      dependencyGroupOrder(left.groupLabel) - dependencyGroupOrder(right.groupLabel)
      || (left.groupLabel ?? "").localeCompare(right.groupLabel ?? "")
      || left.label.localeCompare(right.label)
      || left.id.localeCompare(right.id),
    );
  const dependencyNodesWithHeaders = dependencyNodes.map((node, index) => ({
    ...node,
    showGroupLabel: Boolean(
      node.groupLabel
      && (index === 0 || dependencyNodes[index - 1]?.groupLabel !== node.groupLabel),
    ),
  }));
  const maxRows = Math.max(functionNodes.length, localNodes.length, fieldNodes.length, dependencyNodes.length, 1);
  const storageLayout = functionNodes.length === 0;

  if (storageLayout && fieldNodes.length > 0 && localNodes.length === 1) {
    return relayoutStorageStarNodes(localNodes[0]!, fieldNodes, dependencyNodes);
  }

  return [
    ...functionNodes.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
    ...localNodes.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
    ...fieldNodes.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
    ...dependencyNodesWithHeaders.map((node, index) => repositionRenderNode(node, index, maxRows, storageLayout)),
  ];
}

export function relayoutStorageStarNodes(
  hubNode: RenderNode,
  fieldNodes: RenderNode[],
  dependencyNodes: RenderNode[],
) {
  const hub = {
    ...hubNode,
    showGroupLabel: false,
    x: 0,
    y: 0,
  };
  const hubCenter = {
    x: hub.x + TYPE_GRAPH_NODE_WIDTH / 2,
    y: hub.y + TYPE_GRAPH_NODE_HEIGHT / 2,
  };
  const fieldAngles = starFanAngles(fieldNodes.length);
  const rawFieldLayouts = fieldNodes.map((node, index) => {
    const angle = fieldAngles[index] ?? 0;
    const radians = degreesToRadians(angle);

    return {
      angle,
      centerX: hubCenter.x + TYPE_GRAPH_STAR_FIELD_OFFSET_X + Math.cos(radians) * TYPE_GRAPH_STAR_FIELD_ARC_X,
      centerY: hubCenter.y + Math.sin(radians) * TYPE_GRAPH_STAR_FIELD_ARC_Y,
      node,
    };
  });
  const fieldLayouts = spreadStarLayouts(rawFieldLayouts, TYPE_GRAPH_STAR_FIELD_MIN_GAP);
  const fieldAngleById = new Map(fieldLayouts.map((layout) => [layout.node.id, layout.angle]));
  const targetAngleById = new Map<string, { count: number; x: number; y: number }>();

  for (const layout of fieldLayouts) {
    const targetId = layout.node.fieldInfo?.targetTypeId;

    if (!targetId) {
      continue;
    }

    const radians = degreesToRadians(layout.angle);
    const current = targetAngleById.get(targetId) ?? { count: 0, x: 0, y: 0 };
    current.count += 1;
    current.x += Math.cos(radians);
    current.y += Math.sin(radians);
    targetAngleById.set(targetId, current);
  }

  const rawTargetLayouts = dependencyNodes.map((node, index) => {
    const vector = targetAngleById.get(node.id);
    const fallbackAngle = starFanAngles(dependencyNodes.length)[index] ?? 0;
    const angle = vector && vector.count > 0
      ? radiansToDegrees(Math.atan2(vector.y / vector.count, Math.max(0.16, vector.x / vector.count)))
      : fallbackAngle;
    const radians = degreesToRadians(angle);

    return {
      angle,
      centerX: hubCenter.x + TYPE_GRAPH_STAR_TARGET_OFFSET_X + Math.cos(radians) * TYPE_GRAPH_STAR_TARGET_ARC_X,
      centerY: hubCenter.y + Math.sin(radians) * TYPE_GRAPH_STAR_TARGET_ARC_Y,
      node,
    };
  });
  const targetLayouts = spreadStarTargetLayouts(rawTargetLayouts, TYPE_GRAPH_STAR_TARGET_MIN_GAP);
  const targetHeaderIds = starTargetGroupHeaderIds(targetLayouts);

  return [
    hub,
    ...fieldLayouts.map((layout) => ({
      ...layout.node,
      showGroupLabel: false,
      x: layout.centerX - TYPE_GRAPH_FIELD_NODE_WIDTH / 2,
      y: layout.centerY - TYPE_GRAPH_FIELD_NODE_HEIGHT / 2,
    })),
    ...targetLayouts.map((layout) => ({
      ...layout.node,
      showGroupLabel: targetHeaderIds.has(layout.node.id),
      x: layout.centerX - TYPE_GRAPH_NODE_WIDTH / 2,
      y: layout.centerY - TYPE_GRAPH_NODE_HEIGHT / 2,
    })),
  ].sort((left, right) => {
    if (left.id === hub.id) {
      return -1;
    }
    if (right.id === hub.id) {
      return 1;
    }

    const leftAngle = fieldAngleById.get(left.id);
    const rightAngle = fieldAngleById.get(right.id);

    if (leftAngle != null && rightAngle != null) {
      return leftAngle - rightAngle;
    }

    if (left.layoutRole === right.layoutRole) {
      return left.y - right.y || left.x - right.x;
    }

    return left.layoutRole === "field" ? -1 : 1;
  });
}

export function starFanAngles(count: number) {
  if (count <= 0) {
    return [];
  }

  if (count === 1) {
    return [0];
  }

  const spread = count <= 4 ? 104 : count <= 7 ? 128 : 148;
  const step = spread / Math.max(1, count - 1);
  return Array.from({ length: count }, (_, index) => -spread / 2 + index * step);
}

export function starTargetGroupHeaderIds(
  layouts: Array<{ centerY: number; node: RenderNode }>,
) {
  const ids = new Set<string>();
  const seenGroups = new Set<string>();

  for (const layout of [...layouts].sort((left, right) => left.centerY - right.centerY)) {
    const group = layout.node.groupLabel;

    if (!group || seenGroups.has(group)) {
      continue;
    }

    ids.add(layout.node.id);
    seenGroups.add(group);
  }

  return ids;
}

export function spreadStarTargetLayouts<T extends { centerY: number; node: RenderNode }>(
  layouts: Array<T>,
  minGap: number,
) {
  if (layouts.length <= 1) {
    return layouts;
  }

  const sorted = [...layouts].sort((left, right) => left.centerY - right.centerY);
  const originalAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;

  for (let index = 1; index < sorted.length; index += 1) {
    const previous = sorted[index - 1]!;
    const current = sorted[index]!;
    const previousGroup = previous.node.groupLabel ?? "";
    const currentGroup = current.node.groupLabel ?? "";
    const requiredGap = minGap + (previousGroup !== currentGroup ? TYPE_GRAPH_STAR_TARGET_GROUP_GAP : 0);

    if (current.centerY < previous.centerY + requiredGap) {
      current.centerY = previous.centerY + requiredGap;
    }
  }

  const adjustedAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;
  const recenter = adjustedAverage - originalAverage;

  return sorted.map((layout) => ({
    ...layout,
    centerY: layout.centerY - recenter,
  }));
}

export function spreadStarLayouts<T extends { centerY: number }>(
  layouts: Array<T>,
  minGap: number,
) {
  if (layouts.length <= 1) {
    return layouts;
  }

  const sorted = [...layouts].sort((left, right) => left.centerY - right.centerY);
  const originalAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;

  for (let index = 1; index < sorted.length; index += 1) {
    const previous = sorted[index - 1]!;
    const current = sorted[index]!;
    if (current.centerY < previous.centerY + minGap) {
      current.centerY = previous.centerY + minGap;
    }
  }

  const adjustedAverage = sorted.reduce((total, layout) => total + layout.centerY, 0) / sorted.length;
  const recenter = adjustedAverage - originalAverage;

  return sorted.map((layout) => ({
    ...layout,
    centerY: layout.centerY - recenter,
  }));
}

export function degreesToRadians(degrees: number) {
  return degrees * (Math.PI / 180);
}

export function radiansToDegrees(radians: number) {
  return radians * (180 / Math.PI);
}

export function dependencyGroupOrder(groupLabel: string | null) {
  switch (groupLabel) {
    case "Local":
      return 0;
    case "Builtin":
      return 1;
    case "Sui Framework":
      return 2;
    case "External":
      return 3;
    case "Generic Containers":
      return 4;
    default:
      return 5;
  }
}

export function repositionRenderNode(
  node: RenderNode,
  index: number,
  maxRows: number,
  storageLayout: boolean,
): RenderNode {
  return {
    ...node,
    x: node.layoutRole === "function"
      ? TYPE_GRAPH_FUNCTION_COLUMN_X
      : node.layoutRole === "parent"
        ? storageLayout ? TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X : TYPE_GRAPH_LOCAL_COLUMN_X
        : node.layoutRole === "field"
          ? TYPE_GRAPH_STORAGE_FIELD_COLUMN_X
        : storageLayout ? TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X : TYPE_GRAPH_DEPENDENCY_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

export function normalizeEdge(
  edge: MoveTypeGraphEdge,
  nodeById: Map<string, MoveTypeGraphNode>,
  packagePath: string | null,
): NormalizedEdge | null {
  const sourceNode = nodeById.get(edge.source) ?? null;
  const targetNode = nodeById.get(edge.target) ?? null;
  const sourceIsFunction = sourceBelongsToPackage(edge.source, packagePath);
  const targetIsFunction = sourceBelongsToPackage(edge.target, packagePath);

  if (sourceIsFunction && targetNode && isRenderableTypeNode(targetNode)) {
    return {
      edge,
      source: edge.source,
      sourceKind: "function",
      sourceNode: null,
      target: edge.target,
      targetKind: "type",
      targetNode,
    };
  }

  if (sourceNode && targetNode && isRenderableTypeNode(sourceNode) && isRenderableTypeNode(targetNode)) {
    return {
      edge,
      source: edge.source,
      sourceKind: "type",
      sourceNode,
      target: edge.target,
      targetKind: "type",
      targetNode,
    };
  }

  if (sourceNode && isRenderableTypeNode(sourceNode) && targetIsFunction) {
    return {
      edge,
      source: edge.source,
      sourceKind: "type",
      sourceNode,
      target: edge.target,
      targetKind: "function",
      targetNode: null,
    };
  }

  return null;
}

export function edgeMatchesLens(
  normalized: NormalizedEdge,
  lens: TypeGraphLens,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const relationship = normalized.edge.relationship;
  const sourceOrTargetExternal = edgeHasExternalType(normalized, localIds, packagePath);
  const sourceOrTargetCapability = Boolean(
    (normalized.sourceNode && isCapabilityLike(normalized.sourceNode))
      || (normalized.targetNode && isCapabilityLike(normalized.targetNode)),
  );

  switch (lens) {
    case "storage":
      return STORAGE_RELATIONSHIPS.has(relationship) && normalized.sourceKind === "type" && normalized.targetKind === "type";
    case "functions":
      return (
        normalized.sourceKind === "function"
        || normalized.targetKind === "function"
        || FUNCTION_RELATIONSHIPS.has(relationship)
      );
    case "capabilities":
      return sourceOrTargetCapability || relationship === "parameter" || relationship === "field";
    case "generics":
      return GENERIC_RELATIONSHIPS.has(relationship);
    case "external":
      return sourceOrTargetExternal;
  }
}

export function isStorageFieldRelationship(relationship: string) {
  return relationship === "field" || relationship === "variantField" || relationship === "vectorElement";
}

export function fieldNodeId(declaringTypeId: string, fieldName: string, targetId: string) {
  return `field-node:${declaringTypeId}:${fieldName}:${targetId}`;
}

export function edgeTouchesFieldNode(edge: RenderEdge) {
  return edge.source.startsWith("field-node:") || edge.target.startsWith("field-node:");
}

export function fieldNodeEndpoint(edge: RenderEdge | null) {
  if (!edge) {
    return null;
  }

  if (edge.source.startsWith("field-node:")) {
    return edge.source;
  }

  if (edge.target.startsWith("field-node:")) {
    return edge.target;
  }

  return null;
}

export function edgeTouchesPackage(
  edge: NormalizedEdge,
  localIds: Set<string>,
  packagePath: string | null,
) {
  return (
    localIds.has(edge.source)
    || localIds.has(edge.target)
    || sourceBelongsToPackage(edge.source, packagePath)
    || sourceBelongsToPackage(edge.target, packagePath)
  );
}

export function typeRenderNode(
  node: MoveTypeGraphNode,
  kind: TypeNodeKind,
  index: number,
  maxRows: number,
  metrics: {
    entryFunctionCount: number;
    fieldCount: number;
    functionCount: number;
  } | undefined,
  genericInstance: GenericInstanceInfo | null,
  storageLayout: boolean,
  layoutRole: TypeGraphLayoutRole,
): RenderNode {
  const roleLabel = genericInstance ? "generic instance" : nodeRoleLabel(node, kind);
  const fieldCount = metrics?.fieldCount ?? 0;
  const functionCount = metrics?.functionCount ?? 0;
  const sourceLocation = node.span ?? (kind === "local" ? sourceLocationFromMoveNode(node) : null);
  return {
    addressLabel: shortAddress(node.canonicalAddress ?? node.address),
    abilitiesKnown: typeAbilitiesKnown(node, kind),
    evidenceEdgeId: null,
    fieldInfo: null,
    entryFunctionCount: metrics?.entryFunctionCount ?? 0,
    fieldCount,
    functionContext: null,
    functionCount,
    groupLabel: dependencyGroupLabel(kind, node),
    genericArguments: genericInstance?.arguments ?? genericParameterChips(node),
    id: node.id,
    isGenericInstance: Boolean(genericInstance),
    isSynthetic: false,
    kind,
    label: genericInstance?.label ?? typeDisplayName(node, kind),
    layoutRole,
    metricLabel: typeNodeMetricLabel(kind, roleLabel, fieldCount, functionCount, storageLayout, Boolean(genericInstance)),
    node,
    originLabel: nodeOriginLabel(node, kind),
    riskTags: nodeRiskTags(node),
    roleLabel,
    selectTypeId: node.id,
    showGroupLabel: false,
    subtitle: nodeSubtitle(node, kind),
    tags: nodeTags(node),
    sourceLocation,
    x: kind === "local"
      ? storageLayout ? TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X : TYPE_GRAPH_LOCAL_COLUMN_X
      : storageLayout ? TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X : TYPE_GRAPH_DEPENDENCY_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

export function syntheticFieldNode(
  edge: NormalizedEdge,
  index: number,
  maxRows: number,
  selectedTypeId: string,
  targetId: string,
  evidenceEdgeId: string,
  genericInstance: GenericInstanceInfo | null,
): RenderNode {
  const fieldName = edge.edge.fieldName ?? "field";
  const fieldType = resolvedFieldType(edge, genericInstance);
  const genericArguments = genericArgumentsFromTypeExpression(fieldType, edge.targetNode);
  const fieldTags = fieldSemanticTags(fieldName, fieldType, edge.targetNode);
  const sourceLocation = edgeSourceLocation(edge.edge);

  return {
    addressLabel: null,
    abilitiesKnown: true,
    evidenceEdgeId,
    fieldInfo: {
      baseType: fieldBaseType(edge),
      confidence: edge.edge.confidence || "syntactic",
      declaredIn: edge.sourceNode?.qualifiedName ?? compactEdgeEndpoint(selectedTypeId),
      declaringTypeId: edge.edge.declaringTypeId ?? selectedTypeId,
      fieldName,
      genericArguments,
      resolvedType: fieldType,
      sourceLocation,
      tags: fieldTags,
      targetTypeId: edge.target,
    },
    entryFunctionCount: 0,
    fieldCount: 0,
    functionContext: null,
    functionCount: 0,
    groupLabel: "Fields",
    genericArguments: [],
    id: fieldNodeId(selectedTypeId, fieldName, targetId),
    isGenericInstance: false,
    isSynthetic: true,
    kind: "field",
    label: fieldName,
    layoutRole: "field",
    metricLabel: compactEdgeEndpoint(targetId),
    node: null,
    originLabel: "field",
    riskTags: [],
    roleLabel: "field",
    selectTypeId: selectedTypeId,
    showGroupLabel: false,
    sourceLocation,
    subtitle: compactTypeLabel(fieldType),
    tags: fieldTags,
    x: TYPE_GRAPH_STORAGE_FIELD_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

export function syntheticGenericInstanceNode(
  instance: GenericInstanceInfo,
  baseNode: MoveTypeGraphNode | null,
  index: number,
  maxRows: number,
  packagePath: string | null,
  storageLayout: boolean,
): RenderNode {
  const kind = baseNode ? classifyTypeNode(baseNode, packagePath) : "framework";
  const roleLabel = "generic instance";
  const sourceLocation = baseNode?.span ?? (baseNode && kind === "local" ? sourceLocationFromMoveNode(baseNode) : null);

  return {
    addressLabel: shortAddress(baseNode?.canonicalAddress ?? baseNode?.address),
    abilitiesKnown: baseNode ? typeAbilitiesKnown(baseNode, kind) : false,
    evidenceEdgeId: null,
    fieldInfo: null,
    entryFunctionCount: 0,
    fieldCount: 0,
    functionContext: null,
    functionCount: 0,
    groupLabel: "Generic Containers",
    genericArguments: instance.arguments,
    id: instance.id,
    isGenericInstance: true,
    isSynthetic: true,
    kind,
    label: instance.label,
    layoutRole: "target",
    metricLabel: "generic instance",
    node: baseNode,
    originLabel: baseNode ? nodeOriginLabel(baseNode, kind) : "generic container",
    riskTags: baseNode ? nodeRiskTags(baseNode) : ["generic container"],
    roleLabel,
    selectTypeId: instance.baseTypeId,
    showGroupLabel: false,
    sourceLocation,
    subtitle: baseNode ? nodeSubtitle(baseNode, kind) : instance.declaringTypeId,
    tags: ["generic container", ...instance.arguments.slice(0, 1).map((argument) => `${argument.label}: ${compactTypeLabel(argument.value)}`)],
    x: storageLayout ? TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X : TYPE_GRAPH_DEPENDENCY_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

export function syntheticFunctionNode(
  id: string,
  index: number,
  maxRows: number,
  functionIndex: Map<string, FunctionContext>,
): RenderNode {
  const context = functionDetails(id, functionIndex);

  return {
    addressLabel: null,
    abilitiesKnown: true,
    evidenceEdgeId: null,
    fieldInfo: null,
    entryFunctionCount: context.isEntry ? 1 : 0,
    functionContext: context,
    fieldCount: 0,
    functionCount: 1,
    groupLabel: null,
    genericArguments: [],
    id,
    isGenericInstance: false,
    isSynthetic: false,
    kind: "function",
    label: context.label,
    layoutRole: "function",
    metricLabel: "function",
    node: null,
    originLabel: "function",
    riskTags: context.isEntry ? ["entry surface"] : [],
    roleLabel: context.isEntry ? "entry" : "function",
    selectTypeId: id,
    showGroupLabel: false,
    subtitle: context.moduleName,
    tags: [
      context.visibility,
      ...(context.isEntry ? ["entry"] : []),
      ...(context.isTransactionCallable ? ["tx"] : []),
    ],
    sourceLocation: null,
    x: TYPE_GRAPH_FUNCTION_COLUMN_X,
    y: columnY(index, maxRows, TYPE_GRAPH_ROW_GAP),
  };
}

export function columnY(index: number, maxRows: number, gap: number) {
  const rows = Math.max(maxRows, 1);
  return Math.max(0, (rows - 1) * 8) + index * Math.max(gap, TYPE_GRAPH_NODE_HEIGHT);
}

export function graphStats(edges: RenderEdge[]) {
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, number>();

  for (const edge of edges) {
    outgoing.set(edge.source, (outgoing.get(edge.source) ?? 0) + edge.count);
    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + edge.count);
  }

  return { incoming, outgoing };
}

export function collectEdgeEvidence(
  edges: NormalizedEdge[],
  localIds: Set<string>,
  packagePath: string | null,
) {
  const groups = new Map<string, RenderEdge>();

  for (const edge of edges) {
    if (!edgeTouchesPackage(edge, localIds, packagePath)) {
      continue;
    }

    const category = relationshipCategory(edge, localIds, packagePath);
    const key = edgeGroupKey(edge.source, edge.target, edge.edge, category);
    const current = groups.get(key);

    if (current) {
      current.count += 1;
    } else {
      groups.set(key, {
        category,
        count: 1,
        edge: edge.edge,
        id: key,
        routeCount: 1,
        routeIndex: 0,
        source: edge.source,
        target: edge.target,
      });
    }
  }

  return [...groups.values()].sort((left, right) => left.id.localeCompare(right.id));
}

export function selectedNeighborhoodIds(
  edges: RenderEdge[],
  selectedTypeId: string | null,
  activeEdge: RenderEdge | null,
) {
  const ids = new Set<string>();

  if (activeEdge) {
    const fieldId = fieldNodeEndpoint(activeEdge);

    if (fieldId) {
      for (const edge of edges) {
        if (edge.source === fieldId || edge.target === fieldId) {
          ids.add(edge.source);
          ids.add(edge.target);
        }
      }

      return ids;
    }

    ids.add(activeEdge.source);
    ids.add(activeEdge.target);
    return ids;
  }

  if (!selectedTypeId) {
    return ids;
  }

  ids.add(selectedTypeId);
  let frontier = new Set([selectedTypeId]);

  for (let depth = 0; depth < 2; depth += 1) {
    const next = new Set<string>();

    for (const edge of edges) {
      if (frontier.has(edge.source)) {
        next.add(edge.target);
      }
      if (frontier.has(edge.target)) {
        next.add(edge.source);
      }
    }

    next.forEach((id) => ids.add(id));
    frontier = next;
  }

  return ids;
}

export function edgeGroupKey(
  source: string,
  target: string,
  edge: MoveTypeGraphEdge,
  category: RelationshipCategory,
) {
  return [
    source,
    target,
    category,
    edge.relationship,
    edge.fieldName ?? "",
    edge.variantName ?? "",
    edge.functionName ?? "",
    edge.parameterName ?? "",
    edge.typeArgumentIndex ?? "",
    edge.declaringTypeId ?? "",
    edge.declaringFieldName ?? "",
    edge.typeArgumentName ?? "",
    edge.isMutable ? "mut" : "imm",
    edge.isReference ? "ref" : "value",
  ].join("|");
}

export function relationshipCategory(
  normalized: NormalizedEdge,
  localIds: Set<string>,
  packagePath: string | null,
): RelationshipCategory {
  const relationship = normalized.edge.relationship;
  const hasCapability =
    (normalized.sourceNode ? isCapabilityLike(normalized.sourceNode) : false)
    || (normalized.targetNode ? isCapabilityLike(normalized.targetNode) : false);

  if (hasCapability && (relationship === "field" || relationship === "parameter")) {
    return "capability";
  }

  if (relationship === "field" || relationship === "variantField") {
    return "field";
  }

  if (relationship === "parameter") {
    return "input";
  }

  if (relationship === "return") {
    return "return";
  }

  if (GENERIC_RELATIONSHIPS.has(relationship)) {
    return "generic";
  }

  if (relationship === "annotation" || relationship === "cast") {
    return "annotation";
  }

  if (relationship === "construction" || relationship === "destructuring") {
    return "mutation";
  }

  if (edgeHasExternalType(normalized, localIds, packagePath)) {
    return "external";
  }

  return "annotation";
}

export function relationshipColor(category: RelationshipCategory) {
  switch (category) {
    case "field":
      return "#4ade80";
    case "input":
      return "#60a5fa";
    case "return":
      return FUNCTION_COLOR;
    case "generic":
      return GENERIC_COLOR;
    case "capability":
      return "#f87171";
    case "mutation":
      return "#fb7185";
    case "external":
      return EXTERNAL_COLOR;
    case "annotation":
      return "#94a3b8";
  }
}

export function edgeLabel(edge: MoveTypeGraphEdge, count: number, category: RelationshipCategory) {
  const suffix = count > 1 ? ` x${count}` : "";

  if (edge.fieldName) {
    return `field(${edge.fieldName})${suffix}`;
  }

  if (edge.parameterName) {
    return `${relationshipLabel(edge.relationship)} (${edge.parameterName})${suffix}`;
  }

  if (edge.variantName) {
    return `${edge.variantName}${suffix}`;
  }

  if (edge.typeArgumentIndex != null) {
    return `generic(${edge.typeArgumentName ?? edge.typeArgumentIndex})${suffix}`;
  }

  if (category === "capability") {
    return `authorizes${suffix}`;
  }

  return `${relationshipLabel(edge.relationship)}${suffix}`;
}

export function fieldCauseEdgeLabelIds(
  edges: RenderEdge[],
  selectedTypeId: string | null,
  selectedEdgeId: string | null,
) {
  const ids = new Set<string>();

  if (selectedEdgeId) {
    ids.add(selectedEdgeId);
  }

  if (!selectedTypeId) {
    return ids;
  }

  const fieldEdges = edges
    .filter(
      (edge) =>
        edge.edge.fieldName
        && edge.source === selectedTypeId,
    )
    .sort((left, right) =>
      fieldCausePriority(right, selectedTypeId) - fieldCausePriority(left, selectedTypeId)
      || left.edge.fieldName!.localeCompare(right.edge.fieldName!)
      || left.id.localeCompare(right.id),
    );

  const labelsByTarget = new Map<string, number>();

  for (const edge of fieldEdges) {
    if (ids.size >= MAX_FIELD_CAUSE_LABELS) {
      break;
    }

    const shownForTarget = labelsByTarget.get(edge.target) ?? 0;
    const targetLimit = 1;

    if (shownForTarget >= targetLimit) {
      continue;
    }

    ids.add(edge.id);
    labelsByTarget.set(edge.target, shownForTarget + 1);
  }

  return ids;
}

export function prioritizedLabelEdgeIds(
  edges: RenderEdge[],
  selectedTypeId: string | null,
  limit: number,
) {
  const ids = new Set<string>();

  for (const edge of prioritizedEdges(edges, selectedTypeId).slice(0, limit)) {
    ids.add(edge.id);
  }

  return ids;
}

export function fieldCausePriority(edge: RenderEdge, selectedTypeId: string) {
  let score = edge.count;

  if (edge.source === selectedTypeId) {
    score += 10;
  }

  if (edge.edge.fieldName === "id" || edge.edge.fieldName === "uid") {
    score += 6;
  }

  if (edge.category === "field") {
    score += 3;
  }

  return score;
}

export function relationshipLabel(relationship: string) {
  switch (relationship) {
    case "annotation":
      return "annotation";
    case "callTypeArgument":
      return "type arg";
    case "construction":
      return "packs";
    case "destructuring":
      return "unpacks";
    case "genericArgument":
      return "generic";
    case "phantomTypeParameter":
      return "phantom";
    case "parameter":
      return "takes";
    case "return":
      return "returns";
    case "summaryUsage":
      return "summary";
    case "typeParameter":
      return "type param";
    case "variantField":
      return "variant";
    case "vectorElement":
      return "vector";
    default:
      return relationship;
  }
}

export function nodeColor(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "function") {
    return FUNCTION_COLOR;
  }

  if (kind === "field") {
    return relationshipColor("field");
  }

  if (isCapabilityLike(node)) {
    return CAPABILITY_COLOR;
  }

  if (node?.kind === "enum") {
    return GENERIC_COLOR;
  }

  if (isGenericLike(node)) {
    return GENERIC_COLOR;
  }

  if (kind === "builtin") {
    return BUILTIN_COLOR;
  }

  if (kind === "framework") {
    return FRAMEWORK_COLOR;
  }

  if (kind === "external") {
    return EXTERNAL_COLOR;
  }

  return LOCAL_COLOR;
}

export function nodeIcon(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "function") {
    return Workflow;
  }

  if (kind === "field") {
    return ListTree;
  }

  if (node?.kind === "enum") {
    return Hexagon;
  }

  if (isCapabilityLike(node)) {
    return ShieldAlert;
  }

  if (isResourceLike(node)) {
    return KeyRound;
  }

  if (kind === "builtin") {
    return Braces;
  }

  if (kind === "framework") {
    return Box;
  }

  if (kind === "external") {
    return FileCode2;
  }

  return Boxes;
}

export function nodeKindLabel(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "function") {
    return "function";
  }

  if (kind === "field") {
    return "field";
  }

  if (node?.kind === "enum") {
    return "enum";
  }

  if (kind === "builtin") {
    return "builtin";
  }

  if (kind === "framework") {
    return "framework";
  }

  if (kind === "external") {
    return "external";
  }

  return "type";
}

export function nodeRoleLabel(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (isCapabilityLike(node)) {
    return "capability";
  }

  if (isGenericContainer(node)) {
    return "generic container";
  }

  if (isEventLike(node)) {
    return "event";
  }

  if (isWitnessLike(node)) {
    return "witness";
  }

  if (isResourceLike(node)) {
    return "resource struct";
  }

  if (isGenericLike(node)) {
    return "generic";
  }

  return nodeKindLabel(kind, node);
}

export function nodeOriginLabel(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (kind === "local") {
    return "local";
  }

  if (kind === "builtin") {
    return "primitive";
  }

  if (kind === "framework") {
    const address = node.address?.toLowerCase() ?? node.canonicalAddress?.toLowerCase();
    return address === "0x1" || address?.endsWith("0001") ? "move stdlib" : "sui framework";
  }

  return node.packageName ? "direct dependency" : "external";
}

export function dependencyGroupLabel(kind: TypeNodeKind, node: MoveTypeGraphNode | null) {
  if (kind === "local") {
    return "Local";
  }

  if (kind === "builtin") {
    return "Builtin";
  }

  if (isGenericContainer(node)) {
    return "Generic Containers";
  }

  if (kind === "framework") {
    return "Sui Framework";
  }

  if (kind === "external") {
    return "External";
  }

  return null;
}

export function typeNodeMetricLabel(
  kind: TypeNodeKind,
  roleLabel: string,
  fieldCount: number,
  functionCount: number,
  storageLayout: boolean,
  isGenericInstance: boolean,
) {
  if (isGenericInstance) {
    return "generic instance";
  }

  if (storageLayout && kind !== "local") {
    if (kind === "builtin") {
      return "builtin";
    }

    if (kind === "framework" || kind === "external") {
      return "field target";
    }
  }

  if (fieldCount) {
    return pluralize(fieldCount, "field");
  }

  if (functionCount && !storageLayout) {
    return pluralize(functionCount, "fn");
  }

  return roleLabel;
}

export function nodeRiskTags(node: MoveTypeGraphNode) {
  const tags: string[] = [];

  if (isCapabilityLike(node)) {
    tags.push("capability");
    tags.push("privileged");
  }

  if (isResourceLike(node)) {
    tags.push("resource");
    tags.push("key object");
  }

  if (
    /(vault|pool|escrow|balance|treasury|account|position)/i.test(node.name)
    && !/(config|receipt|event|witness|cap)$/i.test(node.name)
  ) {
    tags.push("value-holding");
  }

  if (isGenericLike(node)) {
    tags.push("generic container");
  }

  if (isEventLike(node)) {
    tags.push("event");
  }

  if (isWitnessLike(node)) {
    tags.push("witness");
  }

  if (isExternalDependencyType(node)) {
    tags.push("external");
    tags.push("trust boundary");
    if (/(asset|price|oracle|feed|object)/i.test(node.name)) {
      tags.push(node.name.toLowerCase().includes("asset") ? "asset metadata" : "external data");
    }
  }

  return [...new Set(tags)];
}

export function nodeSubtitle(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (kind === "local") {
    return node.moduleName ? `${displayMovePackageName(node.packageName ?? "local")}::${node.moduleName}` : "local package";
  }

  if (kind === "builtin") {
    return "Move builtin";
  }

  if (kind === "framework") {
    return node.moduleName ? `sui::${node.moduleName}` : "Sui framework";
  }

  return node.qualifiedName;
}

export function nodeTags(node: MoveTypeGraphNode) {
  const riskTags = nodeRiskTags(node);
  const semanticTags = nodeSemanticTags(node);
  const abilities = effectiveTypeAbilities(node)
    .filter((ability) => !(ability === "key" && riskTags.includes("key object")));
  const tags = [...riskTags, ...semanticTags, ...abilities];

  if (isCapabilityLike(node) && !tags.includes("capability")) {
    tags.unshift("capability");
  } else if (isResourceLike(node) && !tags.includes("resource")) {
    tags.unshift("resource");
  }

  return [...new Set(tags)].slice(0, 3);
}

export function nodeSemanticTags(node: MoveTypeGraphNode) {
  const tags: string[] = [];
  const moduleName = node.moduleName?.toLowerCase() ?? "";
  const name = node.name.toLowerCase();

  if (isFrameworkType(node) && moduleName === "object" && name === "uid") {
    tags.push("framework");
    tags.push("object identity");
  } else if (isFrameworkType(node) && moduleName === "object" && name === "id") {
    tags.push("framework");
    tags.push("object reference");
  } else if (isFrameworkType(node) && node.kind !== "builtin") {
    tags.push("framework");
  }

  if (isGenericContainer(node) && !tags.includes("generic container")) {
    tags.push("generic container");
  }

  return tags;
}

export function typeAbilitiesKnown(node: MoveTypeGraphNode, kind: TypeNodeKind) {
  if (node.abilities.length > 0) {
    return true;
  }

  if (node.kind === "builtin") {
    return Boolean(BUILTIN_TYPE_ABILITIES[node.name]);
  }

  const moduleName = node.moduleName ?? node.qualifiedName.split("::").slice(-2, -1)[0];
  if (moduleName && SUI_FRAMEWORK_TYPE_ABILITIES[`${moduleName}::${node.name}`]) {
    return true;
  }

  return kind === "local";
}

export function abilitySummary(node: RenderNode) {
  const abilities = node.node ? effectiveTypeAbilities(node.node) : node.tags;

  if (abilities.length) {
    return abilities.join(", ");
  }

  return node.abilitiesKnown ? "no abilities" : "abilities unknown";
}

export function effectiveTypeAbilities(node: MoveTypeGraphNode) {
  if (node.abilities.length) {
    return node.abilities;
  }

  const builtinAbilities = BUILTIN_TYPE_ABILITIES[node.name];
  if (node.kind === "builtin" && builtinAbilities) {
    return [...builtinAbilities];
  }

  const moduleName = node.moduleName ?? node.qualifiedName.split("::").slice(-2, -1)[0];
  const typeName = node.name;
  const frameworkAbilities = moduleName
    ? SUI_FRAMEWORK_TYPE_ABILITIES[`${moduleName}::${typeName}`]
    : undefined;

  return frameworkAbilities ? [...frameworkAbilities] : [];
}

export function classifyTypeNode(node: MoveTypeGraphNode, packagePath: string | null): TypeNodeKind {
  if (isLocalPackageType(node, packagePath)) {
    return "local";
  }

  if (node.kind === "builtin") {
    return "builtin";
  }

  if (isFrameworkType(node)) {
    return "framework";
  }

  return "external";
}

export function isLocalPackageType(node: MoveTypeGraphNode, packagePath: string | null) {
  return packagePath !== null && node.packagePath === packagePath && isRenderableTypeNode(node);
}

export function isRenderableTypeNode(node: MoveTypeGraphNode) {
  return DISPLAY_TYPE_KINDS.has(node.kind);
}

export function isFrameworkType(node: MoveTypeGraphNode) {
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

export function isExternalDependencyType(node: MoveTypeGraphNode | null) {
  return Boolean(node && !isFrameworkType(node) && !isLocalPackageType(node, null) && node.isExternal);
}

export function isCapabilityLike(node: MoveTypeGraphNode | null) {
  if (!node) {
    return false;
  }

  const name = node.name.toLowerCase();
  return name.includes("cap") || name.includes("admin") || name.includes("authority");
}

export function isResourceLike(node: MoveTypeGraphNode | null) {
  return Boolean(node && effectiveTypeAbilities(node).includes("key"));
}

export function isEventLike(node: MoveTypeGraphNode | null) {
  return Boolean(node?.name.toLowerCase().includes("event"));
}

export function isWitnessLike(node: MoveTypeGraphNode | null) {
  const name = node?.name.toLowerCase() ?? "";
  return name.includes("witness") || name === "otw";
}

export function isGenericLike(node: MoveTypeGraphNode | null) {
  if (!node) {
    return false;
  }

  return node.qualifiedName.includes("<") || ["coin", "balance", "table", "vec_map", "vec_set"].includes(node.moduleName?.toLowerCase() ?? "");
}

export function isGenericContainer(node: MoveTypeGraphNode | null) {
  if (!node) {
    return false;
  }

  const moduleName = node.moduleName?.toLowerCase() ?? "";
  return (node.typeParameters?.length ?? 0) > 0 || ["table", "vector", "vec_map", "vec_set"].includes(moduleName);
}

export function genericParameterChips(node: MoveTypeGraphNode) {
  const parameters = node.typeParameters?.length
    ? node.typeParameters
    : knownGenericParameterNames(node).map((name) => ({ abilities: [], isPhantom: false, name }));

  return parameters.map((parameter, index) => ({
    label: parameter.name || String(index),
    value: parameter.isPhantom ? "phantom" : "type",
  }));
}

export function edgeHasExternalType(
  edge: NormalizedEdge,
  localIds: Set<string>,
  packagePath: string | null,
) {
  const sourceExternal =
    edge.sourceKind === "type"
    && edge.sourceNode
    && !localIds.has(edge.source)
    && !isLocalPackageType(edge.sourceNode, packagePath)
    && !isFrameworkType(edge.sourceNode);
  const targetExternal =
    edge.targetKind === "type"
    && edge.targetNode
    && !localIds.has(edge.target)
    && !isLocalPackageType(edge.targetNode, packagePath)
    && !isFrameworkType(edge.targetNode);

  return Boolean(sourceExternal || targetExternal);
}

export function sourceBelongsToPackage(sourceId: string, packagePath: string | null) {
  return packagePath !== null && sourceId.startsWith(`function:${packagePath}:`);
}

export function sortTypeNodes(nodes: MoveTypeGraphNode[], selectedTypeId: string | null) {
  return [...nodes].sort((left, right) => {
    if (left.id === selectedTypeId) {
      return -1;
    }
    if (right.id === selectedTypeId) {
      return 1;
    }

    return (
      (left.moduleName ?? "").localeCompare(right.moduleName ?? "")
      || left.name.localeCompare(right.name)
      || left.qualifiedName.localeCompare(right.qualifiedName)
    );
  });
}

export function typeDisplayName(node: MoveTypeGraphNode, kind?: TypeNodeKind) {
  const genericParameterNames = node.typeParameters?.length
    ? node.typeParameters.map((parameter) => parameter.name)
    : knownGenericParameterNames(node);

  if (kind === "local" || node.packagePath) {
    return genericParameterNames.length ? `${node.name}<${genericParameterNames.join(", ")}>` : node.name;
  }

  if (node.kind === "builtin") {
    return genericParameterNames.length ? `${node.name}<${genericParameterNames.join(", ")}>` : node.name;
  }

  if (node.moduleName) {
    const base = `${node.moduleName}::${node.name}`;
    return genericParameterNames.length
      ? `${base}<${genericParameterNames.join(", ")}>`
      : base;
  }

  return node.qualifiedName;
}

export function compactTypeLabel(value: string) {
  const normalized = value.replace(/\s+/g, " ").trim();
  const parts = normalized.split("::");
  return parts.length > 2 ? parts.slice(-2).join("::") : normalized;
}

export function resolvedFieldType(edge: NormalizedEdge, genericInstance: GenericInstanceInfo | null = null) {
  const expression = edge.edge.typeExpression?.trim();

  if (expression) {
    const simplified = simplifyMoveTypeExpression(expression, edge.sourceNode);

    if (genericInstance?.arguments.length && !simplified.includes("<")) {
      return `${simplified}<${genericInstance.arguments.map((argument) => simplifyMoveTypeExpression(argument.value, edge.sourceNode)).join(", ")}>`;
    }

    return simplified;
  }

  if (genericInstance?.arguments.length) {
    return genericInstance.label;
  }

  if (edge.targetNode) {
    return simplifyMoveTypeExpression(edge.targetNode.qualifiedName || edge.targetNode.name, edge.sourceNode);
  }

  return compactEdgeEndpoint(edge.target);
}

export function fieldBaseType(edge: NormalizedEdge) {
  if (edge.targetNode) {
    return compactEdgeEndpoint(edge.targetNode.id);
  }

  return compactEdgeEndpoint(edge.target);
}

export function genericArgumentsFromTypeExpression(
  expression: string,
  targetNode: MoveTypeGraphNode | null,
) {
  const parsed = parseGenericTypeExpression(expression);

  if (!parsed) {
    return [];
  }

  const labels = knownGenericParameterNames(targetNode);
  const fallbackLabels = labels.length
    ? labels
    : knownGenericParameterNamesFromTypeBase(parsed.base);
  return splitTopLevelTypeArguments(parsed.arguments).map((argument, index) => ({
    label: fallbackLabels[index] ?? String(index),
    value: simplifyMoveTypeExpression(argument, targetNode),
  }));
}

export function knownGenericParameterNamesFromTypeBase(base: string) {
  const normalized = base.toLowerCase();

  if (normalized.endsWith("table::table") || normalized === "table") {
    return ["K", "V"];
  }

  if (normalized.endsWith("vec_map::vec_map")) {
    return ["K", "V"];
  }

  if (
    normalized.endsWith("vec_set::vec_set")
    || normalized.endsWith("coin::coin")
    || normalized.endsWith("balance::balance")
    || normalized.endsWith("balance::supply")
    || normalized === "coin"
    || normalized === "balance"
    || normalized === "vector"
  ) {
    return ["T"];
  }

  return [];
}

export function fieldSemanticTags(
  fieldName: string,
  fieldType: string,
  targetNode: MoveTypeGraphNode | null,
) {
  const tags: string[] = [];
  const lowerName = fieldName.toLowerCase();
  const lowerType = fieldType.toLowerCase();

  if ((lowerName === "id" || lowerName === "uid") && lowerType.includes("uid")) {
    tags.push("key field");
  }

  if (lowerName.includes("config")) {
    tags.push("config field");
  }

  if (isExternalDependencyType(targetNode)) {
    tags.push("external field");
  }

  if (
    lowerType.includes("<")
    || lowerType.includes("table")
    || lowerType.includes("coin")
    || lowerType.includes("balance")
    || lowerType.includes("vector")
  ) {
    tags.push("container field");
  }

  if (/(asset|amount|balance|principal|total|saved|account|pool|margin|treasury|vault)/i.test(fieldName)) {
    tags.push("value field");
  }

  return [...new Set(tags)].slice(0, 2);
}

export function simplifyMoveTypeExpression(
  value: string,
  declaringNode: MoveTypeGraphNode | null = null,
): string {
  const normalized = value.replace(/\s+/g, " ").trim();
  const parsed = parseGenericTypeExpression(normalized);

  if (!parsed) {
    return simplifyMoveTypeBase(normalized, declaringNode);
  }

  const base = simplifyMoveTypeBase(parsed.base, declaringNode);
  const args = splitTopLevelTypeArguments(parsed.arguments)
    .map((argument) => simplifyMoveTypeExpression(argument, declaringNode))
    .join(", ");

  return `${base}<${args}>`;
}

export function simplifyMoveTypeBase(value: string, declaringNode: MoveTypeGraphNode | null) {
  const normalized = value.trim();
  const parts = normalized.split("::").filter(Boolean);

  if (parts.length < 2) {
    return normalized;
  }

  const name = parts[parts.length - 1] ?? normalized;
  const moduleName = parts[parts.length - 2] ?? "";
  const lowerModule = moduleName.toLowerCase();
  const lowerName = name.toLowerCase();
  const declaringModule = declaringNode?.moduleName?.toLowerCase();

  if (declaringModule && lowerModule === declaringModule) {
    return name;
  }

  if (
    (lowerModule === "table" && lowerName === "table")
    || (lowerModule === "coin" && lowerName === "coin")
    || (lowerModule === "balance" && (lowerName === "balance" || lowerName === "supply"))
    || lowerName === "vector"
  ) {
    return name;
  }

  return `${moduleName}::${name}`;
}

export function parseGenericTypeExpression(value: string) {
  const trimmed = value.trim();
  const start = trimmed.indexOf("<");

  if (start < 0 || !trimmed.endsWith(">")) {
    return null;
  }

  let depth = 0;
  for (let index = start; index < trimmed.length; index += 1) {
    const char = trimmed[index];
    if (char === "<") {
      depth += 1;
    } else if (char === ">") {
      depth -= 1;
      if (depth === 0 && index !== trimmed.length - 1) {
        return null;
      }
    }
  }

  if (depth !== 0) {
    return null;
  }

  return {
    arguments: trimmed.slice(start + 1, -1),
    base: trimmed.slice(0, start),
  };
}

export function splitTopLevelTypeArguments(value: string) {
  const args: string[] = [];
  let depth = 0;
  let start = 0;

  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if (char === "<") {
      depth += 1;
    } else if (char === ">") {
      depth -= 1;
    } else if (char === "," && depth === 0) {
      args.push(value.slice(start, index).trim());
      start = index + 1;
    }
  }

  const last = value.slice(start).trim();
  if (last) {
    args.push(last);
  }

  return args;
}

export function importantLocalTypeId(graph: MoveTypeGraph, movePackage: MovePackage | null) {
  const packagePath = movePackage?.path ?? null;
  const localNodes = graph.nodes.filter((node) => isLocalPackageType(node, packagePath));

  if (!localNodes.length) {
    return null;
  }

  const degree = new Map<string, number>();
  for (const edge of graph.edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
  }

  return [...localNodes].sort((left, right) =>
    importantTypeScore(right, degree) - importantTypeScore(left, degree)
    || left.name.localeCompare(right.name),
  )[0]?.id ?? null;
}

export function importantTypeScore(node: MoveTypeGraphNode, degree: Map<string, number>) {
  const name = node.name.toLowerCase();
  let score = degree.get(node.id) ?? 0;

  if (/(vault|pool|escrow|market|registry|admincap|cap|bucket|receipt)/.test(name)) {
    score += 100;
  }

  if (isResourceLike(node)) {
    score += 40;
  }

  if (isCapabilityLike(node)) {
    score += 35;
  }

  if (isGenericLike(node)) {
    score += 10;
  }

  return score;
}

export function buildFunctionIndex(movePackage: MovePackage | null) {
  const index = new Map<string, FunctionContext>();

  if (!movePackage) {
    return index;
  }

  for (const moveModule of movePackage.modules) {
    for (const moveFunction of moveModule.functions) {
      const id = functionId(movePackage.path, moveModule.address, moveModule.name, moveFunction.name);
      index.set(id, {
        functionName: moveFunction.name,
        id,
        isEntry: moveFunction.isEntry,
        isTransactionCallable: moveFunction.isTransactionCallable,
        label: `${moveModule.name}::${moveFunction.name}`,
        moduleName: moveModule.name,
        signature: moveFunction.signature,
        visibility: moveFunction.visibility,
      });
    }
  }

  return index;
}

export function functionId(
  packagePath: string,
  address: string | null,
  moduleName: string,
  functionName: string,
) {
  return `function:${packagePath}:${address ?? "_"}::${moduleName}::${functionName}`;
}

export function functionDetails(id: string, functionIndex: Map<string, FunctionContext>) {
  const indexed = functionIndex.get(id);

  if (indexed) {
    return indexed;
  }

  const match = id.match(/^function:(.*):([^:]*)::([^:]+)::([^:]+)$/);
  const moduleName = match?.[3] ?? "function";
  const functionName = match?.[4] ?? functionLabelFromId(id);

  return {
    functionName,
    id,
    isEntry: false,
    isTransactionCallable: false,
    label: `${moduleName}::${functionName}`,
    moduleName,
    signature: `${moduleName}::${functionName}(...)`,
    visibility: "function",
  };
}

export function functionLabel(id: string, functionIndex: Map<string, FunctionContext>) {
  return functionDetails(id, functionIndex).label;
}

export function functionLabelFromId(id: string) {
  const parts = id.split("::");
  return parts[parts.length - 1] ?? id;
}

export function pluralize(value: number, label: string) {
  return `${value} ${label}${value === 1 ? "" : "s"}`;
}
