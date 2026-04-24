import React from "react";

import {
  appThemes,
  defaultThemeId,
  getThemeById,
  type AppTheme,
  type ThemeId,
  type ThemeMode,
  type ThemeTokens,
} from "@/theme/themes";

type ThemeContextValue = {
  themes: readonly AppTheme[];
  theme: AppTheme;
  themeId: ThemeId;
  mode: ThemeMode;
  resolvedMode: "light" | "dark";
  setThemeId: (themeId: ThemeId) => void;
  setMode: (mode: ThemeMode) => void;
};

const storageKey = "peregrine-theme";
const modeStorageKey = "peregrine-theme-mode";

const tokenPropertyMap: Record<keyof ThemeTokens, string> = {
  background: "--background",
  foreground: "--foreground",
  card: "--card",
  cardForeground: "--card-foreground",
  popover: "--popover",
  popoverForeground: "--popover-foreground",
  primary: "--primary",
  primaryForeground: "--primary-foreground",
  secondary: "--secondary",
  secondaryForeground: "--secondary-foreground",
  muted: "--muted",
  mutedForeground: "--muted-foreground",
  accent: "--accent",
  accentForeground: "--accent-foreground",
  destructive: "--destructive",
  border: "--border",
  input: "--input",
  ring: "--ring",
  chart1: "--chart-1",
  chart2: "--chart-2",
  chart3: "--chart-3",
  chart4: "--chart-4",
  chart5: "--chart-5",
  sidebar: "--sidebar",
  sidebarForeground: "--sidebar-foreground",
  sidebarPrimary: "--sidebar-primary",
  sidebarPrimaryForeground: "--sidebar-primary-foreground",
  sidebarAccent: "--sidebar-accent",
  sidebarAccentForeground: "--sidebar-accent-foreground",
  sidebarBorder: "--sidebar-border",
  sidebarRing: "--sidebar-ring",
};

const ThemeContext = React.createContext<ThemeContextValue | null>(null);

function getInitialThemeId(): ThemeId {
  if (typeof window === "undefined") {
    return defaultThemeId;
  }

  const storedTheme = window.localStorage.getItem(storageKey);
  return appThemes.some((theme) => theme.id === storedTheme)
    ? (storedTheme as ThemeId)
    : defaultThemeId;
}

function getInitialMode(): ThemeMode {
  if (typeof window === "undefined") {
    return "system";
  }

  const storedMode = window.localStorage.getItem(modeStorageKey);
  return storedMode === "light" || storedMode === "dark" || storedMode === "system"
    ? storedMode
    : "system";
}

function getSystemMode(): "light" | "dark" {
  if (typeof window === "undefined") {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function applyTheme(theme: AppTheme, mode: "light" | "dark") {
  const root = document.documentElement;
  const tokens = theme[mode];

  root.dataset.theme = theme.id;
  root.classList.toggle("dark", mode === "dark");
  root.style.setProperty("--radius", theme.radius);

  Object.entries(tokenPropertyMap).forEach(([tokenName, cssProperty]) => {
    root.style.setProperty(cssProperty, tokens[tokenName as keyof ThemeTokens]);
  });
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [themeId, setThemeIdState] = React.useState<ThemeId>(getInitialThemeId);
  const [mode, setModeState] = React.useState<ThemeMode>(getInitialMode);
  const [systemMode, setSystemMode] = React.useState<"light" | "dark">(getSystemMode);

  const theme = React.useMemo(() => getThemeById(themeId), [themeId]);
  const resolvedMode = mode === "system" ? systemMode : mode;

  React.useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => setSystemMode(media.matches ? "dark" : "light");

    handleChange();
    media.addEventListener("change", handleChange);

    return () => media.removeEventListener("change", handleChange);
  }, []);

  React.useEffect(() => {
    applyTheme(theme, resolvedMode);
  }, [theme, resolvedMode]);

  const setThemeId = React.useCallback((nextThemeId: ThemeId) => {
    setThemeIdState(nextThemeId);
    window.localStorage.setItem(storageKey, nextThemeId);
  }, []);

  const setMode = React.useCallback((nextMode: ThemeMode) => {
    setModeState(nextMode);
    window.localStorage.setItem(modeStorageKey, nextMode);
  }, []);

  const value = React.useMemo<ThemeContextValue>(
    () => ({
      themes: appThemes,
      theme,
      themeId,
      mode,
      resolvedMode,
      setThemeId,
      setMode,
    }),
    [mode, resolvedMode, setMode, setThemeId, theme, themeId],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme() {
  const context = React.useContext(ThemeContext);

  if (!context) {
    throw new Error("useTheme must be used within ThemeProvider");
  }

  return context;
}
