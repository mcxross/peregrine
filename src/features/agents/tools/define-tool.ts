import type {
  DeterministicToolExecutionContext,
  DeterministicToolExecutionResult,
  DeterministicToolSpec,
  JsonSchemaDefinition,
} from "@peregrine/agent-runtime";
import type { AgentActionRequest } from "@peregrine/agent-runtime";

export function defineAgentTool<Input, Output>(spec: {
  id: string;
  title?: string;
  version?: string;
  description: string;
  inputSchema: JsonSchemaDefinition;
  outputSchema?: JsonSchemaDefinition;
  action: AgentActionRequest;
  examples?: Array<{ input: Input }>;
  execute: (
    input: Input,
    context: DeterministicToolExecutionContext,
  ) =>
    | Output
    | DeterministicToolExecutionResult<Output>
    | Promise<Output | DeterministicToolExecutionResult<Output>>;
}): DeterministicToolSpec {
  return {
    ...spec,
    execute: spec.execute as DeterministicToolSpec["execute"],
  };
}
