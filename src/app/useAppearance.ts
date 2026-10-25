// Theme and interface text size from the settings, applied to <html>.

import { useEffect } from "react";
import { useSettings } from "@/store";

export const DEFAULT_FONT_SIZE = 13;

export function useAppearance() {
  const settings = useSettings();
  const theme = settings?.theme ?? "system";
  const fontSize = settings?.uiFontSize ?? DEFAULT_FONT_SIZE;

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
  }, [theme]);

  useEffect(() => {
    document.documentElement.style.setProperty("--fs", `${fontSize}px`);
  }, [fontSize]);
}
