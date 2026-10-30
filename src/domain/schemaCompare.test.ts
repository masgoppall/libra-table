import { describe, expect, it } from "vitest";
import { type ColumnInfo, type TableStructure } from "@/types";
import { diffSchemas, syncScript } from "./schemaCompare";

const col = (name: string, dataType: string, extra: Partial<ColumnInfo> = {}): ColumnInfo => ({
  name,
  dataType,
  nullable: true,
  default: null,
  pkPosition: null,
  autoIncrement: false,
  comment: null,
  ...extra,
});
const table = (...columns: ColumnInfo[]): TableStructure => ({
  kind: "table",
  columns,
  indexes: [],
  foreignKeys: [],
  ddl: "",
});

const source = {
  people: table(
    col("id", "integer", { nullable: false }),
    col("name", "text"),
    col("age", "integer"),
  ),
  orders: table(col("id", "integer")),
};
const target = {
  people: table(
    col("id", "integer", { nullable: false }),
    col("name", "varchar(20)"),
    col("old", "text"),
  ),
  legacy: table(col("x", "text")),
};

describe("schema compare", () => {
  it("finds missing, extra and changed tables", () => {
    const d = diffSchemas(source, target);
    expect(d.create).toEqual(["orders"]);
    expect(d.drop).toEqual(["legacy"]);
    expect(d.changed).toHaveLength(1);
    expect(d.changed[0]?.add.map((c) => c.name)).toEqual(["age"]);
    expect(d.changed[0]?.drop).toEqual(["old"]);
    expect(d.changed[0]?.alter.map((c) => c.name)).toEqual(["name"]);
  });

  it("writes the statements for Postgres, renaming the schema", () => {
    const d = diffSchemas(source, target);
    const ddl = { orders: 'CREATE TABLE "src"."orders" ("id" integer)' };
    const opts = { driver: "postgres" as const, schema: "public", sourceSchema: "src", ddl };
    expect(syncScript(d, { ...opts, dropExtra: false }).split("\n")).toEqual([
      'CREATE TABLE "public"."orders" ("id" integer);',
      'ALTER TABLE "public"."people" ADD COLUMN "age" integer;',
      'ALTER TABLE "public"."people" ALTER COLUMN "name" TYPE text;',
    ]);
    const all = syncScript(d, { ...opts, dropExtra: true });
    expect(all).toContain('ALTER TABLE "public"."people" DROP COLUMN "old";');
    expect(all).toContain('DROP TABLE "public"."legacy";');
    expect(syncScript(d, { ...opts, dropExtra: true, only: new Set(["orders"]) })).toBe(
      'CREATE TABLE "public"."orders" ("id" integer);',
    );
  });

  it("modifies MySQL columns whole", () => {
    const d = diffSchemas(source, target);
    const s = syncScript(d, {
      driver: "mysql",
      schema: "shop",
      sourceSchema: "shop",
      ddl: {},
      dropExtra: false,
    });
    expect(s).toContain("ALTER TABLE `shop`.`people` MODIFY COLUMN `name` text;");
  });
});
