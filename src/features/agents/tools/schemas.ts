import type { JsonSchemaDefinition } from "@peregrine/agent-runtime";

export const projectPathProperties = {
  rootPath: {
    type: "string",
    description: "Workspace root path. Defaults to the active project root.",
  },
  packagePath: {
    type: "string",
    description: "Move package path relative to the workspace root. Defaults to the active package.",
  },
} as const satisfies Record<string, JsonSchemaDefinition>;

export const projectPathSchema: JsonSchemaDefinition = {
  type: "object",
  properties: projectPathProperties,
  additionalProperties: false,
};

export const functionTargetProperties = {
  ...projectPathProperties,
  functionId: {
    type: "string",
    description: "Indexed function id from rust.index.read_symbols or search results.",
  },
} as const satisfies Record<string, JsonSchemaDefinition>;

export const functionTargetSchema: JsonSchemaDefinition = {
  type: "object",
  properties: functionTargetProperties,
  required: ["functionId"],
  additionalProperties: false,
};

export const moduleTargetProperties = {
  ...projectPathProperties,
  moduleName: {
    type: "string",
    description: "Move module name.",
  },
  functionName: {
    type: "string",
    description: "Optional function name inside the module.",
  },
} as const satisfies Record<string, JsonSchemaDefinition>;

export const moduleTargetSchema: JsonSchemaDefinition = {
  type: "object",
  properties: moduleTargetProperties,
  required: ["moduleName"],
  additionalProperties: false,
};

export const symbolQueryProperties = {
  ...projectPathProperties,
  query: {
    type: "string",
    description: "Symbol search query.",
  },
} as const satisfies Record<string, JsonSchemaDefinition>;

export const symbolQuerySchema: JsonSchemaDefinition = {
  type: "object",
  properties: symbolQueryProperties,
  required: ["query"],
  additionalProperties: false,
};

export const contextLookupProperties = {
  targetId: {
    type: "string",
    description: "Indexed target id for a function, module, or type.",
  },
  level: {
    type: "string",
    enum: ["Level0", "Level1", "Level2", "Level3", "Level4"],
    description: "Context budget level.",
  },
} as const satisfies Record<string, JsonSchemaDefinition>;

export const contextLookupSchema: JsonSchemaDefinition = {
  type: "object",
  properties: contextLookupProperties,
  required: ["targetId"],
  additionalProperties: false,
};

export const findingInputProperties = {
  title: { type: "string" },
  severity: {
    type: "string",
    enum: ["critical", "high", "medium", "low", "info"],
  },
  message: { type: "string" },
  location: { type: "string" },
} as const satisfies Record<string, JsonSchemaDefinition>;

export const findingInputSchema: JsonSchemaDefinition = {
  type: "object",
  properties: findingInputProperties,
  required: ["title", "severity", "message"],
  additionalProperties: false,
};

export const findingAttachmentSchema: JsonSchemaDefinition = {
  type: "object",
  properties: {
    findingId: { type: "string" },
    payload: {
      type: "object",
      description: "Structured evidence payload to attach to the finding.",
    },
  },
  required: ["findingId", "payload"],
  additionalProperties: false,
};
