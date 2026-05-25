import { Codex } from "@openai/codex-sdk";
import type {
  CodexOptions,
  Input,
  Thread,
  ThreadEvent,
  ThreadItem,
  ThreadOptions,
  TurnOptions,
} from "@openai/codex-sdk";

import type {
  CodingAgentTerminalEvent,
  CodingAgentTerminalSendOptions,
  CodingAgentTerminalSession,
  CodingAgentTerminalSessionOptions,
} from "./types";

export type CodexTerminalSessionOptions = CodingAgentTerminalSessionOptions & {
  codex?: CodexOptions;
  thread?: Omit<ThreadOptions, "workingDirectory" | "additionalDirectories" | "model">;
  turn?: Omit<TurnOptions, "signal">;
};

export class CodexTerminalSession implements CodingAgentTerminalSession {
  readonly provider = "codex";
  readonly cwd: string;

  private readonly codex: Codex;
  private readonly thread: Thread;
  private readonly threadOptions: ThreadOptions;
  private readonly turnOptions?: Omit<TurnOptions, "signal">;
  private stopped = false;

  constructor(options: CodexTerminalSessionOptions) {
    this.cwd = options.cwd;
    this.codex = new Codex(options.codex);
    this.threadOptions = {
      ...options.thread,
      additionalDirectories: options.additionalDirectories,
      model: options.model,
      workingDirectory: options.cwd,
    };
    this.turnOptions = options.turn;
    this.thread = this.codex.startThread(this.threadOptions);
  }

  get sessionId(): string | null {
    return this.thread.id;
  }

  async *send(
    input: string,
    options: CodingAgentTerminalSendOptions = {},
  ): AsyncGenerator<CodingAgentTerminalEvent> {
    if (this.stopped) {
      throw new Error("Codex terminal session has been stopped.");
    }

    const outputOffsets = new Map<string, number>();
    const streamed = await this.thread.runStreamed(toCodexInput(input), {
      ...this.turnOptions,
      signal: options.signal,
    });

    for await (const event of streamed.events) {
      yield* codexEventToTerminalEvents(event, outputOffsets);
    }
  }

  async stop(): Promise<void> {
    this.stopped = true;
  }
}

export function createCodexTerminalSession(
  options: CodexTerminalSessionOptions,
): CodexTerminalSession {
  return new CodexTerminalSession(options);
}

function toCodexInput(input: string): Input {
  return input;
}

function* codexEventToTerminalEvents(
  event: ThreadEvent,
  outputOffsets: Map<string, number>,
): Generator<CodingAgentTerminalEvent> {
  switch (event.type) {
    case "thread.started":
      yield {
        provider: "codex",
        sessionId: event.thread_id,
        type: "session-started",
      };
      return;
    case "item.started":
    case "item.updated":
    case "item.completed":
      yield* codexItemToTerminalEvents(event.item, outputOffsets);
      return;
    case "turn.completed":
      yield {
        provider: "codex",
        type: "turn-completed",
      };
      return;
    case "turn.failed":
      yield {
        message: event.error.message,
        provider: "codex",
        type: "error",
      };
      return;
    case "error":
      yield {
        message: event.message,
        provider: "codex",
        type: "error",
      };
      return;
    case "turn.started":
      return;
  }
}

function* codexItemToTerminalEvents(
  item: ThreadItem,
  outputOffsets: Map<string, number>,
): Generator<CodingAgentTerminalEvent> {
  switch (item.type) {
    case "agent_message":
      yield {
        provider: "codex",
        text: `${item.text}\r\n`,
        type: "output",
      };
      return;
    case "reasoning":
      yield {
        provider: "codex",
        text: `${item.text}\r\n`,
        type: "output",
      };
      return;
    case "command_execution": {
      const previousOffset = outputOffsets.get(item.id) ?? 0;
      const nextOutput = item.aggregated_output.slice(previousOffset);
      outputOffsets.set(item.id, item.aggregated_output.length);

      if (nextOutput) {
        yield {
          provider: "codex",
          text: nextOutput,
          type: "output",
        };
      }

      if (item.status !== "in_progress") {
        yield {
          name: item.command,
          provider: "codex",
          status:
            item.exit_code == null
              ? item.status
              : `${item.status} (${item.exit_code})`,
          type: "tool",
        };
      }
      return;
    }
    case "file_change":
      yield {
        name: "file_change",
        provider: "codex",
        status: item.status,
        text: item.changes
          .map((change) => `${change.kind} ${change.path}`)
          .join(", "),
        type: "tool",
      };
      return;
    case "mcp_tool_call":
      yield {
        name: `${item.server}.${item.tool}`,
        provider: "codex",
        status: item.status,
        text: item.error?.message,
        type: "tool",
      };
      return;
    case "web_search":
      yield {
        name: "web_search",
        provider: "codex",
        text: item.query,
        type: "tool",
      };
      return;
    case "todo_list":
      return;
    case "error":
      yield {
        message: item.message,
        provider: "codex",
        type: "error",
      };
      return;
  }
}
