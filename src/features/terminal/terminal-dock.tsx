import React from "react";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  Bot,
  Check,
  ChevronDown,
  GripHorizontal,
  Pencil,
  Plus,
  SquareTerminal,
  X,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import {
  listenTerminalExit,
  listenTerminalOutput,
  resizeTerminal,
  startTerminal,
  stopTerminal,
  writeTerminal,
} from "@peregrine/desktop-runtime";

type TerminalDockProps = {
  cwd: string;
  height: number;
  onClose: () => void;
  onHeightChange: (height: number) => void;
};

type TerminalTab = {
  id: string;
  kind: TerminalSessionKind;
  title: string;
};

type TerminalSessionKind = "terminal" | "codex" | "claude";

const MIN_TERMINAL_HEIGHT = 180;
const MAX_TERMINAL_HEIGHT = 560;
const SESSION_KIND_OPTIONS: Array<{
  description: string;
  kind: TerminalSessionKind;
  label: string;
}> = [
  {
    description: "Start an OpenAI Codex coding-agent session",
    kind: "codex",
    label: "Codex",
  },
  {
    description: "Start a Claude coding-agent session",
    kind: "claude",
    label: "Claude",
  },
];

export function TerminalDock({
  cwd,
  height,
  onClose,
  onHeightChange,
}: TerminalDockProps) {
  const nextTabIndexRef = React.useRef(2);
  const initialTab = React.useMemo(() => createTerminalTab(1), []);
  const [tabs, setTabs] = React.useState<TerminalTab[]>([initialTab]);
  const [activeTabId, setActiveTabId] = React.useState(initialTab.id);
  const [selectedSessionKind, setSelectedSessionKind] =
    React.useState<TerminalSessionKind>("terminal");

  const addTab = React.useCallback((kind: TerminalSessionKind = selectedSessionKind) => {
    const tab = createTerminalTab(nextTabIndexRef.current, kind);
    nextTabIndexRef.current += 1;
    setTabs((current) => [...current, tab]);
    setActiveTabId(tab.id);
    setSelectedSessionKind(kind);
  }, [selectedSessionKind]);

  const renameTab = React.useCallback((tabId: string, title: string) => {
    const nextTitle = title.trim();

    if (!nextTitle) {
      return;
    }

    setTabs((current) =>
      current.map((tab) => (tab.id === tabId ? { ...tab, title: nextTitle } : tab)),
    );
  }, []);

  const closeTab = React.useCallback(
    (tabId: string) => {
      setTabs((current) => {
        if (current.length <= 1) {
          window.setTimeout(onClose, 0);
          return current;
        }

        const nextTabs = current.filter((tab) => tab.id !== tabId);

        if (tabId === activeTabId) {
          const closedIndex = current.findIndex((tab) => tab.id === tabId);
          const nextActiveTab = nextTabs[Math.max(0, closedIndex - 1)] ?? nextTabs[0];

          if (nextActiveTab) {
            setActiveTabId(nextActiveTab.id);
          }
        }

        return nextTabs;
      });
    },
    [activeTabId, onClose],
  );

  const beginResize = React.useCallback(
    (event: React.PointerEvent<HTMLButtonElement>) => {
      event.preventDefault();
      const startY = event.clientY;
      const startHeight = height;

      const onPointerMove = (moveEvent: PointerEvent) => {
        const nextHeight = clampTerminalHeight(startHeight + startY - moveEvent.clientY);
        onHeightChange(nextHeight);
      };

      const onPointerUp = () => {
        window.removeEventListener("pointermove", onPointerMove);
        window.removeEventListener("pointerup", onPointerUp);
      };

      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp, { once: true });
    },
    [height, onHeightChange],
  );

  return (
    <section
      className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border-t border-[color:var(--app-border)] bg-[#0b0b0d] text-foreground"
      style={{ height }}
    >
      <button
        aria-label="Resize terminal"
        className="grid h-2 cursor-row-resize place-items-center border-b border-[color:var(--app-border)] bg-[var(--app-chrome)] text-muted-foreground hover:text-foreground"
        onPointerDown={beginResize}
        type="button"
      >
        <GripHorizontal className="size-3" aria-hidden="true" />
      </button>

      <div className="grid min-h-0 grid-rows-[34px_minmax(0,1fr)]">
        <header className="grid min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 border-b border-white/10 bg-[var(--app-chrome)] px-3">
          <SquareTerminal className="size-3.5 text-primary" aria-hidden="true" />
          <div className="flex min-w-0 items-center gap-1 overflow-x-auto">
            {tabs.map((tab) => (
              <TerminalTabButton
                active={tab.id === activeTabId}
                key={tab.id}
                onClick={() => setActiveTabId(tab.id)}
                onClose={() => closeTab(tab.id)}
                onRename={(title) => renameTab(tab.id, title)}
                tab={tab}
              />
            ))}
            <Button
              aria-label={`Open new ${sessionKindLabel(selectedSessionKind)} session`}
              className="size-7 shrink-0 text-muted-foreground hover:text-foreground"
              onClick={() => addTab()}
              size="icon-xs"
              title={`New ${sessionKindLabel(selectedSessionKind)} session`}
              type="button"
              variant="ghost"
            >
              <Plus className="size-3.5" aria-hidden="true" />
            </Button>
          </div>
          <div className="flex min-w-0 items-center gap-2">
            <AgentSessionPicker
              onSelect={(kind) => addTab(kind)}
              selectedKind={selectedSessionKind}
            />
            <Button
              aria-label="Close terminal"
              className="size-7 shrink-0 text-muted-foreground hover:text-foreground"
              onClick={onClose}
              size="icon-xs"
              title="Close terminal"
              type="button"
              variant="ghost"
            >
              <X className="size-3.5" aria-hidden="true" />
            </Button>
          </div>
        </header>

        <div className="relative min-h-0 overflow-hidden p-2">
          {tabs.map((tab) => (
            <TerminalPane
              active={tab.id === activeTabId}
              cwd={cwd}
              key={tab.id}
              tab={tab}
            />
          ))}
        </div>
      </div>
    </section>
  );
}

function TerminalPane({
  active,
  cwd,
  tab,
}: {
  active: boolean;
  cwd: string;
  tab: TerminalTab;
}) {
  const containerRef = React.useRef<HTMLDivElement | null>(null);
  const terminalRef = React.useRef<Terminal | null>(null);
  const fitAddonRef = React.useRef<FitAddon | null>(null);
  const sessionIdRef = React.useRef<string | null>(null);
  const outputQueueRef = React.useRef("");
  const outputFrameRef = React.useRef<number | null>(null);
  const resizeTimerRef = React.useRef<number | null>(null);
  const activeRef = React.useRef(active);
  const [errorMessage, setErrorMessage] = React.useState<string | null>(null);

  const sessionKind = tab.kind;

  React.useEffect(() => {
    activeRef.current = active;
  }, [active]);

  React.useEffect(() => {
    const terminal = terminalRef.current;
    const fitAddon = fitAddonRef.current;
    const sessionId = sessionIdRef.current;

    if (!active || !terminal || !fitAddon) {
      return;
    }

    terminal.focus();
    fitAddon.fit();

    if (sessionId) {
      void resizeTerminal(sessionId, terminal.cols, terminal.rows).catch((error) => {
        console.warn("Could not resize terminal.", error);
      });
    }
  }, [active]);

  React.useEffect(() => {
    const container = containerRef.current;

    if (!container) {
      return;
    }

    let disposed = false;
    let unlistenOutput: (() => void) | null = null;
    let unlistenExit: (() => void) | null = null;

    const terminal = new Terminal({
      allowProposedApi: false,
      convertEol: true,
      cursorBlink: true,
      cursorStyle: "bar",
      disableStdin: false,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      fontSize: 12,
      lineHeight: 1.2,
      scrollback: 5_000,
      theme: {
        background: "#0b0b0d",
        black: "#151519",
        blue: "#5aa6ff",
        brightBlack: "#555762",
        brightBlue: "#7bb8ff",
        brightCyan: "#72d6d6",
        brightGreen: "#73d99f",
        brightMagenta: "#d69bf7",
        brightRed: "#ff7b7b",
        brightWhite: "#ffffff",
        brightYellow: "#ffe08a",
        cursor: "#78dcca",
        cyan: "#4ec8c8",
        foreground: "#e7e7ec",
        green: "#4fc77b",
        magenta: "#c678dd",
        red: "#f66f6f",
        selectionBackground: "#2f343f",
        white: "#d7d7dd",
        yellow: "#e6c45a",
      },
    });
    terminal.attachCustomKeyEventHandler((event: KeyboardEvent) => {
      if (event.key !== "Tab" || event.type !== "keydown") {
        return true;
      }

      const sessionId = sessionIdRef.current;

      if (!sessionId) {
        return false;
      }

      const data = event.shiftKey ? "\u001b[Z" : "\t";
      void writeTerminal(sessionId, data).catch((error) => {
        console.warn("Could not write terminal tab input.", error);
      });

      return false;
    });
    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();
    terminal.loadAddon(fitAddon);
    terminal.loadAddon(webLinksAddon);
    terminal.open(container);
    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    const flushOutput = () => {
      outputFrameRef.current = null;
      const output = outputQueueRef.current;

      if (!output) {
        return;
      }

      outputQueueRef.current = "";
      terminal.write(output);
    };

    const scheduleOutputFlush = (data: string) => {
      outputQueueRef.current += data;

      if (outputFrameRef.current != null) {
        return;
      }

      outputFrameRef.current = window.requestAnimationFrame(flushOutput);
    };

    const fitAndResize = () => {
      if (disposed || !activeRef.current) {
        return;
      }

      fitAddon.fit();
      const sessionId = sessionIdRef.current;

      if (sessionId) {
        void resizeTerminal(sessionId, terminal.cols, terminal.rows).catch((error) => {
          console.warn("Could not resize terminal.", error);
        });
      }
    };

    window.setTimeout(fitAndResize, 0);

    const resizeObserver = new ResizeObserver(() => {
      if (!activeRef.current) {
        return;
      }

      if (resizeTimerRef.current != null) {
        window.clearTimeout(resizeTimerRef.current);
      }

      resizeTimerRef.current = window.setTimeout(() => {
        resizeTimerRef.current = null;
        fitAndResize();
      }, 40);
    });
    resizeObserver.observe(container);

    const inputSubscription = terminal.onData((data) => {
      const sessionId = sessionIdRef.current;

      if (!sessionId) {
        return;
      }

      void writeTerminal(sessionId, data).catch((error) => {
        console.warn("Could not write terminal input.", error);
      });
    });

    void Promise.all([
      listenTerminalOutput((event) => {
        if (event.sessionId !== sessionIdRef.current) {
          return;
        }

        scheduleOutputFlush(event.data);
      }),
      listenTerminalExit((event) => {
        if (event.sessionId !== sessionIdRef.current) {
          return;
        }

        sessionIdRef.current = null;
        terminal.writeln("");
        terminal.writeln(`[process exited${event.code == null ? "" : ` with code ${event.code}`}]`);
      }),
    ])
      .then(([outputUnlisten, exitUnlisten]) => {
        if (disposed) {
          outputUnlisten();
          exitUnlisten();
          return null;
        }

        unlistenOutput = outputUnlisten;
        unlistenExit = exitUnlisten;
        return startTerminal({
          cols: Math.max(terminal.cols, 80),
          command: sessionKindCommand(sessionKind),
          cwd,
          rows: Math.max(terminal.rows, 24),
        });
      })
      .then((response) => {
        if (!response || disposed) {
          if (response?.sessionId) {
            void stopTerminal(response.sessionId);
          }
          return;
        }

        sessionIdRef.current = response.sessionId;
        window.setTimeout(() => {
          if (!disposed && activeRef.current) {
            fitAddon.fit();
            void resizeTerminal(response.sessionId, terminal.cols, terminal.rows);
            terminal.focus();
          }
        }, 0);
      })
      .catch((error) => {
        const message = error instanceof Error ? error.message : String(error);
        setErrorMessage(message);
        terminal.writeln(`Could not start terminal: ${message}`);
      });

    return () => {
      disposed = true;
      const sessionId = sessionIdRef.current;
      sessionIdRef.current = null;

      if (resizeTimerRef.current != null) {
        window.clearTimeout(resizeTimerRef.current);
        resizeTimerRef.current = null;
      }

      if (outputFrameRef.current != null) {
        window.cancelAnimationFrame(outputFrameRef.current);
        outputFrameRef.current = null;
      }

      resizeObserver.disconnect();
      inputSubscription.dispose();
      unlistenOutput?.();
      unlistenExit?.();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;

      if (sessionId) {
        void stopTerminal(sessionId).catch((error) => {
          console.warn("Could not stop terminal.", error);
        });
      }
    };
  }, [cwd, sessionKind]);

  return (
    <div
      aria-hidden={!active}
      className={cn(
        "absolute inset-0 overflow-hidden bg-[#0b0b0d] px-3 py-2",
        !active && "pointer-events-none invisible",
      )}
    >
      <div className="h-full min-h-0 overflow-hidden" ref={containerRef} />
      {errorMessage ? (
        <span className="sr-only">
          {tab.title} terminal error: {errorMessage}
        </span>
      ) : null}
    </div>
  );
}

function AgentSessionPicker({
  onSelect,
  selectedKind,
}: {
  onSelect: (kind: TerminalSessionKind) => void;
  selectedKind: TerminalSessionKind;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          aria-label="Choose agent session"
          className="h-7 gap-1.5 px-2 text-[11px] text-muted-foreground hover:text-foreground"
          size="sm"
          title="Choose agent session"
          type="button"
          variant="ghost"
        >
          <Bot className="size-3.5" aria-hidden="true" />
          <span className="hidden sm:inline">{sessionKindLabel(selectedKind)}</span>
          <ChevronDown className="size-3" aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        {SESSION_KIND_OPTIONS.map((option) => (
          <DropdownMenuItem
            className="grid grid-cols-[1rem_minmax(0,1fr)] gap-x-2 gap-y-0.5 py-2"
            key={option.kind}
            onSelect={() => onSelect(option.kind)}
          >
            <span className="flex size-4 items-center justify-center">
              {option.kind === selectedKind ? (
                <Check className="size-3.5 text-primary" aria-hidden="true" />
              ) : null}
            </span>
            <span className="min-w-0">
              <span className="block text-xs font-medium leading-4">{option.label}</span>
              <span className="block truncate text-[11px] leading-4 text-muted-foreground">
                {option.description}
              </span>
            </span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function TerminalTabButton({
  active,
  onClick,
  onClose,
  onRename,
  tab,
}: {
  active: boolean;
  onClick: () => void;
  onClose: () => void;
  onRename: (title: string) => void;
  tab: TerminalTab;
}) {
  const inputRef = React.useRef<HTMLInputElement | null>(null);
  const cancelRenameRef = React.useRef(false);
  const [editing, setEditing] = React.useState(false);
  const [draftTitle, setDraftTitle] = React.useState(tab.title);

  React.useEffect(() => {
    if (!editing) {
      setDraftTitle(tab.title);
    }
  }, [editing, tab.title]);

  React.useEffect(() => {
    if (!editing) {
      return;
    }

    inputRef.current?.focus();
    inputRef.current?.select();
  }, [editing]);

  const beginRename = React.useCallback((event: React.MouseEvent) => {
    event.stopPropagation();
    cancelRenameRef.current = false;
    setEditing(true);
  }, []);

  const commitRename = React.useCallback(() => {
    if (cancelRenameRef.current) {
      cancelRenameRef.current = false;
      return;
    }

    const nextTitle = draftTitle.trim();
    onRename(nextTitle || tab.title);
    setEditing(false);
  }, [draftTitle, onRename, tab.title]);

  const cancelRename = React.useCallback(() => {
    cancelRenameRef.current = true;
    setDraftTitle(tab.title);
    setEditing(false);
  }, [tab.title]);

  return (
    <div
      className={cn(
        "group grid h-7 max-w-44 shrink-0 grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-1 rounded border px-2 text-left text-[11px] font-medium transition",
        active
          ? "border-primary/30 bg-[var(--app-subtle)] text-foreground"
          : "border-transparent text-muted-foreground hover:bg-[var(--app-subtle)] hover:text-foreground",
      )}
      title={tab.title}
    >
      {editing ? (
        <input
          aria-label={`Rename ${tab.title}`}
          className="h-5 min-w-0 rounded border border-primary/30 bg-background/70 px-1 text-[11px] text-foreground outline-none"
          maxLength={48}
          onBlur={commitRename}
          onChange={(event) => setDraftTitle(event.target.value)}
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commitRename();
            }

            if (event.key === "Escape") {
              event.preventDefault();
              cancelRename();
            }
          }}
          ref={inputRef}
          value={draftTitle}
        />
      ) : (
        <button
          aria-pressed={active}
          className="min-w-0 truncate text-left outline-none"
          onClick={onClick}
          onDoubleClick={beginRename}
          title={`Switch to ${tab.title}`}
          type="button"
        >
          {tab.title}
        </button>
      )}
      <button
        aria-label={`Rename ${tab.title}`}
        className="grid size-4 place-items-center rounded text-muted-foreground opacity-0 outline-none hover:bg-background/40 hover:text-foreground group-hover:opacity-100"
        onClick={beginRename}
        title={`Rename ${tab.title}`}
        type="button"
      >
        <Pencil className="size-3" aria-hidden="true" />
      </button>
      <button
        aria-label={`Close ${tab.title}`}
        className="grid size-4 place-items-center rounded text-muted-foreground opacity-70 outline-none hover:bg-background/40 hover:text-foreground group-hover:opacity-100"
        onClick={onClose}
        title={`Close ${tab.title}`}
        type="button"
      >
        <X className="size-3" aria-hidden="true" />
      </button>
    </div>
  );
}

function createTerminalTab(index: number, kind: TerminalSessionKind = "terminal"): TerminalTab {
  const token =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`;

  return {
    id: `terminal-tab-${token}`,
    kind,
    title: `${sessionKindLabel(kind)} ${index}`,
  };
}

function sessionKindLabel(kind: TerminalSessionKind) {
  switch (kind) {
    case "terminal":
      return "Terminal";
    case "codex":
      return "Codex";
    case "claude":
      return "Claude";
  }
}

function sessionKindCommand(kind: TerminalSessionKind) {
  switch (kind) {
    case "terminal":
      return undefined;
    case "codex":
      return "codex";
    case "claude":
      return "claude";
  }
}

function clampTerminalHeight(height: number) {
  return Math.max(MIN_TERMINAL_HEIGHT, Math.min(MAX_TERMINAL_HEIGHT, height));
}
