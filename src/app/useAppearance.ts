// Theme and interface text size from the settings, applied to <html>.

import { useEffect, useMemo } from "react";
import { type GridPrefs, ROW_HEIGHTS } from "@/components/grid/prefs";
import { BUILT_IN_THEMES, themeCss } from "@/domain/themes";
import { useSettings } from "@/store";

export const DEFAULT_FONT_SIZE = 13;

export function useAppearance() {
  const settings = useSettings();
  const colorTheme = [...BUILT_IN_THEMES, ...(settings?.customThemes ?? [])].find(
    (t) => t.id === settings?.colorTheme,
  );
  // A color theme brings its own light or dark base.
  const theme = colorTheme?.base ?? settings?.theme ?? "system";
  const fontSize = settings?.uiFontSize ?? DEFAULT_FONT_SIZE;
  const css = colorTheme ? themeCss(colorTheme) : "";

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
  }, [theme]);

  useEffect(() => {
    let style = document.getElementById("color-theme");
    if (!css) return style?.remove();
    if (!style) {
      style = document.createElement("style");
      style.id = "color-theme";
      document.head.appendChild(style);
    }
    style.textContent = css;
  }, [css]);

  useEffect(() => {
    document.documentElement.style.setProperty("--fs", `${fontSize}px`);
  }, [fontSize]);
}

/** Row height, stripes and row numbers of the grids, from the settings. */
export function useGridPrefsFromSettings(): GridPrefs {
  const s = useSettings();
  const rowHeight = ROW_HEIGHTS[s?.rowDensity ?? "normal"] ?? ROW_HEIGHTS.normal;
  const stripes = s?.alternatingRows ?? true;
  const rowNumbers = s?.showRowNumbers ?? true;
  return useMemo(() => ({ rowHeight, stripes, rowNumbers }), [rowHeight, stripes, rowNumbers]);
}
