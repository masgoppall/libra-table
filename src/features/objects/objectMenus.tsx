// Context menu of a function, sequence, trigger… in the sidebar.

import { Copy, FileCode2, RefreshCw, RotateCcw, ScrollText, Table2, Trash2 } from "lucide-react";
import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { KIND_LABEL, objectRef } from "@/domain/objects";
import { copyText } from "@/lib/clipboard";
import { toastSuccess } from "@/services/errors";
import { type SchemaObject, type Workspace } from "@/types";
import * as flows from "./objectFlows";

const icon = (Icon: typeof Copy) => <Icon size={14} />;

function kindItems(ws: Workspace, o: SchemaObject): MenuItem[] {
  if (o.kind === "materialized_view") {
    return [
      {
        label: "Refresh materialized view…",
        icon: icon(RefreshCw),
        onClick: () => void flows.refreshMaterializedView(ws, o.name),
      },
      {
        label: "Refresh concurrently…",
        onClick: () => void flows.refreshMaterializedView(ws, o.name, { concurrently: true }),
      },
    ];
  }
  if (o.kind === "sequence") {
    return [
      {
        label: "Reset…",
        icon: icon(RotateCcw),
        onClick: () => void flows.resetSequence(ws, o.name),
      },
    ];
  }
  return [];
}

/** `extra` (pin/unpin) goes before the destructive item. */
export function schemaObjectMenu(ws: Workspace, o: SchemaObject, extra: MenuItem[]): MenuItem[] {
  const ref = objectRef(o);
  const isView = o.kind === "materialized_view";
  const special = kindItems(ws, o);
  return [
    ...(isView
      ? [{ label: "Open data", icon: icon(Table2), onClick: () => flows.openObject(ws, ref) }]
      : []),
    {
      label: "Show definition",
      icon: icon(ScrollText),
      onClick: () => flows.openDefinition(ws, ref),
    },
    {
      label: "Edit in query tab",
      icon: icon(FileCode2),
      onClick: () => void flows.editInQueryTab(ws, ref),
    },
    ...(special.length ? [SEPARATOR, ...special] : []),
    SEPARATOR,
    {
      label: "Copy name",
      icon: icon(Copy),
      onClick: () => void copyText(o.name).then(() => toastSuccess("Name copied.")),
    },
    ...extra,
    SEPARATOR,
    {
      label: `Drop ${KIND_LABEL[o.kind]}…`,
      icon: icon(Trash2),
      danger: true,
      onClick: () => void flows.dropObject(ws, ref),
    },
  ];
}
