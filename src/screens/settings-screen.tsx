import React from "react";
import {
  ArrowLeft,
  Check,
  ChevronDown,
  Copy,
  Download,
  Eye,
  FolderOpen,
  KeyRound,
  MoreHorizontal,
  Palette,
  Pencil,
  Plus,
  Puzzle,
  RefreshCw,
  TerminalSquare,
  Trash2,
  Upload,
  type LucideIcon,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  checkSuiAdapter,
  exportSuiPrivateKey,
  generateSuiKey,
  getSuiAdapterSettings,
  importSuiKey,
  installAnalyzerPlugin,
  listAnalyzerPlugins,
  listAnalyzerRuleCatalog,
  loadSuiKeyState,
  removeAnalyzerPlugin,
  removeSuiKey,
  renameSuiKeyAlias,
  saveAnalysisRuleConfig,
  saveSuiAdapterSettings,
  setAnalyzerPluginEnabled,
  setActiveSuiAddress,
  type AnalysisRuleCatalog,
  type AnalysisRuleMetadata,
  type AnalysisSeverity,
  type InstalledAnalyzerPlugin,
  type MovePackage,
  type PackageTree,
  type SuiAdapterSettings,
  type SuiAdapterSource,
  type SuiAdapterStatus,
  type SuiGenerateKeyResponse,
  type SuiKeyAccount,
  type SuiKeyState,
} from "@peregrine/desktop-runtime";
import {
  checkSuiMoveAnalyzerAdapter,
  getSuiMoveAnalyzerSettings,
  saveSuiMoveAnalyzerSettings,
  type MoveAnalyzerAdapterSettings,
  type MoveAnalyzerAdapterSource,
  type MoveAnalyzerAdapterStatus,
} from "@peregrine/desktop-runtime";
import { cn } from "@/lib/utils";
import { MoveAnalyzerSettingsSection } from "@/screens/move-analyzer-settings-section";
import { useTheme } from "@/theme/theme-provider";
import type { ThemeId, ThemeMode } from "@/theme/themes";

type SettingsScreenProps = {
  activeMovePackage?: MovePackage | null;
  onBack: () => void;
  packageTree?: PackageTree | null;
};

type SettingsGroupId = "appearance" | "toolchain" | "analyzers";

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
    label: "Toolchain",
    description: "Sui and Move Analyzer",
    icon: TerminalSquare,
  },
  {
    id: "analyzers",
    label: "Analyzers",
    description: "Rules and plugins",
    icon: Puzzle,
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

export function SettingsScreen({ activeMovePackage = null, onBack, packageTree = null }: SettingsScreenProps) {
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
  const [moveAnalyzerSettings, setMoveAnalyzerSettings] = React.useState<MoveAnalyzerAdapterSettings>({
    binaryPath: null,
    source: "bundled",
  });
  const [moveAnalyzerBinaryPathInput, setMoveAnalyzerBinaryPathInput] = React.useState("");
  const [moveAnalyzerStatus, setMoveAnalyzerStatus] = React.useState<MoveAnalyzerAdapterStatus | null>(null);
  const [moveAnalyzerSettingsError, setMoveAnalyzerSettingsError] = React.useState<string | null>(null);
  const [isSavingMoveAnalyzerSettings, setIsSavingMoveAnalyzerSettings] = React.useState(false);
  const [suiKeyState, setSuiKeyState] = React.useState<SuiKeyState | null>(null);
  const [suiKeyError, setSuiKeyError] = React.useState<string | null>(null);
  const [isLoadingSuiKeys, setIsLoadingSuiKeys] = React.useState(false);
  const [analyzerPlugins, setAnalyzerPlugins] = React.useState<InstalledAnalyzerPlugin[]>([]);
  const [analyzerCatalog, setAnalyzerCatalog] = React.useState<AnalysisRuleCatalog | null>(null);
  const [analyzerError, setAnalyzerError] = React.useState<string | null>(null);
  const [isLoadingAnalyzers, setIsLoadingAnalyzers] = React.useState(false);
  const effectiveSuiSource = suiSettings.cliPath?.trim() ? "system" : suiSettings.source;
  const effectiveMoveAnalyzerSource = moveAnalyzerSettings.binaryPath?.trim()
    ? "system"
    : moveAnalyzerSettings.source;
  const activePackagePath = activeMovePackage?.path ?? null;

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

  React.useEffect(() => {
    let isMounted = true;

    Promise.all([getSuiMoveAnalyzerSettings(), checkSuiMoveAnalyzerAdapter()])
      .then(([settings, status]) => {
        if (!isMounted) {
          return;
        }

        setMoveAnalyzerSettings(settings);
        setMoveAnalyzerBinaryPathInput(settings.binaryPath ?? "");
        setMoveAnalyzerStatus(status);
        setMoveAnalyzerSettingsError(null);
      })
      .catch((error) => {
        if (isMounted) {
          setMoveAnalyzerSettingsError(getSettingsErrorMessage(error));
        }
      });

    return () => {
      isMounted = false;
    };
  }, []);

  const refreshSuiKeys = React.useCallback(async () => {
    setIsLoadingSuiKeys(true);
    setSuiKeyError(null);

    try {
      setSuiKeyState(await loadSuiKeyState());
    } catch (error) {
      setSuiKeyError(getSettingsErrorMessage(error));
    } finally {
      setIsLoadingSuiKeys(false);
    }
  }, []);

  React.useEffect(() => {
    if (activeGroup !== "toolchain") {
      return;
    }

    void refreshSuiKeys();
  }, [activeGroup, refreshSuiKeys]);

  const refreshAnalyzers = React.useCallback(async () => {
    setIsLoadingAnalyzers(true);
    setAnalyzerError(null);

    try {
      const [plugins, catalog] = await Promise.all([
        listAnalyzerPlugins(),
        listAnalyzerRuleCatalog(packageTree, activePackagePath),
      ]);

      setAnalyzerPlugins(plugins);
      setAnalyzerCatalog(catalog);
    } catch (error) {
      setAnalyzerError(getSettingsErrorMessage(error));
    } finally {
      setIsLoadingAnalyzers(false);
    }
  }, [activePackagePath, packageTree]);

  React.useEffect(() => {
    if (activeGroup !== "analyzers") {
      return;
    }

    void refreshAnalyzers();
  }, [activeGroup, refreshAnalyzers]);

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

  const updateMoveAnalyzerSource = React.useCallback(
    async (source: MoveAnalyzerAdapterSource) => {
      if (source === effectiveMoveAnalyzerSource || isSavingMoveAnalyzerSettings) {
        return;
      }

      const nextSettings: MoveAnalyzerAdapterSettings = {
        ...moveAnalyzerSettings,
        binaryPath: source === "bundled" ? null : moveAnalyzerSettings.binaryPath ?? null,
        source,
      };
      setMoveAnalyzerSettings(nextSettings);
      setIsSavingMoveAnalyzerSettings(true);
      setMoveAnalyzerSettingsError(null);

      try {
        const savedSettings = await saveSuiMoveAnalyzerSettings(nextSettings);
        const status = await checkSuiMoveAnalyzerAdapter();

        setMoveAnalyzerSettings(savedSettings);
        setMoveAnalyzerBinaryPathInput(savedSettings.binaryPath ?? "");
        setMoveAnalyzerStatus(status);
      } catch (error) {
        setMoveAnalyzerSettingsError(getSettingsErrorMessage(error));
      } finally {
        setIsSavingMoveAnalyzerSettings(false);
      }
    },
    [effectiveMoveAnalyzerSource, isSavingMoveAnalyzerSettings, moveAnalyzerSettings],
  );

  const saveMoveAnalyzerBinaryPath = React.useCallback(
    async (path: string) => {
      if (isSavingMoveAnalyzerSettings) {
        return;
      }

      const binaryPath = path.trim() || null;
      const nextSettings: MoveAnalyzerAdapterSettings = {
        ...moveAnalyzerSettings,
        binaryPath,
        source: binaryPath ? "system" : moveAnalyzerSettings.source,
      };

      setMoveAnalyzerSettings(nextSettings);
      setIsSavingMoveAnalyzerSettings(true);
      setMoveAnalyzerSettingsError(null);

      try {
        const savedSettings = await saveSuiMoveAnalyzerSettings(nextSettings);
        const status = await checkSuiMoveAnalyzerAdapter();

        setMoveAnalyzerSettings(savedSettings);
        setMoveAnalyzerBinaryPathInput(savedSettings.binaryPath ?? "");
        setMoveAnalyzerStatus(status);
      } catch (error) {
        setMoveAnalyzerSettingsError(getSettingsErrorMessage(error));
      } finally {
        setIsSavingMoveAnalyzerSettings(false);
      }
    },
    [isSavingMoveAnalyzerSettings, moveAnalyzerSettings],
  );

  const chooseMoveAnalyzerBinaryPath = React.useCallback(async () => {
    const selectedPath = await openMoveAnalyzerBinaryPath();

    if (!selectedPath) {
      return;
    }

    setMoveAnalyzerBinaryPathInput(selectedPath);
    await saveMoveAnalyzerBinaryPath(selectedPath);
  }, [saveMoveAnalyzerBinaryPath]);
  const chooseAnalyzerPlugin = React.useCallback(async () => {
    const selectedPath = await openAnalyzerPluginPath();

    if (!selectedPath) {
      return;
    }

    setIsLoadingAnalyzers(true);
    setAnalyzerError(null);

    try {
      await installAnalyzerPlugin(selectedPath);
      await refreshAnalyzers();
    } catch (error) {
      setAnalyzerError(getSettingsErrorMessage(error));
    } finally {
      setIsLoadingAnalyzers(false);
    }
  }, [refreshAnalyzers]);
  const toggleAnalyzerPlugin = React.useCallback(
    async (pluginId: string, enabled: boolean) => {
      setIsLoadingAnalyzers(true);
      setAnalyzerError(null);

      try {
        await setAnalyzerPluginEnabled(pluginId, enabled);
        await refreshAnalyzers();
      } catch (error) {
        setAnalyzerError(getSettingsErrorMessage(error));
      } finally {
        setIsLoadingAnalyzers(false);
      }
    },
    [refreshAnalyzers],
  );
  const removeAnalyzer = React.useCallback(
    async (pluginId: string) => {
      setIsLoadingAnalyzers(true);
      setAnalyzerError(null);

      try {
        await removeAnalyzerPlugin(pluginId);
        await refreshAnalyzers();
      } catch (error) {
        setAnalyzerError(getSettingsErrorMessage(error));
      } finally {
        setIsLoadingAnalyzers(false);
      }
    },
    [refreshAnalyzers],
  );
  const saveAnalyzerRulePatch = React.useCallback(
    async (
      rulesetId: string,
      ruleId: string | null,
      patch: {
        active?: boolean | null;
        severity?: AnalysisSeverity | null;
        threshold?: number | null;
        entryThreshold?: number | null;
      },
    ) => {
      if (!packageTree || !activePackagePath) {
        setAnalyzerError("Open a Move package before editing rule configuration.");
        return;
      }

      setIsLoadingAnalyzers(true);
      setAnalyzerError(null);

      try {
        await saveAnalysisRuleConfig(packageTree, activePackagePath, {
          ...patch,
          ruleId,
          rulesetId,
        });
        await refreshAnalyzers();
      } catch (error) {
        setAnalyzerError(getSettingsErrorMessage(error));
      } finally {
        setIsLoadingAnalyzers(false);
      }
    },
    [activePackagePath, packageTree, refreshAnalyzers],
  );
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
            <div className="mb-3 flex size-10 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] text-muted-foreground">
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
              chooseMoveAnalyzerBinaryPath={chooseMoveAnalyzerBinaryPath}
              chooseSuiCliPath={chooseSuiCliPath}
              effectiveMoveAnalyzerSource={effectiveMoveAnalyzerSource}
              effectiveSuiSource={effectiveSuiSource}
              isSavingMoveAnalyzerSettings={isSavingMoveAnalyzerSettings}
              isSavingSuiSettings={isSavingSuiSettings}
              moveAnalyzerBinaryPathInput={moveAnalyzerBinaryPathInput}
              moveAnalyzerSettings={moveAnalyzerSettings}
              moveAnalyzerSettingsError={moveAnalyzerSettingsError}
              moveAnalyzerStatus={moveAnalyzerStatus}
              saveMoveAnalyzerBinaryPath={saveMoveAnalyzerBinaryPath}
              saveSuiCliPath={saveSuiCliPath}
              setMoveAnalyzerBinaryPathInput={setMoveAnalyzerBinaryPathInput}
              suiCliPathInput={suiCliPathInput}
              isLoadingSuiKeys={isLoadingSuiKeys}
              refreshSuiKeys={refreshSuiKeys}
              setSuiKeyError={setSuiKeyError}
              setSuiKeyState={setSuiKeyState}
              suiKeyError={suiKeyError}
              suiKeyState={suiKeyState}
              suiSettings={suiSettings}
              suiSettingsError={suiSettingsError}
              suiStatus={suiStatus}
              updateSuiSource={updateSuiSource}
              updateMoveAnalyzerSource={updateMoveAnalyzerSource}
              setSuiCliPathInput={setSuiCliPathInput}
            />
          ) : null}

          {activeGroup === "analyzers" ? (
            <AnalyzerSettings
              activePackageName={activeMovePackage?.name ?? null}
              analyzerCatalog={analyzerCatalog}
              analyzerError={analyzerError}
              analyzerPlugins={analyzerPlugins}
              chooseAnalyzerPlugin={chooseAnalyzerPlugin}
              isLoadingAnalyzers={isLoadingAnalyzers}
              refreshAnalyzers={refreshAnalyzers}
              removeAnalyzer={removeAnalyzer}
              saveAnalyzerRulePatch={saveAnalyzerRulePatch}
              toggleAnalyzerPlugin={toggleAnalyzerPlugin}
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
          className="h-auto w-full min-w-0 justify-between rounded-md px-3 py-2 sm:w-[28rem]"
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
            className="grid cursor-default grid-cols-[minmax(0,1fr)_auto] gap-3 rounded-md p-2"
            key={theme.id}
            onSelect={() => setThemeId(theme.id as ThemeId)}
          >
            <ThemeSelectSummary theme={theme} />
            <span
              className={cn(
                "mt-1 flex size-5 items-center justify-center rounded border text-primary",
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
          className="flex size-8 shrink-0 items-center justify-center rounded-md border border-[color:var(--app-border)] bg-[var(--app-panel)] text-sm font-semibold shadow-none"
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
  chooseMoveAnalyzerBinaryPath,
  chooseSuiCliPath,
  effectiveMoveAnalyzerSource,
  effectiveSuiSource,
  isSavingMoveAnalyzerSettings,
  isLoadingSuiKeys,
  isSavingSuiSettings,
  moveAnalyzerBinaryPathInput,
  moveAnalyzerSettings,
  moveAnalyzerSettingsError,
  moveAnalyzerStatus,
  refreshSuiKeys,
  saveMoveAnalyzerBinaryPath,
  saveSuiCliPath,
  setMoveAnalyzerBinaryPathInput,
  setSuiCliPathInput,
  setSuiKeyError,
  setSuiKeyState,
  suiCliPathInput,
  suiKeyError,
  suiKeyState,
  suiSettings,
  suiSettingsError,
  suiStatus,
  updateMoveAnalyzerSource,
  updateSuiSource,
}: {
  chooseMoveAnalyzerBinaryPath: () => Promise<void>;
  chooseSuiCliPath: () => Promise<void>;
  effectiveMoveAnalyzerSource: MoveAnalyzerAdapterSource;
  effectiveSuiSource: SuiAdapterSource;
  isSavingMoveAnalyzerSettings: boolean;
  isLoadingSuiKeys: boolean;
  isSavingSuiSettings: boolean;
  moveAnalyzerBinaryPathInput: string;
  moveAnalyzerSettings: MoveAnalyzerAdapterSettings;
  moveAnalyzerSettingsError: string | null;
  moveAnalyzerStatus: MoveAnalyzerAdapterStatus | null;
  refreshSuiKeys: () => Promise<void>;
  saveMoveAnalyzerBinaryPath: (path: string) => Promise<void>;
  saveSuiCliPath: (path: string) => Promise<void>;
  setMoveAnalyzerBinaryPathInput: (path: string) => void;
  setSuiCliPathInput: (path: string) => void;
  setSuiKeyError: (error: string | null) => void;
  setSuiKeyState: (state: SuiKeyState | null) => void;
  suiCliPathInput: string;
  suiKeyError: string | null;
  suiKeyState: SuiKeyState | null;
  suiSettings: SuiAdapterSettings;
  suiSettingsError: string | null;
  suiStatus: SuiAdapterStatus | null;
  updateMoveAnalyzerSource: (source: MoveAnalyzerAdapterSource) => Promise<void>;
  updateSuiSource: (source: SuiAdapterSource) => Promise<void>;
}) {
  return (
    <>
      <MoveAnalyzerSettingsSection
        binaryPathInput={moveAnalyzerBinaryPathInput}
        chooseBinaryPath={chooseMoveAnalyzerBinaryPath}
        effectiveSource={effectiveMoveAnalyzerSource}
        error={moveAnalyzerSettingsError}
        isSaving={isSavingMoveAnalyzerSettings}
        saveBinaryPath={saveMoveAnalyzerBinaryPath}
        settings={moveAnalyzerSettings}
        setBinaryPathInput={setMoveAnalyzerBinaryPathInput}
        status={moveAnalyzerStatus}
        updateSource={updateMoveAnalyzerSource}
      />

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
            <ToolSourceStatusRow
              active={effectiveSuiSource === "bundled"}
              label="Bundled crate"
              path={suiStatus?.bundled.path ?? null}
              version={suiStatus?.bundled.version ?? null}
              available={suiStatus?.bundled.available ?? false}
            />
            <ToolSourceStatusRow
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

      <SuiKeyManagementSettings
        isLoading={isLoadingSuiKeys}
        refreshSuiKeys={refreshSuiKeys}
        setSuiKeyError={setSuiKeyError}
        setSuiKeyState={setSuiKeyState}
        suiKeyError={suiKeyError}
        suiKeyState={suiKeyState}
      />
    </>
  );
}

function SuiKeyManagementSettings({
  isLoading,
  refreshSuiKeys,
  setSuiKeyError,
  setSuiKeyState,
  suiKeyError,
  suiKeyState,
}: {
  isLoading: boolean;
  refreshSuiKeys: () => Promise<void>;
  setSuiKeyError: (error: string | null) => void;
  setSuiKeyState: (state: SuiKeyState | null) => void;
  suiKeyError: string | null;
  suiKeyState: SuiKeyState | null;
}) {
  const [isMutating, setIsMutating] = React.useState(false);
  const [openDialog, setOpenDialog] = React.useState<"generate" | "import" | null>(null);
  const [accountAction, setAccountAction] = React.useState<{
    account: SuiKeyAccount;
    kind: "export" | "remove" | "rename";
  } | null>(null);
  const [selectedAccountId, setSelectedAccountId] = React.useState<string | null>(null);
  const [revealedSecret, setRevealedSecret] = React.useState<{
    label: string;
    secret: string;
  } | null>(null);
  const accounts = suiKeyState?.accounts ?? [];
  const disabled = isLoading || isMutating;
  const selectedAccount = React.useMemo(() => {
    if (!accounts.length) {
      return null;
    }

    return accounts.find((account) => suiAccountId(account) === selectedAccountId)
      ?? accounts.find((account) => account.isActive)
      ?? accounts[0];
  }, [accounts, selectedAccountId]);
  const exportAction = accountAction?.kind === "export"
    ? { account: accountAction.account, kind: "export" as const }
    : null;
  const removeAction = accountAction?.kind === "remove"
    ? { account: accountAction.account, kind: "remove" as const }
    : null;
  const renameAction = accountAction?.kind === "rename"
    ? { account: accountAction.account, kind: "rename" as const }
    : null;
  const statusLabel = suiKeyState
    ? suiKeyState.configStatus === "missing"
      ? "No client config"
      : suiKeyState.configStatus === "invalid"
        ? "Config error"
        : `${accounts.length} account${accounts.length === 1 ? "" : "s"}`
    : "Not loaded";

  React.useEffect(() => {
    if (!selectedAccount) {
      setSelectedAccountId(null);
      return;
    }

    const nextSelectedAccountId = suiAccountId(selectedAccount);
    if (selectedAccountId !== nextSelectedAccountId) {
      setSelectedAccountId(nextSelectedAccountId);
    }
  }, [selectedAccount, selectedAccountId]);

  const runMutation = React.useCallback(
    async (mutation: () => Promise<SuiKeyState | SuiGenerateKeyResponse>) => {
      setIsMutating(true);
      setSuiKeyError(null);

      try {
        const result = await mutation();

        if ("state" in result) {
          setSuiKeyState(result.state);

          if ("recoveryPhrase" in result && result.recoveryPhrase) {
            setRevealedSecret({
              label: `Recovery phrase for ${displaySuiAccountName(result.generated)}`,
              secret: result.recoveryPhrase,
            });
          }
        } else {
          setSuiKeyState(result);
        }
      } catch (error) {
        setSuiKeyError(getSettingsErrorMessage(error));
      } finally {
        setIsMutating(false);
      }
    },
    [setSuiKeyError, setSuiKeyState],
  );

  return (
    <>
      <SettingsSection title="Sui Keys">
        <SettingsRow
          label="Accounts"
          description={suiKeyState?.configDir ?? "~/.sui/sui_config"}
        >
          <div className="flex min-w-0 flex-wrap justify-end gap-2">
            <Badge variant={suiKeyState?.configStatus === "invalid" ? "destructive" : "secondary"}>
              {statusLabel}
            </Badge>
            <Button
              disabled={disabled}
              onClick={() => void refreshSuiKeys()}
              size="sm"
              type="button"
              variant="outline"
            >
              <RefreshCw aria-hidden="true" />
              Refresh
            </Button>
            <Button
              disabled={disabled || suiKeyState?.configStatus === "invalid"}
              onClick={() => setOpenDialog("import")}
              size="sm"
              type="button"
              variant="outline"
            >
              <Upload aria-hidden="true" />
              Import
            </Button>
            <Button
              disabled={disabled || suiKeyState?.configStatus === "invalid"}
              onClick={() => setOpenDialog("generate")}
              size="sm"
              type="button"
            >
              <Plus aria-hidden="true" />
              Generate
            </Button>
          </div>
        </SettingsRow>

        {suiKeyState?.diagnostics.length ? (
          <div className="border-t border-border/70 px-4 py-3.5">
            <div className="grid gap-2">
              {suiKeyState.diagnostics.map((diagnostic) => (
                <p
                  className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
                  key={`${diagnostic.path ?? "sui"}:${diagnostic.message}`}
                >
                  {diagnostic.message}
                </p>
              ))}
            </div>
          </div>
        ) : null}

        <div className="border-t border-border/70">
          {selectedAccount ? (
            <SuiKeyAccountPanel
              account={selectedAccount}
              accounts={accounts}
              disabled={disabled}
              onCopy={(value) => void copyToClipboard(value)}
              onExport={() => setAccountAction({ account: selectedAccount, kind: "export" })}
              onMakeActive={() => {
                void runMutation(() => setActiveSuiAddress(selectedAccount.alias ?? selectedAccount.address));
              }}
              onRemove={() => setAccountAction({ account: selectedAccount, kind: "remove" })}
              onRename={() => setAccountAction({ account: selectedAccount, kind: "rename" })}
              onSelectAccount={setSelectedAccountId}
              selectedAccountId={suiAccountId(selectedAccount)}
            />
          ) : (
            <div className="px-4 py-6 text-[13px] text-muted-foreground">
              {suiKeyState?.configStatus === "invalid"
                ? "Fix the Sui client configuration before managing keys."
                : "No Sui accounts are available in the CLI keystore."}
            </div>
          )}
        </div>

        {suiKeyError ? (
          <div className="border-t border-border/70 px-4 py-3.5">
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {suiKeyError}
            </p>
          </div>
        ) : null}
      </SettingsSection>

      <GenerateSuiKeyDialog
        disabled={disabled}
        onGenerate={(request) => runMutation(() => generateSuiKey(request))}
        onOpenChange={(open) => setOpenDialog(open ? "generate" : null)}
        open={openDialog === "generate"}
        state={suiKeyState}
      />
      <ImportSuiKeyDialog
        disabled={disabled}
        onImport={(request) => runMutation(async () => (await importSuiKey(request)).state)}
        onOpenChange={(open) => setOpenDialog(open ? "import" : null)}
        open={openDialog === "import"}
        state={suiKeyState}
      />
      <RenameSuiKeyDialog
        action={renameAction}
        disabled={disabled}
        onOpenChange={(open) => {
          if (!open) {
            setAccountAction(null);
          }
        }}
        onRename={(account, newAlias) => runMutation(() => renameSuiKeyAlias(account.alias ?? account.address, newAlias))}
      />
      <RemoveSuiKeyDialog
        action={removeAction}
        disabled={disabled}
        onOpenChange={(open) => {
          if (!open) {
            setAccountAction(null);
          }
        }}
        onRemove={(account, confirmation) => runMutation(() => removeSuiKey(account.alias ?? account.address, confirmation))}
      />
      <ExportSuiPrivateKeyDialog
        action={exportAction}
        disabled={disabled}
        onExport={async (account, confirmation) => {
          setIsMutating(true);
          setSuiKeyError(null);

          try {
            const exported = await exportSuiPrivateKey(account.alias ?? account.address, confirmation);
            setRevealedSecret({
              label: `Bech32 private key for ${displaySuiAccountName(exported.account)}`,
              secret: exported.exportedPrivateKey,
            });
            setAccountAction(null);
          } catch (error) {
            setSuiKeyError(getSettingsErrorMessage(error));
          } finally {
            setIsMutating(false);
          }
        }}
        onOpenChange={(open) => {
          if (!open) {
            setAccountAction(null);
          }
        }}
      />
      <SecretRevealDialog
        onOpenChange={(open) => {
          if (!open) {
            setRevealedSecret(null);
          }
        }}
        revealedSecret={revealedSecret}
      />
    </>
  );
}

function SuiKeyAccountPanel({
  account,
  accounts,
  disabled,
  onCopy,
  onExport,
  onMakeActive,
  onRemove,
  onRename,
  onSelectAccount,
  selectedAccountId,
}: {
  account: SuiKeyAccount;
  accounts: SuiKeyAccount[];
  disabled: boolean;
  onCopy: (value: string) => void;
  onExport: () => void;
  onMakeActive: () => void;
  onRemove: () => void;
  onRename: () => void;
  onSelectAccount: (accountId: string) => void;
  selectedAccountId: string;
}) {
  return (
    <div className="grid gap-3 px-4 py-3.5">
      <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-start">
        <div className="min-w-0">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <SuiKeyAccountSelector
              accounts={accounts}
              disabled={disabled}
              onSelectAccount={onSelectAccount}
              selectedAccountId={selectedAccountId}
            />
            <SuiAccountBadges account={account} />
          </div>
          <div className="mt-3 grid gap-x-4 gap-y-2 text-[11px] sm:grid-cols-2 xl:grid-cols-4">
            <SuiKeyCompactDetail label="Address" title={account.address} value={truncateMiddle(account.address, 14, 10)} />
            <SuiKeyCompactDetail label="Public key" title={account.publicBase64Key} value={truncateMiddle(account.publicBase64Key, 18, 10)} />
            <SuiKeyCompactDetail label="Flag" value={String(account.flag)} />
            <SuiKeyCompactDetail
              label="Peer ID"
              title={account.peerId ?? "Unavailable"}
              value={account.peerId ? truncateMiddle(account.peerId, 12, 8) : "Unavailable"}
            />
          </div>
        </div>
        <div className="flex min-w-0 flex-wrap gap-2 lg:justify-end">
        <Button
          disabled={disabled}
          onClick={() => onCopy(account.address)}
          size="sm"
          title="Copy address"
          type="button"
          variant="outline"
        >
          <Copy aria-hidden="true" />
          Address
        </Button>
        <Button
          disabled={disabled}
          onClick={() => onCopy(publicSuiAccountJson(account))}
          size="sm"
          title="Copy public account JSON"
          type="button"
          variant="outline"
        >
          <Download aria-hidden="true" />
          Public
        </Button>
        <Button
          disabled={disabled || account.isActive}
          onClick={onMakeActive}
          size="sm"
          type="button"
          variant={account.isActive ? "default" : "outline"}
        >
          <KeyRound aria-hidden="true" />
          Active
        </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button disabled={disabled} size="icon-sm" title="More key actions" type="button" variant="outline">
                <MoreHorizontal aria-hidden="true" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-52">
              <DropdownMenuLabel className="text-xs text-muted-foreground">
                Key actions
              </DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem disabled={account.isExternal} onSelect={onRename}>
                <Pencil aria-hidden="true" />
                Rename alias
              </DropdownMenuItem>
              <DropdownMenuItem disabled={!account.canExportPrivateKey} onSelect={onExport}>
                <Eye aria-hidden="true" />
                Reveal private key
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem disabled={!account.canRemove} onSelect={onRemove} variant="destructive">
                <Trash2 aria-hidden="true" />
                Remove key
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </div>
  );
}

function SuiKeyAccountSelector({
  accounts,
  disabled,
  onSelectAccount,
  selectedAccountId,
}: {
  accounts: SuiKeyAccount[];
  disabled: boolean;
  onSelectAccount: (accountId: string) => void;
  selectedAccountId: string;
}) {
  const selectedAccount = accounts.find((account) => suiAccountId(account) === selectedAccountId) ?? accounts[0];

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          className="h-auto min-h-10 w-full justify-between gap-3 px-3 py-2 text-left sm:w-[22rem]"
          disabled={disabled}
          type="button"
          variant="outline"
        >
          <span className="min-w-0">
            <span className="block truncate text-[13px] font-medium">
              {displaySuiAccountName(selectedAccount)}
            </span>
            <span className="block truncate font-mono text-[11px] text-muted-foreground">
              {truncateMiddle(selectedAccount.address, 16, 10)}
            </span>
          </span>
          <ChevronDown className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-h-80 w-[32rem] max-w-[calc(100vw-2rem)]">
        <DropdownMenuLabel className="text-xs text-muted-foreground">
          Choose account
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuRadioGroup value={selectedAccountId} onValueChange={onSelectAccount}>
          {accounts.map((account) => {
            const accountId = suiAccountId(account);

            return (
              <DropdownMenuRadioItem
                className="items-start py-2"
                key={accountId}
                value={accountId}
              >
                <span className="grid min-w-0 flex-1 gap-1">
                  <span className="flex min-w-0 items-center gap-2">
                    <span className="truncate font-medium">{displaySuiAccountName(account)}</span>
                    {account.isActive ? (
                      <Badge className="rounded px-1.5 py-0 text-[10px]" variant="secondary">
                        Active
                      </Badge>
                    ) : null}
                    {account.isExternal ? (
                      <Badge className="rounded px-1.5 py-0 text-[10px]" variant="outline">
                        External
                      </Badge>
                    ) : null}
                  </span>
                  <span className="truncate font-mono text-[11px] text-muted-foreground">
                    {truncateMiddle(account.address, 18, 12)}
                  </span>
                </span>
                <span className="ml-auto rounded border px-1.5 py-0.5 text-[10px] text-muted-foreground">
                  {account.keyScheme}
                </span>
              </DropdownMenuRadioItem>
            );
          })}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function SuiAccountBadges({ account }: { account: SuiKeyAccount }) {
  return (
    <>
      {account.isActive ? (
        <Badge className="rounded px-1.5 py-0 text-[10px]" variant="secondary">
          Active
        </Badge>
      ) : null}
      {account.isExternal ? (
        <Badge className="rounded px-1.5 py-0 text-[10px]" variant="outline">
          External
        </Badge>
      ) : null}
      <Badge className="rounded px-1.5 py-0 text-[10px]" variant="outline">
        {account.keyScheme}
      </Badge>
    </>
  );
}

function SuiKeyCompactDetail({
  label,
  title,
  value,
}: {
  label: string;
  title?: string;
  value: string;
}) {
  return (
    <div className="min-w-0">
      <div className="text-[10px] font-medium uppercase text-muted-foreground">{label}</div>
      <div className="mt-0.5 truncate font-mono text-[11px] text-foreground" title={title ?? value}>
        {value}
      </div>
    </div>
  );
}

function GenerateSuiKeyDialog({
  disabled,
  onGenerate,
  onOpenChange,
  open,
  state,
}: {
  disabled: boolean;
  onGenerate: (request: {
    alias: string | null;
    derivationPath: string | null;
    keyScheme: string;
    revealRecoveryPhrase: boolean;
    wordLength: string | null;
  }) => Promise<void>;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  state: SuiKeyState | null;
}) {
  const [alias, setAlias] = React.useState("");
  const [derivationPath, setDerivationPath] = React.useState("");
  const [keyScheme, setKeyScheme] = React.useState("ed25519");
  const [revealRecoveryPhrase, setRevealRecoveryPhrase] = React.useState(false);
  const [wordLength, setWordLength] = React.useState("word12");

  React.useEffect(() => {
    if (!open) {
      setAlias("");
      setDerivationPath("");
      setKeyScheme("ed25519");
      setRevealRecoveryPhrase(false);
      setWordLength("word12");
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Generate Sui Key</DialogTitle>
          <DialogDescription>
            Creates a key in the standard Sui CLI keystore.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <LabeledField label="Alias">
            <Input
              autoComplete="off"
              onChange={(event) => setAlias(event.target.value)}
              placeholder="Optional"
              value={alias}
            />
          </LabeledField>
          <div className="grid gap-3 sm:grid-cols-2">
            <LabeledField label="Scheme">
              <SuiOptionDropdown
                options={state?.supportedSchemes ?? ["ed25519", "secp256k1", "secp256r1"]}
                value={keyScheme}
                onChange={setKeyScheme}
              />
            </LabeledField>
            <LabeledField label="Words">
              <SuiOptionDropdown
                options={state?.supportedWordLengths ?? ["word12", "word15", "word18", "word21", "word24"]}
                value={wordLength}
                onChange={setWordLength}
              />
            </LabeledField>
          </div>
          <LabeledField label="Derivation path">
            <Input
              autoComplete="off"
              onChange={(event) => setDerivationPath(event.target.value)}
              placeholder="Use Sui default"
              value={derivationPath}
            />
          </LabeledField>
          <label className="flex items-center gap-2 text-sm text-muted-foreground">
            <input
              checked={revealRecoveryPhrase}
              className="size-4 accent-primary"
              onChange={(event) => setRevealRecoveryPhrase(event.target.checked)}
              type="checkbox"
            />
            Reveal recovery phrase after generation
          </label>
        </div>
        <DialogFooter>
          <Button disabled={disabled} onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            disabled={disabled}
            onClick={() => {
              void onGenerate({
                alias: alias.trim() || null,
                derivationPath: derivationPath.trim() || null,
                keyScheme,
                revealRecoveryPhrase,
                wordLength,
              }).then(() => onOpenChange(false));
            }}
            type="button"
          >
            Generate
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ImportSuiKeyDialog({
  disabled,
  onImport,
  onOpenChange,
  open,
  state,
}: {
  disabled: boolean;
  onImport: (request: {
    alias: string | null;
    derivationPath: string | null;
    inputString: string;
    keyScheme: string;
  }) => Promise<void>;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  state: SuiKeyState | null;
}) {
  const [alias, setAlias] = React.useState("");
  const [derivationPath, setDerivationPath] = React.useState("");
  const [inputString, setInputString] = React.useState("");
  const [keyScheme, setKeyScheme] = React.useState("ed25519");

  React.useEffect(() => {
    if (!open) {
      setAlias("");
      setDerivationPath("");
      setInputString("");
      setKeyScheme("ed25519");
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Import Sui Key</DialogTitle>
          <DialogDescription>
            Accepts a `suiprivkey` Bech32 private key or mnemonic phrase.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <LabeledField label="Secret input">
            <textarea
              autoComplete="off"
              className="min-h-28 w-full rounded-md border border-input bg-[var(--app-panel)] px-3 py-2 text-sm shadow-none outline-none focus-visible:border-ring focus-visible:ring-[2px] focus-visible:ring-ring/35"
              onChange={(event) => setInputString(event.target.value)}
              value={inputString}
            />
          </LabeledField>
          <div className="grid gap-3 sm:grid-cols-2">
            <LabeledField label="Alias">
              <Input
                autoComplete="off"
                onChange={(event) => setAlias(event.target.value)}
                placeholder="Optional"
                value={alias}
              />
            </LabeledField>
            <LabeledField label="Mnemonic scheme">
              <SuiOptionDropdown
                options={state?.supportedSchemes ?? ["ed25519", "secp256k1", "secp256r1"]}
                value={keyScheme}
                onChange={setKeyScheme}
              />
            </LabeledField>
          </div>
          <LabeledField label="Derivation path">
            <Input
              autoComplete="off"
              onChange={(event) => setDerivationPath(event.target.value)}
              placeholder="Use Sui default for mnemonic import"
              value={derivationPath}
            />
          </LabeledField>
        </div>
        <DialogFooter>
          <Button disabled={disabled} onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            disabled={disabled || !inputString.trim()}
            onClick={() => {
              void onImport({
                alias: alias.trim() || null,
                derivationPath: derivationPath.trim() || null,
                inputString,
                keyScheme,
              }).then(() => onOpenChange(false));
            }}
            type="button"
          >
            Import
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RenameSuiKeyDialog({
  action,
  disabled,
  onOpenChange,
  onRename,
}: {
  action: { account: SuiKeyAccount; kind: "rename" } | null;
  disabled: boolean;
  onOpenChange: (open: boolean) => void;
  onRename: (account: SuiKeyAccount, newAlias: string) => Promise<void>;
}) {
  const [newAlias, setNewAlias] = React.useState("");
  const account = action?.account ?? null;

  React.useEffect(() => {
    setNewAlias(account?.alias ?? "");
  }, [account]);

  return (
    <Dialog open={Boolean(account)} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Rename Alias</DialogTitle>
          <DialogDescription>
            Updates the alias stored in Sui’s aliases file.
          </DialogDescription>
        </DialogHeader>
        <LabeledField label="Alias">
          <Input
            autoComplete="off"
            onChange={(event) => setNewAlias(event.target.value)}
            value={newAlias}
          />
        </LabeledField>
        <DialogFooter>
          <Button disabled={disabled} onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            disabled={disabled || !account || !newAlias.trim()}
            onClick={() => {
              if (!account) {
                return;
              }
              void onRename(account, newAlias).then(() => onOpenChange(false));
            }}
            type="button"
          >
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RemoveSuiKeyDialog({
  action,
  disabled,
  onOpenChange,
  onRemove,
}: {
  action: { account: SuiKeyAccount; kind: "remove" } | null;
  disabled: boolean;
  onOpenChange: (open: boolean) => void;
  onRemove: (account: SuiKeyAccount, confirmation: string) => Promise<void>;
}) {
  const [confirmation, setConfirmation] = React.useState("");
  const account = action?.account ?? null;
  const expected = account ? account.alias ?? account.address : "";

  React.useEffect(() => {
    if (!account) {
      setConfirmation("");
    }
  }, [account]);

  return (
    <Dialog open={Boolean(account)} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Remove Sui Key</DialogTitle>
          <DialogDescription>
            This removes the keypair from the Sui CLI keystore and cannot be undone from Peregrine.
          </DialogDescription>
        </DialogHeader>
        {account ? (
          <div className="grid gap-4">
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              Type `{expected}` to confirm removal.
            </p>
            <Input
              autoComplete="off"
              onChange={(event) => setConfirmation(event.target.value)}
              value={confirmation}
            />
          </div>
        ) : null}
        <DialogFooter>
          <Button disabled={disabled} onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            disabled={disabled || !account || confirmation.trim() !== expected}
            onClick={() => {
              if (!account) {
                return;
              }
              void onRemove(account, confirmation).then(() => onOpenChange(false));
            }}
            type="button"
            variant="destructive"
          >
            Remove
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ExportSuiPrivateKeyDialog({
  action,
  disabled,
  onExport,
  onOpenChange,
}: {
  action: { account: SuiKeyAccount; kind: "export" } | null;
  disabled: boolean;
  onExport: (account: SuiKeyAccount, confirmation: string) => Promise<void>;
  onOpenChange: (open: boolean) => void;
}) {
  const [confirmation, setConfirmation] = React.useState("");
  const account = action?.account ?? null;
  const expected = account ? account.alias ?? account.address : "";

  React.useEffect(() => {
    if (!account) {
      setConfirmation("");
    }
  }, [account]);

  return (
    <Dialog open={Boolean(account)} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Reveal Private Key</DialogTitle>
          <DialogDescription>
            Exports the Bech32 `suiprivkey` for this local Sui key.
          </DialogDescription>
        </DialogHeader>
        {account ? (
          <div className="grid gap-4">
            <p className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-300">
              Anyone with this value can control the account. Type `{expected}` to reveal it.
            </p>
            <Input
              autoComplete="off"
              onChange={(event) => setConfirmation(event.target.value)}
              value={confirmation}
            />
          </div>
        ) : null}
        <DialogFooter>
          <Button disabled={disabled} onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            disabled={disabled || !account || confirmation.trim() !== expected}
            onClick={() => {
              if (!account) {
                return;
              }
              void onExport(account, confirmation);
            }}
            type="button"
            variant="destructive"
          >
            Reveal
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function SecretRevealDialog({
  onOpenChange,
  revealedSecret,
}: {
  onOpenChange: (open: boolean) => void;
  revealedSecret: { label: string; secret: string } | null;
}) {
  return (
    <Dialog open={Boolean(revealedSecret)} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{revealedSecret?.label ?? "Secret"}</DialogTitle>
          <DialogDescription>
            This value is not stored by Peregrine. Closing this dialog clears it from the app state.
          </DialogDescription>
        </DialogHeader>
        <textarea
          className="min-h-28 w-full rounded-md border border-input bg-[var(--app-panel)] px-3 py-2 font-mono text-sm shadow-none outline-none"
          readOnly
          value={revealedSecret?.secret ?? ""}
        />
        <DialogFooter>
          <Button
            disabled={!revealedSecret}
            onClick={() => revealedSecret ? void copyToClipboard(revealedSecret.secret) : undefined}
            type="button"
            variant="outline"
          >
            <Copy aria-hidden="true" />
            Copy
          </Button>
          <Button onClick={() => onOpenChange(false)} type="button">
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function SuiOptionDropdown({
  onChange,
  options,
  value,
}: {
  onChange: (value: string) => void;
  options: string[];
  value: string;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button className="w-full justify-between" type="button" variant="outline">
          {value}
          <ChevronDown className="size-4 text-muted-foreground" aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[var(--radix-dropdown-menu-trigger-width)]">
        <DropdownMenuRadioGroup value={value} onValueChange={onChange}>
          {options.map((option) => (
            <DropdownMenuRadioItem key={option} value={option}>
              {option}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function LabeledField({
  children,
  label,
}: {
  children: React.ReactNode;
  label: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium text-foreground">{label}</span>
      {children}
    </label>
  );
}

function AnalyzerSettings({
  activePackageName,
  analyzerCatalog,
  analyzerError,
  analyzerPlugins,
  chooseAnalyzerPlugin,
  isLoadingAnalyzers,
  refreshAnalyzers,
  removeAnalyzer,
  saveAnalyzerRulePatch,
  toggleAnalyzerPlugin,
}: {
  activePackageName: string | null;
  analyzerCatalog: AnalysisRuleCatalog | null;
  analyzerError: string | null;
  analyzerPlugins: InstalledAnalyzerPlugin[];
  chooseAnalyzerPlugin: () => Promise<void>;
  isLoadingAnalyzers: boolean;
  refreshAnalyzers: () => Promise<void>;
  removeAnalyzer: (pluginId: string) => Promise<void>;
  saveAnalyzerRulePatch: (
    rulesetId: string,
    ruleId: string | null,
    patch: {
      active?: boolean | null;
      severity?: AnalysisSeverity | null;
      threshold?: number | null;
      entryThreshold?: number | null;
    },
  ) => Promise<void>;
  toggleAnalyzerPlugin: (pluginId: string, enabled: boolean) => Promise<void>;
}) {
  return (
    <>
      <SettingsSection title="Plugins">
        <SettingsRow
          label="Installed plugins"
          description={`${analyzerPlugins.length} user-global plugin${analyzerPlugins.length === 1 ? "" : "s"}`}
        >
          <div className="flex min-w-0 gap-2">
            <Button
              disabled={isLoadingAnalyzers}
              onClick={() => void refreshAnalyzers()}
              size="sm"
              type="button"
              variant="outline"
            >
              <RefreshCw aria-hidden="true" />
              Refresh
            </Button>
            <Button
              disabled={isLoadingAnalyzers}
              onClick={() => void chooseAnalyzerPlugin()}
              size="sm"
              type="button"
            >
              <Upload aria-hidden="true" />
              Install
            </Button>
          </div>
        </SettingsRow>

        <div className="border-t border-border/70">
          {analyzerPlugins.length ? (
            analyzerPlugins.map((plugin) => (
              <PluginRow
                disabled={isLoadingAnalyzers}
                key={`${plugin.pluginId}:${plugin.version}`}
                plugin={plugin}
                removeAnalyzer={removeAnalyzer}
                toggleAnalyzerPlugin={toggleAnalyzerPlugin}
              />
            ))
          ) : (
            <div className="px-4 py-3.5 text-[13px] text-muted-foreground">
              No unbundled analyzer plugins installed.
            </div>
          )}
        </div>
      </SettingsSection>

      <SettingsSection title="Rules">
        <SettingsRow
          label="Package"
          description={activePackageName ? `Editing ${activePackageName}` : "Open a package to persist rule configuration."}
        >
          <Badge variant="secondary">{activePackageName ?? "No package"}</Badge>
        </SettingsRow>

        <div className="border-t border-border/70">
          {analyzerCatalog?.rulesets.length ? (
            analyzerCatalog.rulesets.map((ruleset) => (
              <RuleSetRow
                disabled={isLoadingAnalyzers || !activePackageName}
                key={`${ruleset.pluginId ?? "bundled"}:${ruleset.id}`}
                ruleset={ruleset}
                saveAnalyzerRulePatch={saveAnalyzerRulePatch}
              />
            ))
          ) : (
            <div className="px-4 py-3.5 text-[13px] text-muted-foreground">
              No analyzer catalog loaded.
            </div>
          )}
        </div>

        {analyzerCatalog?.diagnostics.length ? (
          <div className="border-t border-border/70 px-4 py-3.5">
            <div className="grid gap-2">
              {analyzerCatalog.diagnostics.map((diagnostic) => (
                <p
                  className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-300"
                  key={`${diagnostic.source}:${diagnostic.message}`}
                >
                  {diagnostic.source}: {diagnostic.message}
                </p>
              ))}
            </div>
          </div>
        ) : null}

        {analyzerError ? (
          <div className="border-t border-border/70 px-4 py-3.5">
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {analyzerError}
            </p>
          </div>
        ) : null}
      </SettingsSection>
    </>
  );
}

function PluginRow({
  disabled,
  plugin,
  removeAnalyzer,
  toggleAnalyzerPlugin,
}: {
  disabled: boolean;
  plugin: InstalledAnalyzerPlugin;
  removeAnalyzer: (pluginId: string) => Promise<void>;
  toggleAnalyzerPlugin: (pluginId: string, enabled: boolean) => Promise<void>;
}) {
  const name = plugin.manifest.name ?? plugin.pluginId;

  return (
    <div className="grid gap-3 border-t border-border/70 px-4 py-3.5 first:border-t-0 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[13px] font-medium text-foreground">{name}</span>
          <Badge className="rounded px-1.5 py-0 text-[10px]" variant="secondary">
            v{plugin.version}
          </Badge>
          <Badge className="rounded px-1.5 py-0 text-[10px]" variant="outline">
            {plugin.runtime === "wasm" ? "WASM" : "Native"}
          </Badge>
        </div>
        <p className="mt-1 truncate font-mono text-[11px] text-muted-foreground">{plugin.path}</p>
      </div>
      <div className="flex justify-end gap-2">
        <Button
          disabled={disabled}
          onClick={() => void toggleAnalyzerPlugin(plugin.pluginId, !plugin.enabled)}
          size="sm"
          type="button"
          variant={plugin.enabled ? "default" : "outline"}
        >
          {plugin.enabled ? "Enabled" : "Disabled"}
        </Button>
        <Button
          disabled={disabled}
          onClick={() => void removeAnalyzer(plugin.pluginId)}
          size="sm"
          title="Remove plugin"
          type="button"
          variant="outline"
        >
          <Trash2 aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}

function RuleSetRow({
  disabled,
  ruleset,
  saveAnalyzerRulePatch,
}: {
  disabled: boolean;
  ruleset: AnalysisRuleCatalog["rulesets"][number];
  saveAnalyzerRulePatch: (
    rulesetId: string,
    ruleId: string | null,
    patch: {
      active?: boolean | null;
      severity?: AnalysisSeverity | null;
      threshold?: number | null;
      entryThreshold?: number | null;
    },
  ) => Promise<void>;
}) {
  return (
    <div className="border-t border-border/70 first:border-t-0">
      <div className="grid gap-3 px-4 py-3.5 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <span className="truncate text-[13px] font-medium text-foreground">{ruleset.name}</span>
            <Badge className="rounded px-1.5 py-0 text-[10px]" variant="secondary">
              {ruleset.bundled ? "Bundled" : ruleset.pluginId}
            </Badge>
          </div>
          <p className="mt-1 text-[12px] text-muted-foreground">{ruleset.description || ruleset.id}</p>
        </div>
        <Button
          disabled={disabled}
          onClick={() => void saveAnalyzerRulePatch(ruleset.id, null, { active: !ruleset.active })}
          size="sm"
          type="button"
          variant={ruleset.active ? "default" : "outline"}
        >
          {ruleset.active ? "Enabled" : "Disabled"}
        </Button>
      </div>
      <div className="grid border-t border-border/70">
        {ruleset.rules.map((rule) => (
          <RuleRow
            disabled={disabled || !ruleset.active}
            key={rule.id}
            rule={rule}
            rulesetId={ruleset.id}
            saveAnalyzerRulePatch={saveAnalyzerRulePatch}
          />
        ))}
      </div>
    </div>
  );
}

function RuleRow({
  disabled,
  rule,
  rulesetId,
  saveAnalyzerRulePatch,
}: {
  disabled: boolean;
  rule: AnalysisRuleMetadata;
  rulesetId: string;
  saveAnalyzerRulePatch: (
    rulesetId: string,
    ruleId: string | null,
    patch: {
      active?: boolean | null;
      severity?: AnalysisSeverity | null;
      threshold?: number | null;
      entryThreshold?: number | null;
    },
  ) => Promise<void>;
}) {
  const effectiveSeverity = rule.configuredSeverity ?? rule.defaultSeverity;

  return (
    <div className="grid gap-3 px-4 py-3.5 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[13px] font-medium text-foreground">{rule.name}</span>
          <Badge className="rounded px-1.5 py-0 text-[10px]" variant="secondary">
            {effectiveSeverity}
          </Badge>
        </div>
        <p className="mt-1 text-[12px] text-muted-foreground">{rule.description || rule.id}</p>
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <SeverityDropdown
          disabled={disabled}
          onChange={(severity) => void saveAnalyzerRulePatch(rulesetId, rule.id, { severity })}
          value={effectiveSeverity}
        />
        <RuleNumericControls
          disabled={disabled}
          onSave={(patch) => void saveAnalyzerRulePatch(rulesetId, rule.id, patch)}
          rule={rule}
        />
        <Button
          disabled={disabled}
          onClick={() => void saveAnalyzerRulePatch(rulesetId, rule.id, { active: !rule.active })}
          size="sm"
          type="button"
          variant={rule.active ? "default" : "outline"}
        >
          {rule.active ? "Enabled" : "Disabled"}
        </Button>
      </div>
    </div>
  );
}

function SeverityDropdown({
  disabled,
  onChange,
  value,
}: {
  disabled: boolean;
  onChange: (severity: AnalysisSeverity) => void;
  value: AnalysisSeverity;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button disabled={disabled} size="sm" type="button" variant="outline">
          {value}
          <ChevronDown className="size-4 text-muted-foreground" aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {(["info", "warning", "error"] as AnalysisSeverity[]).map((severity) => (
          <DropdownMenuItem key={severity} onSelect={() => onChange(severity)}>
            {severity}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function RuleNumericControls({
  disabled,
  onSave,
  rule,
}: {
  disabled: boolean;
  onSave: (patch: { threshold?: number | null; entryThreshold?: number | null }) => void;
  rule: AnalysisRuleMetadata;
}) {
  const numericProperties = rule.configSchema.filter((property) => property.valueKind === "integer");

  if (!numericProperties.length) {
    return null;
  }

  return (
    <>
      {numericProperties.map((property) => (
        <Input
          className="h-8 w-24"
          disabled={disabled}
          inputMode="numeric"
          key={property.key}
          min={0}
          onBlur={(event) => {
            const value = event.currentTarget.value.trim();
            if (!value) {
              return;
            }
            const parsed = Number(value);
            if (!Number.isFinite(parsed) || parsed < 0) {
              return;
            }
            onSave(property.key === "entry_threshold"
              ? { entryThreshold: Math.floor(parsed) }
              : { threshold: Math.floor(parsed) });
          }}
          placeholder={property.defaultValue ?? property.key}
          type="number"
        />
      ))}
    </>
  );
}

function displaySuiAccountName(account: SuiKeyAccount) {
  return account.alias?.trim() || truncateMiddle(account.address, 12, 8);
}

function suiAccountId(account: SuiKeyAccount) {
  return `${account.isExternal ? "external" : "local"}:${account.address}`;
}

function publicSuiAccountJson(account: SuiKeyAccount) {
  return JSON.stringify(
    {
      address: account.address,
      alias: account.alias ?? null,
      flag: account.flag,
      keyScheme: account.keyScheme,
      peerId: account.peerId ?? null,
      publicBase64Key: account.publicBase64Key,
    },
    null,
    2,
  );
}

async function copyToClipboard(value: string) {
  await navigator.clipboard.writeText(value);
}

function truncateMiddle(value: string, prefixLength: number, suffixLength: number) {
  if (value.length <= prefixLength + suffixLength + 1) {
    return value;
  }

  return `${value.slice(0, prefixLength)}...${value.slice(-suffixLength)}`;
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
        "flex min-w-[12rem] items-center gap-3 rounded-md px-3 py-2.5 text-left text-sm transition lg:min-w-0",
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
      <div className="-mx-4 overflow-hidden rounded-md border border-[color:var(--app-border)] bg-[var(--app-surface)]">
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

async function openMoveAnalyzerBinaryPath(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");

  const selectedPath = await open({
    directory: false,
    multiple: false,
    title: "Select move-analyzer",
  });

  return typeof selectedPath === "string" ? selectedPath : null;
}

async function openAnalyzerPluginPath(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");

  const selectedPath = await open({
    directory: false,
    filters: [{ extensions: ["wasm", "dylib", "so", "dll"], name: "Analyzer plugin" }],
    multiple: false,
    title: "Install analyzer plugin",
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

  return typeof error === "string" ? error : "Could not update settings.";
}
