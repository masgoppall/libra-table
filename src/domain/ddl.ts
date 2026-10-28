// Defaults and suggestions for the table designer, per driver.

import { type ColumnInfo, type ColumnSpec, type Dialect } from "@/types";

export const FK_ACTIONS = ["NO ACTION", "RESTRICT", "CASCADE", "SET NULL", "SET DEFAULT"];

export const SQL_TYPES: Record<Dialect, string[]> = {
  postgres: [
    "integer",
    "bigint",
    "smallint",
    "serial",
    "bigserial",
    "numeric(12,2)",
    "real",
    "double precision",
    "boolean",
    "text",
    "varchar(255)",
    "char(1)",
    "uuid",
    "date",
    "time",
    "timestamp",
    "timestamptz",
    "interval",
    "json",
    "jsonb",
    "bytea",
    "inet",
    "text[]",
    "integer[]",
  ],
  mysql: [
    "int",
    "bigint",
    "smallint",
    "tinyint(1)",
    "int unsigned",
    "bigint unsigned",
    "decimal(12,2)",
    "float",
    "double",
    "varchar(255)",
    "char(36)",
    "text",
    "mediumtext",
    "longtext",
    "date",
    "datetime",
    "timestamp",
    "time",
    "year",
    "json",
    "blob",
    "enum('a','b')",
    "boolean",
  ],
  sqlite: ["INTEGER", "TEXT", "REAL", "NUMERIC", "BLOB", "BOOLEAN", "DATETIME", "VARCHAR(255)"],
};

export const SQL_DEFAULTS: Record<Dialect, string[]> = {
  postgres: ["NULL", "0", "''", "now()", "CURRENT_DATE", "gen_random_uuid()", "true", "false"],
  mysql: ["NULL", "0", "''", "CURRENT_TIMESTAMP", "(uuid())", "1"],
  sqlite: ["NULL", "0", "''", "CURRENT_TIMESTAMP", "(datetime('now'))", "1"],
};

/** The type menu of the structure tab, as TablePlus lists it (lengths added on pick). */
export const TYPE_MENU: Record<Dialect, string[]> = {
  mysql: [
    "bigint",
    "binary",
    "bit",
    "blob",
    "boolean",
    "char",
    "date",
    "datetime",
    "decimal",
    "double",
    "enum",
    "float",
    "geometry",
    "geometrycollection",
    "int",
    "json",
    "linestring",
    "longblob",
    "longtext",
    "mediumblob",
    "mediumint",
    "mediumtext",
    "multilinestring",
    "multipoint",
    "multipolygon",
    "point",
    "polygon",
    "smallint",
    "text",
    "time",
    "timestamp",
    "tinyblob",
    "tinyint",
    "tinytext",
    "varbinary",
    "varchar",
    "year",
  ],
  postgres: [
    "bigint",
    "bigserial",
    "bit",
    "boolean",
    "bytea",
    "char",
    "cidr",
    "date",
    "double precision",
    "inet",
    "integer",
    "interval",
    "json",
    "jsonb",
    "macaddr",
    "money",
    "numeric",
    "real",
    "serial",
    "smallint",
    "smallserial",
    "text",
    "time",
    "timestamp",
    "timestamptz",
    "tsvector",
    "uuid",
    "varchar",
    "xml",
  ],
  sqlite: ["BLOB", "BOOLEAN", "DATE", "DATETIME", "INTEGER", "NUMERIC", "REAL", "TEXT", "VARCHAR"],
};

/** Types that can't be written without a length or values. */
const TYPE_TEMPLATE: Record<string, string> = {
  varchar: "varchar(255)",
  varbinary: "varbinary(255)",
  enum: "enum('')",
  VARCHAR: "VARCHAR(255)",
};

/** The type a menu pick writes: `varchar` → `varchar(255)`. */
export function typeFromMenu(picked: string): string {
  return TYPE_TEMPLATE[picked] ?? picked;
}

/** MySQL EXTRA without what isn't written back: AUTO_INCREMENT has its own flag and
 * DEFAULT_GENERATED only marks an expression default. */
export function writableExtra(extra: string | null | undefined): string | null {
  const words = (extra ?? "")
    .split(/\s+/)
    .filter((w) => w && !/^(auto_increment|default_generated)$/i.test(w));
  return words.length ? words.join(" ") : null;
}

export function columnSpec(c: ColumnInfo): ColumnSpec {
  return {
    name: c.name,
    dataType: c.dataType,
    nullable: c.nullable,
    default: c.default,
    primaryKey: c.pkPosition !== null,
    autoIncrement: c.autoIncrement,
    unique: false,
    comment: c.comment,
    charset: c.charset ?? null,
    collation: c.collation ?? null,
    extra: writableExtra(c.extra),
  };
}

/** The first column of a new table is an auto-increment id; later ones are text. */
export function blankColumn(driver: Dialect, first: boolean): ColumnSpec {
  const base = { default: null, unique: false, comment: null, charset: null, collation: null };
  if (first) {
    const dataType = driver === "sqlite" ? "INTEGER" : "bigint";
    return {
      ...base,
      name: "id",
      dataType,
      nullable: false,
      primaryKey: true,
      autoIncrement: true,
    };
  }
  const dataType = { sqlite: "TEXT", mysql: "varchar(255)", postgres: "text" }[driver];
  return { ...base, name: "", dataType, nullable: true, primaryKey: false, autoIncrement: false };
}
