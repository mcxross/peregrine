import type { LanguageModel } from "ai";

import type {
  AgentProviderConfig,
  ModelProviderDescriptor,
} from "@/features/agents/types";

export type ModelProviderAdapter = ModelProviderDescriptor & {
  resolveLanguageModel: (config: AgentProviderConfig) => LanguageModel | Promise<LanguageModel>;
};

const OLLAMA_DEFAULT_ENDPOINT = "http://127.0.0.1:11434";

export const modelProviderAdapters: ModelProviderAdapter[] = [
  {
    id: "ai-gateway",
    label: "AI Gateway",
    scope: "cloud",
    defaultModelId: "openai/gpt-5.2",
    modelIds: [
      "openai/gpt-5.2",
      "anthropic/claude-sonnet-4-5",
      "google/gemini-3-pro",
    ],
    supportsTools: true,
    supportsLocalModels: false,
    resolveLanguageModel: (config) => config.modelId,
  },
  {
    id: "openai",
    label: "OpenAI",
    scope: "cloud",
    defaultModelId: "openai/gpt-5.2",
    modelIds: ["openai/gpt-5.2", "openai/gpt-5.1", "openai/gpt-4.1"],
    supportsTools: true,
    supportsLocalModels: false,
    resolveLanguageModel: (config) => config.modelId,
  },
  {
    id: "anthropic",
    label: "Anthropic",
    scope: "cloud",
    defaultModelId: "anthropic/claude-sonnet-4-5",
    modelIds: [
      "anthropic/claude-sonnet-4-5",
      "anthropic/claude-opus-4-5",
      "anthropic/claude-haiku-4-5",
    ],
    supportsTools: true,
    supportsLocalModels: false,
    resolveLanguageModel: (config) => config.modelId,
  },
  {
    id: "google",
    label: "Google",
    scope: "cloud",
    defaultModelId: "google/gemini-3-pro",
    modelIds: ["google/gemini-3-pro", "google/gemini-2.5-pro"],
    supportsTools: true,
    supportsLocalModels: false,
    resolveLanguageModel: (config) => config.modelId,
  },
  {
    id: "ollama",
    label: "Ollama",
    scope: "local",
    defaultEndpoint: OLLAMA_DEFAULT_ENDPOINT,
    defaultModelId: "llama3.2",
    modelIds: ["llama3.2", "qwen2.5-coder", "mistral", "deepseek-r1"],
    supportsTools: true,
    supportsLocalModels: true,
    resolveLanguageModel: async (config) => {
      const { createOllama } = await import("ai-sdk-ollama/browser");
      const provider = createOllama({
        baseURL: config.endpoint || OLLAMA_DEFAULT_ENDPOINT,
      });

      return provider(config.modelId);
    },
  },
];

export function providerById(providerId: string) {
  return (
    modelProviderAdapters.find((provider) => provider.id === providerId)
    ?? modelProviderAdapters[0]
  );
}

export function providerModelOptions(config: AgentProviderConfig) {
  const provider = providerById(config.providerId);
  const dynamicModel = config.modelId && !provider.modelIds.includes(config.modelId)
    ? [config.modelId]
    : [];

  return [...dynamicModel, ...provider.modelIds];
}
