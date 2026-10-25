// Defaults and suggestions for the table designer, per driver.

import { type ColumnInfo, type ColumnSpec, type Driver } from "@/types";

export const FK_ACTIONS = ["NO ACTION", "RESTRICT", "CASCADE", "SET NULL", "SET DEFAULT"];

export const SQL_TYPES: Record<Driver, string[]> = {
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

export const SQL_DEFAULTS: Record<Driver, string[]> = {
  postgres: ["NULL", "0", "''", "now()", "CURRENT_DATE", "gen_random_uuid()", "true", "false"],
  mysql: ["NULL", "0", "''", "CURRENT_TIMESTAMP", "(uuid())", "1"],
  sqlite: ["NULL", "0", "''", "CURRENT_TIMESTAMP", "(datetime('now'))", "1"],
};

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
  };
}

/** The first column of a new table is an auto-increment id; later ones are text. */
export function blankColumn(driver: Driver, first: boolean): ColumnSpec {
  const base = { default: null, unique: false, comment: null };
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
