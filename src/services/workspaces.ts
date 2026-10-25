// Closing a connection the way its window would close in TablePlus.

import { HOME, store } from "@/store";

/** Disconnects a connection and shows the connections page (not the next connection). */
export async function closeToHome(id: string) {
  await store.get().closeWorkspace(id);
  store.get().setView(HOME);
}
