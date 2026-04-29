import React from "react";
import {
  Check,
  ChevronDown,
  Loader2,
  Maximize2,
  Minimize2,
  RefreshCw,
  Sparkles,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import type {
  MovePackage,
  PackageTree,
} from "@/features/empty-project/filesystem-tree";
import { MarkdownMessage } from "@/features/project-workspace/ai/markdown-message";
import { buildMovePackageAiContext } from "@/features/project-workspace/ai/move-package-ai-context";
import {
  listOllamaModels,
  preloadOllamaModel,
  streamChatWithOllama,
  type AiChatMessage,
  type OllamaModel,
} from "@/features/project-workspace/ai/ollama-client";
import { cn } from "@/lib/utils";
import peregrineMarkUrl from "@/assets/peregrine-white.svg";

type AiFloatingWindowProps = {
  activeMovePackage: MovePackage | null;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  packageTree: PackageTree;
};

type ChatMessage = {
  content: string;
  id: number;
  role: "assistant" | "user";
};

const DEFAULT_MODEL = "llama3.2";
const MODEL_STORAGE_KEY = "peregrine.ai.ollama.model";
const MAX_CHAT_HISTORY_MESSAGES = 10;

const starterPrompts = [
  "Summarize the security surface.",
  "Which entry functions are riskiest?",
  "Review admin controls.",
  "Explain external calls.",
];

export function AiFloatingWindow({
  activeMovePackage,
  isOpen,
  onOpenChange,
  packageTree,
}: AiFloatingWindowProps) {
  const [models, setModels] = React.useState<OllamaModel[]>([]);
  const [model, setModel] = React.useState(() => localStorage.getItem(MODEL_STORAGE_KEY) ?? DEFAULT_MODEL);
  const [input, setInput] = React.useState("");
  const [messages, setMessages] = React.useState<ChatMessage[]>(() => [initialAssistantMessage()]);
  const [isLoadingModels, setIsLoadingModels] = React.useState(false);
  const [isSending, setIsSending] = React.useState(false);
  const [isExpanded, setIsExpanded] = React.useState(false);
  const [modelError, setModelError] = React.useState<string | null>(null);
  const [panelPosition, setPanelPosition] = React.useState<{ x: number; y: number } | null>(null);
  const [streamingMessageId, setStreamingMessageId] = React.useState<number | null>(null);
  const contextCacheRef = React.useRef<{ context: string; key: string } | null>(null);
  const lastPreloadedModelRef = React.useRef<string | null>(null);
  const messagesEndRef = React.useRef<HTMLDivElement | null>(null);
  const panelRef = React.useRef<HTMLDivElement | null>(null);
  const dragStateRef = React.useRef<{
    offsetX: number;
    offsetY: number;
    pointerId: number;
  } | null>(null);
  const hasUserMessages = messages.some((message) => message.role === "user");

  React.useEffect(() => {
    localStorage.setItem(MODEL_STORAGE_KEY, model);
  }, [model]);

  React.useEffect(() => {
    contextCacheRef.current = null;
    setMessages([initialAssistantMessage(activeMovePackage?.name)]);
  }, [activeMovePackage?.manifestPath, activeMovePackage?.name, packageTree.rootPath]);

  const refreshModels = React.useCallback(async () => {
    setIsLoadingModels(true);
    setModelError(null);

    try {
      const nextModels = await listOllamaModels();

      setModels(nextModels);
      setModel((current) =>
        nextModels.length && !nextModels.some((candidate) => candidate.name === current)
          ? nextModels[0].name
          : current,
      );
    } catch (error) {
      setModelError(getErrorMessage(error));
    } finally {
      setIsLoadingModels(false);
    }
  }, []);

  React.useEffect(() => {
    void refreshModels();
  }, [refreshModels]);

  React.useEffect(() => {
    const trimmedModel = model.trim();

    if (!isOpen || !activeMovePackage || !trimmedModel || lastPreloadedModelRef.current === trimmedModel) {
      return;
    }

    let isStale = false;
    const timeout = window.setTimeout(() => {
      preloadOllamaModel(trimmedModel)
        .then((response) => {
          if (isStale) {
            return;
          }

          lastPreloadedModelRef.current = response.model;
        })
        .catch((error) => {
          if (isStale) {
            return;
          }

          setModelError(getErrorMessage(error));
        });
    }, 350);

    return () => {
      isStale = true;
      window.clearTimeout(timeout);
    };
  }, [activeMovePackage, isOpen, model]);

  React.useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ block: "end" });
  }, [messages, isSending]);

  React.useLayoutEffect(() => {
    const panel = panelRef.current;

    if (!panelPosition || !panel) {
      return;
    }

    const rect = panel.getBoundingClientRect();
    const margin = 12;
    const nextPosition = {
      x: clamp(panelPosition.x, margin, window.innerWidth - rect.width - margin),
      y: clamp(panelPosition.y, margin, window.innerHeight - rect.height - margin),
    };

    if (nextPosition.x !== panelPosition.x || nextPosition.y !== panelPosition.y) {
      setPanelPosition(nextPosition);
    }
  }, [isExpanded, panelPosition]);

  const handleDragStart = React.useCallback((event: React.PointerEvent<HTMLElement>) => {
    if (
      event.button !== 0 ||
      (event.target as HTMLElement).closest("[data-ai-no-drag]")
    ) {
      return;
    }

    const panel = panelRef.current;

    if (!panel) {
      return;
    }

    const rect = panel.getBoundingClientRect();
    dragStateRef.current = {
      offsetX: event.clientX - rect.left,
      offsetY: event.clientY - rect.top,
      pointerId: event.pointerId,
    };
    panel.setPointerCapture(event.pointerId);
    setPanelPosition({ x: rect.left, y: rect.top });
  }, []);

  const handleDragMove = React.useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const dragState = dragStateRef.current;
    const panel = panelRef.current;

    if (!dragState || dragState.pointerId !== event.pointerId || !panel) {
      return;
    }

    const rect = panel.getBoundingClientRect();
    const margin = 12;
    const nextX = clamp(
      event.clientX - dragState.offsetX,
      margin,
      window.innerWidth - rect.width - margin,
    );
    const nextY = clamp(
      event.clientY - dragState.offsetY,
      margin,
      window.innerHeight - rect.height - margin,
    );

    setPanelPosition({ x: nextX, y: nextY });
  }, []);

  const handleDragEnd = React.useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const dragState = dragStateRef.current;

    if (!dragState || dragState.pointerId !== event.pointerId) {
      return;
    }

    dragStateRef.current = null;

    if (panelRef.current?.hasPointerCapture(event.pointerId)) {
      panelRef.current.releasePointerCapture(event.pointerId);
    }
  }, []);

  const sendMessage = React.useCallback(
    async (content: string) => {
      const trimmed = content.trim();

      if (!trimmed || isSending || !activeMovePackage) {
        return;
      }

      const userMessage: ChatMessage = {
        content: trimmed,
        id: Date.now(),
        role: "user",
      };
      const assistantMessageId = Date.now() + 1;
      const assistantPlaceholder: ChatMessage = {
        content: "",
        id: assistantMessageId,
        role: "assistant",
      };
      const nextMessages = [...messages, userMessage];

      setMessages([...nextMessages, assistantPlaceholder]);
      setInput("");
      setIsSending(true);
      setStreamingMessageId(assistantMessageId);

      try {
        const contextKey = `${packageTree.rootPath}::${activeMovePackage.manifestPath}`;
        const context =
          contextCacheRef.current?.key === contextKey
            ? contextCacheRef.current.context
            : await buildMovePackageAiContext({
                movePackage: activeMovePackage,
                packageTree,
              });

        contextCacheRef.current = {
          context,
          key: contextKey,
        };

        let streamedContent = "";

        const appendAssistantContent = (chunk: string) => {
          streamedContent += chunk;
          setMessages((current) => {
            const hasMessage = current.some((message) => message.id === assistantMessageId);

            if (!hasMessage) {
              return [
                ...current,
                {
                  content: streamedContent,
                  id: assistantMessageId,
                  role: "assistant",
                },
              ];
            }

            return current.map((message) =>
              message.id === assistantMessageId
                ? { ...message, content: streamedContent }
                : message,
            );
          });
        };

        const answer = await streamChatWithOllama({
          model,
          messages: [
            systemMessage(context),
            ...nextMessages.slice(-MAX_CHAT_HISTORY_MESSAGES).map(toAiMessage),
          ],
          onChunk: appendAssistantContent,
          onDebug: () => undefined,
        });

        if (!streamedContent.trim()) {
          appendAssistantContent(answer.trim() || "The model returned an empty response.");
        }
      } catch (error) {
        setMessages((current) =>
          current.map((message) =>
            message.id === assistantMessageId
              ? { ...message, content: ollamaErrorMessage(error) }
              : message,
          ),
        );
      } finally {
        setIsSending(false);
        setStreamingMessageId(null);
      }
    },
    [activeMovePackage, isSending, messages, model, packageTree],
  );

  if (!activeMovePackage) {
    return null;
  }

  if (!isOpen) {
    return (
      <button
        className="ai-liquid-launcher absolute bottom-9 right-5 z-30 flex h-10 items-center gap-2 rounded-[14px] px-3 text-sm font-medium text-foreground transition"
        onClick={() => onOpenChange(true)}
        type="button"
      >
        <Sparkles className="size-4 text-primary" aria-hidden="true" />
        Ask AI
      </button>
    );
  }

  return (
    <section
      className={cn("pointer-events-none z-30", panelPosition ? "fixed" : "absolute bottom-9 right-5")}
      style={panelPosition ? { left: panelPosition.x, top: panelPosition.y } : undefined}
    >
      <div
        ref={panelRef}
        className={cn(
          "ai-liquid-panel pointer-events-auto grid overflow-hidden rounded-[18px] transition-[width,height,box-shadow] duration-200",
          "grid-rows-[auto_minmax(0,1fr)_auto]",
          isExpanded
            ? "h-[min(760px,calc(100vh-7rem))] w-[min(760px,calc(100vw-3rem))]"
            : "h-[560px] w-[420px]",
        )}
        onPointerCancel={handleDragEnd}
        onPointerMove={handleDragMove}
        onPointerUp={handleDragEnd}
      >
        <header
          className="ai-liquid-titlebar relative cursor-move touch-none px-3 py-3"
          onPointerDown={handleDragStart}
        >
          <div className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-3">
            <div className="flex min-w-0 items-center gap-2.5">
              <span className="ai-liquid-symbol grid size-8 shrink-0 place-items-center rounded-[10px] text-primary">
                <PeregrineMark className="size-4" />
              </span>
              <div className="min-w-0">
                <p className="truncate text-sm font-semibold leading-5 text-foreground">
                  {activeMovePackage.name}
                </p>
              </div>
            </div>

            <div className="flex items-center gap-1" data-ai-no-drag>
              <ModelSelector
                isLoading={isLoadingModels}
                model={model}
                models={models}
                onRefresh={() => void refreshModels()}
                onSelect={setModel}
              />
              <Button
                aria-label={isExpanded ? "Collapse AI window" : "Expand AI window"}
                className="ai-liquid-control size-7 text-muted-foreground"
                onClick={() => setIsExpanded((current) => !current)}
                size="icon-xs"
                type="button"
                variant="ghost"
              >
                {isExpanded ? (
                  <Minimize2 className="size-3.5" aria-hidden="true" />
                ) : (
                  <Maximize2 className="size-3.5" aria-hidden="true" />
                )}
              </Button>
              <Button
                aria-label="Minimize AI window"
                className="ai-liquid-control size-7 text-muted-foreground"
                onClick={() => onOpenChange(false)}
                size="icon-xs"
                type="button"
                variant="ghost"
              >
                <ChevronDown className="size-4" aria-hidden="true" />
              </Button>
            </div>
          </div>

          {modelError ? (
            <p className="mt-2 rounded-[10px] border border-amber-300/20 bg-amber-500/10 px-2 py-1 text-[11px] leading-4 text-amber-200">
              {modelError}
            </p>
          ) : null}
        </header>

        <ScrollArea className="ai-liquid-scroll relative min-h-0">
          <div className="grid gap-2 p-3 pb-4">
            {messages.map((message) => (
              <MessageBubble
                isStreaming={message.id === streamingMessageId}
                key={message.id}
                message={message}
              />
            ))}
            <div ref={messagesEndRef} />
          </div>
        </ScrollArea>

        <form
          className="ai-liquid-composer relative grid gap-2 p-3"
          onSubmit={(event) => {
            event.preventDefault();
            void sendMessage(input);
          }}
        >
          {!hasUserMessages ? (
            <div className="flex flex-wrap gap-1.5">
              {starterPrompts.map((prompt) => (
                <button
                  className="ai-liquid-chip rounded-[10px] px-2 py-1 text-[11px] text-muted-foreground transition"
                  disabled={isSending}
                  key={prompt}
                  onClick={() => void sendMessage(prompt)}
                  type="button"
                >
                  {prompt}
                </button>
              ))}
            </div>
          ) : null}
          <div className="relative">
            <textarea
              className="ai-liquid-input h-11 max-h-28 min-h-11 w-full resize-none rounded-[12px] px-3 py-2 text-sm leading-5 text-foreground outline-none placeholder:text-muted-foreground"
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void sendMessage(input);
                }
              }}
              placeholder="Ask about Move security, entry functions, capabilities..."
              value={input}
            />
          </div>
        </form>
      </div>
    </section>
  );
}

function ModelSelector({
  isLoading,
  model,
  models,
  onRefresh,
  onSelect,
}: {
  isLoading: boolean;
  model: string;
  models: OllamaModel[];
  onRefresh: () => void;
  onSelect: (model: string) => void;
}) {
  const [isOpen, setIsOpen] = React.useState(false);
  const selectedModel = model || models[0]?.name || DEFAULT_MODEL;
  const selectModel = React.useCallback(
    (nextModel: string) => {
      onSelect(nextModel);
      setIsOpen(false);
    },
    [onSelect],
  );

  return (
    <DropdownMenu open={isOpen} onOpenChange={setIsOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          aria-label="Select Ollama model"
          className="ai-liquid-control h-7 max-w-44 justify-between gap-2 px-2 text-[11px] font-medium text-muted-foreground"
          size="sm"
          type="button"
          variant="outline"
        >
          <span className="truncate">{selectedModel}</span>
          <ChevronDown className="size-3 shrink-0" aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="ai-liquid-popover w-72 text-foreground"
      >
        <div className="flex items-center justify-between gap-2 px-2 py-1.5">
          <DropdownMenuLabel className="px-0 py-0 text-[11px] uppercase text-muted-foreground">
            Installed Ollama models
          </DropdownMenuLabel>
          <Button
            aria-label="Refresh models"
            className="ai-liquid-control size-6 text-muted-foreground"
            disabled={isLoading}
            onClick={(event) => {
              event.preventDefault();
              onRefresh();
            }}
            size="icon-xs"
            type="button"
            variant="ghost"
          >
            {isLoading ? (
              <Loader2 className="size-3 animate-spin" aria-hidden="true" />
            ) : (
              <RefreshCw className="size-3" aria-hidden="true" />
            )}
          </Button>
        </div>
        <DropdownMenuSeparator className="bg-white/10" />
        {models.length ? (
          models.map((candidate) => (
            <DropdownMenuItem
              className="min-w-0 cursor-pointer gap-2 text-xs"
              key={candidate.name}
              onClick={() => selectModel(candidate.name)}
              onPointerUp={() => selectModel(candidate.name)}
              onSelect={(event) => {
                event.preventDefault();
                selectModel(candidate.name);
              }}
            >
              <span className="truncate font-mono">{candidate.name}</span>
              {candidate.name === model ? (
                <Check className="ml-auto size-3.5 text-primary" aria-hidden="true" />
              ) : null}
            </DropdownMenuItem>
          ))
        ) : (
          <DropdownMenuItem className="text-xs text-muted-foreground" disabled>
            No installed models found
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function MessageBubble({
  isStreaming,
  message,
}: {
  isStreaming: boolean;
  message: ChatMessage;
}) {
  const isUser = message.role === "user";

  return (
    <article
      className={cn(
        "px-1 py-1 text-xs leading-5",
        isUser
          ? "ml-auto max-w-[88%] text-right text-foreground"
          : "mr-auto w-full text-foreground",
      )}
    >
      {isUser ? (
        <p className="whitespace-pre-wrap break-words">{message.content}</p>
      ) : message.content ? (
        <MarkdownMessage content={message.content} />
      ) : null}
      {!isUser && isStreaming ? (
        <PeregrineStreamingIndicator hasContent={Boolean(message.content.trim())} />
      ) : null}
    </article>
  );
}

function PeregrineMark({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cn("peregrine-mask-icon", className)}
      style={
        {
          "--peregrine-mark": `url(${peregrineMarkUrl})`,
        } as React.CSSProperties
      }
    />
  );
}

function PeregrineStreamingIndicator({ hasContent }: { hasContent: boolean }) {
  return (
    <div
      className={cn(
        "ai-peregrine-thinking flex items-center",
        hasContent ? "mt-3" : "min-h-9",
      )}
      role="status"
    >
      <span
        aria-hidden="true"
        className="ai-peregrine-thinking__mark"
        style={
          {
            "--peregrine-mark": `url(${peregrineMarkUrl})`,
          } as React.CSSProperties
        }
      />
      <span className="sr-only">Generating response</span>
    </div>
  );
}

function systemMessage(context: string): AiChatMessage {
  return {
    role: "system",
    content: [
      "You are Peregrine's local AI assistant for Sui Move package security review.",
      "Answer using only the loaded package context when discussing the code.",
      "Prioritize concrete security risks, entry points, capabilities, object ownership, admin controls, external calls, and missing tests/specs.",
      "Be concise. Reference module and function names when possible. If context is insufficient, say exactly what is missing.",
      "",
      context,
    ].join("\n"),
  };
}

function toAiMessage(message: ChatMessage): AiChatMessage {
  return {
    content: message.content,
    role: message.role,
  };
}

function initialAssistantMessage(packageName?: string): ChatMessage {
  return {
    content: packageName
      ? `I can answer security questions about ${packageName} using the loaded package surface and source context.`
      : "I can answer security questions about the loaded Move package.",
    id: 1,
    role: "assistant",
  };
}

function ollamaErrorMessage(error: unknown) {
  return [
    "I could not get a response from local Ollama.",
    getErrorMessage(error),
    "Check that Ollama is running and that the selected model is installed.",
  ].join("\n");
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Unknown error.";
}

function clamp(value: number, min: number, max: number) {
  if (max < min) {
    return min;
  }

  return Math.min(Math.max(value, min), max);
}
