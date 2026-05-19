import type { DeterministicToolSpec } from "@peregrine/agent-runtime";
import type { ToolRegistry } from "./types";

export class InMemoryToolRegistry implements ToolRegistry {
  private readonly tools = new Map<string, DeterministicToolSpec>();

  register(tool: DeterministicToolSpec) {
    this.tools.set(tool.id, tool);
  }

  get(toolId: string) {
    return this.tools.get(toolId);
  }

  list() {
    return Array.from(this.tools.values());
  }
}

