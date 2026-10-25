// The database and schema a query tab runs in, and what the editor completes.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { defaultSchema } from "@/domain/schema";
import { tabDb } from "@/store";
import { type CompletionSchema, type Workspace, type WorkspaceTab } from "@/types";

function tabSchema(ws: Workspace, tab: WorkspaceTab, database: string): string {
  if (ws.info.driver === "mysql") return database;
  if (tab.schema) return tab.schema;
  return database === ws.database ? ws.schema : defaultSchema(ws.info.driver, database);
}

export function useQueryContext(ws: Workspace, tab: WorkspaceTab) {
  const id = ws.connectionId;
  const db = tabDb(ws, tab);
  const database = tab.database ?? ws.database;
  const schema = tabSchema(ws, tab, database);
  const [completion, setCompletion] = useState<CompletionSchema | null>(null);
  const [databases, setDatabases] = useState<string[]>([]);

  useEffect(() => {
    api
      .completionMetadata(id, db, schema)
      .then(setCompletion)
      .catch(() => setCompletion(null));
  }, [id, db, schema, ws.refreshKey]);

  useEffect(() => {
    if (!ws.caps.switchDatabase) return;
    api
      .listDatabases(id)
      .then(setDatabases)
      .catch(() => setDatabases([]));
  }, [id, ws.refreshKey]);

  // The tab's database stays pickable even if the list doesn't have it (yet).
  const choices = !database || databases.includes(database) ? databases : [database, ...databases];
  return { db, database, schema, completion, databases: ws.caps.switchDatabase ? choices : [] };
}
