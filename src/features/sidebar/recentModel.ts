// Pure rules for the sidebar's recent items and pins. Items are per database (and per
// schema for pins); the store keeps one list per connection.

import { type SidebarItem } from "@/types";

/** Recent items kept per database. */
export const RECENT_LIMIT = 10;

type ItemId = Pick<SidebarItem, "database" | "schema" | "kind" | "name" | "arguments" | "table">;

/** Identifies an object across sessions: overloads and triggers of the same name differ. */
export function itemKey(i: ItemId): string {
  return [i.database, i.schema, i.kind, i.name, i.arguments ?? "", i.table ?? ""].join("\u0000");
}

/** `item` moved to the front; at most RECENT_LIMIT per database. */
export function addRecent(list: SidebarItem[], item: SidebarItem): SidebarItem[] {
  const key = itemKey(item);
  const rest = list.filter((i) => itemKey(i) !== key);
  const sameDb = rest.filter((i) => i.database === item.database).slice(0, RECENT_LIMIT - 1);
  const keep = new Set(sameDb);
  return [item, ...rest.filter((i) => i.database !== item.database || keep.has(i))];
}

export function recentIn(list: SidebarItem[], database: string): SidebarItem[] {
  return list.filter((i) => i.database === database).slice(0, RECENT_LIMIT);
}

export function removeRecent(list: SidebarItem[], item: ItemId): SidebarItem[] {
  const key = itemKey(item);
  return list.filter((i) => itemKey(i) !== key);
}

export function isPinned(pinned: SidebarItem[], item: ItemId): boolean {
  const key = itemKey(item);
  return pinned.some((p) => itemKey(p) === key);
}

export function togglePin(pinned: SidebarItem[], item: SidebarItem): SidebarItem[] {
  const { openedAt: _openedAt, ...plain } = item;
  return isPinned(pinned, item) ? removeRecent(pinned, item) : [...pinned, plain];
}

/** Pinned items first (in the order they were pinned), the rest in their own order. */
export function pinnedFirst<T>(items: T[], pinned: SidebarItem[], idOf: (t: T) => ItemId): T[] {
  const order = new Map(pinned.map((p, i) => [itemKey(p), i]));
  const rank = (t: T) => order.get(itemKey(idOf(t)));
  const top = items.filter((t) => rank(t) !== undefined);
  top.sort((a, b) => (rank(a) ?? 0) - (rank(b) ?? 0));
  return [...top, ...items.filter((t) => rank(t) === undefined)];
}
