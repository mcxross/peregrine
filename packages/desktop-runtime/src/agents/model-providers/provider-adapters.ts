import type { LanguageModel } from "ai";

import type {
  AgentProviderConfig,
  ModelProviderDescriptor,
} from "../types";

export type ModelProviderAdapter = ModelProviderDescriptor & {
  listModelIds?: (config: AgentProviderConfig) => Promise<ModelListResult>;
  resolveLanguageModel: (config: AgentProviderConfig) => LanguageModel | Promise<LanguageModel>;
};

export type ModelListResult = {
  error?: string;
  modelIds: string[];
  source: string;
};

type OllamaTagsResponse = {
  models?: Array<{
    model?: string;
    name?: string;
  }>;
};

type GatewayModelsResponse = {
  data?: Array<{
    id?: string;
  }>;
};

const AI_GATEWAY_MODELS_ENDPOINT = "https://ai-gateway.vercel.sh/v1/models";
const OLLAMA_DEFAULT_ENDPOINT = "http://127.0.0.1:11434";

export const modelProviderAdapters: ModelProviderAdapter[] = [
  {
    id: "ollama",
    label: "Ollama",
    scope: "local",
    defaultEndpoint: OLLAMA_DEFAULT_ENDPOINT,
    supportsTools: true,
    supportsLocalModels: true,
    listModelIds: async (config) => {
      const endpoint = normalizeEndpoint(config.endpoint || OLLAMA_DEFAULT_ENDPOINT);
      const tagsUrl = `${endpoint}/api/tags`;

      try {
        const response = await fetch(tagsUrl);

        if (!response.ok) {
          throw new Error(`Ollama returned HTTP ${response.status}`);
        }

        const payload = await response.json() as OllamaTagsResponse;
        const modelIds = Array.from(
          new Set(
            (payload.models ?? [])
              .map((model) => model.model || model.name)
              .filter((modelId): modelId is string => Boolean(modelId)),
          ),
        );

        return {
          modelIds,
          source: tagsUrl,
        };
      } catch (error) {
        return {
          error: error instanceof Error ? error.message : String(error),
          modelIds: [],
          source: tagsUrl,
        };
      }
    },
    resolveLanguageModel: async (config) => {
      const { createOllama } = await import("ai-sdk-ollama/browser");
      const provider = createOllama({
        baseURL: normalizeEndpoint(config.endpoint || OLLAMA_DEFAULT_ENDPOINT),
      });

      return provider(config.modelId);
    },
  },
  {
    id: "ai-gateway",
    label: "AI Gateway",
    scope: "cloud",
    supportsTools: true,
    supportsLocalModels: false,
    listModelIds: async () => {
      try {
        const response = await fetch(AI_GATEWAY_MODELS_ENDPOINT);

        if (!response.ok) {
          throw new Error(`AI Gateway returned HTTP ${response.status}`);
        }

        const payload = await response.json() as GatewayModelsResponse;
        const modelIds = Array.from(
          new Set(
            (payload.data ?? [])
              .map((model) => model.id)
              .filter((modelId): modelId is string => Boolean(modelId)),
          ),
        );

        return {
          modelIds,
          source: AI_GATEWAY_MODELS_ENDPOINT,
        };
      } catch (error) {
        return {
          error: error instanceof Error ? error.message : String(error),
          modelIds: [],
          source: AI_GATEWAY_MODELS_ENDPOINT,
        };
      }
    },
    resolveLanguageModel: (config) => config.modelId,
  },
];

export function providerById(providerId: string) {
  return (
    modelProviderAdapters.find((provider) => provider.id === providerId)
    ?? modelProviderAdapters[0]
  );
}

export async function loadProviderModelOptions(config: AgentProviderConfig) {
  const provider = providerById(config.providerId);

  if (!provider.listModelIds) {
    return {
      modelIds: [],
      source: provider.label,
    };
  }

  return provider.listModelIds(config);
}

function normalizeEndpoint(endpoint: string) {
  return endpoint.replace(/\/+$/g, "");
}
