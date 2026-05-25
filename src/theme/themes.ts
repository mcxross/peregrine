export type ThemeMode = "light" | "dark" | "system";

export type ThemeTokens = {
  background: string;
  foreground: string;
  card: string;
  cardForeground: string;
  popover: string;
  popoverForeground: string;
  primary: string;
  primaryForeground: string;
  secondary: string;
  secondaryForeground: string;
  muted: string;
  mutedForeground: string;
  accent: string;
  accentForeground: string;
  destructive: string;
  border: string;
  input: string;
  ring: string;
  chart1: string;
  chart2: string;
  chart3: string;
  chart4: string;
  chart5: string;
  sidebar: string;
  sidebarForeground: string;
  sidebarPrimary: string;
  sidebarPrimaryForeground: string;
  sidebarAccent: string;
  sidebarAccentForeground: string;
  sidebarBorder: string;
  sidebarRing: string;
};

export type AppTheme = {
  id: string;
  name: string;
  family: "base" | "classic";
  swatch: string;
  radius: string;
  light: ThemeTokens;
  dark: ThemeTokens;
};

type Palette = {
  50: string;
  100: string;
  200: string;
  300: string;
  400: string;
  500: string;
  600: string;
  700: string;
  800: string;
  900: string;
  950: string;
};

const palettes: Record<string, Palette> = {
  neutral: {
    50: "oklch(0.985 0 0)",
    100: "oklch(0.97 0 0)",
    200: "oklch(0.922 0 0)",
    300: "oklch(0.87 0 0)",
    400: "oklch(0.708 0 0)",
    500: "oklch(0.556 0 0)",
    600: "oklch(0.439 0 0)",
    700: "oklch(0.371 0 0)",
    800: "oklch(0.269 0 0)",
    900: "oklch(0.205 0 0)",
    950: "oklch(0.145 0 0)",
  },
  zinc: {
    50: "oklch(0.985 0 0)",
    100: "oklch(0.967 0.001 286.375)",
    200: "oklch(0.92 0.004 286.32)",
    300: "oklch(0.871 0.006 286.286)",
    400: "oklch(0.705 0.015 286.067)",
    500: "oklch(0.552 0.016 285.938)",
    600: "oklch(0.442 0.017 285.786)",
    700: "oklch(0.37 0.013 285.805)",
    800: "oklch(0.274 0.006 286.033)",
    900: "oklch(0.21 0.006 285.885)",
    950: "oklch(0.141 0.005 285.823)",
  },
  stone: {
    50: "oklch(0.985 0.001 106.423)",
    100: "oklch(0.97 0.001 106.424)",
    200: "oklch(0.923 0.003 48.717)",
    300: "oklch(0.869 0.005 56.366)",
    400: "oklch(0.709 0.01 56.259)",
    500: "oklch(0.553 0.013 58.071)",
    600: "oklch(0.444 0.011 73.639)",
    700: "oklch(0.374 0.01 67.558)",
    800: "oklch(0.268 0.007 34.298)",
    900: "oklch(0.216 0.006 56.043)",
    950: "oklch(0.147 0.004 49.25)",
  },
  gray: {
    50: "oklch(0.985 0.002 247.839)",
    100: "oklch(0.967 0.003 264.542)",
    200: "oklch(0.928 0.006 264.531)",
    300: "oklch(0.872 0.01 258.338)",
    400: "oklch(0.707 0.022 261.325)",
    500: "oklch(0.551 0.027 264.364)",
    600: "oklch(0.446 0.03 256.802)",
    700: "oklch(0.373 0.034 259.733)",
    800: "oklch(0.278 0.033 256.848)",
    900: "oklch(0.21 0.034 264.665)",
    950: "oklch(0.13 0.028 261.692)",
  },
  slate: {
    50: "oklch(0.984 0.003 247.858)",
    100: "oklch(0.968 0.007 247.896)",
    200: "oklch(0.929 0.013 255.508)",
    300: "oklch(0.869 0.022 252.894)",
    400: "oklch(0.704 0.04 256.788)",
    500: "oklch(0.554 0.046 257.417)",
    600: "oklch(0.446 0.043 257.281)",
    700: "oklch(0.372 0.044 257.287)",
    800: "oklch(0.279 0.041 260.031)",
    900: "oklch(0.208 0.042 265.755)",
    950: "oklch(0.129 0.042 264.695)",
  },
  red: {
    50: "oklch(0.971 0.013 17.38)",
    100: "oklch(0.936 0.032 17.717)",
    200: "oklch(0.885 0.062 18.334)",
    300: "oklch(0.808 0.114 19.571)",
    400: "oklch(0.704 0.191 22.216)",
    500: "oklch(0.637 0.237 25.331)",
    600: "oklch(0.577 0.245 27.325)",
    700: "oklch(0.505 0.213 27.518)",
    800: "oklch(0.444 0.177 26.899)",
    900: "oklch(0.396 0.141 25.723)",
    950: "oklch(0.258 0.092 26.042)",
  },
  orange: {
    50: "oklch(0.98 0.016 73.684)",
    100: "oklch(0.954 0.038 75.164)",
    200: "oklch(0.901 0.076 70.697)",
    300: "oklch(0.837 0.128 66.29)",
    400: "oklch(0.75 0.183 55.934)",
    500: "oklch(0.705 0.213 47.604)",
    600: "oklch(0.646 0.222 41.116)",
    700: "oklch(0.553 0.195 38.402)",
    800: "oklch(0.47 0.157 37.304)",
    900: "oklch(0.408 0.123 38.172)",
    950: "oklch(0.266 0.079 36.259)",
  },
  yellow: {
    50: "oklch(0.987 0.026 102.212)",
    100: "oklch(0.973 0.071 103.193)",
    200: "oklch(0.945 0.129 101.54)",
    300: "oklch(0.905 0.182 98.111)",
    400: "oklch(0.852 0.199 91.936)",
    500: "oklch(0.795 0.184 86.047)",
    600: "oklch(0.681 0.162 75.834)",
    700: "oklch(0.554 0.135 66.442)",
    800: "oklch(0.476 0.114 61.907)",
    900: "oklch(0.421 0.095 57.708)",
    950: "oklch(0.286 0.066 53.813)",
  },
  green: {
    50: "oklch(0.982 0.018 155.826)",
    100: "oklch(0.962 0.044 156.743)",
    200: "oklch(0.925 0.084 155.995)",
    300: "oklch(0.871 0.15 154.449)",
    400: "oklch(0.792 0.209 151.711)",
    500: "oklch(0.723 0.219 149.579)",
    600: "oklch(0.627 0.194 149.214)",
    700: "oklch(0.527 0.154 150.069)",
    800: "oklch(0.448 0.119 151.328)",
    900: "oklch(0.393 0.095 152.535)",
    950: "oklch(0.266 0.065 152.934)",
  },
  blue: {
    50: "oklch(0.97 0.014 254.604)",
    100: "oklch(0.932 0.032 255.585)",
    200: "oklch(0.882 0.059 254.128)",
    300: "oklch(0.809 0.105 251.813)",
    400: "oklch(0.707 0.165 254.624)",
    500: "oklch(0.623 0.214 259.815)",
    600: "oklch(0.546 0.245 262.881)",
    700: "oklch(0.488 0.243 264.376)",
    800: "oklch(0.424 0.199 265.638)",
    900: "oklch(0.379 0.146 265.522)",
    950: "oklch(0.282 0.091 267.935)",
  },
  violet: {
    50: "oklch(0.969 0.016 293.756)",
    100: "oklch(0.943 0.029 294.588)",
    200: "oklch(0.894 0.057 293.283)",
    300: "oklch(0.811 0.111 293.571)",
    400: "oklch(0.702 0.183 293.541)",
    500: "oklch(0.606 0.25 292.717)",
    600: "oklch(0.541 0.281 293.009)",
    700: "oklch(0.491 0.27 292.581)",
    800: "oklch(0.432 0.232 292.759)",
    900: "oklch(0.38 0.189 293.745)",
    950: "oklch(0.283 0.141 291.089)",
  },
  rose: {
    50: "oklch(0.969 0.015 12.422)",
    100: "oklch(0.941 0.03 12.58)",
    200: "oklch(0.892 0.058 10.001)",
    300: "oklch(0.81 0.117 11.638)",
    400: "oklch(0.712 0.194 13.428)",
    500: "oklch(0.645 0.246 16.439)",
    600: "oklch(0.586 0.253 17.585)",
    700: "oklch(0.514 0.222 16.935)",
    800: "oklch(0.455 0.188 13.697)",
    900: "oklch(0.41 0.159 10.272)",
    950: "oklch(0.271 0.105 12.094)",
  },
  mauve: {
    50: "oklch(0.985 0.004 304)",
    100: "oklch(0.962 0.009 304)",
    200: "oklch(0.918 0.018 304)",
    300: "oklch(0.858 0.032 304)",
    400: "oklch(0.704 0.055 304)",
    500: "oklch(0.552 0.072 304)",
    600: "oklch(0.45 0.078 304)",
    700: "oklch(0.372 0.07 304)",
    800: "oklch(0.282 0.054 304)",
    900: "oklch(0.218 0.038 304)",
    950: "oklch(0.15 0.028 304)",
  },
  olive: {
    50: "oklch(0.985 0.009 124)",
    100: "oklch(0.962 0.018 124)",
    200: "oklch(0.918 0.032 124)",
    300: "oklch(0.858 0.05 124)",
    400: "oklch(0.704 0.074 124)",
    500: "oklch(0.552 0.09 124)",
    600: "oklch(0.45 0.087 124)",
    700: "oklch(0.372 0.074 124)",
    800: "oklch(0.282 0.055 124)",
    900: "oklch(0.218 0.039 124)",
    950: "oklch(0.15 0.028 124)",
  },
  mist: {
    50: "oklch(0.985 0.008 215)",
    100: "oklch(0.962 0.016 215)",
    200: "oklch(0.918 0.03 215)",
    300: "oklch(0.858 0.045 215)",
    400: "oklch(0.704 0.067 215)",
    500: "oklch(0.552 0.082 215)",
    600: "oklch(0.45 0.082 215)",
    700: "oklch(0.372 0.07 215)",
    800: "oklch(0.282 0.052 215)",
    900: "oklch(0.218 0.037 215)",
    950: "oklch(0.15 0.027 215)",
  },
  taupe: {
    50: "oklch(0.985 0.006 70)",
    100: "oklch(0.962 0.013 70)",
    200: "oklch(0.918 0.024 70)",
    300: "oklch(0.858 0.039 70)",
    400: "oklch(0.704 0.058 70)",
    500: "oklch(0.552 0.07 70)",
    600: "oklch(0.45 0.069 70)",
    700: "oklch(0.372 0.06 70)",
    800: "oklch(0.282 0.045 70)",
    900: "oklch(0.218 0.033 70)",
    950: "oklch(0.15 0.024 70)",
  },
};

const destructive = palettes.red[600];
const darkDestructive = palettes.red[400];
const darkPrimary = "oklch(0.76 0.16 166)";

function makeTokens(palette: Palette, neutral: Palette, mode: "light" | "dark"): ThemeTokens {
  const light = mode === "light";

  return {
    background: light ? "oklch(1 0 0)" : "oklch(0.118 0.003 286)",
    foreground: light ? neutral[950] : "oklch(0.94 0.004 286)",
    card: light ? "oklch(1 0 0)" : "oklch(0.165 0.004 286)",
    cardForeground: light ? neutral[950] : "oklch(0.94 0.004 286)",
    popover: light ? "oklch(1 0 0)" : "oklch(0.18 0.004 286)",
    popoverForeground: light ? neutral[950] : neutral[50],
    primary: light ? palette[600] : darkPrimary,
    primaryForeground: light ? "oklch(0.985 0 0)" : neutral[950],
    secondary: light ? neutral[100] : "oklch(0.225 0.005 286)",
    secondaryForeground: light ? neutral[900] : neutral[50],
    muted: light ? neutral[100] : "oklch(0.225 0.005 286)",
    mutedForeground: light ? neutral[500] : "oklch(0.68 0.012 286)",
    accent: light ? palette[100] : "oklch(0.24 0.006 286)",
    accentForeground: light ? palette[900] : "oklch(0.95 0.004 286)",
    destructive: light ? destructive : darkDestructive,
    border: light ? neutral[200] : "oklch(1 0 0 / 9%)",
    input: light ? neutral[200] : "oklch(1 0 0 / 12%)",
    ring: light ? palette[500] : darkPrimary,
    chart1: palette[600],
    chart2: palette[500],
    chart3: palette[400],
    chart4: neutral[600],
    chart5: neutral[400],
    sidebar: light ? neutral[50] : "oklch(0.145 0.004 286)",
    sidebarForeground: light ? neutral[950] : neutral[50],
    sidebarPrimary: light ? palette[600] : darkPrimary,
    sidebarPrimaryForeground: light ? "oklch(0.985 0 0)" : neutral[950],
    sidebarAccent: light ? palette[100] : "oklch(0.225 0.005 286)",
    sidebarAccentForeground: light ? palette[900] : neutral[50],
    sidebarBorder: light ? neutral[200] : "oklch(1 0 0 / 9%)",
    sidebarRing: light ? palette[500] : darkPrimary,
  };
}

function makeTheme(
  id: keyof typeof palettes,
  name: string,
  family: AppTheme["family"],
  neutralKey: keyof typeof palettes = id,
): AppTheme {
  const palette = palettes[id];
  const neutral = palettes[neutralKey];

  return {
    id,
    name,
    family,
    swatch: palette[600],
    radius: "0.5rem",
    light: makeTokens(palette, neutral, "light"),
    dark: makeTokens(palette, neutral, "dark"),
  };
}

export const appThemes = [
  makeTheme("neutral", "Neutral", "base"),
  makeTheme("stone", "Stone", "base"),
  makeTheme("zinc", "Zinc", "base"),
  makeTheme("mauve", "Mauve", "base"),
  makeTheme("olive", "Olive", "base"),
  makeTheme("mist", "Mist", "base"),
  makeTheme("taupe", "Taupe", "base"),
  makeTheme("slate", "Slate", "classic"),
  makeTheme("gray", "Gray", "classic"),
  makeTheme("red", "Red", "classic", "neutral"),
  makeTheme("rose", "Rose", "classic", "neutral"),
  makeTheme("orange", "Orange", "classic", "stone"),
  makeTheme("yellow", "Yellow", "classic", "stone"),
  makeTheme("green", "Green", "classic", "neutral"),
  makeTheme("blue", "Blue", "classic", "slate"),
  makeTheme("violet", "Violet", "classic", "zinc"),
] as const satisfies AppTheme[];

export type ThemeId = (typeof appThemes)[number]["id"];

export const defaultThemeId: ThemeId = "neutral";

export function getThemeById(themeId: string): AppTheme {
  return appThemes.find((theme) => theme.id === themeId) ?? appThemes[0];
}
