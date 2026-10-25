// "Save" in a query tab: under its saved name, or ask for one (QRY-7).

import { api } from "@/api";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type WorkspaceTab } from "@/types";

async function askName(tab: WorkspaceTab): Promise<string | null> {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Save query",
    label: "Name",
    initial: tab.title === "Query" ? "" : tab.title,
    placeholder: "e.g. monthly revenue",
    okLabel: "Save",
  });
  return (r.ok && r.value?.trim()) || null;
}

export async function saveQuery(connectionId: string, tab: WorkspaceTab, sql: string) {
  const name = tab.savedName ?? (await askName(tab));
  if (!name) return;
  try {
    await api.writeSavedQuery(connectionId, name, sql);
    store.get().updateTab(connectionId, tab.id, { savedName: name, title: name, sql });
    store.get().refresh(connectionId);
    toastSuccess(`Saved "${name}".`);
  } catch (e) {
    showError(e);
  }
}
