import type { CodingAgentTerminalEvent } from "./types";

export function formatTerminalEvent(event: CodingAgentTerminalEvent): string {
  switch (event.type) {
    case "session-started":
      return `[${event.provider}] session ${event.sessionId}\r\n`;
    case "output":
      return event.text;
    case "tool": {
      const suffix = event.text ? `: ${event.text}` : "";
      const status = event.status ? ` ${event.status}` : "";
      return `[${event.provider}] ${event.name}${status}${suffix}\r\n`;
    }
    case "error":
      return `[${event.provider}] error: ${event.message}\r\n`;
    case "turn-completed":
      return event.finalResponse ? `${event.finalResponse}\r\n` : "";
  }
}

export async function* terminalText(
  events: AsyncIterable<CodingAgentTerminalEvent>,
): AsyncGenerator<string> {
  for await (const event of events) {
    const text = formatTerminalEvent(event);

    if (text) {
      yield text;
    }
  }
}
