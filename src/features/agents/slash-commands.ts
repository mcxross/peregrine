export interface SlashCommandDef {
  command: string;
  description: string;
}

export const SLASH_COMMANDS: SlashCommandDef[] = [
  { command: "new", description: "start a new chat during a conversation" },
  { command: "init", description: "create an AGENTS.md file with instructions for Peregrine" },
  { command: "compact", description: "summarize conversation to prevent hitting the context limit" },
  { command: "review", description: "review my current changes and find issues" },
  { command: "rename", description: "rename the current thread" },
  { command: "resume", description: "resume a saved chat" },
  { command: "archive", description: "archive this session and exit" },
  { command: "clear", description: "clear the terminal and start a new chat" },
  { command: "fork", description: "fork the current chat" },
  { command: "skills", description: "use skills to improve how Peregrine performs specific tasks" },
  { command: "hooks", description: "view and manage lifecycle hooks" },
  { command: "status", description: "show current session configuration and token usage" },
  { command: "plan", description: "switch to Plan mode" },
  { command: "goal", description: "set or view the goal for a long-running task" },
  { command: "scan", description: "start a Sui Move security scan goal" },
  { command: "audit", description: "manage autonomous security audit runs" },
  { command: "agent", description: "switch the active agent thread" },
  { command: "subagents", description: "switch the active agent thread" },
  { command: "side", description: "start a side conversation in an ephemeral fork" },
  { command: "btw", description: "start a side conversation in an ephemeral fork" },
  { command: "permissions", description: "choose what Peregrine is allowed to do" },
  { command: "setup-default-sandbox", description: "set up elevated agent sandbox" },
  { command: "sandbox-add-read-dir", description: "let sandbox read a directory: /sandbox-add-read-dir <absolute_path>" },
  { command: "experimental", description: "toggle experimental features" },
  { command: "approve", description: "approve one retry of a recent auto-review denial" },
  { command: "memories", description: "configure memory use and generation" },
  { command: "mcp", description: "list configured MCP tools; use /mcp verbose for details" },
  { command: "apps", description: "manage apps" },
  { command: "plugins", description: "browse plugins" },
  { command: "rollout", description: "print the rollout file path" },
  { command: "test-approval", description: "test approval request" }
];
