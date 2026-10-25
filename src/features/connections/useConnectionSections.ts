// The connection list: searched, sorted, and grouped into sections.

import { useMemo } from "react";
import { searchText, TAG_ORDER } from "@/domain/connection";
import { search } from "@/lib/fuzzy";
import { type ConnectionView, type Group } from "@/types";

export type SortBy = "name" | "driver" | "tag";

export interface Section {
  group: Group | null;
  items: ConnectionView[];
}

const byName = (a: ConnectionView, b: ConnectionView) => a.name.localeCompare(b.name);

const COMPARE: Record<SortBy, (a: ConnectionView, b: ConnectionView) => number> = {
  name: byName,
  driver: (a, b) => a.driver.localeCompare(b.driver) || byName(a, b),
  tag: (a, b) => TAG_ORDER.indexOf(a.tag) - TAG_ORDER.indexOf(b.tag) || byName(a, b),
};

function group(connections: ConnectionView[], groups: Group[], query: string): Section[] {
  const known = new Set(groups.map((g) => g.id));
  const sections: Section[] = [...groups]
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((g) => ({ group: g, items: connections.filter((c) => c.groupId === g.id) }));
  const ungrouped = connections.filter((c) => !c.groupId || !known.has(c.groupId));
  sections.push({ group: null, items: ungrouped });
  // Empty groups show (so they can be used) unless a search is narrowing the list.
  return sections.filter((s) => s.items.length > 0 || (s.group && !query));
}

export function isCollapsed(section: Section, query: string): boolean {
  return !!section.group?.collapsed && !query;
}

export function useConnectionSections(
  connections: ConnectionView[],
  groups: Group[],
  query: string,
  sortBy: SortBy,
) {
  const sorted = useMemo(() => {
    const matches = search(connections, query, "contains", searchText).map((m) => m.item);
    // A search orders by match; otherwise the chosen sort applies.
    return query ? matches : [...matches].sort(COMPARE[sortBy]);
  }, [connections, query, sortBy]);

  const sections = useMemo(() => group(sorted, groups, query), [groups, sorted, query]);

  /** What the arrow keys move through (collapsed groups are skipped). */
  const visible = useMemo(
    () => sections.flatMap((s) => (isCollapsed(s, query) ? [] : s.items)),
    [sections, query],
  );

  return { sections, visible };
}
