// The connection list of the welcome window: nested groups and connection rows.

import { DriverCircle } from "@/components/DriverLogo";
import { Spinner } from "@/components/Spinner";
import { type ContextMenuApi, type MenuItem } from "@/components/ContextMenu";
import { connectionSubtitle } from "@/domain/connection";
import { type ConnectionView, type Group } from "@/types";
import { toggleGroup } from "../connectionActions";
import { LabelChips } from "../LabelChips";
import { ChevronIcon, PlugCircleIcon } from "./icons";
import { groupKey, type TreeRow } from "./tree";

export interface ConnectionTreeProps {
  rows: TreeRow[];
  openIds: string[];
  selected: string | null;
  connecting: string | null;
  menu: ContextMenuApi;
  onSelect: (id: string) => void;
  onConnect: (c: ConnectionView) => void;
  connectionMenu: (c: ConnectionView) => MenuItem[];
  groupMenu: (g: Group) => MenuItem[];
}

const indent = (depth: number) => ({ "--depth": depth }) as React.CSSProperties;

function GroupRow(props: { row: Extract<TreeRow, { kind: "group" }>; p: ConnectionTreeProps }) {
  const { group, depth, collapsed } = props.row;
  const { p } = props;
  const key = groupKey(group.id);
  const selected = p.selected === key;
  return (
    <div
      data-row={key}
      className={`tree-row tree-group${selected ? " selected" : ""}`}
      style={indent(depth)}
      role="treeitem"
      aria-expanded={!collapsed}
      aria-selected={selected}
      onClick={() => p.onSelect(key)}
      onDoubleClick={() => void toggleGroup(group)}
      onContextMenu={(e) => {
        p.onSelect(key);
        p.menu.open(e, p.groupMenu(group));
      }}
    >
      <span
        className="tree-disclosure"
        onClick={(e) => {
          e.stopPropagation();
          void toggleGroup(group);
        }}
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <ChevronIcon open={!collapsed} />
      </span>
      {group.icon ? (
        <img className="tree-group-icon" src={group.icon} alt="" draggable={false} />
      ) : (
        <PlugCircleIcon />
      )}
      <span className="tree-group-name">{group.name}</span>
    </div>
  );
}

function ConnectionRow(props: { row: Extract<TreeRow, { kind: "conn" }>; p: ConnectionTreeProps }) {
  const { conn: c, depth } = props.row;
  const { p } = props;
  const isOpen = p.openIds.includes(c.id);
  return (
    <div
      data-row={c.id}
      className={`tree-row tree-conn${p.selected === c.id ? " selected" : ""}`}
      style={indent(depth)}
      role="treeitem"
      aria-selected={p.selected === c.id}
      onClick={() => p.onSelect(c.id)}
      onDoubleClick={() => p.onConnect(c)}
      onContextMenu={(e) => {
        p.onSelect(c.id);
        p.menu.open(e, p.connectionMenu(c));
      }}
    >
      <DriverCircle driver={c.driver} />
      <div className="tree-conn-text">
        <div className="tree-conn-line">
          <span className="tree-conn-name">{c.name}</span>
          <span className={`tag-badge ${c.tag}`}>{c.tag}</span>
          <LabelChips labels={c.labels} />
          {isOpen && <span className="tree-open-dot" title="Open" />}
          {p.connecting === c.id && <Spinner size={11} className="tree-spinner" />}
        </div>
        <div className="tree-conn-sub">{connectionSubtitle(c)}</div>
      </div>
    </div>
  );
}

export function ConnectionTree(p: ConnectionTreeProps) {
  return (
    <div className="tree" role="tree">
      {p.rows.map((row) =>
        row.kind === "group" ? (
          <GroupRow key={`g:${row.group.id}`} row={row} p={p} />
        ) : (
          <ConnectionRow key={row.conn.id} row={row} p={p} />
        ),
      )}
    </div>
  );
}
