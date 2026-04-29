import React from "react";
import { CheckCircle2, GripHorizontal, Loader2, RotateCcw, Terminal, X, XCircle } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { CommandOutput } from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";

export type BuildRunState = "running" | "success" | "error";

export type BuildLogRun = {
  canRerun?: boolean;
  command: string;
  error: string | null;
  emptyText?: string;
  finishedAt: Date | null;
  id: number;
  metadata?: { label: string; value: string }[];
  note?: string | null;
  output: CommandOutput | null;
  packageName: string;
  packagePath: string;
  runningText?: string;
  startedAt: Date;
  state: BuildRunState;
  title?: string;
  workingDirectory: string;
};

type BuildLogSheetProps = {
  bottomInset?: number;
  isOpen: boolean;
  runs: BuildLogRun[];
  onClose: () => void;
  onRerun: () => void;
};

export type BuildLogSheetController = Omit<BuildLogSheetProps, "bottomInset">;
export type BuildLogUpdateOptions = {
  open?: boolean;
  reset?: boolean;
};

const DEFAULT_SHEET_HEIGHT = 360;
const MIN_SHEET_HEIGHT = 180;
const MAX_SHEET_HEIGHT_RATIO = 0.72;
const ANSI_ESCAPE_PATTERN = /(?:\u001B|\u009B|\uFFFD)\[[0-?]*[ -/]*[@-~]/g;
const ANSI_OSC_PATTERN = /\u001B\][\s\S]*?(?:\u0007|\u001B\\)/g;

export function BuildLogSheet({
  bottomInset = 0,
  isOpen,
  onClose,
  onRerun,
  runs,
}: BuildLogSheetProps) {
  const [height, setHeight] = React.useState(DEFAULT_SHEET_HEIGHT);
  const [isResizing, setIsResizing] = React.useState(false);
  const latestRun = runs[runs.length - 1] ?? null;

  const handleResizeStart = React.useCallback((event: React.PointerEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    const startY = event.clientY;
    const startHeight = height;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "ns-resize";
    document.body.style.userSelect = "none";
    setIsResizing(true);

    const handleResizeMove = (moveEvent: PointerEvent) => {
      const availableHeight = Math.max(MIN_SHEET_HEIGHT, window.innerHeight - bottomInset - 80);
      const maxHeight = Math.min(availableHeight, window.innerHeight * MAX_SHEET_HEIGHT_RATIO);
      const nextHeight = startHeight + startY - moveEvent.clientY;

      setHeight(Math.min(maxHeight, Math.max(MIN_SHEET_HEIGHT, nextHeight)));
    };

    const handleResizeEnd = () => {
      window.removeEventListener("pointermove", handleResizeMove);
      window.removeEventListener("pointerup", handleResizeEnd);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      setIsResizing(false);
    };

    window.addEventListener("pointermove", handleResizeMove);
    window.addEventListener("pointerup", handleResizeEnd, { once: true });
  }, [bottomInset, height]);

  if (!latestRun) {
    return null;
  }

  const isRunning = runs.some((run) => run.state === "running");
  const statusLabel = buildStatusLabel(latestRun);
  const statusTone = latestRun.state === "success" ? "success" : latestRun.state === "error" ? "error" : "running";

  return (
    <section
      aria-label="Command logs"
      aria-hidden={!isOpen}
      className={cn(
        "absolute inset-x-0 z-40 grid grid-rows-[auto_minmax(0,1fr)] border-x-0 border-b-0 border-t border-[color:var(--app-border)] bg-[var(--app-panel)] shadow-[0_-18px_60px_rgba(0,0,0,0.45)] transition-transform duration-200",
        isResizing && "transition-none",
        isOpen
          ? "pointer-events-auto translate-y-0"
          : "pointer-events-none translate-y-[calc(100%+var(--build-sheet-bottom-inset))]",
      )}
      style={{
        "--build-sheet-bottom-inset": `${bottomInset}px`,
        bottom: bottomInset,
        height,
      } as React.CSSProperties}
    >
      <button
        aria-label="Resize build log sheet"
        className="absolute left-1/2 top-0 z-10 flex h-5 w-16 -translate-x-1/2 -translate-y-1/2 cursor-ns-resize items-center justify-center rounded-full border border-[color:var(--app-border)] bg-[var(--app-panel)] text-muted-foreground shadow-sm hover:text-foreground"
        onPointerDown={handleResizeStart}
        type="button"
      >
        <GripHorizontal className="size-4" aria-hidden="true" />
      </button>

      <header className="flex items-start justify-between gap-4 border-b border-[color:var(--app-border)] px-4 py-3">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <Terminal className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            <h2 className="truncate text-sm font-semibold">
              {runs.length > 1 ? "Execution logs" : latestRun.title ?? "Move build"}
            </h2>
            <StatusBadge tone={statusTone}>{statusLabel}</StatusBadge>
          </div>
          <p className="mt-1 truncate text-xs text-muted-foreground" title={latestRun.workingDirectory}>
            {latestRun.packageName} - {latestRun.workingDirectory}
          </p>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {latestRun.canRerun !== false ? (
            <Button
              className="h-8 gap-2"
              disabled={isRunning}
              onClick={onRerun}
              type="button"
              variant="outline"
            >
              <RotateCcw className="size-3.5" aria-hidden="true" />
              Rerun
            </Button>
          ) : null}
          <Button
            aria-label="Close build logs"
            className="size-8 text-muted-foreground"
            onClick={onClose}
            size="icon-sm"
            type="button"
            variant="ghost"
          >
            <X className="size-4" aria-hidden="true" />
          </Button>
        </div>
      </header>

      <div className="grid min-h-0 px-4 py-3">
        <ScrollArea className="min-h-0 rounded-md border border-[color:var(--app-border)] bg-black/25">
          <div className="grid gap-3 p-3">
            {runs.map((run, index) => (
              <LogRunGroup
                key={run.id}
                index={index}
                run={run}
              />
            ))}
          </div>
        </ScrollArea>
      </div>
    </section>
  );
}

function LogRunGroup({ index, run }: { index: number; run: BuildLogRun }) {
  const statusTone = run.state === "success" ? "success" : run.state === "error" ? "error" : "running";

  return (
    <article className="overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
      <header className="flex items-center justify-between gap-3 border-b border-[color:var(--app-border)] px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="flex size-5 shrink-0 items-center justify-center rounded bg-muted text-[11px] font-semibold text-muted-foreground">
            {index + 1}
          </span>
          <h3 className="truncate text-xs font-semibold text-foreground">{logRunTitle(run)}</h3>
        </div>
        <StatusBadge tone={statusTone}>{buildStatusLabel(run)}</StatusBadge>
      </header>

      <div className="grid gap-2 px-3 py-2">
        <div className="grid gap-1 text-xs">
          <LogMeta label="Package" value={run.packagePath} />
          <LogMeta label="Command" value={run.command} />
          {run.metadata?.map((item) => (
            <LogMeta key={`${run.id}:${item.label}:${item.value}`} label={item.label} value={item.value} />
          ))}
          <LogMeta label="Started" value={run.startedAt.toLocaleTimeString()} />
          {run.finishedAt ? <LogMeta label="Finished" value={run.finishedAt.toLocaleTimeString()} /> : null}
        </div>

        <pre className="max-h-[460px] overflow-auto rounded border border-[color:var(--app-border)] bg-black/35 p-3 font-mono text-xs leading-5 text-muted-foreground whitespace-pre-wrap break-words">
          {buildLogText(run)}
        </pre>
      </div>
    </article>
  );
}

function StatusBadge({
  children,
  tone,
}: {
  children: React.ReactNode;
  tone: "error" | "running" | "success";
}) {
  return (
    <Badge
      className={cn(
        "gap-1 rounded px-1.5 py-0.5 text-[11px] font-semibold",
        tone === "success" && "bg-emerald-500/15 text-emerald-400",
        tone === "error" && "bg-red-500/15 text-red-400",
        tone === "running" && "bg-muted text-muted-foreground",
      )}
      variant="secondary"
    >
      {tone === "success" ? <CheckCircle2 className="size-3" aria-hidden="true" /> : null}
      {tone === "error" ? <XCircle className="size-3" aria-hidden="true" /> : null}
      {tone === "running" ? <Loader2 className="size-3 animate-spin" aria-hidden="true" /> : null}
      {children}
    </Badge>
  );
}

function LogMeta({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid min-w-0 grid-cols-[5rem_minmax(0,1fr)] gap-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="min-w-0 truncate font-mono text-foreground" title={value}>
        {value}
      </span>
    </div>
  );
}

function logRunTitle(run: BuildLogRun) {
  const step = run.metadata?.find((item) => item.label === "Step")?.value;

  if (step) {
    return step;
  }

  return run.title ?? "Move build";
}

function buildStatusLabel(run: BuildLogRun) {
  if (run.state === "running") {
    return "Running";
  }

  if (run.state === "success" && run.output?.status == null) {
    return "Succeeded";
  }

  if (run.output?.status === 0) {
    return "Succeeded";
  }

  return run.output?.status == null ? "Failed" : `Failed ${run.output.status}`;
}

function buildLogText(run: BuildLogRun) {
  const lines = [
    `$ ${run.command}`,
    `cwd: ${run.workingDirectory}`,
    "",
  ];

  const stdout = sanitizeTerminalText(run.output?.stdout ?? "").trim();
  const stderr = sanitizeTerminalText(run.output?.stderr ?? "").trim();
  const note = sanitizeTerminalText(run.note ?? "").trim();
  const error = sanitizeTerminalText(run.error ?? "").trim();

  if (run.state === "running") {
    lines.push(run.runningText ?? "Running build...");

    if (stdout || stderr) {
      lines.push("");
    }
  }

  if (stdout) {
    lines.push("stdout", stdout, "");
  }

  if (stderr) {
    lines.push("stderr", stderr, "");
  }

  if (note) {
    lines.push("details", note, "");
  }

  if (error) {
    lines.push("error", error, "");
  }

  if (run.state !== "running" && !stdout && !stderr && !note && !error) {
    lines.push(run.emptyText ?? "Build finished without output.");
  }

  return lines.join("\n").trimEnd();
}

function sanitizeTerminalText(value: string) {
  return value
    .replace(ANSI_OSC_PATTERN, "")
    .replace(ANSI_ESCAPE_PATTERN, "");
}
