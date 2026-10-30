// Color themes (Settings › Appearance): a light or dark base plus a few colors that
// replace the base's surfaces, text, accent and grid colors. Built-in themes are
// LibraTable's own; a theme file in TablePlus' JSON format can be imported too.

import { type ColorTheme, type ThemeSlot } from "@/types";

/** The CSS variables each slot sets (several palettes share a meaning). */
const SLOT_VARS: Record<ThemeSlot, string[]> = {
  window: ["--chrome", "--panel", "--sh-bg"],
  content: ["--bg", "--w-main", "--st-field", "--side-field"],
  sidebar: ["--side-bg"],
  tabs: ["--tabs-bg", "--side-tab"],
  tabSelected: ["--tabs-selected"],
  tabHover: ["--tabs-hover", "--st-btn"],
  tabText: ["--tabs-text", "--side-tab-text"],
  tabSelectedText: ["--tabs-selected-text", "--side-tab-active-text"],
  text: ["--text", "--st-text", "--st-label", "--side-text", "--st-btn-text"],
  textDim: ["--text-2", "--text-3", "--st-icon", "--side-icon", "--side-muted"],
  border: ["--border", "--border-strong", "--side-border", "--st-field-border", "--tabs-sep"],
  accent: ["--accent", "--accent-hover", "--accent-text", "--st-on"],
  selection: ["--st-sel", "--sel-strong"],
  gridOdd: ["--st-row1"],
  gridEven: ["--st-row2", "--row-alt"],
  gridHeader: ["--st-bar", "--bg-2"],
  gridLine: ["--st-line", "--st-head-line"],
  inserted: ["--st-added", "--inserted"],
  modified: ["--st-changed", "--edited"],
  deleted: ["--st-dropped", "--deleted"],
};

export const BUILT_IN_THEMES: ColorTheme[] = [
  {
    id: "midnight-teal",
    name: "Midnight Teal",
    base: "dark",
    colors: {
      window: "#0e1a1e",
      content: "#102328",
      sidebar: "#0c171b",
      tabs: "#0b1518",
      tabSelected: "#173039",
      tabHover: "#132a31",
      tabText: "#7d979f",
      tabSelectedText: "#dcebee",
      text: "#d6e5e8",
      textDim: "#7f9aa2",
      border: "#1f3a43",
      accent: "#2bb3a3",
      selection: "#1d6e73",
      gridOdd: "#102328",
      gridEven: "#13292f",
      gridHeader: "#0f1f24",
      gridLine: "#1b343c",
    },
  },
  {
    id: "graphite",
    name: "Graphite",
    base: "dark",
    colors: {
      window: "#1a1a1a",
      content: "#1f1f1f",
      sidebar: "#181818",
      tabs: "#151515",
      tabSelected: "#2a2a2a",
      tabHover: "#222222",
      tabText: "#8f8f8f",
      tabSelectedText: "#ececec",
      text: "#e6e6e6",
      textDim: "#999999",
      border: "#333333",
      accent: "#f08c2e",
      selection: "#9a5418",
      gridOdd: "#1f1f1f",
      gridEven: "#252525",
      gridHeader: "#1b1b1b",
      gridLine: "#2e2e2e",
    },
  },
  {
    id: "dusk",
    name: "Dusk",
    base: "dark",
    colors: {
      window: "#181525",
      content: "#1d1a2c",
      sidebar: "#161322",
      tabs: "#13111e",
      tabSelected: "#2a2540",
      tabHover: "#221e35",
      tabText: "#8c86a8",
      tabSelectedText: "#ebe7fb",
      text: "#e4e0f5",
      textDim: "#948eb0",
      border: "#2f2a47",
      accent: "#a78bfa",
      selection: "#5b48a8",
      gridOdd: "#1d1a2c",
      gridEven: "#221f33",
      gridHeader: "#1a1728",
      gridLine: "#2b2742",
    },
  },
  {
    id: "paper",
    name: "Paper",
    base: "light",
    colors: {
      window: "#f3eee5",
      content: "#fbf8f2",
      sidebar: "#efe8dc",
      tabs: "#e6ddce",
      tabSelected: "#fbf8f2",
      tabHover: "#ddd3c2",
      tabText: "#7b7262",
      tabSelectedText: "#2c2821",
      text: "#2c2821",
      textDim: "#7b7262",
      border: "#ddd2c1",
      accent: "#b5651d",
      selection: "#b5651d",
      gridOdd: "#fbf8f2",
      gridEven: "#f4eee3",
      gridHeader: "#f0e9dc",
      gridLine: "#e4dacb",
    },
  },
];

const HEX = /^#[0-9a-fA-F]{6}$/;

/** CSS that applies the theme over its base (more specific than the base's rules). */
export function themeCss(t: ColorTheme): string {
  const lines: string[] = [];
  for (const [slot, color] of Object.entries(t.colors) as [ThemeSlot, string][]) {
    if (!HEX.test(color)) continue;
    for (const v of SLOT_VARS[slot] ?? []) lines.push(`  ${v}: ${color};`);
  }
  const accent = t.colors.accent;
  if (accent && HEX.test(accent)) {
    const bg = t.colors.content ?? (t.base === "dark" ? "#161618" : "#ffffff");
    lines.push(`  --accent-weak: color-mix(in srgb, ${accent} 18%, ${bg});`);
    lines.push(`  --accent-soft: color-mix(in srgb, ${accent} 32%, ${bg});`);
    lines.push(`  --sel: color-mix(in srgb, ${accent} 22%, ${bg});`);
  }
  return `:root:root:root {\n${lines.join("\n")}\n}\n`;
}

/** TablePlus theme keys → slots. */
const TABLEPLUS_KEYS: Record<string, ThemeSlot[]> = {
  AppWindowBackground: ["window"],
  AppContentBackground: ["content"],
  AppSidebarBackground: ["sidebar"],
  AppTabBackground: ["tabs"],
  AppTabSelectedBackground: ["tabSelected"],
  AppTabHoverBackground: ["tabHover"],
  AppTabUnselectedText: ["tabText"],
  AppTabSelectedWindowInactiveText: ["tabSelectedText"],
  GridText: ["text"],
  GridRowHeaderForeground: ["textDim"],
  AppControlBorder: ["border"],
  GridCellSelectionBorder: ["accent", "selection"],
  GridOddRowBackground: ["gridOdd"],
  GridEvenRowBackground: ["gridEven"],
  GridHeaderBackground: ["gridHeader"],
  GridSeparator: ["gridLine"],
  ContentInsertedBackground: ["inserted"],
  ContentModifiedBackground: ["modified"],
  ContentDeletedRowBackground: ["deleted"],
};

/** A theme from a TablePlus-format theme file; throws when it isn't one. */
export function themeFromFile(json: string, id: string): ColorTheme {
  const data = JSON.parse(json) as {
    name?: unknown;
    base?: unknown;
    colors?: Record<string, unknown>;
  };
  if (!data || typeof data !== "object" || !data.colors || typeof data.colors !== "object") {
    throw new Error("This isn't a theme file: it has no colors.");
  }
  const colors: ColorTheme["colors"] = {};
  for (const [key, slots] of Object.entries(TABLEPLUS_KEYS)) {
    const value = data.colors[key];
    if (typeof value !== "string" || !HEX.test(value)) continue;
    for (const slot of slots) colors[slot] ??= value.toLowerCase();
  }
  if (!Object.keys(colors).length) throw new Error("The theme file has no colors LibraTable uses.");
  const name = typeof data.name === "string" && data.name.trim() ? data.name.trim() : "Imported";
  return { id, name: name.slice(0, 40), base: data.base === "light" ? "light" : "dark", colors };
}
