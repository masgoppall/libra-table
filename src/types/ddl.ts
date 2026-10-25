// Schema changes (src-tauri/src/db/ddl). Types and defaults are SQL the user typed.

import { type SchemaObjectRef } from "./objects";

export interface ColumnSpec {
  name: string;
  dataType: string;
  nullable: boolean;
  default: string | null;
  primaryKey: boolean;
  autoIncrement: boolean;
  unique: boolean;
  comment: string | null;
}

export interface IndexSpec {
  name: string;
  columns: string[];
  unique: boolean;
}

export interface ForeignKeySpec {
  name: string;
  columns: string[];
  refSchema: string;
  refTable: string;
  refColumns: string[];
  onUpdate: string;
  onDelete: string;
}

export interface DatabaseOptions {
  charset?: string;
  collation?: string;
  encoding?: string;
  template?: string;
  owner?: string;
}

export type AlterOp =
  | { op: "add_column"; column: ColumnSpec }
  | { op: "drop_column"; name: string }
  | { op: "modify_column"; original: ColumnSpec; column: ColumnSpec }
  | { op: "set_primary_key"; columns: string[] }
  | { op: "add_index"; index: IndexSpec }
  | { op: "drop_index"; name: string }
  | { op: "add_foreign_key"; foreignKey: ForeignKeySpec }
  | { op: "drop_foreign_key"; name: string }
  | { op: "set_comment"; comment: string | null };

export interface CreateTable {
  kind: "create_table";
  schema: string;
  name: string;
  columns: ColumnSpec[];
  indexes: IndexSpec[];
  foreignKeys: ForeignKeySpec[];
  comment: string | null;
}

export interface AlterTable {
  kind: "alter_table";
  schema: string;
  name: string;
  ops: AlterOp[];
}

export type SchemaChange =
  | { kind: "create_database"; name: string; options: DatabaseOptions }
  | { kind: "drop_database"; name: string; force?: boolean }
  | { kind: "create_schema"; name: string }
  | { kind: "drop_schema"; name: string; cascade?: boolean }
  | CreateTable
  | AlterTable
  | { kind: "rename_table"; schema: string; from: string; to: string }
  | { kind: "drop_table"; schema: string; name: string; cascade?: boolean }
  | { kind: "truncate_table"; schema: string; name: string; cascade?: boolean }
  | { kind: "duplicate_table"; schema: string; from: string; to: string; withData: boolean }
  | { kind: "drop_object"; schema: string; object: SchemaObjectRef; cascade?: boolean }
  | { kind: "refresh_materialized_view"; schema: string; name: string; concurrently?: boolean }
  | { kind: "reset_sequence"; schema: string; name: string; value: number };
