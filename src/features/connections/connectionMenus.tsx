import { Copy, CopyPlus, FolderInput, Pencil, Plug, Trash2, Unplug, Upload } from "lucide-react";
import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { store } from "@/store";
import { type ConnectionView, type Group } from "@/types";
import * as actions from "./connectionActions";
import { exportConnections } from "./transfer";

export interface ConnectionMenuContext {
  isOpen: boolean;
  groups: Group[];
  connect: (c: ConnectionView) => void;
  edit: (c: ConnectionView) => void;
}

function groupItems(c: ConnectionView, groups: Group[]): MenuItem[] {
  const moves = groups.map((g) => ({
    label: `Move to ${g.name}`,
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

export function groupMenu(g: Group): MenuItem[] {
  return [
    { label: "Rename…", icon: <Pencil size={14} />, onClick: () => void actions.renameGroup(g) },
    {
      label: "Delete group",
      icon: <Trash2 size={14} />,
      danger: true,
      onClick: () => void actions.deleteGroup(g),
    },
  ];
}
