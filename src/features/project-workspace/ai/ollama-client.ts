import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type AiChatRole = "assistant" | "system" | "user";

export type AiChatMessage = {
  content: string;
  role: AiChatRole;
};

export type OllamaModel = {
  name: string;
};

type OllamaChatResponse = {
  content: string;
};

type OllamaPreloadResponse = {
  keepAlive: string;
  model: string;
};

type OllamaChatStreamEvent = {
  streamId: string;
  kind: "chunk" | "debug" | "done" | "error";
  content: string;
};

const OLLAMA_CHAT_STREAM_EVENT = "ollama-chat-stream";

export async function listOllamaModels(endpoint?: string) {
  return invoke<OllamaModel[]>("list_ollama_models", {
    baseUrl: endpoint?.trim() || null,
  });
}

export async function preloadOllamaModel(model: string) {
  return invoke<OllamaPreloadResponse>("preload_ollama_model", { model });
}

export async function chatWithOllama({
  messages,
  model,
}: {
  messages: AiChatMessage[];
  model: string;
}) {
  const response = await invoke<OllamaChatResponse>("chat_with_ollama", {
    messages,
    model,
  });

  return response.content;
}

export async function streamChatWithOllama({
  messages,
  model,
  onChunk,
  onDebug,
}: {
  messages: AiChatMessage[];
  model: string;
  onChunk: (chunk: string) => void;
  onDebug?: (message: string) => void;
}) {
  const streamId = createStreamId();
  const unlisten = await listen<OllamaChatStreamEvent>(
    OLLAMA_CHAT_STREAM_EVENT,
    (event) => {
      if (event.payload.streamId !== streamId) {
        return;
      }

      if (event.payload.kind === "chunk") {
        onChunk(event.payload.content);
        return;
      }

      const message = `[ollama:${streamId}] ${event.payload.content}`;

      if (event.payload.kind === "error") {
        console.error(message);
      } else if (event.payload.kind === "debug") {
        console.debug(message);
      }

      onDebug?.(event.payload.content);
    },
  );

  try {
    const response = await invoke<OllamaChatResponse>("stream_chat_with_ollama", {
      messages,
      model,
      streamId,
    });

    return response.content;
  } finally {
    unlisten();
  }
}

function createStreamId() {
  if ("randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
