// Where a driver keeps tables: schemas (Postgres), databases (MySQL), attached files (SQLite).

import { type Dialect } from "@/types";

/** Default schema when the sidebar moves to `database`. */
export function defaultSchema(driver: Dialect, database: string): string {
  if (driver === "postgres") return "public";
  if (driver === "sqlite") return "main";
  return database;
}

/** A schema to fall back to when the saved one doesn't exist. */
export function fallbackSchema(schemas: string[]): string | undefined {
  if (schemas.includes("public")) return "public";
  if (schemas.includes("main")) return "main";
  // Snowflake folds unquoted names to upper case.
  if (schemas.includes("PUBLIC")) return "PUBLIC";
  return schemas[0];
}
