// Closing a connection the way its window would close in TablePlus.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { isConnectionWindow } from "@/lib/windowRole";
import { HOME, store } from "@/store";

/** Disconnects a connection and shows the connections page (not the next connection);
 * a connection window closes instead. */
export async function closeToHome(id: string) {
  await store.get().closeWorkspace(id);
  if (isConnectionWindow) await getCurrentWindow().close();
  else store.get().setView(HOME);
}
