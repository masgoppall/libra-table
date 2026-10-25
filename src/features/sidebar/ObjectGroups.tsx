// Sidebar sections for functions, procedures, materialized views, triggers, sequences,
// types and events of the current schema. Pinned items come first in their section.

import { Pin } from "lucide-react";
import { type ContextMenuApi } from "@/components/ContextMenu";
import { Highlight } from "@/components/Highlight";
import { objectRef, objectTitle, sameObject } from "@/domain/objects";
import { type SearchResult } from "@/lib/fuzzy";
import { activeTab } from "@/store";
import { type SchemaObject, type Workspace } from "@/types";
import { openObject } from "../objects/objectFlows";
import { schemaObjectMenu } from "../objects/objectMenus";
import { OBJECT_SECTIONS, type ObjectSection } from "../objects/objectKinds";
import { pinMenuItem, sidebarItem } from "./pins";
import { isPinned, pinnedFirst } from "./recentModel";
import { type Collapsed } from "./SidebarLists";
import { SideSection } from "./SideSection";
import { type SidebarItems } from "./useSidebarItems";

type Match = SearchResult<SchemaObject>;

const itemOf = (ws: Workspace, o: SchemaObject) => sidebarItem(ws, o.kind, o.name, o);

function isOpen(ws: Workspace, o: SchemaObject): boolean {
  const t = activeTab(ws);
  if (!t || (t.database ?? ws.database) !== ws.database || t.schema !== ws.schema) return false;
  if (o.kind === "materialized_view" && t.kind === "data") return t.table === o.name;
  return t.kind === "definition" && sameObject(t.object, objectRef(o));
}

/** Arguments of routines, the table of triggers, otherwise the description. */
function hint(o: SchemaObject): string | null {
  if (o.kind === "function" || o.kind === "procedure") return `(${o.arguments ?? ""})`;
  if (o.kind === "trigger") return o.table;
  if (o.kind === "type" && o.labels.length) return o.labels.join(", ");
  return o.detail;
}

function ObjectRow(props: {
  ws: Workspace;
  match: Match;
  section: ObjectSection;
  menu: ContextMenuApi;
  items: SidebarItems;
}) {
  const { ws, items, match } = props;
  const o = match.item;
  const Icon = props.section.icon;
  const item = itemOf(ws, o);
  const pinned = isPinned(items.pinned, item);
  const menu = schemaObjectMenu(ws, o, [pinMenuItem(items, item)]);
  const title = [objectTitle(o), o.detail].filter(Boolean).join(" · ");
  return (
    <div
      className={`side-item obj${isOpen(ws, o) ? " active" : ""}`}
      onClick={() => openObject(ws, objectRef(o))}
      onContextMenu={(e) => props.menu.open(e, menu)}
      title={title}
    >
      <Icon size={13} className={`side-icon obj ${o.kind}`} />
      <span className="label">
        <Highlight text={o.name} indexes={match.match.indexes} />
        {hint(o) && <span className="side-hint-inline"> {hint(o)}</span>}
      </span>
      {pinned && <Pin size={11} className="side-pin" />}
    </div>
  );
}

export function SchemaObjectSections(props: {
  ws: Workspace;
  matches: Match[];
  collapsed: Collapsed;
  menu: ContextMenuApi;
  items: SidebarItems;
}) {
  const { ws, items } = props;
  return (
    <>
      {OBJECT_SECTIONS.map((section) => {
        const of = props.matches.filter((m) => m.item.kind === section.kind);
        if (!of.length) return null;
        const rows = pinnedFirst(of, items.pinned, (m) => itemOf(ws, m.item));
        return (
          <SideSection
            key={section.kind}
            label={section.label}
            count={rows.length}
            collapsed={props.collapsed.isCollapsed(section.kind)}
            onToggle={() => props.collapsed.toggle(section.kind)}
          >
            {rows.map((m) => (
              <ObjectRow
                key={`${m.item.name}(${m.item.arguments ?? ""})${m.item.table ?? ""}`}
                ws={ws}
                match={m}
                section={section}
                menu={props.menu}
                items={items}
              />
            ))}
          </SideSection>
        );
      })}
    </>
  );
}
