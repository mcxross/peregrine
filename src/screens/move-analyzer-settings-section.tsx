import { FolderOpen } from "lucide-react";
import React from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  type MoveAnalyzerAdapterSettings,
  type MoveAnalyzerAdapterSource,
  type MoveAnalyzerAdapterStatus,
} from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";

const moveAnalyzerSourceOptions: { value: MoveAnalyzerAdapterSource; label: string }[] = [
  { value: "bundled", label: "Bundled library" },
  { value: "system", label: "User installed" },
];

type MoveAnalyzerSettingsSectionProps = {
  binaryPathInput: string;
  chooseBinaryPath: () => Promise<void>;
  effectiveSource: MoveAnalyzerAdapterSource;
  error: string | null;
  isSaving: boolean;
  saveBinaryPath: (path: string) => Promise<void>;
  settings: MoveAnalyzerAdapterSettings;
  setBinaryPathInput: (path: string) => void;
  status: MoveAnalyzerAdapterStatus | null;
  updateSource: (source: MoveAnalyzerAdapterSource) => Promise<void>;
};

export function MoveAnalyzerSettingsSection({
  binaryPathInput,
  chooseBinaryPath,
  effectiveSource,
  error,
  isSaving,
  saveBinaryPath,
  settings,
  setBinaryPathInput,
  status,
  updateSource,
}: MoveAnalyzerSettingsSectionProps) {
  return (
    <section className="mb-10">
      <h2 className="mb-3 text-[13px] font-medium text-muted-foreground">Move Analyzer</h2>
      <div className="-mx-4 overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
        <SettingsRow
          label="Source"
          description={moveAnalyzerSourceLabel(effectiveSource)}
        >
          <SegmentedControl>
            {moveAnalyzerSourceOptions.map((option) => {
              const unavailableSystem =
                option.value === "system" && status ? !status.system.available : false;

              return (
                <Button
                  disabled={isSaving || unavailableSystem}
                  key={option.value}
                  onClick={() => void updateSource(option.value)}
                  size="sm"
                  title={unavailableSystem ? status?.system.error ?? "move-analyzer not found on PATH." : undefined}
                  variant={effectiveSource === option.value ? "default" : "ghost"}
                >
                  {option.label}
                </Button>
              );
            })}
          </SegmentedControl>
        </SettingsRow>

        <div className="border-t border-border/70">
          <div className="grid gap-2 px-4 py-3.5 text-xs text-muted-foreground">
            <ToolSourceStatusRow
              active={effectiveSource === "bundled"}
              label="Bundled library"
              path={status?.bundled.path ?? null}
              version={status?.bundled.version ?? null}
              available={status?.bundled.available ?? false}
            />
            <ToolSourceStatusRow
              active={effectiveSource === "system"}
              label="User installed"
              path={status?.system.path ?? null}
              version={status?.system.version ?? null}
              available={status?.system.available ?? false}
            />
          </div>
        </div>

        <div className="border-t border-border/70">
          <SettingsRow
            label="Binary path"
            description="Set a move-analyzer binary path instead of the bundled library."
            align="start"
          >
            <div className="grid w-full min-w-0 gap-2 sm:w-[22rem]">
              <Input
                autoComplete="off"
                id="move-analyzer-binary-path"
                onChange={(event) => setBinaryPathInput(event.target.value)}
                placeholder="Use bundled library or PATH"
                type="text"
                value={binaryPathInput}
              />
              <div className="flex justify-end gap-2">
                <Button
                  disabled={isSaving}
                  onClick={() => void chooseBinaryPath()}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  <FolderOpen aria-hidden="true" />
                  Browse
                </Button>
                <Button
                  disabled={isSaving || binaryPathInput === (settings.binaryPath ?? "")}
                  onClick={() => void saveBinaryPath(binaryPathInput)}
                  size="sm"
                  type="button"
                >
                  Save
                </Button>
              </div>
            </div>
          </SettingsRow>
        </div>

        {error ? (
          <div className="border-t border-border/70 px-4 py-3.5">
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </p>
          </div>
        ) : null}
      </div>
    </section>
  );
}

function SettingsRow({
  align = "center",
  children,
  description,
  label,
}: {
  align?: "center" | "start";
  children: React.ReactNode;
  description?: string;
  label: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col justify-between gap-3 px-4 py-3.5 sm:flex-row",
        align === "center" ? "sm:items-center" : "sm:items-start",
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-foreground">{label}</div>
        {description ? (
          <div className="mt-0.5 text-[13px] text-muted-foreground">{description}</div>
        ) : null}
      </div>
      <div className="flex min-w-0 shrink-0 items-center justify-end">{children}</div>
    </div>
  );
}

function SegmentedControl({ children }: { children: React.ReactNode }) {
  return (
    <div className="grid grid-flow-col auto-cols-fr gap-1 rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] p-1">
      {children}
    </div>
  );
}

function ToolSourceStatusRow({
  active,
  available,
  label,
  path,
  version,
}: {
  active: boolean;
  available: boolean;
  label: string;
  path: string | null;
  version: string | null;
}) {
  const stateLabel = active
    ? version
      ? `Active v${version}`
      : available
        ? "Active"
        : "Selected"
    : version
      ? `v${version}`
      : "Idle";

  return (
    <div className="grid min-w-0 gap-1 rounded border bg-card px-2 py-1.5">
      <div className="flex min-w-0 items-center justify-between gap-2">
        <span className="min-w-0 truncate">{label}</span>
        <div className="flex min-w-0 items-center gap-2">
          <Badge
            className={cn(
              "rounded px-1.5 py-0 text-[10px]",
              available
                ? "bg-emerald-500/15 text-emerald-400"
                : "bg-amber-500/15 text-amber-400",
            )}
            variant="secondary"
          >
            {available ? "Ready" : "Missing"}
          </Badge>
          <span className="max-w-[8rem] truncate text-right">{stateLabel}</span>
        </div>
      </div>
      {path ? (
        <p className="truncate font-mono text-[11px] text-muted-foreground">{path}</p>
      ) : null}
    </div>
  );
}

function moveAnalyzerSourceLabel(source: MoveAnalyzerAdapterSource) {
  return source === "bundled" ? "Bundled library" : "User installed";
}
