// DDL: Rust builds the SQL; the UI previews it, then applies the same change.

import { type Db, type SchemaChange } from "@/types";
import { call } from "./client";

export const schemaChangesApi = {
  previewSchemaChange: (id: string, db: Db, change: SchemaChange) =>
    call<string[]>("preview_schema_change", { id, database: db, change }),
  applySchemaChange: (id: string, db: Db, change: SchemaChange, confirmed: boolean) =>
    call<string[]>("apply_schema_change", { id, database: db, change, confirmed }),
};
