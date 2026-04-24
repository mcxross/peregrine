export type ChromeStyle = "unified" | "sidebar" | "compact";
export type Density = "comfortable" | "compact";

export type LayoutSettings = {
  chrome: ChromeStyle;
  density: Density;
  sidebarVisible: boolean;
};

export const defaultLayoutSettings: LayoutSettings = {
  chrome: "sidebar",
  density: "comfortable",
  sidebarVisible: true,
};
