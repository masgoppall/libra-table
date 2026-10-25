import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { shortcutFor } from "@/lib/actions";
import { store } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { newQueryTab, tableTab } from "./tabs";

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

function kindItems(id: string, t: WorkspaceTab): MenuItem[] {
  const s = store.get();
  if (t.kind === "query") {
    const copy = newQueryTab(t.sql ?? "", {
      title: `${t.title} copy`,
      database: t.database,
      schema: t.schema,
    });
    return [
      { label: "Duplicate tab", onClick: () => s.addTab(id, copy) },
      { label: "Rename tab…", onClick: () => void renameTab(id, t) },
    ];
  }
  if ((t.kind === "data" || t.kind === "structure") && t.table) {
    const other = t.kind === "data" ? "structure" : "data";
    const label = other === "structure" ? "Open structure" : "Open data";
    return [{ label, onClick: () => s.addTab(id, tableTab(other, t.table!, t), { reuse: true }) }];
  }
  return [];
}

export function tabMenu(ws: Workspace, t: WorkspaceTab): MenuItem[] {
  const id = ws.connectionId;
  const s = store.get();
  const isLast = ws.tabs[ws.tabs.length - 1]?.id === t.id;
  return [
    { label: "Close tab", hint: shortcutFor("close-tab"), onClick: () => s.closeTab(id, t.id) },
    {
      label: "Close other tabs",
      disabled: ws.tabs.length < 2,
      onClick: () => s.closeOtherTabs(id, t.id),
    },
    {
      label: "Close tabs to the right",
      disabled: isLast,
      onClick: () => s.closeTabsAfter(id, t.id),
    },
    SEPARATOR,
    ...kindItems(id, t),
  ];
}
