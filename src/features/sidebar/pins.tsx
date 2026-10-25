// Pin / unpin entries for the sidebar's context menus.

import { Pin, PinOff } from "lucide-react";
import { type MenuItem } from "@/components/ContextMenu";
import { type SidebarItem, type SidebarItemKind, type Workspace } from "@/types";
import { isPinned } from "./recentModel";
import { type SidebarItems } from "./useSidebarItems";

/** A sidebar object of the current database and schema, as a pinnable item. */
export function sidebarItem(
  ws: Workspace,
  kind: SidebarItemKind,
  name: string,
  extra: { arguments?: string | null; table?: string | null } = {},
): SidebarItem {
  return {
    database: ws.database,
    schema: ws.schema,
    kind,
    name,
    arguments: extra.arguments ?? undefined,
    table: extra.table ?? undefined,
  };
}

export function pinMenuItem(items: SidebarItems, item: SidebarItem): MenuItem {
  const pinned = isPinned(items.pinned, item);
  return {
    label: pinned ? "Unpin" : "Pin to top",
    icon: pinned ? <PinOff size={14} /> : <Pin size={14} />,
    onClick: () => items.togglePin(item),
  };
}
