// The diagram's data and table positions: loaded together, positions saved per schema
// (Rust keeps them in diagrams/<connectionId>.json).

import { useEffect, useRef, useState } from "react";
import { api } from "@/api";
import { showError } from "@/services/errors";
import { tabDb } from "@/store";
import {
  type AppError,
  type DiagramLayout,
  type Point,
  type SchemaDiagram,
  type Workspace,
  type WorkspaceTab,
} from "@/types";
import { autoLayout, mergeLayout } from "./layout";

export function useDiagram(ws: Workspace, tab: WorkspaceTab) {
  const id = ws.connectionId;
  const db = tabDb(ws, tab);
  const schema = tab.schema ?? ws.schema;
  const key = `${tab.database ?? ws.database}/${schema}`;
  const [data, setData] = useState<SchemaDiagram | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [layout, setLayout] = useState<DiagramLayout>({});
  const latest = useRef(layout);
  latest.current = layout;

  const load = () => {
    setError(null);
    const saved = api.getDiagramLayout(id, key).catch(() => ({}) as DiagramLayout);
    Promise.all([api.schemaDiagram(id, db, schema), saved])
      .then(([d, s]) => {
        setData(d);
        setLayout(mergeLayout(d.tables, s));
      })
      .catch((e) => setError(e as AppError));
  };
  useEffect(load, [id, key, ws.refreshKey]);

  const move = (name: string, p: Point) => setLayout((l) => ({ ...l, [name]: p }));

  /** Saves the positions of the tables on the diagram. */
  const persist = () => {
    const names = new Set(data?.tables.map((t) => t.name));
    const keep = Object.fromEntries(Object.entries(latest.current).filter(([n]) => names.has(n)));
    api.saveDiagramLayout(id, key, keep).catch(showError);
  };

  const reset = () => {
    if (!data) return;
    setLayout(autoLayout(data.tables));
    api.saveDiagramLayout(id, key, {}).catch(showError);
  };

  return { schema, db, data, error, layout, move, persist, reset, reload: load };
}

export type DiagramState = ReturnType<typeof useDiagram>;
