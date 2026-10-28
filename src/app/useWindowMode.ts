// The window is TablePlus' compact welcome window while no connection is open, and the
// large workspace window otherwise (src-tauri/src/window/mode.rs resizes it).

import { useEffect } from "react";
import { api } from "@/api";
import { useStore } from "@/store";

/** True while the compact welcome window is shown. */
export function useWindowMode(): boolean {
  const compact = useStore((s) => Object.keys(s.workspaces).length === 0);
  useEffect(() => {
    api.setWindowMode(compact ? "welcome" : "workspace").catch(() => undefined);
  }, [compact]);
  return compact;
}
