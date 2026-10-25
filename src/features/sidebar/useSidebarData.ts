// What the sidebar lists: databases, schemas, objects of the current schema, saved queries.

import { useCallback, useEffect, useState } from "react";
import { api } from "@/api";
import { fallbackSchema } from "@/domain/schema";
import { showError } from "@/services/errors";
import { store, tabDb } from "@/store";
import { type ObjectInfo, type SavedQuery, type Workspace } from "@/types";

/** Postgres-style schemas, SQLite files, or an engine with schemas (e.g. BigQuery datasets). */
export function hasSchemaPicker(ws: Workspace): boolean {
  return ws.info.dialect === "postgres" || ws.info.dialect === "sqlite" || ws.caps.hasSchemas;
}

/** MySQL without a default database: pick one first. */
export function needsDatabase(ws: Workspace): boolean {
  return ws.info.dialect === "mysql" && !ws.database;
}

async function listAll(ws: Workspace) {
  const id = ws.connectionId;
  const db = tabDb(ws);
  const none = <T>(): Promise<T[]> => Promise.resolve([]);
  return Promise.all([
    needsDatabase(ws) ? none<ObjectInfo>() : api.listObjects(id, db, ws.schema),
    ws.caps.switchDatabase ? api.listDatabases(id) : none<string>(),
    hasSchemaPicker(ws) ? api.listSchemas(id, db) : none<string>(),
  ]);
}

export function useSidebarData(ws: Workspace) {
  const id = ws.connectionId;
  const [databases, setDatabases] = useState<string[]>([]);
  const [schemas, setSchemas] = useState<string[]>([]);
  const [objects, setObjects] = useState<ObjectInfo[] | null>(null);
  const [saved, setSaved] = useState<SavedQuery[]>([]);
  const [loading, setLoading] = useState(false);
  const db = tabDb(ws);

  const loadObjects = useCallback(async () => {
    setLoading(true);
    setObjects(null);
    try {
      const [objs, dbs, sch] = await listAll(ws);
      setObjects(objs);
      setDatabases(dbs);
      setSchemas(sch);
      // The saved schema may not exist in this database (or anymore).
      const fallback = fallbackSchema(sch);
      if (hasSchemaPicker(ws) && fallback && !sch.includes(ws.schema)) {
        store.get().updateWorkspace(id, { schema: fallback });
      }
    } catch (e) {
      showError(e);
      setObjects([]);
    } finally {
      setLoading(false);
    }
  }, [id, db, ws.schema, ws.refreshKey, needsDatabase(ws)]);

  const loadSaved = useCallback(() => {
    api
      .listSavedQueries(id)
      .then(setSaved)
      .catch(() => setSaved([]));
  }, [id]);

  useEffect(() => {
    void loadObjects();
  }, [loadObjects]);
  useEffect(() => {
    loadSaved();
  }, [loadSaved, ws.refreshKey]);

  return { databases, schemas, objects, saved, loading, loadSaved };
}
