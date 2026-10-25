// Schema objects besides tables and views (src-tauri/src/db/objects), and the sidebar's
// recent and pinned items.

export type SchemaObjectKind =
  "function" | "procedure" | "sequence" | "materialized_view" | "trigger" | "type" | "event";

export interface SchemaObject {
  kind: SchemaObjectKind;
  name: string;
  /** Postgres routines: identity arguments (tell overloads apart). MySQL: parameters. */
  arguments: string | null;
  /** Triggers: their table. */
  table: string | null;
  /** Result type, trigger timing and events, type category, event schedule… */
  detail: string | null;
  /** Postgres enums: labels in order. */
  labels: string[];
}

/** Which object a definition tab or a change is about. */
export interface SchemaObjectRef {
  kind: SchemaObjectKind;
  name: string;
  arguments?: string | null;
  table?: string | null;
}

export interface ObjectDefinition {
  ddl: string;
  /** CREATE OR REPLACE, or DROP + CREATE, for "Edit in query tab". */
  editScript: string;
}

/** Numbers are text: a bigint doesn't fit a JavaScript number. */
export interface SequenceInfo {
  dataType: string;
  startValue: string;
  minValue: string;
  maxValue: string;
  increment: string;
  cycle: boolean;
  cache: string;
  lastValue: string | null;
}

/** "table" | "view" | "query" (saved query) | a schema object kind. */
export type SidebarItemKind = "table" | "view" | "query" | SchemaObjectKind;

export interface SidebarItem {
  database: string;
  schema: string;
  kind: SidebarItemKind;
  name: string;
  arguments?: string;
  table?: string;
  /** Recent items: when it was opened (ms). */
  openedAt?: number;
}

export interface SidebarState {
  recent: SidebarItem[];
  pinned: SidebarItem[];
}
