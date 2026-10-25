import { useEffect, useState } from "react";
import { type Theme } from "@/types";

const QUERY = "(prefers-color-scheme: dark)";

/** Whether the UI is dark: the setting, or the system appearance for "system". */
export function useDarkMode(theme: Theme | undefined): boolean {
  const [system, setSystem] = useState(() => window.matchMedia(QUERY).matches);
  useEffect(() => {
    const m = window.matchMedia(QUERY);
    const onChange = () => setSystem(m.matches);
    m.addEventListener("change", onChange);
    return () => m.removeEventListener("change", onChange);
  }, []);
  return theme === "dark" || (theme !== "light" && system);
}
