// What the schema browser reads: databases, objects, columns, keys and indexes.

/** Database of the server a call is about; null = the one the connection opened. */
export type Db = string | null;

export interface OperatorInfo {
  id: string;
  label: string;
  valueKind: "none" | "single" | "list" | "range";
  placeholder: string;
}

export interface Capabilities {
  operators: OperatorInfo[];
  hasSchemas: boolean;
  switchDatabase: boolean;
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
