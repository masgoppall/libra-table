// Names the grant dialog offers: databases, schemas of the tab's database, and tables
// of the chosen schema (MySQL: of the chosen database).

import { useEffect, useState } from "react";
import { api } from "@/api";
import { type Db, type Workspace } from "@/types";
import { type TargetFields } from "./userModel";

export function useGrantTargets(ws: Workspace, db: Db, fields: TargetFields) {
  const id = ws.connectionId;
  const pg = ws.info.driver === "postgres";
  const [databases, setDatabases] = useState<string[]>([]);
  const [schemas, setSchemas] = useState<string[]>([]);
  const [tables, setTables] = useState<string[]>([]);

  useEffect(() => {
    void api.listDatabases(id).then(setDatabases, () => setDatabases([]));
    if (pg) void api.listSchemas(id, db).then(setSchemas, () => setSchemas([]));
  }, [id, db, pg]);

  // MySQL keeps tables in databases (the "schema" of a table grant is its database).
  const container = fields.schema;
  useEffect(() => {
    if (!container) return setTables([]);
    api.listObjects(id, pg ? db : container, container).then(
      (objs) => setTables(objs.filter((o) => o.kind === "table").map((o) => o.name)),
      () => setTables([]),
    );
  }, [id, db, pg, container]);

  return { databases, schemas: pg ? schemas : databases, tables };
}
