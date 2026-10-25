// The ER diagram: tables, columns and foreign keys of a schema, and saved positions.

export interface DiagramColumn {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
}

export interface DiagramForeignKey {
  name: string;
  columns: string[];
  refSchema: string;
  refTable: string;
  refColumns: string[];
}

export interface DiagramTable {
  name: string;
  columns: DiagramColumn[];
  foreignKeys: DiagramForeignKey[];
}

export interface SchemaDiagram {
  schema: string;
  tables: DiagramTable[];
  /** More than 1,000 tables: the rest are left out. */
  truncated: boolean;
}

export interface Point {
  x: number;
  y: number;
}

/** Table name → top-left corner. */
export type DiagramLayout = Record<string, Point>;
