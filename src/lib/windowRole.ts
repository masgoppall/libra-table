// Which window this page is: the main window (connections page and workspaces), or a
// connection window opened with "Open in New Window" (src-tauri/src/window/windows.rs
// sets `window.__LIBRATABLE_WINDOW__` before the page loads), which shows one connection
// and closes with it.

import { type NewTab } from "@/types";

interface WindowInit {
  connection?: string;
  /** A tab moved here, as JSON text. */
  tab?: string | null;
}

const init: WindowInit =
  (typeof window !== "undefined" &&
    (window as unknown as { __LIBRATABLE_WINDOW__?: WindowInit }).__LIBRATABLE_WINDOW__) ||
  {};

/** The connection a connection window shows; null in the main window. */
export const windowConnection: string | null = init.connection ?? null;

export const isConnectionWindow = windowConnection !== null;

/** A tab moved here from another window ("Move Tab to New Window"). */
export function movedTab(): NewTab | null {
  if (!init.tab) return null;
  try {
    const tab = JSON.parse(init.tab) as NewTab;
    return typeof tab.kind === "string" && typeof tab.title === "string" ? tab : null;
  } catch {
    return null;
  }
}
