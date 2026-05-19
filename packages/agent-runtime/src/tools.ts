import { jsonSchema, tool } from "ai";
import type { Tool } from "ai";

import type {
  AiSdkToolSet,
  DeterministicToolSpec,
  JsonRecord,
  JsonSchemaDefinition,
  ToolGateway,
} from "./types";

type AiJsonSchemaInput = Parameters<typeof jsonSchema>[0];

export function createAiSdkToolSet({
  specs,
  gateway,
  context,
}: {
  specs: DeterministicToolSpec[];
  gateway: ToolGateway;
  context: {
    sessionId?: string;
    taskId: string;
    metadata?: JsonRecord;
  };
}): AiSdkToolSet {
  const usedNames = new Set<string>();
  const tools: Record<string, Tool> = {};
  const toolNamesById = new Map<string, string>();
  const toolIdsByName = new Map<string, string>();

  for (const spec of specs) {
    const name = uniqueToolName(spec.id, usedNames);
    usedNames.add(name);
    toolNamesById.set(spec.id, name);
    toolIdsByName.set(name, spec.id);

    tools[name] = tool({
      title: spec.title,
      description: spec.description,
      inputSchema: jsonSchema(spec.inputSchema as AiJsonSchemaInput),
      inputExamples: spec.examples,
      metadata: {
        peregrineToolId: spec.id,
        version: spec.version,
        actionClass: spec.action.actionClass,
        risk: spec.action.risk,
      },
      execute: async (input, options) =>
        gateway.runTool({
          tool: spec,
          input,
          toolCallId: options.toolCallId,
          context: {
            sessionId: context.sessionId,
            taskId: context.taskId,
            abortSignal: options.abortSignal,
            messages: options.messages,
            metadata: context.metadata,
          },
        }),
    });
  }

  return {
    tools,
    toolNamesById,
    toolIdsByName,
  };
}

export function filterToolsById(
  specs: DeterministicToolSpec[],
  activeToolIds?: string[],
) {
  if (!activeToolIds?.length) {
    return specs;
  }

  const active = new Set(activeToolIds);

  return specs.filter((spec) => active.has(spec.id));
}

export function createAiSdkToolName(toolId: string) {
  const normalized = toolId.replace(/[^A-Za-z0-9_]/g, "_");
  const withPrefix = /^[A-Za-z_]/.test(normalized)
    ? normalized
    : `tool_${normalized}`;

  return trimToolName(withPrefix || "tool");
}

function uniqueToolName(toolId: string, usedNames: ReadonlySet<string>) {
  const base = createAiSdkToolName(toolId);

  if (!usedNames.has(base)) {
    return base;
  }

  for (let suffix = 2; suffix < 10_000; suffix += 1) {
    const candidate = trimToolName(`${base}_${suffix}`);

    if (!usedNames.has(candidate)) {
      return candidate;
    }
  }

  throw new Error(`Unable to create a unique AI SDK tool name for ${toolId}.`);
}

function trimToolName(name: string) {
  return name.slice(0, 64).replace(/_+$/g, "") || "tool";
}

export function assertJsonSchemaDefinition(
  schema: JsonSchemaDefinition,
): JsonSchemaDefinition {
  if (!schema || typeof schema !== "object") {
    throw new Error("Tool input schema must be a JSON schema object.");
  }

  return schema;
}
