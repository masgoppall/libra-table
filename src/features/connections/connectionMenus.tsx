import {
  Check,
  Copy,
  CopyPlus,
  Download,
  FolderInput,
  FolderPlus,
  Link2,
  Pencil,
  Plug,
  Plus,
  Trash2,
  Unplug,
  Upload,
} from "lucide-react";
import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { store } from "@/store";
import { type ConnectionView, type Group } from "@/types";
import * as actions from "./connectionActions";
import { exportConnections, importConnections } from "./transfer";
import { groupPath, type SortBy } from "./welcome/tree";

export interface ConnectionMenuContext {
  isOpen: boolean;
  groups: Group[];
  connect: (c: ConnectionView) => void;
  edit: (c: ConnectionView) => void;
}

function groupItems(c: ConnectionView, groups: Group[]): MenuItem[] {
  const moves = groups.map((g) => ({
    label: `Move to ${groupPath(g, groups)}`,
    icon: <FolderInput size={14} />,
    disabled: c.groupId === g.id,
    onClick: () => void actions.moveToGroup(c, g.id),
  }));
  if (!c.groupId) return moves;
  return [
    ...moves,
    { label: "Remove from group", onClick: () => void actions.moveToGroup(c, null) },
  ];
}

export function connectionMenu(c: ConnectionView, ctx: ConnectionMenuContext): MenuItem[] {
  const disconnect: MenuItem = {
    label: "Disconnect",
    icon: <Unplug size={14} />,
    onClick: () => void store.get().closeWorkspace(c.id),
  };
  return [
    {
      label: ctx.isOpen ? "Show" : "Connect",
      icon: <Plug size={14} />,
      hint: "↵",
      onClick: () => ctx.connect(c),
    },
    ...(ctx.isOpen ? [disconnect] : []),
    { label: "Edit…", icon: <Pencil size={14} />, onClick: () => ctx.edit(c) },
    {
      label: "Duplicate",
      icon: <CopyPlus size={14} />,
      onClick: () => void actions.duplicateConnection(c),
    },
    {
      label: "Copy as URL",
      icon: <Copy size={14} />,
      onClick: () => void actions.copyConnectionUrl(c),
    },
    {
      label: "Export…",
      icon: <Upload size={14} />,
      onClick: () => void exportConnections([c.id], c.name.replace(/[^\w.-]+/g, "_")),
    },
    SEPARATOR,
    ...groupItems(c, ctx.groups),
    SEPARATOR,
    {
      label: "Delete…",
      icon: <Trash2 size={14} />,
      hint: "⌫",
      danger: true,
      onClick: () => void actions.deleteConnection(c),
    },
  ];
}

/** True when `id` is `g` or one of its subgroups (a group can't move into those). */
function isWithin(id: string, g: Group, groups: Group[]): boolean {
  let at: Group | undefined = groups.find((x) => x.id === id);
  for (let i = 0; at && i <= groups.length; i++) {
    if (at.id === g.id) return true;
    const parent = at.parentId;
    at = groups.find((x) => x.id === parent);
  }
  return false;
}

export interface GroupMenuContext {
  groups: Group[];
  newConnection: (groupId: string) => void;
}

export function groupMenu(g: Group, ctx: GroupMenuContext): MenuItem[] {
  const targets = ctx.groups.filter((x) => !isWithin(x.id, g, ctx.groups) && x.id !== g.parentId);
  const moves: MenuItem[] = targets.map((x) => ({
    label: `Move into ${groupPath(x, ctx.groups)}`,
    icon: <FolderInput size={14} />,
    onClick: () => void actions.moveGroup(g, x.id),
  }));
  if (g.parentId) {
    moves.push({ label: "Move to top level", onClick: () => void actions.moveGroup(g, null) });
  }
  return [
    {
      label: "New connection in group…",
      icon: <Plus size={14} />,
      onClick: () => ctx.newConnection(g.id),
    },
    {
      label: "New group inside…",
      icon: <FolderPlus size={14} />,
      onClick: () => actions.createGroup(g.id),
    },
    SEPARATOR,
    { label: "Edit group…", icon: <Pencil size={14} />, onClick: () => actions.editGroup(g) },
    ...(moves.length ? [SEPARATOR, ...moves] : []),
    SEPARATOR,
    {
      label: "Delete group",
      icon: <Trash2 size={14} />,
      danger: true,
      onClick: () => void actions.deleteGroup(g),
    },
  ];
}

export interface ListMenuContext {
  count: number;
  sortBy: SortBy;
  onSortBy: (s: SortBy) => void;
  newConnection: () => void;
  importUrl: () => void;
}

const SORTS: [SortBy, string][] = [
  ["manual", "Manually"],
  ["name", "By name"],
  ["driver", "By driver"],
  ["tag", "By tag"],
];

/** Right-click on the list's empty space. */
export function listMenu(ctx: ListMenuContext): MenuItem[] {
  return [
    { label: "New connection…", icon: <Plus size={14} />, hint: "⌘N", onClick: ctx.newConnection },
    {
      label: "New group…",
      icon: <FolderPlus size={14} />,
      onClick: () => actions.createGroup(),
    },
    { label: "Import from URL…", icon: <Link2 size={14} />, onClick: ctx.importUrl },
    SEPARATOR,
    {
      label: "Import connections…",
      icon: <Download size={14} />,
      onClick: () => void importConnections(),
    },
    {
      label: "Import from TablePlus…",
      icon: <Download size={14} />,
      onClick: () => void importConnections("tableplus"),
    },
    {
      label: "Export all connections…",
      icon: <Upload size={14} />,
      disabled: ctx.count === 0,
      onClick: () => void exportConnections([]),
    },
    SEPARATOR,
    ...SORTS.map(([id, label]) => ({
      label: `Sort ${label.toLowerCase()}`,
      icon: ctx.sortBy === id ? <Check size={14} /> : <span className="menu-icon-space" />,
      onClick: () => ctx.onSortBy(id),
    })),
  ];
}
