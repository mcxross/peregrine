import React from "react";
import {
  ArrowLeft,
  Check,
  ChevronDown,
  FolderOpen,
  Palette,
  TerminalSquare,
  type LucideIcon,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  checkSuiAdapter,
  getSuiAdapterSettings,
  saveSuiAdapterSettings,
  type SuiAdapterSettings,
  type SuiAdapterSource,
  type SuiAdapterStatus,
} from "@/features/empty-project/filesystem-tree";
import { cn } from "@/lib/utils";
import { useTheme } from "@/theme/theme-provider";
import type { ThemeId, ThemeMode } from "@/theme/themes";

type SettingsScreenProps = {
  onBack: () => void;
};

type SettingsGroupId = "appearance" | "toolchain";

const settingsGroups: {
  id: SettingsGroupId;
  label: string;
  description: string;
  icon: LucideIcon;
}[] = [
  {
    id: "appearance",
    label: "Appearance",
    description: "Mode and theme",
    icon: Palette,
  },
  {
    id: "toolchain",
    label: "Sui CLI",
    description: "Move toolchain",
    icon: TerminalSquare,
  },
];

const modeOptions: { value: ThemeMode; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

const suiSourceOptions: { value: SuiAdapterSource; label: string }[] = [
  { value: "bundled", label: "Bundled crate" },
  { value: "system", label: "User installed" },
];

export function SettingsScreen({ onBack }: SettingsScreenProps) {
  const { themes, themeId, mode, resolvedMode, setMode, setThemeId } = useTheme();
  const [activeGroup, setActiveGroup] = React.useState<SettingsGroupId>("appearance");
  const [suiSettings, setSuiSettings] = React.useState<SuiAdapterSettings>({
    cliPath: null,
    source: "bundled",
  });
  const [suiCliPathInput, setSuiCliPathInput] = React.useState("");
  const [suiStatus, setSuiStatus] = React.useState<SuiAdapterStatus | null>(null);
  const [suiSettingsError, setSuiSettingsError] = React.useState<string | null>(null);
  const [isSavingSuiSettings, setIsSavingSuiSettings] = React.useState(false);
  const effectiveSuiSource = suiSettings.cliPath?.trim() ? "system" : suiSettings.source;

  React.useEffect(() => {
    let isMounted = true;

    Promise.all([getSuiAdapterSettings(), checkSuiAdapter()])
      .then(([settings, status]) => {
        if (!isMounted) {
          return;
        }

        setSuiSettings(settings);
        setSuiCliPathInput(settings.cliPath ?? "");
        setSuiStatus(status);
        setSuiSettingsError(null);
      })
      .catch((error) => {
        if (isMounted) {
          setSuiSettingsError(getSettingsErrorMessage(error));
        }
      });

    return () => {
      isMounted = false;
    };
  }, []);

  const updateSuiSource = React.useCallback(
    async (source: SuiAdapterSource) => {
      if (source === effectiveSuiSource || isSavingSuiSettings) {
        return;
      }

      const nextSettings: SuiAdapterSettings = {
        ...suiSettings,
        cliPath: source === "bundled" ? null : suiSettings.cliPath ?? null,
        source,
      };
      setSuiSettings(nextSettings);
      setIsSavingSuiSettings(true);
      setSuiSettingsError(null);

      try {
        const savedSettings = await saveSuiAdapterSettings(nextSettings);
        const status = await checkSuiAdapter();

        setSuiSettings(savedSettings);
        setSuiCliPathInput(savedSettings.cliPath ?? "");
        setSuiStatus(status);
      } catch (error) {
        setSuiSettingsError(getSettingsErrorMessage(error));
      } finally {
        setIsSavingSuiSettings(false);
      }
    },
    [effectiveSuiSource, isSavingSuiSettings, suiSettings],
  );
  const saveSuiCliPath = React.useCallback(
    async (path: string) => {
      if (isSavingSuiSettings) {
        return;
      }

      const cliPath = path.trim() || null;
      const nextSettings: SuiAdapterSettings = {
        ...suiSettings,
        cliPath,
        source: cliPath ? "system" : suiSettings.source,
      };

      setSuiSettings(nextSettings);
      setIsSavingSuiSettings(true);
      setSuiSettingsError(null);

      try {
        const savedSettings = await saveSuiAdapterSettings(nextSettings);
        const status = await checkSuiAdapter();

        setSuiSettings(savedSettings);
        setSuiCliPathInput(savedSettings.cliPath ?? "");
        setSuiStatus(status);
      } catch (error) {
        setSuiSettingsError(getSettingsErrorMessage(error));
      } finally {
        setIsSavingSuiSettings(false);
      }
    },
    [isSavingSuiSettings, suiSettings],
  );
  const chooseSuiCliPath = React.useCallback(async () => {
    const selectedPath = await openSuiCliPath();

    if (!selectedPath) {
      return;
    }

    setSuiCliPathInput(selectedPath);
    await saveSuiCliPath(selectedPath);
  }, [saveSuiCliPath]);
  const activeSettingsGroup = settingsGroups.find((group) => group.id === activeGroup) ?? settingsGroups[0];
  const ActiveGroupIcon = activeSettingsGroup.icon;

  return (
    <main className="grid h-full min-h-0 bg-background text-foreground lg:grid-cols-[260px_minmax(0,1fr)]">
      <aside className="flex min-h-0 flex-col border-b border-[color:var(--app-border)] bg-[var(--app-panel)] px-4 py-4 lg:border-b-0 lg:border-r">
        <Button
          className="mb-4 w-fit justify-start px-2 text-muted-foreground hover:text-foreground"
          onClick={onBack}
          size="sm"
          type="button"
          variant="ghost"
        >
          <ArrowLeft aria-hidden="true" />
          Back to app
        </Button>

        <nav className="flex gap-1 overflow-x-auto lg:grid lg:overflow-visible" aria-label="Settings sections">
          {settingsGroups.map((group) => (
            <SettingsNavButton
              active={activeGroup === group.id}
              group={group}
              key={group.id}
              onClick={() => setActiveGroup(group.id)}
            />
          ))}
        </nav>
      </aside>

      <section className="min-h-0 overflow-auto">
        <div className="mx-auto flex w-full max-w-4xl flex-col px-6 pb-24 pt-10 sm:px-8 lg:px-12 lg:pt-20">
          <header className="mb-10">
            <div className="mb-3 flex size-10 items-center justify-center rounded-lg border border-[color:var(--app-border)] bg-[var(--app-surface)] text-muted-foreground">
              <ActiveGroupIcon className="size-5" aria-hidden="true" />
            </div>
            <h1 className="text-2xl font-semibold tracking-tight">{activeSettingsGroup.label}</h1>
            <p className="mt-1 text-[13px] text-muted-foreground">{activeSettingsGroup.description}</p>
          </header>

          {activeGroup === "appearance" ? (
            <AppearanceSettings
              mode={mode}
              resolvedMode={resolvedMode}
              setMode={setMode}
              setThemeId={setThemeId}
              themeId={themeId}
              themes={themes}
            />
          ) : null}

          {activeGroup === "toolchain" ? (
            <ToolchainSettings
              chooseSuiCliPath={chooseSuiCliPath}
              effectiveSuiSource={effectiveSuiSource}
              isSavingSuiSettings={isSavingSuiSettings}
              saveSuiCliPath={saveSuiCliPath}
              suiCliPathInput={suiCliPathInput}
              suiSettings={suiSettings}
              suiSettingsError={suiSettingsError}
              suiStatus={suiStatus}
              updateSuiSource={updateSuiSource}
              setSuiCliPathInput={setSuiCliPathInput}
            />
          ) : null}
        </div>
      </section>
    </main>
  );
}

function AppearanceSettings({
  mode,
  resolvedMode,
  setMode,
  setThemeId,
  themeId,
  themes,
}: {
  mode: ThemeMode;
  resolvedMode: ThemeMode;
  setMode: (mode: ThemeMode) => void;
  setThemeId: (themeId: ThemeId) => void;
  themeId: string;
  themes: ReturnType<typeof useTheme>["themes"];
}) {
  const selectedTheme = themes.find((theme) => theme.id === themeId) ?? themes[0];

  return (
    <>
      <SettingsSection title="Mode">
        <SettingsRow
          label="Color mode"
          description={`Currently rendering ${resolvedMode} surfaces.`}
        >
          <SegmentedControl>
            {modeOptions.map((option) => (
              <Button
                key={option.value}
                variant={mode === option.value ? "default" : "ghost"}
                size="sm"
                onClick={() => setMode(option.value)}
              >
                {option.label}
              </Button>
            ))}
          </SegmentedControl>
        </SettingsRow>
      </SettingsSection>

      <SettingsSection title="Themes">
        <SettingsRow
          label="Theme"
          description="Choose the base color family used across the app."
          align="start"
        >
          <ThemeDropdown
            selectedTheme={selectedTheme}
            setThemeId={setThemeId}
            themeId={themeId}
            themes={themes}
          />
        </SettingsRow>
      </SettingsSection>
    </>
  );
}

function ThemeDropdown({
  selectedTheme,
  setThemeId,
  themeId,
  themes,
}: {
  selectedTheme: ReturnType<typeof useTheme>["themes"][number];
  setThemeId: (themeId: ThemeId) => void;
  themeId: string;
  themes: ReturnType<typeof useTheme>["themes"];
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          className="h-auto w-full min-w-0 justify-between rounded-xl px-3 py-2 sm:w-[28rem]"
          type="button"
          variant="outline"
        >
          <ThemeSelectSummary theme={selectedTheme} />
          <ChevronDown className="size-4 text-muted-foreground" aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="max-h-[min(28rem,var(--radix-dropdown-menu-content-available-height))] w-[min(32rem,calc(100vw-2rem))] p-1.5"
      >
        {themes.map((theme) => (
          <DropdownMenuItem
            className="grid cursor-default grid-cols-[minmax(0,1fr)_auto] gap-3 rounded-lg p-2"
            key={theme.id}
            onSelect={() => setThemeId(theme.id as ThemeId)}
          >
            <ThemeSelectSummary theme={theme} />
            <span
              className={cn(
                "mt-1 flex size-5 items-center justify-center rounded-full border text-primary",
                themeId === theme.id
                  ? "border-primary/50 bg-primary/10"
                  : "border-border text-transparent",
              )}
              aria-hidden="true"
            >
              <Check className="size-3" />
            </span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function ThemeSelectSummary({
  theme,
}: {
  theme: ReturnType<typeof useTheme>["themes"][number];
}) {
  return (
    <span className="grid min-w-0 flex-1 gap-2 text-left">
      <span className="flex min-w-0 items-center gap-3">
        <span
          className="flex size-8 shrink-0 items-center justify-center rounded-lg border bg-card text-sm font-semibold shadow-xs"
          style={{ color: theme.swatch }}
        >
          Aa
        </span>
        <span className="min-w-0">
          <span className="block truncate text-sm font-medium text-foreground">
            {theme.name}
          </span>
          <span className="block text-xs capitalize text-muted-foreground">
            {theme.family}
          </span>
        </span>
      </span>
      <span className="grid min-w-0 gap-2 sm:grid-cols-2">
        <ThemePreviewStrip label="Light" tokens={theme.light} />
        <ThemePreviewStrip label="Dark" tokens={theme.dark} />
      </span>
    </span>
  );
}

function ThemePreviewStrip({
  label,
  tokens,
}: {
  label: string;
  tokens: ReturnType<typeof useTheme>["themes"][number]["light"];
}) {
  return (
    <div
      className="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-2 rounded-md border px-2 py-1.5"
      style={{
        background: tokens.background,
        borderColor: tokens.border,
        color: tokens.foreground,
      }}
    >
      <span className="text-[10px] font-medium uppercase tracking-normal opacity-70">
        {label}
      </span>
      <span className="flex min-w-0 items-center justify-end gap-1.5">
        <ThemePreviewPill color={tokens.primary} width="2.25rem" />
        <ThemePreviewPill color={tokens.accent} width="1.5rem" />
        <ThemePreviewPill color={tokens.muted} width="1rem" />
      </span>
    </div>
  );
}

function ThemePreviewPill({
  color,
  width,
}: {
  color: string;
  width: string;
}) {
  return (
    <span
      className="h-2 rounded-full border border-black/5"
      style={{ background: color, width }}
    />
  );
}

function ToolchainSettings({
  chooseSuiCliPath,
  effectiveSuiSource,
  isSavingSuiSettings,
  saveSuiCliPath,
  setSuiCliPathInput,
  suiCliPathInput,
  suiSettings,
  suiSettingsError,
  suiStatus,
  updateSuiSource,
}: {
  chooseSuiCliPath: () => Promise<void>;
  effectiveSuiSource: SuiAdapterSource;
  isSavingSuiSettings: boolean;
  saveSuiCliPath: (path: string) => Promise<void>;
  setSuiCliPathInput: (path: string) => void;
  suiCliPathInput: string;
  suiSettings: SuiAdapterSettings;
  suiSettingsError: string | null;
  suiStatus: SuiAdapterStatus | null;
  updateSuiSource: (source: SuiAdapterSource) => Promise<void>;
}) {
  return (
    <SettingsSection title="Sui CLI">
      <SettingsRow
        label="Source"
        description={suiSourceLabel(effectiveSuiSource)}
      >
        <SegmentedControl>
          {suiSourceOptions.map((option) => {
            const unavailableSystem =
              option.value === "system" && suiStatus ? !suiStatus.system.available : false;

            return (
              <Button
                disabled={isSavingSuiSettings || unavailableSystem}
                key={option.value}
                onClick={() => void updateSuiSource(option.value)}
                size="sm"
                title={unavailableSystem ? suiStatus?.system.error ?? "Sui CLI not found on PATH." : undefined}
                variant={effectiveSuiSource === option.value ? "default" : "ghost"}
              >
                {option.label}
              </Button>
            );
          })}
        </SegmentedControl>
      </SettingsRow>

      <div className="border-t border-border/70">
        <div className="grid gap-2 px-4 py-3.5 text-xs text-muted-foreground">
          <SuiSourceStatusRow
            active={effectiveSuiSource === "bundled"}
            label="Bundled crate"
            path={suiStatus?.bundled.path ?? null}
            version={suiStatus?.bundled.version ?? null}
            available={suiStatus?.bundled.available ?? false}
          />
          <SuiSourceStatusRow
            active={effectiveSuiSource === "system"}
            label="User installed"
            path={suiStatus?.system.path ?? null}
            version={suiStatus?.system.version ?? null}
            available={suiStatus?.system.available ?? false}
          />
        </div>
      </div>

      <div className="border-t border-border/70">
        <SettingsRow
          label="CLI path"
          description="Set a binary path instead of the embedded toolchain."
          align="start"
        >
          <div className="grid w-full min-w-0 gap-2 sm:w-[22rem]">
            <Input
              autoComplete="off"
              id="sui-cli-path"
              onChange={(event) => setSuiCliPathInput(event.target.value)}
              placeholder="Use bundled crate or PATH"
              type="text"
              value={suiCliPathInput}
            />
            <div className="flex justify-end gap-2">
              <Button
                disabled={isSavingSuiSettings}
                onClick={() => void chooseSuiCliPath()}
                size="sm"
                type="button"
                variant="outline"
              >
                <FolderOpen aria-hidden="true" />
                Browse
              </Button>
              <Button
                disabled={isSavingSuiSettings || suiCliPathInput === (suiSettings.cliPath ?? "")}
                onClick={() => void saveSuiCliPath(suiCliPathInput)}
                size="sm"
                type="button"
              >
                Save
              </Button>
            </div>
          </div>
        </SettingsRow>
      </div>

      {suiSettingsError ? (
        <div className="border-t border-border/70 px-4 py-3.5">
          <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {suiSettingsError}
          </p>
        </div>
      ) : null}
    </SettingsSection>
  );
}

function SettingsNavButton({
  active,
  group,
  onClick,
}: {
  active: boolean;
  group: (typeof settingsGroups)[number];
  onClick: () => void;
}) {
  const Icon = group.icon;

  return (
    <button
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex min-w-[12rem] items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm transition lg:min-w-0",
        active
          ? "bg-accent text-foreground"
          : "text-muted-foreground hover:bg-accent/55 hover:text-foreground",
      )}
      onClick={onClick}
      type="button"
    >
      <Icon className="size-4 shrink-0" aria-hidden="true" />
      <span className="min-w-0">
        <span className="block truncate font-medium">{group.label}</span>
        <span className="block truncate text-xs opacity-75">{group.description}</span>
      </span>
    </button>
  );
}

function SettingsSection({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="mb-10">
      <h2 className="mb-3 text-[13px] font-medium text-muted-foreground">{title}</h2>
      <div className="-mx-4 overflow-hidden rounded-2xl border border-border/70 bg-card">
        {children}
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
    <div className="grid grid-flow-col auto-cols-fr gap-1 rounded-lg border bg-muted p-1">
      {children}
    </div>
  );
}

function SuiSourceStatusRow({
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
          <span className="max-w-[8rem] truncate text-right">
            {stateLabel}
          </span>
        </div>
      </div>
      {path ? (
        <p className="truncate font-mono text-[11px] text-muted-foreground">{path}</p>
      ) : null}
    </div>
  );
}

async function openSuiCliPath(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");

  const selectedPath = await open({
    directory: false,
    multiple: false,
    title: "Select Sui CLI",
  });

  return typeof selectedPath === "string" ? selectedPath : null;
}

function suiSourceLabel(source: SuiAdapterSource) {
  return source === "bundled" ? "Bundled crate" : "User installed";
}

function getSettingsErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : "Could not update Sui CLI settings.";
}
