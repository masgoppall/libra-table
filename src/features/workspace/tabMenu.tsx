import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { openInNewWindow } from "@/services/connect";
import { store } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { newQueryTab } from "./tabs";

async function renameTab(id: string, t: WorkspaceTab) {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Rename tab",
    label: "Title",
    initial: t.title,
  });
  const title = r.ok ? r.value?.trim() : "";
  if (title) store.get().updateTab(id, t.id, { title });
}

/** Opens the tab in a new window of its connection and closes it here. */
async function moveToWindow(id: string, t: WorkspaceTab) {
  const { id: _id, back: _back, forward: _forward, ...tab } = t;
  if (await openInNewWindow(id, tab)) store.get().closeTab(id, t.id);
}

/** TablePlus' tab menu. */
export function tabMenu(ws: Workspace, t: WorkspaceTab): MenuItem[] {
  const id = ws.connectionId;
  const s = store.get();
  const isLast = ws.tabs[ws.tabs.length - 1]?.id === t.id;
  return [
    { label: "New Tab", onClick: () => s.addTab(id, newQueryTab()) },
    { label: "Close Tab", onClick: () => s.closeTab(id, t.id) },
    {
      label: "Close other Tabs",
      disabled: ws.tabs.length < 2,
      onClick: () => s.closeOtherTabs(id, t.id),
    },
    {
      label: "Close Tabs to the Right",
      disabled: isLast,
      onClick: () => s.closeTabsAfter(id, t.id),
    },
    SEPARATOR,
    { label: "Close all Tabs", onClick: () => s.closeTabsWhere(id, () => true) },
    SEPARATOR,
    { label: "Rename Tab", onClick: () => void renameTab(id, t) },
    { label: "Move Tab to New Window", onClick: () => void moveToWindow(id, t) },
  ];
}
