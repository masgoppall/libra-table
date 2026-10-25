// Grouped connection rows (CON-1, CON-2).

import { ChevronDown, ChevronRight } from "lucide-react";
import { type ContextMenuApi } from "@/components/ContextMenu";
import { type ConnectionView, type Group } from "@/types";
import { connectionMenu, groupMenu } from "./connectionMenus";
import { toggleGroup } from "./connectionActions";
import { ConnectionRow } from "./ConnectionRow";
import { isCollapsed, type Section } from "./useConnectionSections";

export interface ConnectionListProps {
  sections: Section[];
  groups: Group[];
  query: string;
  openIds: string[];
  selected: string | null;
  connecting: string | null;
  menu: ContextMenuApi;
  onSelect: (id: string) => void;
  onConnect: (c: ConnectionView) => void;
  onEdit: (c: ConnectionView) => void;
}

function GroupHead(props: { section: Section; collapsed: boolean; menu: ContextMenuApi }) {
  const g = props.section.group;
  const chevron = props.collapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />;
  return (
    <div
      className="group-head"
      onClick={() => g && toggleGroup(g)}
      onContextMenu={(e) => g && props.menu.open(e, groupMenu(g))}
    >
      {g && chevron}
      {g ? g.name : "Ungrouped"}
      <span className="group-count">{props.section.items.length}</span>
    </div>
  );
}

export function ConnectionList(p: ConnectionListProps) {
  const itemsFor = (c: ConnectionView) =>
    connectionMenu(c, {
      isOpen: p.openIds.includes(c.id),
      groups: p.groups,
      connect: p.onConnect,
      edit: p.onEdit,
    });
  return (
    <>
      {p.sections.map((s) => {
        const collapsed = isCollapsed(s, p.query);
        return (
          <div key={s.group?.id ?? "ungrouped"} className="conn-group">
            {(p.groups.length > 0 || s.group) && (
              <GroupHead section={s} collapsed={collapsed} menu={p.menu} />
            )}
            {!collapsed &&
              s.items.map((c) => (
                <ConnectionRow
                  key={c.id}
                  conn={c}
                  selected={p.selected === c.id}
                  isOpen={p.openIds.includes(c.id)}
                  connecting={p.connecting === c.id}
                  onSelect={() => p.onSelect(c.id)}
                  onConnect={() => p.onConnect(c)}
                  onMenu={(e) => {
                    p.onSelect(c.id);
                    p.menu.open(e, itemsFor(c));
                  }}
                  onMenuButton={(el) => p.menu.openAt(el, itemsFor(c))}
                />
              ))}
          </div>
        );
      })}
    </>
  );
}
