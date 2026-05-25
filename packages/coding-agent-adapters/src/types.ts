export type CodingAgentProvider = "codex" | "claude";

export type CodingAgentTerminalEvent =
  | {
      type: "session-started";
      provider: CodingAgentProvider;
      sessionId: string;
    }
  | {
      type: "output";
      provider: CodingAgentProvider;
      text: string;
    }
  | {
      type: "tool";
      provider: CodingAgentProvider;
      name: string;
      status?: string;
      text?: string;
    }
  | {
      type: "error";
      provider: CodingAgentProvider;
      message: string;
    }
  | {
      type: "turn-completed";
      provider: CodingAgentProvider;
      finalResponse?: string;
      sessionId?: string;
    };

export type CodingAgentTerminalSendOptions = {
  signal?: AbortSignal;
};

export type CodingAgentTerminalSession = {
  readonly provider: CodingAgentProvider;
  readonly cwd: string;
  readonly sessionId: string | null;
  send(
    input: string,
    options?: CodingAgentTerminalSendOptions,
  ): AsyncGenerator<CodingAgentTerminalEvent>;
  stop(): Promise<void>;
};

export type CodingAgentTerminalSessionOptions = {
  cwd: string;
  additionalDirectories?: string[];
  env?: Record<string, string | undefined>;
  model?: string;
};
