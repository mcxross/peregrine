import { ToolLoopAgent, stepCountIs } from "ai";

import { buildAgentInstructions, buildAgentPrompt } from "./instructions";
import { createAiSdkToolSet, filterToolsById } from "./tools";
import type {
  AgentGenerateRequest,
  AgentGenerateResult,
  AgentRuntimeConfig,
  AgentStreamRequest,
  AgentStreamResult,
} from "./types";

export class PeregrineAgentRuntime {
  private readonly config: AgentRuntimeConfig;

  constructor(config: AgentRuntimeConfig) {
    this.config = config;
  }

  createAgent(request: AgentGenerateRequest | AgentStreamRequest) {
    const toolSpecs = filterToolsById(this.config.tools, request.activeToolIds);
    const toolSet = createAiSdkToolSet({
      specs: toolSpecs,
      gateway: this.config.toolGateway,
      context: {
        sessionId: request.sessionId,
        taskId: request.packet.task.id,
        metadata: request.metadata,
      },
    });

    return {
      agent: new ToolLoopAgent({
        model: this.config.model,
        instructions: buildAgentInstructions(request.packet),
        tools: toolSet.tools,
        toolChoice: request.toolChoice,
        stopWhen:
          request.stopWhen ??
          stepCountIs(request.maxSteps ?? this.config.maxSteps ?? 12),
      }),
      toolSet,
    };
  }

  async generate(request: AgentGenerateRequest): Promise<AgentGenerateResult> {
    const { agent } = this.createAgent(request);
    const callOptions = {
      abortSignal: request.abortSignal,
      timeout: request.timeout,
    };
    const result = request.messages
      ? await agent.generate({
          ...callOptions,
          messages: request.messages,
        })
      : await agent.generate({
          ...callOptions,
          prompt: buildAgentPrompt(request.packet, request.prompt),
        });

    return {
      packet: request.packet,
      text: result.text,
      result,
    };
  }

  async stream(request: AgentStreamRequest): Promise<AgentStreamResult> {
    const { agent } = this.createAgent(request);
    const callOptions = {
      abortSignal: request.abortSignal,
      timeout: request.timeout,
    };
    const result = request.messages
      ? await agent.stream({
          ...callOptions,
          messages: request.messages,
        })
      : await agent.stream({
          ...callOptions,
          prompt: buildAgentPrompt(request.packet, request.prompt),
        });

    return {
      packet: request.packet,
      result,
    };
  }
}
