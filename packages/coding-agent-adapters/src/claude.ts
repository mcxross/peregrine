import { query } from "@anthropic-ai/claude-agent-sdk";
import type {
  Options as ClaudeOptions,
  SDKMessage,
} from "@anthropic-ai/claude-agent-sdk";

import type {
  CodingAgentTerminalEvent,
  CodingAgentTerminalSendOptions,
  CodingAgentTerminalSession,
  CodingAgentTerminalSessionOptions,
} from "./types";

export type ClaudeTerminalSessionOptions = CodingAgentTerminalSessionOptions & {
  claude?: Omit<ClaudeOptions, "abortController" | "additionalDirectories" | "cwd" | "env" | "model" | "resume">;
  resumeSessionId?: string;
};

export class ClaudeTerminalSession implements CodingAgentTerminalSession {
  readonly provider = "claude";
  readonly cwd: string;

  private readonly baseOptions: ClaudeTerminalSessionOptions;
  private currentSessionId: string | null;
  private stopped = false;
  private activeAbortController: AbortController | null = null;

  constructor(options: ClaudeTerminalSessionOptions) {
    this.cwd = options.cwd;
    this.baseOptions = options;
    this.currentSessionId = options.resumeSessionId ?? null;
  }

  get sessionId(): string | null {
    return this.currentSessionId;
  }

  async *send(
    input: string,
    options: CodingAgentTerminalSendOptions = {},
  ): AsyncGenerator<CodingAgentTerminalEvent> {
    if (this.stopped) {
      throw new Error("Claude terminal session has been stopped.");
    }

    const abortController = new AbortController();
    this.activeAbortController = abortController;
    const abortListener = () => abortController.abort();
    options.signal?.addEventListener("abort", abortListener, { once: true });

    try {
      const stream = query({
        prompt: input,
        options: {
          ...this.baseOptions.claude,
          abortController,
          additionalDirectories: this.baseOptions.additionalDirectories,
          cwd: this.baseOptions.cwd,
          env: this.baseOptions.env,
          model: this.baseOptions.model,
          resume: this.currentSessionId ?? undefined,
        },
      });

      for await (const message of stream) {
        yield* claudeMessageToTerminalEvents(message, (sessionId) => {
          this.currentSessionId = sessionId;
        });
      }
    } finally {
      options.signal?.removeEventListener("abort", abortListener);

      if (this.activeAbortController === abortController) {
        this.activeAbortController = null;
      }
    }
  }

  async stop(): Promise<void> {
    this.stopped = true;
    this.activeAbortController?.abort();
    this.activeAbortController = null;
  }
}

export function createClaudeTerminalSession(
  options: ClaudeTerminalSessionOptions,
): ClaudeTerminalSession {
  return new ClaudeTerminalSession(options);
}

function* claudeMessageToTerminalEvents(
  message: SDKMessage,
  updateSessionId: (sessionId: string) => void,
): Generator<CodingAgentTerminalEvent> {
  const sessionId = "session_id" in message ? message.session_id : null;

  if (sessionId) {
    updateSessionId(sessionId);
  }

  switch (message.type) {
    case "system":
      if (message.subtype === "init") {
        yield {
          provider: "claude",
          sessionId: message.session_id,
          type: "session-started",
        };
      }

      if (message.subtype === "local_command_output") {
        yield {
          provider: "claude",
          text: message.content,
          type: "output",
        };
      }

      if (message.subtype === "permission_denied") {
        yield {
          message: message.message,
          provider: "claude",
          type: "error",
        };
      }
      return;
    case "assistant":
      yield* claudeContentToTerminalEvents(message.message.content);
      return;
    case "result":
      if (message.subtype === "success") {
        yield {
          finalResponse: message.result,
          provider: "claude",
          sessionId: message.session_id,
          type: "turn-completed",
        };
      } else {
        yield {
          message: message.errors.join("\n") || "Claude query failed.",
          provider: "claude",
          type: "error",
        };
      }
      return;
    case "tool_progress":
      yield {
        name: message.tool_name,
        provider: "claude",
        status: `${message.elapsed_time_seconds}s`,
        type: "tool",
      };
      return;
    case "tool_use_summary":
      yield {
        name: "tool_use_summary",
        provider: "claude",
        text: message.summary,
        type: "tool",
      };
      return;
    default:
      return;
  }
}

function* claudeContentToTerminalEvents(
  content: unknown,
): Generator<CodingAgentTerminalEvent> {
  if (!Array.isArray(content)) {
    return;
  }

  for (const block of content) {
    if (!isRecord(block)) {
      continue;
    }

    if (block.type === "text" && typeof block.text === "string") {
      yield {
        provider: "claude",
        text: `${block.text}\r\n`,
        type: "output",
      };
      continue;
    }

    if (block.type === "tool_use" && typeof block.name === "string") {
      yield {
        name: block.name,
        provider: "claude",
        text: typeof block.input === "string" ? block.input : undefined,
        type: "tool",
      };
    }
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
