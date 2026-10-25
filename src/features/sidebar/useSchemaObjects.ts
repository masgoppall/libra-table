// Functions, sequences, triggers… of the sidebar's schema. Loaded after the tables so
// they never hold up the list; Rust caches them until a refresh.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { tabDb } from "@/store";
import { type SchemaObject, type Workspace } from "@/types";
import { needsDatabase } from "./useSidebarData";

export function useSchemaObjects(ws: Workspace, ready: boolean) {
  const id = ws.connectionId;
  const db = tabDb(ws);
  const [objects, setObjects] = useState<SchemaObject[]>([]);
  const skip = !ready || needsDatabase(ws);

  useEffect(() => {
    setObjects([]);
    if (skip) return;
    let current = true;
    api
      .listSchemaObjects(id, db, ws.schema)
      .then((list) => current && setObjects(list))
      .catch(() => current && setObjects([]));
    return () => {
      current = false;
    };
  }, [id, db, ws.schema, ws.refreshKey, skip]);

  return objects;
}
