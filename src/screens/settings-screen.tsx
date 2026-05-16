import React from "react";
import { FolderOpen } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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

  return (
    <main className="h-full min-h-0 overflow-auto bg-background text-foreground">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-5 py-5 sm:px-8 sm:py-7">
        <header className="flex items-center justify-between gap-4 border-b pb-5">
          <div>
            <h1 className="text-xl font-semibold tracking-tight">Settings</h1>
            <p className="text-sm text-muted-foreground">
              Appearance · {resolvedMode}
            </p>
          </div>
          <Button variant="outline" onClick={onBack}>
            Done
          </Button>
        </header>

        <section className="grid gap-4 md:grid-cols-[240px_1fr]">
          <div className="grid h-fit gap-4">
            <Card>
              <CardHeader>
                <CardTitle>Appearance</CardTitle>
                <CardDescription>Mode</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="grid grid-cols-3 gap-1 rounded-lg border bg-muted p-1 md:grid-cols-1">
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
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Sui CLI</CardTitle>
                <CardDescription>{suiSourceLabel(effectiveSuiSource)}</CardDescription>
              </CardHeader>
              <CardContent className="grid gap-3">
                <div className="grid grid-cols-2 gap-1 rounded-lg border bg-muted p-1 md:grid-cols-1">
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
                </div>

                <div className="grid gap-2 text-xs text-muted-foreground">
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
                  <div className="grid gap-2 rounded border bg-card p-2">
                    <label className="text-xs font-medium text-foreground" htmlFor="sui-cli-path">
                      Sui CLI path
                    </label>
                    <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto_auto]">
                      <Input
                        autoComplete="off"
                        id="sui-cli-path"
                        onChange={(event) => setSuiCliPathInput(event.target.value)}
                        placeholder="Use bundled crate or PATH"
                        type="text"
                        value={suiCliPathInput}
                      />
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
                    <p>
                      Set a CLI path to make project creation and package commands use that binary instead of the embedded toolchain.
                    </p>
                  </div>
                  {suiSettingsError ? (
                    <p className="rounded border border-destructive/30 bg-destructive/10 px-2 py-1 text-destructive">
                      {suiSettingsError}
                    </p>
                  ) : null}
                </div>
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Themes</CardTitle>
              <CardDescription>shadcn families</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                {themes.map((theme) => (
                  <button
                    key={theme.id}
                    type="button"
                    onClick={() => setThemeId(theme.id as ThemeId)}
                    className={cn(
                      "group rounded-lg border bg-card p-3 text-left text-card-foreground transition hover:border-ring hover:shadow-sm focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
                      themeId === theme.id && "border-ring ring-[3px] ring-ring/20",
                    )}
                  >
                    <div className="mb-3 flex items-center justify-between gap-2">
                      <div>
                        <div className="font-medium leading-none">{theme.name}</div>
                        <div className="mt-1 text-xs capitalize text-muted-foreground">
                          {theme.family}
                        </div>
                      </div>
                      <span
                        className="size-5 rounded-full border shadow-xs"
                        style={{ background: theme.swatch }}
                      />
                    </div>

                    <div className="overflow-hidden rounded-md border">
                      <div className="flex h-9 items-center gap-1.5 px-2">
                        <span className="h-2.5 w-10 rounded-full" style={{ background: theme.light.primary }} />
                        <span className="h-2.5 w-6 rounded-full" style={{ background: theme.light.accent }} />
                        <span className="h-2.5 w-4 rounded-full" style={{ background: theme.light.border }} />
                      </div>
                      <div className="flex h-9 items-center gap-1.5 border-t px-2">
                        <span className="h-2.5 w-10 rounded-full" style={{ background: theme.dark.primary }} />
                        <span className="h-2.5 w-6 rounded-full" style={{ background: theme.dark.accent }} />
                        <span className="h-2.5 w-4 rounded-full" style={{ background: theme.dark.border }} />
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            </CardContent>
          </Card>
        </section>
      </div>
    </main>
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
