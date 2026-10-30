// The structure of every table of a schema, a few requests at a time.

import { api } from "@/api";
import { type Db, type TableStructure } from "@/types";

const PARALLEL = 6;

export async function loadSchema(
  connectionId: string,
  db: Db,
  schema: string,
  onProgress: (done: number, total: number) => void,
): Promise<Record<string, TableStructure>> {
  const objects = await api.listObjects(connectionId, db, schema);
  const tables = objects.filter((o) => o.kind === "table").map((o) => o.name);
  const out: Record<string, TableStructure> = {};
  let next = 0;
  let done = 0;
  const worker = async () => {
    while (next < tables.length) {
      const name = tables[next++] as string;
      out[name] = await api.tableStructure(connectionId, db, schema, name);
      onProgress(++done, tables.length);
    }
  };
  await Promise.all(Array.from({ length: Math.min(PARALLEL, tables.length) }, worker));
  return out;
}
