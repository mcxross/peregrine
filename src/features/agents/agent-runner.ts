import {
  ToolLoopAgent,
  stepCountIs,
  type ToolLoopAgentOnFinishCallback,
  type ToolLoopAgentOnStepFinishCallback,
} from "ai";

import { providerById } from "@/features/agents/model-providers/provider-adapters";
import type {
  AgentDefinition,
  AgentWorkflow,
  AgentWorkflowNode,
} from "@/features/agents/types";

export type AgentRunResult = {
  text: string;
};

export type AgentRunTraceEvent = {
  level: "info" | "warning" | "error" | "trace";
  message: string;
};

export async function runAgentWorkflowWithModel({
  agent,
  onTrace,
  signal,
  workflow,
}: {
  agent: AgentDefinition;
  onTrace?: (event: AgentRunTraceEvent) => void;
  signal?: AbortSignal;
  workflow: AgentWorkflow;
}): Promise<AgentRunResult> {
  const provider = providerById(agent.provider.providerId);
  const model = await provider.resolveLanguageModel(agent.provider);
  const instructions = buildAgentInstructions(agent, workflow);
  const prompt = buildAgentPrompt(agent, workflow);
  const onStepFinish: ToolLoopAgentOnStepFinishCallback = (step) => {
    onTrace?.({
      level: "trace",
      message: [
        `Model step ${step.stepNumber + 1} finished.`,
        `finish=${step.finishReason}`,
        `model=${step.model.provider}/${step.model.modelId}`,
        formatUsage(step.usage),
        step.toolCalls.length ? `toolCalls=${step.toolCalls.length}` : "toolCalls=0",
      ].join(" "),
    });

    if (step.reasoningText) {
      onTrace?.({
        level: "trace",
        message: `Visible model reasoning output: ${formatTraceText(step.reasoningText, 900)}`,
      });
    }

    if (step.text.trim()) {
      onTrace?.({
        level: "trace",
        message: `Visible model output chunk: ${formatTraceText(step.text, 900)}`,
      });
    }

    for (const warning of step.warnings ?? []) {
      onTrace?.({
        level: "warning",
        message: `Provider warning: ${JSON.stringify(warning)}`,
      });
    }
  };
  const onFinish: ToolLoopAgentOnFinishCallback = (event) => {
    onTrace?.({
      level: "info",
      message: `Model finished after ${event.steps.length} step(s). ${formatUsage(event.totalUsage)}`,
    });
  };
  const runner = new ToolLoopAgent({
    model,
    instructions,
    onFinish,
    onStepFinish,
    stopWhen: stepCountIs(Math.max(1, agent.execution.maxSteps)),
    toolChoice: "none",
  });

  onTrace?.({
    level: "trace",
    message: `Prepared model call for ${agent.provider.providerId}/${agent.provider.modelId}. Endpoint: ${agent.provider.endpoint ?? "provider default"}.`,
  });
  onTrace?.({
    level: "trace",
    message: `System prompt sent: ${formatTraceText(instructions, 1_200)}`,
  });
  onTrace?.({
    level: "trace",
    message: `User prompt sent: ${formatTraceText(prompt, 1_200)}`,
  });

  const result = await runner.generate({
    abortSignal: signal,
    prompt,
    timeout: {
      totalMs: 120_000,
    },
  });

  return {
    text: result.text.trim(),
  };
}

function buildAgentInstructions(agent: AgentDefinition, workflow: AgentWorkflow) {
  return [
    agent.systemPrompt,
    "",
    "You are running inside Peregrine's Agents workspace.",
    "Use the workflow graph as the task boundary.",
    "Do not claim that deterministic repository tools ran unless the execution trace explicitly includes their output.",
    "Do not reveal hidden chain-of-thought. Provide concise rationale, observable decisions, evidence needs, and uncertainty.",
    "Return a report with these sections: Run Summary, Reasoning Trace Summary, Findings or Output, Evidence Needed, Next Actions.",
    "",
    `Agent: ${agent.name}`,
    `Agent description: ${agent.description}`,
    `Workflow: ${workflow.name}`,
    `Workflow description: ${workflow.description}`,
  ].join("\n");
}

function buildAgentPrompt(agent: AgentDefinition, workflow: AgentWorkflow) {
  const nodes = workflow.nodes.map(formatNode).join("\n");
  const edges = workflow.edges.length
    ? workflow.edges
      .map((edge) => `- ${edge.source} -> ${edge.target}`)
      .join("\n")
    : "- none";

  return [
    `Run the "${agent.name}" workflow and produce the agent's response.`,
    "",
    "Workflow nodes:",
    nodes || "- none",
    "",
    "Workflow edges:",
    edges,
    "",
    "Available tool identifiers configured for this agent:",
    agent.tools.length ? agent.tools.map((tool) => `- ${tool}`).join("\n") : "- none",
    "",
    "Because this workspace run does not yet expose deterministic tool outputs to the model, focus on analysis planning, requested checks, and expected evidence.",
  ].join("\n");
}

function formatNode(node: AgentWorkflowNode) {
  return [
    `- ${node.id}`,
    `  type: ${node.data.nodeType}`,
    `  label: ${node.data.label}`,
    `  description: ${node.data.description}`,
    node.data.toolId ? `  tool: ${node.data.toolId}` : null,
    node.data.provider
      ? `  provider: ${node.data.provider.providerId}/${node.data.provider.modelId}`
      : null,
  ]
    .filter(Boolean)
    .join("\n");
}

function formatUsage(usage: {
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
}) {
  const input = usage.inputTokens ?? 0;
  const output = usage.outputTokens ?? 0;
  const total = usage.totalTokens ?? input + output;

  return `tokens=input:${input} output:${output} total:${total}`;
}

function formatTraceText(text: string, maxLength: number) {
  const compact = text.replace(/\s+/g, " ").trim();

  if (compact.length <= maxLength) {
    return compact;
  }

  return `${compact.slice(0, maxLength)}...`;
}
