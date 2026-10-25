// "Recent": the last tables, views, objects and saved queries opened in this database.

import { Eye, FileCode2, Table2 } from "lucide-react";
import { type ContextMenuApi } from "@/components/ContextMenu";
import { Highlight } from "@/components/Highlight";
import { type SearchResult } from "@/lib/fuzzy";
import { store } from "@/store";
import { type SidebarItem, type Workspace } from "@/types";
import { openDefinition, openObject } from "../objects/objectFlows";
import { objectIcon } from "../objects/objectKinds";
import { openSavedQuery } from "../query/savedQueries";
import { tableTab } from "../workspace/tabs";
import { type Collapsed } from "./SidebarLists";
import { SideSection } from "./SideSection";
import { type SidebarItems } from "./useSidebarItems";

export function openSidebarItem(ws: Workspace, i: SidebarItem) {
  const id = ws.connectionId;
  if (i.kind === "query") return void openSavedQuery(id, i.name);
  if (i.kind === "table" || i.kind === "view") {
    const tab = tableTab("data", i.name, { schema: i.schema });
    return store.get().addTab(id, tab, { reuse: true });
  }
  const ref = { kind: i.kind, name: i.name, arguments: i.arguments, table: i.table };
  if (i.kind === "materialized_view") openObject(ws, ref, i.schema);
  else openDefinition(ws, ref, i.schema);
}

function ItemIcon({ item }: { item: SidebarItem }) {
  if (item.kind === "table") return <Table2 size={13} className="side-icon table" />;
  if (item.kind === "view") return <Eye size={13} className="side-icon view" />;
  if (item.kind === "query") return <FileCode2 size={13} className="side-icon query" />;
  const Icon = objectIcon(item.kind);
  return <Icon size={13} className="side-icon obj" />;
}

export function RecentSection(props: {
  ws: Workspace;
  matches: SearchResult<SidebarItem>[];
  collapsed: Collapsed;
  menu: ContextMenuApi;
  items: SidebarItems;
}) {
  const { ws, items } = props;
  if (!props.matches.length) return null;
  const menuFor = (i: SidebarItem) => [
    { label: "Open", onClick: () => openSidebarItem(ws, i) },
    { label: "Remove from recent", onClick: () => items.forget(i) },
    { label: "Clear recent", onClick: () => items.clearRecent(ws.database) },
  ];
  return (
    <SideSection
      label="Recent"
      count={props.matches.length}
      collapsed={props.collapsed.isCollapsed("recent")}
      onToggle={() => props.collapsed.toggle("recent")}
    >
      {props.matches.map(({ item: i, match }) => (
        <div
          key={`${i.kind}:${i.schema}:${i.name}:${i.arguments ?? ""}:${i.table ?? ""}`}
          className="side-item recent"
          onClick={() => openSidebarItem(ws, i)}
          onContextMenu={(e) => props.menu.open(e, menuFor(i))}
          title={i.schema && i.schema !== ws.schema ? `${i.schema}.${i.name}` : i.name}
        >
          <ItemIcon item={i} />
          <span className="label">
            <Highlight text={i.name} indexes={match.indexes} />
          </span>
          {i.schema && i.schema !== ws.schema && <span className="side-tag">{i.schema}</span>}
        </div>
      ))}
    </SideSection>
  );
}
