import type {
  MoveSourceSpan,
  MoveTypeGraphEdge,
  MoveTypeGraphNode,
} from "@/features/empty-project/filesystem-tree";

export type TypeGraphSourceLocation = {
  filePath: string;
  line: number;
};

export type TypeGraphLens = "storage" | "functions" | "capabilities" | "generics" | "external";
export type TypeGraphScope = "oneHop" | "twoHop" | "module" | "package" | "custom";
export type TypeNodeKind = "builtin" | "external" | "field" | "framework" | "function" | "local";
export type TypeGraphLayoutRole = "field" | "function" | "parent" | "target";
export type RelationshipCategory =
  | "annotation"
  | "capability"
  | "external"
  | "field"
  | "generic"
  | "input"
  | "mutation"
  | "return";

export type FunctionContext = {
  functionName: string;
  id: string;
  isEntry: boolean;
  isTransactionCallable: boolean;
  label: string;
  moduleName: string;
  signature: string;
  visibility: string;
};

export type NormalizedEdge = {
  edge: MoveTypeGraphEdge;
  source: string;
  sourceKind: "function" | "type";
  sourceNode: MoveTypeGraphNode | null;
  target: string;
  targetKind: "function" | "type";
  targetNode: MoveTypeGraphNode | null;
};

export type RenderNode = {
  addressLabel: string | null;
  abilitiesKnown: boolean;
  evidenceEdgeId: string | null;
  fieldInfo: FieldNodeInfo | null;
  entryFunctionCount: number;
  fieldCount: number;
  functionContext: FunctionContext | null;
  functionCount: number;
  groupLabel: string | null;
  genericArguments: Array<{ label: string; value: string }>;
  id: string;
  isGenericInstance: boolean;
  isSynthetic: boolean;
  kind: TypeNodeKind;
  label: string;
  layoutRole: TypeGraphLayoutRole;
  metricLabel: string;
  node: MoveTypeGraphNode | null;
  originLabel: string;
  riskTags: string[];
  roleLabel: string;
  selectTypeId: string;
  showGroupLabel: boolean;
  subtitle: string;
  tags: string[];
  sourceLocation: MoveSourceSpan | null;
  x: number;
  y: number;
};

export type FieldNodeInfo = {
  baseType: string;
  confidence: string;
  declaredIn: string;
  declaringTypeId: string;
  fieldName: string;
  genericArguments: Array<{ label: string; value: string }>;
  resolvedType: string;
  sourceLocation: MoveSourceSpan | null;
  tags: string[];
  targetTypeId: string;
};

export type RenderEdge = {
  category: RelationshipCategory;
  count: number;
  edge: MoveTypeGraphEdge;
  id: string;
  routeIndex: number;
  routeCount: number;
  source: string;
  target: string;
};

export type GenericInstanceInfo = {
  arguments: Array<{ label: string; value: string }>;
  baseTypeId: string;
  declaringFieldName: string;
  declaringTypeId: string;
  id: string;
  label: string;
  sourceLocation: MoveSourceSpan | null;
};

export type TypeRenderGraph = {
  capabilityCount: number;
  edges: RenderEdge[];
  edgeEvidence: RenderEdge[];
  externalCount: number;
  frameworkCount: number;
  functionCount: number;
  genericEdgeCount: number;
  hiddenEdgeCount: number;
  hiddenNodeCount: number;
  localCount: number;
  nodes: RenderNode[];
  rawEdgeCount: number;
  resourceCount: number;
  selectedLabel: string | null;
  selectedNode: RenderNode | null;
};

export type TypeFlowNodeData = RenderNode & {
  color: string;
  dimmed: boolean;
  focused: boolean;
  incoming: number;
  onCollapseNeighbors: (typeId: string) => void;
  onExpandNode: (typeId: string) => void;
  onOpenSource: (span: MoveSourceSpan | null) => void;
  onSelectEvidenceEdge: (edgeId: string) => void;
  onShowFields: (typeId: string) => void;
  onShowFunctions: (typeId: string) => void;
  onSelectType: (typeId: string) => void;
  outgoing: number;
  selected: boolean;
};

export type TypeFlowEdgeData = {
  active: boolean;
  category: RelationshipCategory;
  color: string;
  count: number;
  dimmed: boolean;
  label: string | null;
  routeCount: number;
  routeIndex: number;
};

export const TYPE_GRAPH_LENSES: Array<{
  caption: string;
  id: TypeGraphLens;
  label: string;
}> = [
  { caption: "Who owns what", id: "storage", label: "Storage Shape" },
  { caption: "Inputs & outputs", id: "functions", label: "Function Surface" },
  { caption: "Authority & permissions", id: "capabilities", label: "Capability View" },
  { caption: "Concrete forms", id: "generics", label: "Generic Instantiations" },
  { caption: "Imports & dependencies", id: "external", label: "External Types" },
];
export const TYPE_GRAPH_SCOPES: Array<{
  id: TypeGraphScope;
  label: string;
  shortLabel: string;
}> = [
  { id: "oneHop", label: "1-hop", shortLabel: "1" },
  { id: "twoHop", label: "2-hop", shortLabel: "2" },
  { id: "module", label: "Module", shortLabel: "Mod" },
  { id: "package", label: "Package", shortLabel: "Pkg" },
  { id: "custom", label: "Query", shortLabel: "Q" },
];

export const LOCAL_COLOR = "#38bdf8";
export const SELECTED_COLOR = "#38bdf8";
export const FRAMEWORK_COLOR = "#34d399";
export const EXTERNAL_COLOR = "#a78bfa";
export const BUILTIN_COLOR = "#94a3b8";
export const FUNCTION_COLOR = "#60a5fa";
export const GENERIC_COLOR = "#eab308";
export const CAPABILITY_COLOR = "#f87171";
export const DISPLAY_TYPE_KINDS = new Set(["struct", "enum", "datatype", "summaryType", "builtin"]);
export const FRAMEWORK_ADDRESSES = new Set([
  "std",
  "sui",
  "0x1",
  "0x2",
  "0x0000000000000000000000000000000000000000000000000000000000000001",
  "0x0000000000000000000000000000000000000000000000000000000000000002",
]);
export const FRAMEWORK_MODULES = new Set([
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
export const STORAGE_RELATIONSHIPS = new Set([
  "field",
  "genericArgument",
  "variantField",
  "vectorElement",
]);
export const FUNCTION_RELATIONSHIPS = new Set([
  "annotation",
  "callTypeArgument",
  "cast",
  "parameter",
  "return",
]);
export const GENERIC_RELATIONSHIPS = new Set([
  "callTypeArgument",
  "genericArgument",
  "phantomTypeParameter",
  "typeParameter",
  "vectorElement",
]);
export const MAX_OVERVIEW_NODES = 72;
export const MAX_FOCUSED_NODES = 110;
export const MAX_RENDER_EDGES = 180;
export const MAX_EDGE_LABELS = 10;
export const MAX_FIELD_CAUSE_LABELS = 5;
export const EDGE_LABEL_ZOOM_THRESHOLD = 1.15;
export const TYPE_GRAPH_NODE_WIDTH = 256;
export const TYPE_GRAPH_NODE_HEIGHT = 132;
export const TYPE_GRAPH_FIELD_NODE_WIDTH = 208;
export const TYPE_GRAPH_FIELD_NODE_HEIGHT = 78;
export const TYPE_GRAPH_COLUMN_GAP = 192;
export const TYPE_GRAPH_ROW_GAP = 152;
export const TYPE_GRAPH_FUNCTION_COLUMN_X = 0;
export const TYPE_GRAPH_LOCAL_COLUMN_X = TYPE_GRAPH_NODE_WIDTH + TYPE_GRAPH_COLUMN_GAP;
export const TYPE_GRAPH_DEPENDENCY_COLUMN_X = TYPE_GRAPH_LOCAL_COLUMN_X + TYPE_GRAPH_NODE_WIDTH + TYPE_GRAPH_COLUMN_GAP;
export const TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X = 112;
export const TYPE_GRAPH_STORAGE_FIELD_COLUMN_X = TYPE_GRAPH_STORAGE_LOCAL_COLUMN_X + TYPE_GRAPH_NODE_WIDTH + 170;
export const TYPE_GRAPH_STORAGE_DEPENDENCY_COLUMN_X = TYPE_GRAPH_STORAGE_FIELD_COLUMN_X + TYPE_GRAPH_NODE_WIDTH + 180;
export const TYPE_GRAPH_STAR_FIELD_OFFSET_X = 350;
export const TYPE_GRAPH_STAR_FIELD_ARC_X = 76;
export const TYPE_GRAPH_STAR_FIELD_ARC_Y = 245;
export const TYPE_GRAPH_STAR_TARGET_OFFSET_X = 670;
export const TYPE_GRAPH_STAR_TARGET_ARC_X = 70;
export const TYPE_GRAPH_STAR_TARGET_ARC_Y = 345;
export const TYPE_GRAPH_STAR_FIELD_MIN_GAP = 94;
export const TYPE_GRAPH_STAR_TARGET_MIN_GAP = 166;
export const TYPE_GRAPH_STAR_TARGET_GROUP_GAP = 26;
export const COPY_DROP_STORE_ABILITIES = ["copy", "drop", "store"] as const;
export const BUILTIN_TYPE_ABILITIES: Record<string, readonly string[]> = {
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
export const SUI_FRAMEWORK_TYPE_ABILITIES: Record<string, readonly string[]> = {
  "object::ID": COPY_DROP_STORE_ABILITIES,
  "object::UID": ["store"],
  "table::Table": ["key", "store"],
};
