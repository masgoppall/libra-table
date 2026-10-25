// Closing the window on a connection closes that connection and shows the connections
// page (Rust keeps the window open while a connection is visible: src-tauri/src/window.rs).

import { useEffect } from "react";
import { api } from "@/api";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { closeToHome } from "@/services/workspaces";
import { HOME, store, useStore } from "@/store";

export function useWindowClose() {
  const inWorkspace = useStore((s) => s.view !== HOME && !!s.workspaces[s.view]);

  useEffect(() => {
    api.setWindowView(inWorkspace).catch(() => undefined);
  }, [inWorkspace]);

  useTauriEvent("close-requested", () => {
    const { view, workspaces } = store.get();
    if (view === HOME || !workspaces[view]) return;
    void closeToHome(view);
  });
}
