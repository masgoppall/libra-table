// One place turns keys into actions; the native menu sends the same action ids.

import { useEffect } from "react";
import { type ActionId, dispatch, matchKey, setCustomShortcuts } from "@/lib/actions";
import { store, useSettings } from "@/store";
import { useTauriEvent } from "@/hooks/useTauriEvent";

/** ⌘R, ⌘[ and ⌘] would reload or navigate the webview. */
function isNavigationKey(e: KeyboardEvent): boolean {
  return (e.metaKey || e.ctrlKey) && ["r", "[", "]"].includes(e.key.toLowerCase());
}

export function useGlobalKeys() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || store.get().locked) return;
      const id = matchKey(e);
      if ((id && dispatch(id, "key")) || isNavigationKey(e)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useTauriEvent<string>("menu", (id) => {
    if (!store.get().locked) dispatch(id as ActionId, "menu");
  });
}

/** Custom shortcuts from Settings replace the defaults for key matching and labels. */
export function useCustomShortcuts() {
  const shortcuts = useSettings()?.shortcuts;
  useEffect(() => setCustomShortcuts(shortcuts ?? {}), [shortcuts]);
}
