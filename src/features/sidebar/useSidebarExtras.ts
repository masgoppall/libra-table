// What the sidebar shows besides tables and views: recent items, pins and the other
// schema objects, filtered by the sidebar search like the tables.

import { useMemo } from "react";
import { type MatchMode, search } from "@/lib/fuzzy";
import { type ObjectInfo, type SidebarItemKind, type Workspace } from "@/types";
import { recentIn } from "./recentModel";
import { useSchemaObjects } from "./useSchemaObjects";
import { useSidebarItems } from "./useSidebarItems";

export function useSidebarExtras(
  ws: Workspace,
  tables: ObjectInfo[] | null,
  filter: { query: string; mode: MatchMode },
) {
  const { query, mode } = filter;
  const schemaObjects = useSchemaObjects(ws, tables !== null);
  const matviews = useMemo(
    () => new Set(schemaObjects.filter((o) => o.kind === "materialized_view").map((o) => o.name)),
    [schemaObjects],
  );
  const kindOf = (name: string): SidebarItemKind =>
    matviews.has(name)
      ? "materialized_view"
      : (tables?.find((o) => o.name === name)?.kind ?? "table");
  const items = useSidebarItems(ws, kindOf);

  // Materialized views have their own section, so they leave "Views".
  const visibleTables = useMemo(
    () => (tables ?? []).filter((o) => !(o.kind === "view" && matviews.has(o.name))),
    [tables, matviews],
  );
  const objectMatches = useMemo(
    () => search(schemaObjects, query, mode, (o) => o.name),
    [schemaObjects, query, mode],
  );
  const recentMatches = useMemo(
    () => search(recentIn(items.recent, ws.database), query, mode, (i) => i.name),
    [items.recent, ws.database, query, mode],
  );
  return { items, visibleTables, objectMatches, recentMatches };
}
