// ⌘P entries for functions, sequences, triggers… of the current schema.

import { api } from "@/api";
import { objectRef, objectTitle } from "@/domain/objects";
import { tabDb } from "@/store";
import { type Workspace } from "@/types";
import { type PaletteItem } from "../palette/paletteItems";
import { openObject } from "./objectFlows";
import { objectIcon, OBJECT_SECTIONS } from "./objectKinds";

export async function objectPaletteItems(ws: Workspace): Promise<PaletteItem[]> {
  const list = await api.listSchemaObjects(ws.connectionId, tabDb(ws), ws.schema).catch(() => []);
  // Materialized views are already listed with the views.
  return list
    .filter((o) => o.kind !== "materialized_view")
    .map((o) => {
      const Icon = objectIcon(o.kind);
      const group = OBJECT_SECTIONS.find((s) => s.kind === o.kind)?.label ?? "Objects";
      return {
        key: `schema-obj:${o.kind}:${objectTitle(o)}`,
        group,
        label: objectTitle(o),
        detail: o.detail ?? undefined,
        icon: <Icon size={14} />,
        run: () => openObject(ws, objectRef(o)),
      };
    });
}
