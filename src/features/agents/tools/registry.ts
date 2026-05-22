import { InMemoryToolRegistry } from "@peregrine/harness-control";

import { createAgentToolCatalog } from "@/features/agents/tools/catalog";
import type { AgentToolRuntimeState } from "@/features/agents/tools/types";

export function createAgentToolRegistry(state: AgentToolRuntimeState) {
  const registry = new InMemoryToolRegistry();

  for (const tool of createAgentToolCatalog(state)) {
    registry.register(tool);
  }

  return registry;
}

export function resolveAgentTools(
  state: AgentToolRuntimeState,
  activeToolIds?: string[],
) {
  const catalog = createAgentToolCatalog(state);

  if (!activeToolIds?.length) {
    return catalog;
  }

  const active = new Set(activeToolIds);

  return catalog.filter((tool) => active.has(tool.id));
}
