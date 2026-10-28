// The connection list as a tree of nested groups, flattened into rows to render.
// Groups list their subgroups first, then their connections (as TablePlus does).

import { searchText, TAG_ORDER } from "@/domain/connection";
import { search } from "@/lib/fuzzy";
import { type ConnectionView, type Group } from "@/types";

/** "manual" keeps the order connections and groups were created in. */
export type SortBy = "manual" | "name" | "driver" | "tag";

export type TreeRow =
  | { kind: "group"; group: Group; depth: number; collapsed: boolean }
  | { kind: "conn"; conn: ConnectionView; depth: number };

const byName = (a: ConnectionView, b: ConnectionView) => a.name.localeCompare(b.name);

const COMPARE: Record<
  Exclude<SortBy, "manual">,
  (a: ConnectionView, b: ConnectionView) => number
> = {
  name: byName,
  driver: (a, b) => a.driver.localeCompare(b.driver) || byName(a, b),
  tag: (a, b) => TAG_ORDER.indexOf(a.tag) - TAG_ORDER.indexOf(b.tag) || byName(a, b),
};

function ordered(connections: ConnectionView[], query: string, sortBy: SortBy) {
  if (query) return search(connections, query, "contains", searchText).map((m) => m.item);
  return sortBy === "manual" ? connections : [...connections].sort(COMPARE[sortBy]);
}

/** A parent that doesn't exist puts the group at the top level. */
function parentOf(g: Group, known: Set<string>): string | null {
  return g.parentId && known.has(g.parentId) ? g.parentId : null;
}

export function buildTree(
  connections: ConnectionView[],
  groups: Group[],
  query: string,
  sortBy: SortBy,
): TreeRow[] {
  const known = new Set(groups.map((g) => g.id));
  const shown = ordered(connections, query, sortBy);
  const childGroups = (parent: string | null) =>
    groups.filter((g) => parentOf(g, known) === parent);
  const connsIn = (parent: string | null) =>
    shown.filter((c) => (c.groupId && known.has(c.groupId) ? c.groupId : null) === parent);

  /** While searching, a group shows only when something below it matches. */
  const hasMatch = (g: Group): boolean =>
    connsIn(g.id).length > 0 || childGroups(g.id).some(hasMatch);

  const rows: TreeRow[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const g of childGroups(parent)) {
      if (query && !hasMatch(g)) continue;
      const collapsed = g.collapsed && !query;
      rows.push({ kind: "group", group: g, depth, collapsed });
      if (!collapsed) walk(g.id, depth + 1);
    }
    for (const c of connsIn(parent)) rows.push({ kind: "conn", conn: c, depth });
  };
  walk(null, 0);
  return rows;
}

/** The connections on screen (collapsed groups hide theirs). */
export function visibleConnections(rows: TreeRow[]): ConnectionView[] {
  return rows.flatMap((r) => (r.kind === "conn" ? [r.conn] : []));
}

/** What the selection holds for a row: the connection id, or "group:<id>". */
export function rowKey(r: TreeRow): string {
  return r.kind === "group" ? groupKey(r.group.id) : r.conn.id;
}

export const groupKey = (id: string) => `group:${id}`;

/** "Clients / Acme" — a group's name with its parents, for "Move to" menus. */
export function groupPath(g: Group, groups: Group[]): string {
  const names = [g.name];
  let parent = groups.find((x) => x.id === g.parentId);
  while (parent && names.length < groups.length) {
    names.unshift(parent.name);
    const next = parent.parentId;
    parent = groups.find((x) => x.id === next);
  }
  return names.join(" / ");
}
