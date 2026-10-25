// Schema browsing. `db` picks the database of the server (null = the connection's own).

import {
  type Capabilities,
  type ColumnInfo,
  type CompletionSchema,
  type Db,
  type Driver,
  type DriverEntry,
  type ObjectInfo,
  type TableStructure,
} from "@/types";
import { call } from "./client";

export const metadataApi = {
  driverCapabilities: (driver: Driver) => call<Capabilities>("driver_capabilities", { driver }),
  listDrivers: () => call<DriverEntry[]>("list_drivers"),
  listDatabases: (id: string) => call<string[]>("list_databases", { id }),
  listSchemas: (id: string, db: Db) => call<string[]>("list_schemas", { id, database: db }),
  listObjects: (id: string, db: Db, schema: string) =>
    call<ObjectInfo[]>("list_objects", { id, database: db, schema }),
  listColumns: (id: string, db: Db, schema: string, table: string) =>
    call<ColumnInfo[]>("list_columns", { id, database: db, schema, table }),
  tableStructure: (id: string, db: Db, schema: string, table: string) =>
    call<TableStructure>("table_structure", { id, database: db, schema, table }),
  completionMetadata: (id: string, db: Db, schema: string) =>
    call<CompletionSchema>("completion_metadata", { id, database: db, schema }),
  refreshMetadata: (id: string) => call<void>("refresh_metadata", { id }),
};
