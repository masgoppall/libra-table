// What the schema browser reads: databases, objects, columns, keys and indexes.

import { type DataKind } from "./connection";

/** Database of the server a call is about; null = the one the connection opened. */
export type Db = string | null;

export interface OperatorInfo {
  id: string;
  label: string;
  valueKind: "none" | "single" | "list" | "range";
  placeholder: string;
}

/** What a driver's workspace can show and do (src-tauri/src/drivers.rs `Features`). */
export interface Capabilities {
  operators: OperatorInfo[];
  kind: DataKind;
  /** postgres, mysql, sqlite, mssql, oracle, clickhouse, duckdb, cql, snowflake, bigquery. */
  sqlDialect: string | null;
  /** sql, redis, mongo, partiql or elastic. */
  queryLanguage: string;
  ready: boolean;
  hasSchemas: boolean;
  switchDatabase: boolean;
  query: boolean;
  editRows: boolean;
  structure: "none" | "view" | "edit";
  createTable: boolean;
  createDatabase: boolean;
  schemaObjects: boolean;
  processes: boolean;
  users: boolean;
  backup: boolean;
  import: boolean;
  diagram: boolean;
  explain: boolean;
  metrics: boolean;
  quickLook: boolean;
}

export type ObjectKind = "table" | "view";

export interface ObjectInfo {
  name: string;
  kind: ObjectKind;
}

export interface ColumnInfo {
  name: string;
  dataType: string;
  nullable: boolean;
  default: string | null;
  pkPosition: number | null;
  autoIncrement: boolean;
  comment: string | null;
}

export interface IndexInfo {
  name: string;
  columns: string[];
  unique: boolean;
  primary: boolean;
  definition: string;
}

export interface ForeignKeyInfo {
  name: string;
  columns: string[];
  refTable: string;
  refColumns: string[];
  onUpdate: string;
  onDelete: string;
  definition: string;
}

export interface TableStructure {
  kind: ObjectKind;
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
  ddl: string;
}

export interface CompletionSchema {
  schema: string;
  schemas: string[];
  tables: Record<string, string[]>;
}
